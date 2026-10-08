//! # harness-runtime
//!
//! Agent 主循环 + 轮次协调（ConversationCoordinator）。
//!
//! ## 设计要点
//! - `AgentHandle` 持有 Agent 所有权，可 `dispose()` / `resume()`
//! - 主循环状态机：turn/start → claim → prompt → step → model.chat → tool execute → turn/end
//! - 取消 signal 可中断正在进行的 step

pub mod agent;
pub mod loop_;
pub mod coordinator;
pub mod error;
pub mod traits;

pub use agent::*;
pub use coordinator::*;
pub use error::*;
pub use traits::*;

// Re-export key types from harness-session for convenience
pub use harness_session::{derive_state, derive_messages, DerivedState, SessionStore};
