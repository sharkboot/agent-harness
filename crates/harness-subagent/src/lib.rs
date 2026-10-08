//! # harness-subagent
//!
//! 子 Agent 编排。
//!
//! ## P3.2 子 Agent 编排
//! - **层级**：Lead Agent（直接响应用户、决定委派、整合结果）→ Sub Agent（独立 Session/工具集/权限，完成后返回结果）
//! - **委派模式**：Fork（并行研究）/ Pipeline（A 完成触发 B）/ Delegation（内联委托专家）/ Handoff（角色切换）
//! - **上下文隔离**：创建时复制 Lead 快照（fork cut）+ 可选 seed 前缀 + 工具 allowlist 继承 + 独立 system prompt；完成后结果经 AgentResult 事件返回，Lead 上下文不受子 Agent 内部操作影响

pub mod subagent;
pub mod orchestration;
pub mod error;

pub use subagent::*;
pub use orchestration::*;
pub use error::*;
