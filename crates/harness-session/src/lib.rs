//! # harness-session
//!
//! Session 事件日志：仅追加、可持久化、可重放。
//! 消息历史从事件日志派生，不单独存储。
//!
//! ## 设计要点
//! - `SessionStore` 负责事件的追加与读取
//! - 持久化 = 持久化事件日志（当前为内存，后续可接入 SQLite/Postgres）
//! - 崩溃恢复 = 重放事件日志到检查点

pub mod store;
pub mod derived;
pub mod error;

pub use error::SessionError;
pub use store::*;
pub use derived::*;
