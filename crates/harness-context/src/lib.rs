//! # harness-context
//!
//! System prompt 组装 + 上下文压缩（Compaction）。
//!
//! ## P3.1 上下文压缩
//! - 触发条件：上下文接近模型窗口上限（>80%）/ 用户手动 / 长时间运行自动
//! - 策略：保留最近 N 条 → 对早期消息生成摘要 → 摘要替换原始消息并写 compaction/summary 事件
//! - 关键：压缩是可逆日志操作（不删除），原始事件保留用于重放但不再派生到 prompt
//! - 工具输出卸载：长工具输出写入临时文件，仅注入文件路径到 tool_result

pub mod compaction;
pub mod prompt_assembly;
pub mod error;

pub use compaction::*;
pub use prompt_assembly::*;
pub use error::*;
