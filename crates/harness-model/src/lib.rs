//! # harness-model
//!
//! Provider 适配器：将各家 LLM API 统一映射到 `ContentBlock` / `Message` 契约。
//!
//! ## 设计要点
//! - Provider 适配器严格隔离，API 变更只影响单个适配器
//! - 输出统一映射到 ContentBlock，harness 核心不感知 Provider 差异
//! - 流式 SSE 事件映射到 BlockPool，最终组装为完整消息

pub mod provider;
pub mod anthropic;
pub mod streaming;
pub mod error;

pub use provider::*;
pub use anthropic::*;
pub use streaming::*;
pub use error::*;
