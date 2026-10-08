//! 工具定义契约 — 每个工具声明完整生命周期

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use harness_event::{ContentBlock, ToolCallBlock};
use tokio::sync::Mutex;

/// 工具输出渲染定义。
///
/// 不同工具有不同的输出形态（diff / 表格 / 文本 / 图片）。
/// 渲染逻辑随工具定义，不集中在 harness 全局（ADR-003）。
#[derive(Clone)]
pub struct ToolOutputDefinition {
    /// 输出 JSON Schema
    pub schema: serde_json::Value,
    /// 渲染函数：将执行结果投影为 ContentBlock 列表
    /// 入参：(arguments, canonical_value)
    pub render: Arc<dyn ToolRenderFnTrait>,
    /// 可选的展示元数据变换
    pub presentation_meta: Option<Arc<dyn PresentationMetaFnTrait>>,
}

/// 工具渲染函数 trait。
pub trait ToolRenderFnTrait: Send + Sync {
    fn call(&self, args: &serde_json::Value, value: &serde_json::Value) -> Vec<ContentBlock>;
}

impl<F> ToolRenderFnTrait for F
where
    F: Fn(&serde_json::Value, &serde_json::Value) -> Vec<ContentBlock> + Send + Sync + 'static,
{
    fn call(&self, args: &serde_json::Value, value: &serde_json::Value) -> Vec<ContentBlock> {
        self(args, value)
    }
}

/// 展示元数据变换函数 trait。
pub trait PresentationMetaFnTrait: Send + Sync {
    fn call(&self, args: &serde_json::Value, value: &serde_json::Value) -> serde_json::Value;
}

impl<F> PresentationMetaFnTrait for F
where
    F: Fn(&serde_json::Value, &serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
{
    fn call(&self, args: &serde_json::Value, value: &serde_json::Value) -> serde_json::Value {
        self(args, value)
    }
}

/// 执行后替换展示内容的钩子（projectContent）。
///
/// 在执行后、进入日志前**替换**渲染内容。
/// 例如把原始 diff 文本换成带行号的 HTML。
pub type ProjectContentFn = Arc<
    dyn Fn(&serde_json::Value, serde_json::Value) -> Result<serde_json::Value, ToolError> + Send + Sync,
>;

/// 最终变换（finalizeContent）。
///
/// 同步、不可失败 — 即使返回 Err 也应使用原始值。
pub type FinalizeContentFn = Arc<
    dyn Fn(&serde_json::Value, serde_json::Value) -> serde_json::Value + Send + Sync,
>;

/// 工具定义，完整声明工具的生命周期契约。
///
/// 面向模型的字段会进入 system prompt，面向执行的字段不暴露给模型。
#[derive(Clone)]
pub struct ToolDefinition {
    // ── 面向模型的字段（进入 system prompt）──
    /// 工具名称
    pub name: String,
    /// 工具描述（最长 1024 字符，OpenAI 限制）
    pub description: String,
    /// 工具的 JSON Schema 参数定义
    pub parameters: serde_json::Value,

    // ── 面向执行的字段（不暴露给模型）──
    /// 工具执行器（trait object）
    pub execute: Arc<dyn ToolExecutor>,
    /// 超时时间（毫秒），None 表示无限制
    pub timeout_ms: Option<u64>,
    /// 是否支持并发执行
    pub is_concurrency_safe: bool,

    // ── 输出渲染契约 ──
    /// 输出定义（schema + render）
    pub output: ToolOutputDefinition,

    // ── 生命周期钩子 ──
    /// 执行后替换展示内容
    pub project_content: Option<ProjectContentFn>,
    /// 最终变换（同步、不可失败）
    pub finalize_content: Option<FinalizeContentFn>,
}

impl ToolDefinition {
    /// 创建新的工具定义。
    ///
    /// description 自动截断至 1024 字符（OpenAI 限制）。
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        execute: Arc<dyn ToolExecutor>,
        output: ToolOutputDefinition,
    ) -> Self {
        // 截断 description 至 1024 字符
        let mut desc = description.into();
        if desc.len() > 1024 {
            desc.truncate(1024);
        }

        Self {
            name: name.into(),
            description: desc,
            parameters,
            execute,
            timeout_ms: None,
            is_concurrency_safe: true,
            output,
            project_content: None,
            finalize_content: None,
        }
    }

    /// 设置超时时间。
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// 设置是否支持并发。
    pub fn with_concurrency(mut self, safe: bool) -> Self {
        self.is_concurrency_safe = safe;
        self
    }

    /// 设置项目内容钩子。
    pub fn with_project_content<F>(mut self, f: F) -> Self
    where
        F: Fn(&serde_json::Value, serde_json::Value) -> Result<serde_json::Value, ToolError> + Send + Sync + 'static,
    {
        self.project_content = Some(Arc::new(f));
        self
    }

    /// 设置最终变换钩子。
    pub fn with_finalize_content<F>(mut self, f: F) -> Self
    where
        F: Fn(&serde_json::Value, serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
    {
        self.finalize_content = Some(Arc::new(f));
        self
    }

    /// 获取工具名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取工具描述。
    pub fn description(&self) -> &str {
        &self.description
    }

    /// 获取工具参数 Schema。
    pub fn parameters(&self) -> &serde_json::Value {
        &self.parameters
    }

    /// 判断工具是否支持并发。
    pub fn is_concurrency_safe(&self) -> bool {
        self.is_concurrency_safe
    }

    /// 获取超时时间。
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }
}

/// 工具执行器 trait，供外部实现具体工具逻辑。
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync + 'static {
    /// 执行工具。
    ///
    /// - `args`: 工具调用参数（已解析的 JSON Value）
    /// - `signal`: 取消信号，可用于中断长时间执行
    ///
    /// 返回执行结果的 canonical_value（serde_json::Value）。
    async fn execute(
        &self,
        args: &serde_json::Value,
        signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<serde_json::Value, ToolError>;
}

/// 工具执行错误。
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("工具执行超时: {tool_name}")]
    Timeout { tool_name: String },

    #[error("工具执行被取消: {tool_name}")]
    Cancelled { tool_name: String },

    #[error("工具执行失败: {tool_name}: {reason}")]
    ExecutionFailed { tool_name: String, reason: String },

    #[error("工具未找到: {0}")]
    NotFound(String),

    #[error("参数验证失败: {0}")]
    ValidationError(String),

    #[error("渲染失败: {0}")]
    RenderError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_truncation() {
        let long_desc = "x".repeat(2000);
        let tool = ToolDefinition::new(
            "test_tool",
            long_desc,
            serde_json::json!({}),
            Arc::new(MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        );
        assert!(tool.description().len() <= 1024);
        assert_eq!(tool.name(), "test_tool");
    }

    #[test]
    fn tool_builder_pattern() {
        let tool = ToolDefinition::new(
            "my_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
            Arc::new(MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        )
        .with_timeout(5000)
        .with_concurrency(false);

        assert_eq!(tool.timeout_ms(), Some(5000));
        assert!(!tool.is_concurrency_safe());
    }
}

/// Mock executor for testing.
pub struct MockExecutor;

#[async_trait::async_trait]
impl ToolExecutor for MockExecutor {
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<serde_json::Value, ToolError> {
        Ok(serde_json::json!({"result": "ok"}))
    }
}
