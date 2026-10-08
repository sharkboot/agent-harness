//! Turn/Step 协调器，管理轮次和步骤的生命周期。

use harness_event::{StepId, TurnId};
use tokio::sync::Mutex;
use tracing::info;

use super::error::RuntimeError;

/// 轮次协调器，管理多个 Turn 的状态。
#[derive(Debug)]
pub struct TurnCoordinator {
    /// 当前活跃的 Turn
    current_turn: Mutex<Option<TurnInfo>>,
    /// 历史 Turn 列表
    history: Vec<TurnInfo>,
}

#[derive(Debug, Clone)]
pub struct TurnInfo {
    pub turn_id: TurnId,
    pub start_seq: u64,
    pub end_seq: Option<u64>,
    pub step_count: usize,
}

impl TurnCoordinator {
    pub fn new() -> Self {
        Self {
            current_turn: Mutex::new(None),
            history: Vec::new(),
        }
    }

    /// 开始一个新的 Turn。
    pub async fn start_turn(&mut self, turn_id: TurnId, start_seq: u64) {
        let info = TurnInfo {
            turn_id: turn_id.clone(),
            start_seq,
            end_seq: None,
            step_count: 0,
        };
        *self.current_turn.lock().await = Some(info.clone());
        info!(turn_id = %turn_id, "Turn started");
    }

    /// 结束当前 Turn。
    pub async fn end_turn(&mut self, end_seq: u64) -> Result<TurnId, RuntimeError> {
        let mut current = self.current_turn.lock().await;
        match current.as_mut() {
            Some(info) => {
                info.end_seq = Some(end_seq);
                let turn_id = info.turn_id.clone();
                self.history.push(info.clone());
                *current = None;
                info!(turn_id = %turn_id, "Turn ended");
                Ok(turn_id)
            }
            None => Err(RuntimeError::AgentStopped),
        }
    }

    /// 增加当前 Turn 的 step 计数。
    pub async fn increment_steps(&mut self) {
        let mut current = self.current_turn.lock().await;
        if let Some(info) = current.as_mut() {
            info.step_count += 1;
        }
    }

    /// 获取当前活跃的 Turn（返回克隆以避免生命周期问题）。
    pub async fn current_turn(&self) -> Option<TurnInfo> {
        self.current_turn.lock().await.clone()
    }

    /// 获取所有历史 Turn。
    pub fn history(&self) -> &[TurnInfo] {
        &self.history
    }

    /// 判断是否有活跃的 Turn。
    pub async fn has_active_turn(&self) -> bool {
        self.current_turn.lock().await.is_some()
    }
}

impl Default for TurnCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// 步骤协调器，管理单个 Turn 内的多个 Step。
#[derive(Debug)]
pub struct StepCoordinator {
    current_step: Mutex<Option<StepInfo>>,
}

#[derive(Debug, Clone)]
pub struct StepInfo {
    pub step_id: StepId,
    pub turn_id: TurnId,
    pub start_seq: u64,
    pub end_seq: Option<u64>,
}

impl StepCoordinator {
    pub fn new() -> Self {
        Self {
            current_step: Mutex::new(None),
        }
    }

    /// 开始一个新的 Step。
    pub async fn start_step(&mut self, step_id: StepId, turn_id: &TurnId, start_seq: u64) {
        let info = StepInfo {
            step_id: step_id.clone(),
            turn_id: turn_id.clone(),
            start_seq,
            end_seq: None,
        };
        *self.current_step.lock().await = Some(info);
        info!(step_id = %step_id, turn_id = %turn_id, "Step started");
    }

    /// 结束当前 Step。
    pub async fn end_step(&mut self, end_seq: u64) -> Result<StepId, RuntimeError> {
        let mut current = self.current_step.lock().await;
        match current.as_mut() {
            Some(info) => {
                info.end_seq = Some(end_seq);
                let step_id = info.step_id.clone();
                *current = None;
                info!(step_id = %step_id, "Step ended");
                Ok(step_id)
            }
            None => Err(RuntimeError::AgentStopped),
        }
    }

    /// 获取当前活跃的 Step（返回克隆以避免生命周期问题）。
    pub async fn current_step(&self) -> Option<StepInfo> {
        self.current_step.lock().await.clone()
    }
}

impl Default for StepCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_turn_lifecycle() {
        let mut coord = TurnCoordinator::new();
        let turn = TurnId(uuid::Uuid::new_v4());

        coord.start_turn(turn.clone(), 0).await;
        assert!(coord.has_active_turn().await);

        coord.end_turn(5).await.unwrap();
        assert!(!coord.has_active_turn().await);
        assert_eq!(coord.history().len(), 1);
    }

    #[tokio::test]
    async fn test_step_lifecycle() {
        let mut coord = StepCoordinator::new();
        let step = StepId(uuid::Uuid::new_v4());
        let turn = TurnId(uuid::Uuid::new_v4());

        coord.start_step(step.clone(), &turn, 0).await;
        assert!(coord.current_step().await.is_some());

        coord.end_step(3).await.unwrap();
        assert!(coord.current_step().await.is_none());
    }
}
