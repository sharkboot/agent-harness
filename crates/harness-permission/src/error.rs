//! Permission 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PermissionError {
    #[error("审批失败: {0}")]
    ApprovalFailed(String),

    #[error("权限策略未找到: {0}")]
    PolicyNotFound(String),

    #[error("审批门未注册")]
    NoApprovalGate,
}
