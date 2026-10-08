//! 工具错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("工具执行超时: {tool_name}")]
    Timeout { tool_name: String },

    #[error("工具执行被取消: {tool_name}")]
    Cancelled { tool_name: String },

    #[error("工具执行失败: {tool_name}: {reason}")]
    ExecutionFailed { tool_name: String, reason: String },

    #[error("工具未找到: {0}")]
    NotFound(String),

    #[error("参数验证失败: {0}")]
    ValidationError(String),

    #[error("渲染失败: {0}")]
    RenderError(String),
}
