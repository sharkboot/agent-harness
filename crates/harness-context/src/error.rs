//! Context 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextError {
    #[error(transparent)]
    Session(#[from] harness_session::SessionError),

    #[error("压缩失败: {0}")]
    CompactionFailed(String),

    #[error("Prompt 组装失败: {0}")]
    PromptAssemblyFailed(String),
}
