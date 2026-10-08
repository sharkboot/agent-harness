//! # harness-permission
//!
//! 权限策略引擎 + 审批门。
//!
//! ## P3.3 权限系统
//! 三层权限模型：
//! | 层 | 范围 |
//! |---|---|
//! | L1 工具级 | 每个 Skill 声明可用工具，未声明模型不可见，slash-activate 后生效 |
//! | L2 执行级 | 文件写入（overwrite）/ Shell 危险命令（rm, sudo）/ 出站 HTTP / 审批门 |
//! | L3 进程级 | 第三方 Plugin 隔离子进程，CPU/内存/时间限制，故障回收 |
//!
//! 审批门：高风险强制审批，中风险默认审批可配置静默，低风险免审批。

pub mod policy;
pub mod gate;
pub mod error;

pub use policy::*;
pub use gate::*;
pub use error::*;
