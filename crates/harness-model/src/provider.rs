//! Provider 适配器接口

use async_trait::async_trait;
use harness_event::{ContentBlock, Message};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::ModelError;

/// Provider 适配器 trait，统一 `chat()` 流式调用契约。
///
/// 所有 Provider（OpenAI / Anthropic / Grok / DeepSeek）实现此 trait。
#[async_trait]
pub trait ModelProvider: Send + Sync + 'static {
    /// Provider 名称，如 "openai"、"anthropic"。
    fn name(&self) -> &str;

    /// 当前配置模型名称。
    fn model_name(&self) -> &str;

    /// 流式聊天调用，返回 ContentBlock 列表。
    ///
    /// - `messages`: 消息历史
    /// - `tools`: 工具 schema 列表（用于 tool_use）
    /// - `signal`: 取消信号
    /// - `temperature`: 采样温度
    /// - `max_tokens`: 最大生成 token 数
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
        temperature: Option<f64>,
        max_tokens: Option<usize>,
    ) -> Result<Vec<ContentBlock>, ModelError>;

    /// 非流式聊天调用（用于测试或简单场景）。
    async fn chat_non_streaming(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        temperature: Option<f64>,
        max_tokens: Option<usize>,
    ) -> Result<Vec<ContentBlock>, ModelError>;

    /// 估算 token 使用量（基于字符数）。
    fn estimate_tokens(&self, text: &str) -> usize {
        // 简化估算：约每 4 字符一个 token
        (text.len() as f64 / 4.0).ceil() as usize
    }
}

/// 工具 schema，进入 system prompt。
#[derive(Debug, Clone)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// 模型配置。
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Provider 名称
    pub provider: String,
    /// 模型名称
    pub model: String,
    /// API key
    pub api_key: String,
    /// Base URL
    pub base_url: String,
    /// 默认 temperature
    pub temperature: Option<f64>,
    /// 默认 max_tokens
    pub max_tokens: Option<usize>,
    /// 请求超时（秒）
    pub timeout_secs: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            model: String::from("gpt-4o"),
            api_key: String::new(),
            base_url: String::from("https://api.openai.com/v1"),
            temperature: None,
            max_tokens: None,
            timeout_secs: 120,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_schema_creation() {
        let schema = ToolSchema::new(
            "test_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
        );
        assert_eq!(schema.name, "test_tool");
        assert_eq!(schema.description, "A test tool");
    }

    #[test]
    fn config_default() {
        let config = ModelConfig::default();
        assert_eq!(config.provider, "");
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.timeout_secs, 120);
    }
}
