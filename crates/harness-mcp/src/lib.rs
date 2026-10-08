//! # harness-mcp
//!
//! MCP 协议桥接。
//!
//! ## P3.4 MCP 集成
//! - 基于官方 MCP SDK 的协议桥接
//! - MCP 工具纳入统一工具注册表与权限策略

pub mod client;
pub mod server;
pub mod error;

pub use client::*;
pub use server::*;
pub use error::*;
