//! Session 持久化错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("事件已存在，seq={seq}")]
    DuplicateEvent { seq: u64 },

    #[error("事件索引越界: 请求 seq={req}, 最大 seq={max}")]
    EventNotFound { req: u64, max: u64 },

    #[error("Session 不存在: {0}")]
    NotFound(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("检查点加载失败: {0}")]
    CheckpointError(String),
}
