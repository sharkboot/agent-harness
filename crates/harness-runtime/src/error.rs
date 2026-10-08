//! Runtime 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Agent 已停止，无法继续操作")]
    AgentStopped,

    #[error("Turn 已存在: {0}")]
    TurnExists(String),

    #[error("Step 执行超时")]
    StepTimeout,

    #[error("模型调用失败: {0}")]
    ModelCallError(String),

    #[error("工具执行失败: {tool_name}: {reason}")]
    ToolExecutionError { tool_name: String, reason: String },

    #[error("预算耗尽")]
    BudgetExhausted,

    #[error("操作被取消")]
    Cancelled,

    #[error(transparent)]
    Session(#[from] harness_session::SessionError),
}
