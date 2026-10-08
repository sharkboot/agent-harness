//! Agent 主循环 trait，供外部实现以注入自定义行为。

use async_trait::async_trait;
use harness_event::{ContentBlock, Message, ToolCallBlock};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::error::RuntimeError;

/// 模型聊天 trait，供外部实现不同 Provider（OpenAI / Anthropic / ...）。
#[async_trait]
pub trait ModelService: Send + Sync + 'static {
    /// 流式聊天调用，返回 ContentBlock 列表。
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        _signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<Vec<ContentBlock>, RuntimeError>;
}

/// 工具 schema 定义（进入 system prompt）。
#[derive(Debug, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 工具执行器 trait，供外部实现具体工具逻辑。
#[async_trait]
pub trait ToolExecutor: Send + Sync + 'static {
    /// 执行工具，返回结果内容块。
    async fn execute(
        &self,
        name: &str,
        args: &Value,
        _signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<Vec<ContentBlock>, RuntimeError>;

    /// 判断工具是否支持并发执行。
    fn is_concurrency_safe(&self) -> bool {
        true
    }
}

/// 工具注册表，按名称索引所有可用工具。
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Arc<dyn ToolExecutor>>,
    schemas: Vec<ToolSchema>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tool_count", &self.tools.len())
            .field("schema_count", &self.schemas.len())
            .finish()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个工具。
    pub fn register(&mut self, name: impl Into<String>, executor: Arc<dyn ToolExecutor>) {
        self.tools.insert(name.into(), executor);
    }

    /// 获取工具执行器。
    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// 获取所有工具名称。
    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// 设置工具 schema。
    pub fn set_schemas(&mut self, schemas: Vec<ToolSchema>) {
        self.schemas = schemas;
    }

    /// 获取所有工具 schema。
    pub fn schemas(&self) -> &[ToolSchema] {
        &self.schemas
    }
}

/// Agent 主循环配置。
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// 最大步骤数（防止无限循环）
    pub max_steps: usize,
    /// 系统提示
    pub system_prompt: String,
    /// 工具注册表
    pub tools: ToolRegistry,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 10,
            system_prompt: String::new(),
            tools: ToolRegistry::new(),
        }
    }
}

/// Agent 主循环状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// 空闲，等待输入
    Idle,
    /// 正在执行中
    Running,
    /// 已停止
    Stopped,
}

/// Agent 句柄，持有 Agent 所有权。
///
/// 只有持有 `AgentHandle` 的调用方可以 `dispose()`。
#[derive(Debug)]
pub struct AgentHandle {
    /// Agent 内部状态（不可变引用）
    pub state: AgentState,
    /// 会话 ID
    pub session_id: uuid::Uuid,
}

impl AgentHandle {
    pub fn new(session_id: uuid::Uuid, state: AgentState) -> Self {
        Self { state, session_id }
    }

    /// 判断 Agent 是否正在运行。
    pub fn is_running(&self) -> bool {
        self.state == AgentState::Running
    }

    /// 判断 Agent 是否已停止。
    pub fn is_stopped(&self) -> bool {
        self.state == AgentState::Stopped
    }
}

/// 工具调用审批回调。
pub type ApprovalCallback = Arc<
    dyn Fn(&str, &Value, &Vec<ContentBlock>) -> tokio::sync::Mutex<Option<bool>> + Send + Sync,
>;
