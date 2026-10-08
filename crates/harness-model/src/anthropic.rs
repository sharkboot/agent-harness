//! Anthropic Provider 适配器

use async_trait::async_trait;
use futures::StreamExt;
use harness_event::{ContentBlock, Message, MessageRole};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::error::ModelError;
use crate::provider::{ModelConfig, ModelProvider, ToolSchema};
use crate::streaming::StreamingProcessor;

/// Anthropic Provider 适配器。
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    config: ModelConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// 创建新的 Anthropic Provider。
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        if config.api_key.is_empty() {
            return Err(ModelError::InvalidConfig("Anthropic API key is required".to_string()));
        }
        Ok(Self {
            config,
            client: reqwest::Client::new(),
        })
    }

    /// 构建请求体。
    fn build_request_body(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        temperature: Option<f64>,
        max_tokens: Option<usize>,
    ) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": &self.config.model,
            "max_tokens": max_tokens.or(self.config.max_tokens).unwrap_or(1024),
            "messages": Self::messages_to_anthropic_format(messages),
            "stream": true,
        });

        if let Some(temp) = temperature.or(self.config.temperature) {
            body["temperature"] = serde_json::json!(temp);
        }

        // 添加工具定义
        if !tools.is_empty() {
            let anthropic_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(anthropic_tools);
        }

        body
    }

    /// 将内部 Message 转换为 Anthropic 格式。
    pub fn messages_to_anthropic_format(messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|msg| {
                let mut json = match msg.role {
                    MessageRole::User => serde_json::json!({"role": "user"}),
                    MessageRole::Assistant => serde_json::json!({"role": "assistant"}),
                    MessageRole::System => serde_json::json!({"role": "system"}),
                    MessageRole::Developer => serde_json::json!({"role": "user"}),
                    MessageRole::Tool => serde_json::json!({"role": "user"}),
                };

                let mut content = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(tb) => {
                            content.push(serde_json::json!({
                                "type": "text",
                                "text": &tb.text,
                            }));
                        }
                        ContentBlock::ToolCall(tc) => {
                            content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id.to_string(),
                                "name": &tc.name,
                                "input": tc.arguments_parsed.clone().unwrap_or(serde_json::json!({})),
                            }));
                        }
                        ContentBlock::Reasoning(rb) => {
                            content.push(serde_json::json!({
                                "type": "text",
                                "text": &rb.text,
                                "cache_control": {"type": "ephemeral"},
                            }));
                        }
                        _ => {}
                    }
                }

                json["content"] = serde_json::json!(content);

                json
            })
            .collect()
    }

    /// 解析 Anthropic 流式响应。
    async fn parse_stream(
        &self,
        stream: impl futures::Stream<Item = reqwest::Result<Vec<u8>>>,
    ) -> Result<Vec<ContentBlock>, ModelError> {
        let mut processor = StreamingProcessor::new();
        let mut buffer = String::new();

        tokio::pin!(stream);

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ModelError::StreamError(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // 处理完整的 SSE 行
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(event_type) = json.get("type").and_then(|t| t.as_str()) {
                            processor.process_anthropic_chunk(event_type, &json);
                        }
                    }
                }
            }
        }

        Ok(processor.finish())
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }

    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>>,
        temperature: Option<f64>,
        max_tokens: Option<usize>,
    ) -> Result<Vec<ContentBlock>, ModelError> {
        let body = self.build_request_body(messages, tools, temperature, max_tokens);

        info!(
            model = %self.config.model,
            messages_count = messages.len(),
            tools_count = tools.len(),
            "Sending Anthropic chat request"
        );

        let response = self
            .client
            .post(format!("{}/v1/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::RequestError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            warn!(status, error = %error_text, "Anthropic API error");
            return Err(ModelError::ApiError {
                status,
                message: error_text,
            });
        }

        let stream = response.bytes_stream();
        self.parse_stream(stream).await
    }

    async fn chat_non_streaming(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        temperature: Option<f64>,
        max_tokens: Option<usize>,
    ) -> Result<Vec<ContentBlock>, ModelError> {
        let mut body = self.build_request_body(messages, tools, temperature, max_tokens);
        body["stream"] = serde_json::json!(false);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::RequestError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ModelError::ApiError {
                status,
                message: error_text,
            });
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::JsonParseError(e))?;

        let mut blocks = Vec::new();
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            for item in content {
                match item.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            blocks.push(ContentBlock::Text(harness_event::TextBlock::new(text)));
                        }
                    }
                    Some("tool_use") => {
                        if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                                if let Some(input) = item.get("input") {
                                    let arguments_json = serde_json::to_string(input)
                                        .unwrap_or_default();
                                    blocks.push(ContentBlock::ToolCall(harness_event::ToolCallBlock {
                                        id: harness_event::ToolCallId(
                                            uuid::Uuid::parse_str(id).unwrap_or(uuid::Uuid::nil()),
                                        ),
                                        name: name.to_string(),
                                        arguments_json,
                                        arguments_parsed: Some(input.clone()),
                                    }));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{ContentBlock, Message, MessageRole, TextBlock};

    #[test]
    fn test_message_conversion() {
        let messages = vec![
            Message::new(
                MessageRole::User,
                vec![ContentBlock::Text(TextBlock::new("Hello"))],
                harness_event::MessageSource::User {
                    kind: "direct".to_string(),
                },
                harness_event::MessageId(uuid::Uuid::new_v4()),
            ),
            Message::new(
                MessageRole::Assistant,
                vec![ContentBlock::Text(TextBlock::new("Hi there"))],
                harness_event::MessageSource::Model {
                    provider: "anthropic".to_string(),
                    model: "claude-3".to_string(),
                    replay_state: None,
                },
                harness_event::MessageId(uuid::Uuid::new_v4()),
            ),
        ];

        let anthropic_msgs = AnthropicProvider::messages_to_anthropic_format(&messages);
        assert_eq!(anthropic_msgs.len(), 2);
        assert_eq!(anthropic_msgs[0]["role"], "user");
        assert_eq!(anthropic_msgs[1]["role"], "assistant");
    }

    #[test]
    fn test_build_request_body() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-opus-20240229".to_string(),
            api_key: "test-key".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(1000),
            timeout_secs: 60,
        };
        let provider = AnthropicProvider::new(config).unwrap();

        let messages = vec![];
        let tools = vec![ToolSchema::new(
            "test_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
        )];

        let body = provider.build_request_body(&messages, &tools, None, None);
        assert_eq!(body["model"], "claude-3-opus-20240229");
        assert_eq!(body["stream"], true);
        assert!(body["tools"].is_array());
    }
}
