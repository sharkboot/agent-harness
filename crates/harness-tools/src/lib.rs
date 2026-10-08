//! # harness-tools
//!
//! 工具系统：注册表 + 执行流水线 + 审批门。
//!
//! ## 设计要点
//! - `ToolDefinition` 声明完整生命周期契约
//! - `ToolRegistry` 管理工具注册/注销/白名单过滤
//! - 执行流水线：execute → finalizeContent → 写入 session 日志
//! - Diff 渲染随工具定义，不集中在 harness 全局（ADR-003）
//! - 审批门支持 human-in-the-loop 挂点

pub mod definitions;
pub mod registry;
pub mod pipeline;
pub mod approval;
pub mod error;

pub use definitions::*;
pub use registry::*;
pub use pipeline::*;
pub use approval::*;
pub use error::ToolError;
