//! Provider 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("API 请求失败: {0}")]
    RequestError(String),

    #[error("API 响应错误 [{status}]: {message}")]
    ApiError { status: u16, message: String },

    #[error("JSON 解析失败: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("流式响应错误: {0}")]
    StreamError(String),

    #[error("超时")]
    Timeout,

    #[error("取消")]
    Cancelled,

    #[error("无效配置: {0}")]
    InvalidConfig(String),

    #[error("模型不支持: {0}")]
    ModelNotSupported(String),
}
