//! 工具执行流水线
//!
//! 流程：
//! ```
//! 模型调用产出 tool_call
//! → [审批门 human-in-the-loop]
//! → foreach tool_call:
//!    execute(args, signal) → canonical_value
//!    → projectContent?() 可选替换
//!    → finalizeContent?() 最终变换
//!    → 写入 session 日志（ToolResultMessage）
//!    → 派生 assistant message（含 tool_result）
//!    → 检查 budget / 中断信号 → 继续或结束
//! ```

use std::sync::Arc;

use harness_event::{
    ContentBlock, MessageId, SessionEvent, StepId, ToolCallBlock, ToolResultMessage, TurnId,
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::definitions::ToolDefinition;
use crate::error::ToolError;
use crate::registry::ToolRegistry;

/// 工具执行结果。
#[derive(Debug)]
pub struct ToolExecutionResult {
    /// 工具名称
    pub tool_name: String,
    /// 工具调用 ID
    pub tool_call_id: harness_event::ToolCallId,
    /// 执行结果（canonical value）
    pub result: serde_json::Value,
    /// 渲染后的 ContentBlock 列表
    pub content_blocks: Vec<ContentBlock>,
    /// 是否出错
    pub is_error: bool,
}

/// 工具执行流水线。
///
/// 负责将 tool_call 转化为 ToolResultMessage 并写入 session 日志。
pub struct ToolPipeline {
    registry: ToolRegistry,
}

impl ToolPipeline {
    /// 创建新的工具执行流水线。
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    /// 执行单个工具调用。
    ///
    /// 流程：
    /// 1. 查找工具定义
    /// 2. 验证参数
    /// 3. 执行工具
    /// 4. 应用 projectContent 钩子（如有）
    /// 5. 应用 finalizeContent 钩子（如有）
    /// 6. 渲染输出
    pub async fn execute_tool_call(
        &self,
        tool_call: &ToolCallBlock,
        signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<ToolExecutionResult, ToolError> {
        // 1. 查找工具定义
        let tool_def = self
            .registry
            .get(&tool_call.name)
            .ok_or_else(|| ToolError::NotFound(tool_call.name.clone()))?;

        info!(
            tool_name = %tool_call.name,
            tool_call_id = %tool_call.id,
            "Executing tool call"
        );

        // 2. 验证参数
        let args = tool_call
            .arguments_parsed
            .as_ref()
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        if let Err(e) = self.registry.validate_arguments(&tool_call.name, &args) {
            warn!(tool_name = %tool_call.name, error = %e, "Parameter validation failed");
            return Err(e);
        }

        // 3. 执行工具
        let canonical_value = tool_def
            .execute
            .execute(&args, signal)
            .await
            .map_err(|e| {
                warn!(tool_name = %tool_call.name, error = %e, "Tool execution failed");
                ToolError::ExecutionFailed {
                    tool_name: tool_call.name.clone(),
                    reason: e.to_string(),
                }
            })?;

        // 4. 应用 projectContent 钩子
        let mut value = canonical_value;
        if let Some(ref project_fn) = tool_def.project_content {
            match project_fn(&args, value.clone()) {
                Ok(replaced) => {
                    info!(tool_name = %tool_call.name, "Applied projectContent hook");
                    value = replaced;
                }
                Err(e) => {
                    warn!(tool_name = %tool_call.name, error = %e, "projectContent hook failed, using original");
                }
            }
        }

        // 5. 应用 finalizeContent 钩子（不可失败）
        if let Some(ref finalize_fn) = tool_def.finalize_content {
            value = finalize_fn(&args, value);
            info!(tool_name = %tool_call.name, "Applied finalizeContent hook");
        }

        // 6. 渲染输出
        let content_blocks = tool_def.output.render.call(&args, &value);

        // 7. 构建执行结果
        let result = ToolExecutionResult {
            tool_name: tool_call.name.clone(),
            tool_call_id: tool_call.id.clone(),
            result: value,
            content_blocks,
            is_error: false,
        };

        Ok(result)
    }

    /// 批量执行多个工具调用。
    ///
    /// 根据工具的 is_concurrency_safe 属性决定是否并发执行。
    pub async fn execute_tool_calls(
        &self,
        tool_calls: &[ToolCallBlock],
        signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Vec<Result<ToolExecutionResult, ToolError>> {
        let mut results = Vec::with_capacity(tool_calls.len());

        // 分离并发安全和串行工具
        let (safe_calls, unsafe_calls): (Vec<_>, Vec<_>) = tool_calls
            .iter()
            .partition(|tc| {
                self.registry
                    .get(&tc.name)
                    .map(|t| t.is_concurrency_safe())
                    .unwrap_or(false)
            });

        // 并发执行安全工具
        let safe_results: Vec<_> = if safe_calls.is_empty() {
            vec![]
        } else {
            let handles: Vec<_> = safe_calls
                .iter()
                .map(|tc| {
                    let pipeline = self.clone();
                    let signal = signal.clone();
                    tokio::spawn(async move {
                        pipeline.execute_tool_call(tc, &signal).await
                            .map_err(|e| e.clone())
                    })
                })
                .collect();

            futures::future::join_all(handles).await
        };

        for handle in safe_results {
            match handle {
                Ok(Ok(result)) => results.push(Ok(result)),
                Ok(Err(e)) => results.push(Err(e)),
                Err(e) => results.push(Err(ToolError::ExecutionFailed {
                    tool_name: "unknown".to_string(),
                    reason: e.to_string(),
                })),
            }
        }

        // 串行执行不安全工具
        for tc in unsafe_calls {
            match self.execute_tool_call(tc, signal).await {
                Ok(result) => results.push(Ok(result)),
                Err(e) => results.push(Err(e)),
            }
        }

        results
    }

    /// 将执行结果转化为 SessionEvent 列表。
    ///
    /// 每个工具调用产生一个 ToolResultMessage 事件。
    pub fn results_to_events(
        &self,
        turn_id: &TurnId,
        step_id: &StepId,
        results: &[ToolExecutionResult],
    ) -> Vec<SessionEvent> {
        results
            .iter()
            .map(|r| {
                SessionEvent::ToolResultMessage(ToolResultMessage {
                    seq: harness_event::SessionSeq::new(0), // 由 store 分配
                    turn: turn_id.clone(),
                    step: step_id.clone(),
                    tool_call_id: r.tool_call_id.clone(),
                    content: r.content_blocks.clone(),
                    is_error: r.is_error,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{ToolDefinition, ToolOutputDefinition};

    #[tokio::test]
    async fn execute_single_tool_call() {
        let mut registry = ToolRegistry::new();
        let tool = ToolDefinition::new(
            "test_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
            Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        );
        registry.register(tool);

        let pipeline = ToolPipeline::new(registry);
        let tool_call = ToolCallBlock::new(
            harness_event::ToolCallId(uuid::Uuid::new_v4()),
            "test_tool",
            "{}",
        );
        let signal = Arc::new(Mutex::new(None));

        let result = pipeline.execute_tool_call(&tool_call, &signal).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn execute_nonexistent_tool_fails() {
        let registry = ToolRegistry::new();
        let pipeline = ToolPipeline::new(registry);
        let tool_call = ToolCallBlock::new(
            harness_event::ToolCallId(uuid::Uuid::new_v4()),
            "nonexistent",
            "{}",
        );
        let signal = Arc::new(Mutex::new(None));

        let result = pipeline.execute_tool_call(&tool_call, &signal).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn batch_execute_mixed_concurrency() {
        let mut registry = ToolRegistry::new();

        // 并发安全工具
        let safe_tool = ToolDefinition::new(
            "safe_tool",
            "Safe tool",
            serde_json::json!({"type": "object"}),
            Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        )
        .with_concurrency(true);

        // 不并发工具
        let unsafe_tool = ToolDefinition::new(
            "unsafe_tool",
            "Unsafe tool",
            serde_json::json!({"type": "object"}),
            Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        )
        .with_concurrency(false);

        registry.register(safe_tool);
        registry.register(unsafe_tool);

        let pipeline = ToolPipeline::new(registry);
        let signal = Arc::new(Mutex::new(None));

        let tool_calls = vec![
            ToolCallBlock::new(harness_event::ToolCallId(uuid::Uuid::new_v4()), "safe_tool", "{}"),
            ToolCallBlock::new(harness_event::ToolCallId(uuid::Uuid::new_v4()), "unsafe_tool", "{}"),
        ];

        let results = pipeline.execute_tool_calls(&tool_calls, &signal).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}
