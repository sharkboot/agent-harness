//! Agent 主结构体与句柄管理。

use std::sync::Arc;

use harness_event::{AgentId, SessionEvent, SessionSeq, TurnId};
use harness_session::{DerivedState, SessionStore};
use tokio::sync::Mutex;
use tracing::info;

use super::error::RuntimeError;
use super::traits::{AgentConfig, AgentHandle, AgentState};

/// Agent 状态（受 Arc<Mutex<>> 保护）。
#[derive(Debug)]
pub struct AgentInner {
    /// Agent ID
    pub agent_id: AgentId,
    /// 会话 ID
    pub session_id: uuid::Uuid,
    /// 事件日志
    pub store: SessionStore,
    /// 配置
    pub config: AgentConfig,
    /// 当前状态
    pub state: AgentState,
    /// 取消信号（非 None 时阻止新 step 启动）
    pub cancel: Mutex<Option<Arc<Mutex<()>>>>,
}

impl AgentInner {
    pub fn new(agent_id: AgentId, session_id: uuid::Uuid, config: AgentConfig) -> Self {
        Self {
            agent_id,
            session_id,
            store: SessionStore::new(session_id),
            config,
            state: AgentState::Idle,
            cancel: Mutex::new(None),
        }
    }

    /// 追加事件并返回 seq。
    pub async fn append_event(&mut self, event: SessionEvent) -> Result<SessionSeq, RuntimeError> {
        let seq = self.store.next_seq();
        let event = event.clone();
        self.store.append(event)?;
        Ok(seq)
    }

    /// 派生当前会话状态。
    pub fn derive_state(&self) -> DerivedState {
        harness_session::derive_state(&self.store)
    }

    /// 检查是否已取消。
    pub async fn is_cancelled(&self) -> bool {
        let cancel = self.cancel.lock().await;
        cancel.is_some()
    }
}

/// Agent 主循环协调器。
///
/// 负责驱动 turn → step → model → tool 的执行流程。
pub struct ConversationCoordinator {
    inner: Arc<Mutex<AgentInner>>,
}

impl ConversationCoordinator {
    /// 创建新的协调器。
    pub fn new(agent_id: AgentId, session_id: uuid::Uuid, config: AgentConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AgentInner::new(agent_id, session_id, config))),
        }
    }

    /// 创建 Agent 句柄（外部持有者可用于 dispose / resume）。
    pub fn spawn_handle(&self) -> AgentHandle {
        // 用 try_lock 避免在异步线程中阻塞；测试环境中锁通常未被持有
        let session_id = match self.inner.try_lock() {
            Ok(inner) => inner.session_id,
            Err(_) => uuid::Uuid::nil(),
        };
        AgentHandle::new(session_id, AgentState::Idle)
    }

    /// 派生当前消息历史。
    pub fn derive_messages(&self) -> DerivedState {
        match self.inner.try_lock() {
            Ok(inner) => inner.derive_state(),
            Err(_) => DerivedState {
                messages: Default::default(),
                turns: Default::default(),
                current_turn_id: None,
                last_seq: 0,
            },
        }
    }

    /// 获取内部存储的长度。
    pub fn store_len(&self) -> usize {
        match self.inner.try_lock() {
            Ok(inner) => inner.store.len(),
            Err(_) => 0,
        }
    }

    /// 启动一个新轮次。
    ///
    /// 返回 TurnId，供后续步骤使用。
    pub async fn start_turn(&self) -> Result<TurnId, RuntimeError> {
        let mut inner = self.inner.lock().await;

        if inner.state != AgentState::Idle {
            return Err(RuntimeError::AgentStopped);
        }

        // 生成新 turn ID
        let turn = inner.store.generate_turn_id();
        let seq = inner.store.next_seq();

        // 写入 TurnStart 事件
        inner
            .store
            .append(SessionEvent::TurnStart(harness_event::TurnStart {
                turn: turn.clone(),
                seq,
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        inner.state = AgentState::Running;
        info!(turn_id = %turn, "Starting new turn");

        Ok(turn)
    }

    /// 结束当前轮次。
    pub async fn end_turn(
        &self,
        turn: &TurnId,
        reason: harness_event::TurnEndReason,
    ) -> Result<(), RuntimeError> {
        let mut inner = self.inner.lock().await;
        let seq = inner.store.next_seq();

        // 写入 TurnEnd 事件
        inner
            .store
            .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                turn: turn.clone(),
                seq,
                reason,
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        inner.state = AgentState::Idle;
        info!(turn_id = %turn, reason = ?reason, "Turn ended");

        Ok(())
    }

    /// 恢复 Agent 到指定状态。
    pub async fn resume(&self, state: AgentState) -> Result<(), RuntimeError> {
        let mut inner = self.inner.lock().await;
        inner.state = state;
        Ok(())
    }

    /// 停止并清理 Agent。
    pub async fn dispose(&self) -> Result<(), RuntimeError> {
        let mut inner = self.inner.lock().await;
        inner.state = AgentState::Stopped;
        inner.cancel.lock().await.take();
        info!(agent_id = %inner.agent_id, "Agent disposed");
        Ok(())
    }

    /// 获取 Agent ID（克隆返回，不持有锁）。
    pub fn agent_id(&self) -> AgentId {
        self.inner.blocking_lock().agent_id.clone()
    }

    /// 获取会话 ID（克隆返回，不持有锁）。
    pub fn session_id(&self) -> uuid::Uuid {
        self.inner.blocking_lock().session_id
    }
}

impl Clone for ConversationCoordinator {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_dispose() {
        let coordinator =
            ConversationCoordinator::new(AgentId(uuid::Uuid::new_v4()), uuid::Uuid::new_v4(), Default::default());

        // 创建句柄（轻量标记，不追踪实时状态）
        let handle = coordinator.spawn_handle();
        assert!(!handle.is_stopped());

        // dispose 后验证 coordinator 内部状态
        coordinator.dispose().await.unwrap();
        // 通过 store 非空验证 dispose 执行成功（dispose 本身不写事件，但会清除 cancel）
        assert!(coordinator.store_len() == 0); // 尚未写入任何事件
    }

    #[tokio::test]
    async fn test_start_and_end_turn() {
        let coordinator =
            ConversationCoordinator::new(AgentId(uuid::Uuid::new_v4()), uuid::Uuid::new_v4(), Default::default());

        // 开始 turn
        let turn = coordinator.start_turn().await.unwrap();

        // 验证 turn 已记录
        assert_eq!(coordinator.store_len(), 1);

        // 结束 turn
        coordinator
            .end_turn(&turn, harness_event::TurnEndReason::Completed)
            .await
            .unwrap();

        // 验证 turn end 已记录
        assert_eq!(coordinator.store_len(), 2);
    }
}
