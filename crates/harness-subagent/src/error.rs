//! SubAgent 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubAgentError {
    #[error("子 Agent 不存在: {0}")]
    NotFound(String),

    #[error("子 Agent 执行失败: {0}")]
    ExecutionFailed(String),

    #[error("子 Agent 超时")]
    Timeout,

    #[error("上下文同步失败: {0}")]
    ContextSyncError(String),
}
