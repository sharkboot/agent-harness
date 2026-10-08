//! MCP 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("连接失败: {0}")]
    ConnectionError(String),

    #[error("未连接")]
    NotConnected,

    #[error("解析失败: {0}")]
    ParseError(String),

    #[error("工具不存在: {0}")]
    ToolNotFound(String),

    #[error("内部错误: {0}")]
    Internal(String),
}
