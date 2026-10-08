//! 审批门实现

use async_trait::async_trait;
use harness_event::ToolCallBlock;
use harness_tools::definitions::ToolDefinition;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::policy::{PermissionLevel, PermissionResult};
use crate::error::PermissionError;

/// 审批决策
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Reject,
    EditAndApprove { new_args: serde_json::Value },
}

/// 审批门 trait
#[async_trait]
pub trait ApprovalGate: Send + Sync + 'static {
    async fn request_approval(
        &self,
        tool_def: &ToolDefinition,
        tool_call: &ToolCallBlock,
        permission_result: &PermissionResult,
    ) -> ApprovalDecision;

    fn requires_approval(&self, level: &PermissionLevel) -> bool {
        matches!(level, PermissionLevel::High)
    }
}

/// 静默审批门（测试用）
#[derive(Debug, Clone, Default)]
pub struct SilentApprovalGate;

#[async_trait]
impl ApprovalGate for SilentApprovalGate {
    async fn request_approval(
        &self,
        _tool_def: &ToolDefinition,
        _tool_call: &ToolCallBlock,
        _permission_result: &PermissionResult,
    ) -> ApprovalDecision {
        ApprovalDecision::Approve
    }
}

/// 权限管理器
pub struct PermissionManager {
    gates: Vec<Arc<dyn ApprovalGate>>,
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionManager {
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    pub fn register_gate(&mut self, gate: Arc<dyn ApprovalGate>) {
        self.gates.push(gate);
    }

    /// 检查工具调用是否需要审批
    pub async fn check_approval(
        &self,
        tool_def: &ToolDefinition,
        tool_call: &ToolCallBlock,
        permission_result: &PermissionResult,
    ) -> Result<ApprovalDecision, PermissionError> {
        // 检查是否需要审批
        let level = tool_def.timeout_ms().map_or(PermissionLevel::Low, |_| {
            PermissionLevel::Medium
        });

        if !self
            .gates
            .iter()
            .any(|g| g.requires_approval(&level))
        {
            return Ok(ApprovalDecision::Approve);
        }

        // 调用审批门
        for gate in &self.gates {
            let decision = gate.request_approval(tool_def, tool_call, permission_result).await;
            match &decision {
                ApprovalDecision::Approve => continue,
                _ => return Ok(decision),
            }
        }

        Ok(ApprovalDecision::Approve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_tools::{ToolDefinition, ToolOutputDefinition};

    #[tokio::test]
    async fn test_silent_gate() {
        let gate = SilentApprovalGate;
        let tool_def = ToolDefinition::new(
            "test",
            "Test",
            serde_json::json!({}),
            std::sync::Arc::new(harness_tools::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: std::sync::Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        );
        let tool_call = ToolCallBlock {
            id: harness_event::ToolCallId(uuid::Uuid::new_v4()),
            name: "test".to_string(),
            arguments_json: "{}".to_string(),
            arguments_parsed: Some(serde_json::json!({})),
        };
        let permission = PermissionResult {
            allowed: true,
            level: PermissionLevel::Low,
            requires_approval: false,
            reason: None,
        };

        let decision = gate
            .request_approval(&tool_def, &tool_call, &permission)
            .await;
        assert!(matches!(decision, ApprovalDecision::Approve));
    }
}
