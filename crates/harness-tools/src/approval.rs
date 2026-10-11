//! 审批门（Human-in-the-Loop Gate）
//!
//! 三层权限模型：
//! - L1 工具级：每个 Skill 声明可用工具，未声明模型不可见
//! - L2 执行级：文件写入/rm/sudo/网络等高风险操作需审批
//! - L3 进程级：第三方 Plugin 隔离子进程
//!
//! 审批门在 INPUT_AVAILABLE 阶段暂停，支持批准/编辑参数/拒绝。

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::definitions::ToolDefinition;
use harness_event::ToolCallBlock;

/// 审批决策。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// 批准执行
    Approve,
    /// 拒绝执行
    Reject,
    /// 编辑参数后执行
    EditAndApprove { new_args: serde_json::Value },
}

/// 审批回调 trait，供外部实现自定义审批逻辑。
#[async_trait::async_trait]
pub trait ApprovalGate: Send + Sync + 'static {
    /// 请求审批。
    ///
    /// - `tool_def`: 工具定义
    /// - `tool_call`: 工具调用块
    ///
    /// 返回审批决策。
    async fn request_approval(
        &self,
        tool_def: &ToolDefinition,
        tool_call: &ToolCallBlock,
    ) -> ApprovalDecision;

    /// 判断工具是否需要审批。
    ///
    /// 默认实现：高风险工具（rm, sudo, 网络请求等）需要审批。
    fn requires_approval(&self, tool_def: &ToolDefinition) -> bool {
        // 默认：所有工具都需要审批（保守策略）
        // 具体实现可覆盖此方法
        true
    }
}

/// 默认审批门实现（静默批准所有工具）。
///
/// 适用于 headless 模式或测试环境。
#[derive(Debug, Clone, Default)]
pub struct SilentApprovalGate;

#[async_trait::async_trait]
impl ApprovalGate for SilentApprovalGate {
    async fn request_approval(
        &self,
        _tool_def: &ToolDefinition,
        _tool_call: &ToolCallBlock,
    ) -> ApprovalDecision {
        ApprovalDecision::Approve
    }

    fn requires_approval(&self, _tool_def: &ToolDefinition) -> bool {
        false
    }
}

/// 模拟审批门实现（用于测试）。
///
/// 所有工具自动批准。
#[derive(Debug, Clone, Default)]
pub struct TestApprovalGate;

#[async_trait::async_trait]
impl ApprovalGate for TestApprovalGate {
    async fn request_approval(
        &self,
        _tool_def: &ToolDefinition,
        _tool_call: &ToolCallBlock,
    ) -> ApprovalDecision {
        ApprovalDecision::Approve
    }
}

/// 审批状态。
#[derive(Clone)]
pub struct ApprovalState {
    /// 待审批的工具调用 ID 列表
    pub pending_approvals: Vec<(ToolCallBlock, Arc<dyn ApprovalGate>)>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            pending_approvals: Vec::new(),
        }
    }

    /// 添加待审批的工具调用。
    pub fn add_pending(&mut self, tool_call: ToolCallBlock, gate: Arc<dyn ApprovalGate>) {
        self.pending_approvals.push((tool_call, gate));
    }

    /// 获取待审批数量。
    pub fn pending_count(&self) -> usize {
        self.pending_approvals.len()
    }

    /// 清空所有待审批。
    pub fn clear(&mut self) {
        self.pending_approvals.clear();
    }
}

/// 审批门管理器。
///
/// 管理多个审批门的实例，支持动态切换。
#[derive(Clone)]
pub struct ApprovalManager {
    gates: Vec<Arc<dyn ApprovalGate>>,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalManager {
    /// 创建新的审批管理器。
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    /// 注册一个审批门。
    pub fn register_gate(&mut self, gate: Arc<dyn ApprovalGate>) {
        self.gates.push(gate);
    }

    /// 移除一个审批门。
    pub fn remove_gate(&mut self, index: usize) -> Option<Arc<dyn ApprovalGate>> {
        if index < self.gates.len() {
            Some(self.gates.remove(index))
        } else {
            None
        }
    }

    /// 获取当前审批门数量。
    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    /// 对工具调用执行审批检查。
    ///
    /// 返回最终的审批决策。
    pub async fn check_approval(
        &self,
        tool_def: &ToolDefinition,
        tool_call: &ToolCallBlock,
    ) -> ApprovalDecision {
        // 按优先级依次调用审批门，首个返回非 Approve 的决策生效
        for gate in &self.gates {
            if gate.requires_approval(tool_def) {
                let decision = gate.request_approval(tool_def, tool_call).await;
                match &decision {
                    ApprovalDecision::Approve => continue,
                    ApprovalDecision::Reject | ApprovalDecision::EditAndApprove { .. } => {
                        info!(
                            tool_name = %tool_call.name,
                            decision = ?decision,
                            "Approval decision made"
                        );
                        return decision;
                    }
                }
            }
        }

        // 没有审批门要求审批，默认批准
        ApprovalDecision::Approve
    }

    /// 审批所有待处理工具调用。
    pub async fn process_pending(
        &self,
        pending: Vec<(ToolCallBlock, Arc<dyn ApprovalGate>)>,
    ) -> Vec<(ToolCallBlock, ApprovalDecision)> {
        let mut results = Vec::with_capacity(pending.len());
        for (tool_call, gate) in pending {
            // 创建临时工具定义用于审批
            let tool_def = ToolDefinition::new(
                tool_call.name.clone(),
                "审批中",
                serde_json::json!({}),
                std::sync::Arc::new(crate::definitions::MockExecutor),
                crate::definitions::ToolOutputDefinition {
                    schema: serde_json::json!({}),
                    render: std::sync::Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                    presentation_meta: None,
                },
            );
            let decision = gate.request_approval(&tool_def, &tool_call).await;
            results.push((tool_call, decision));
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{ToolDefinition, ToolOutputDefinition};

    #[tokio::test]
    async fn silent_gate_approves_all() {
        let gate = SilentApprovalGate;
        let tool_def = ToolDefinition::new(
            "test",
            "Test",
            serde_json::json!({}),
            std::sync::Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: std::sync::Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        );
        let tool_call = ToolCallBlock::new(
            harness_event::ToolCallId(uuid::Uuid::new_v4()),
            "test",
            "{}",
        );

        let decision = gate.request_approval(&tool_def, &tool_call).await;
        assert!(matches!(decision, ApprovalDecision::Approve));
    }

    #[tokio::test]
    async fn approval_manager_checks_gates() {
        let mut manager = ApprovalManager::new();
        manager.register_gate(Arc::new(SilentApprovalGate));

        let tool_def = ToolDefinition::new(
            "test",
            "Test",
            serde_json::json!({}),
            std::sync::Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: std::sync::Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        );
        let tool_call = ToolCallBlock::new(
            harness_event::ToolCallId(uuid::Uuid::new_v4()),
            "test",
            "{}",
        );

        let decision = manager.check_approval(&tool_def, &tool_call).await;
        assert!(matches!(decision, ApprovalDecision::Approve));
    }

    #[test]
    fn approval_state_operations() {
        let mut state = ApprovalState::new();
        assert_eq!(state.pending_count(), 0);

        state.add_pending(
            ToolCallBlock::new(
                harness_event::ToolCallId(uuid::Uuid::new_v4()),
                "test",
                "{}",
            ),
            Arc::new(SilentApprovalGate),
        );
        assert_eq!(state.pending_count(), 1);

        state.clear();
        assert_eq!(state.pending_count(), 0);
    }
}
