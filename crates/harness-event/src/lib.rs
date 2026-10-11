//! # harness-event
//!
//! 事件类型定义与序列化核心：ContentBlock、Message、SessionEvent。
//!
//! 所有 Provider（OpenAI / Anthropic / Grok / DeepSeek）的输出最终映射到这个类型，
//! SessionEvent 仅追加日志是 Session 的唯一真源。

mod content_block;
mod message;
mod session_event;
mod types;

pub use content_block::*;
pub use message::*;
pub use session_event::*;
pub use types::*;
