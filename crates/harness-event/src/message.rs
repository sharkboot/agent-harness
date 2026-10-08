//! # Message — 消息类型
//!
//! 不可变消息结构，带来源标记。
//! 消息历史由 SessionEvent 日志派生，不单独持久化。

use serde::{Deserialize, Serialize};

use crate::ContentBlock;
use crate::types::MessageId;

/// 消息角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::Developer => write!(f, "developer"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

/// 消息来源，标识消息是从哪里产生的。
///
/// 这是 Session 是唯一真源设计的关键：每条消息都有明确来源，方便追踪和调试。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum MessageSource {
    /// 用户直接输入。
    User { kind: String },
    /// 模型输出（来自某个 provider）。
    Model {
        provider: String,
        model: String,
        /// 流式回放状态（可选，用于恢复中断的流式过程）。
        replay_state: Option<serde_json::Value>,
    },
    /// 工具执行结果。
    Tool { tool_name: String, is_error: bool },
    /// 系统提示（来自 system-prompt 插件）。
    SystemPrompt { route: String, node_index: usize },
    /// 注入型消息（skill 内容、cron 通知、文件变更等）。
    Developer { kind: String },
}

/// 消息元数据，存放不直接影响解析的辅助信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MessageMetadata {
    /// 创建时间（ISO 8601）。
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// token 用量（可选，来自 provider 统计）。
    pub usage: Option<TokenUsage>,
    /// 附加的任意元数据。
    pub extra: serde_json::Value,
}

/// Token 使用统计。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 不可变消息，是会话历史的基本单元。
///
/// 注意：消息本身不持久化，持久化的是 SessionEvent。
/// 此结构由事件日志派生得到，保证一致性。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// 消息角色。
    pub role: MessageRole,
    /// 内容块列表（一个消息可以包含多个不同类型的块）。
    pub content: Vec<ContentBlock>,
    /// 来源。
    pub source: MessageSource,
    /// 元数据。
    pub metadata: MessageMetadata,
    /// 消息 ID（由事件日志中的序列号派生，用于引用）。
    pub id: MessageId,
}

impl Message {
    pub fn new(
        role: MessageRole,
        content: Vec<ContentBlock>,
        source: MessageSource,
        id: MessageId,
    ) -> Self {
        Self {
            role,
            content,
            source,
            metadata: MessageMetadata::default(),
            id,
        }
    }

    /// 提取消息中的所有工具调用块。
    pub fn tool_calls(&self) -> impl Iterator<Item = &crate::ToolCallBlock> {
        self.content.iter().filter_map(|b| {
            if let ContentBlock::ToolCall(tc) = b {
                Some(tc)
            } else {
                None
            }
        })
    }

    /// 提取消息中所有文本块，拼接为纯文本。
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::Text(tb) = b {
                    Some(tb.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 判断消息是否为空（无内容块或内容全为空文本）。
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
            || self
                .content
                .iter()
                .all(|b| matches!(b, ContentBlock::Text(tb) if tb.text.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_roundtrip() {
        let msg = Message::new(
            MessageRole::User,
            vec![ContentBlock::Text(crate::TextBlock::new("hi"))],
            MessageSource::User {
                kind: "direct".to_string(),
            },
            MessageId(uuid::Uuid::new_v4()),
        );
        let json = serde_json::to_string(&msg).unwrap();
        let restored: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, restored);
    }

    #[test]
    fn tool_calls_extraction() {
        let id1 = super::super::ToolCallId(uuid::Uuid::new_v4());
        let id2 = super::super::ToolCallId(uuid::Uuid::new_v4());
        let msg = Message::new(
            MessageRole::Assistant,
            vec![
                ContentBlock::Text(crate::TextBlock::new("let me check")),
                ContentBlock::ToolCall(crate::ToolCallBlock::new(id1.clone(), "ls", "{}")),
                ContentBlock::ToolCall(crate::ToolCallBlock::new(id2.clone(), "cat", "{}")),
            ],
            MessageSource::Model {
                provider: "openai".to_string(),
                model: "gpt-4".to_string(),
                replay_state: None,
            },
            MessageId(uuid::Uuid::new_v4()),
        );
        let calls: Vec<_> = msg.tool_calls().collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "ls");
        assert_eq!(calls[1].name, "cat");
    }

    #[test]
    fn text_content_joins() {
        let msg = Message::new(
            MessageRole::Assistant,
            vec![
                ContentBlock::Text(crate::TextBlock::new("hello")),
                ContentBlock::Reasoning(crate::ReasoningBlock::new("thinking", None)),
                ContentBlock::Text(crate::TextBlock::new("world")),
            ],
            MessageSource::Model {
                provider: "anthropic".to_string(),
                model: "claude-3".to_string(),
                replay_state: None,
            },
            MessageId(uuid::Uuid::new_v4()),
        );
        assert_eq!(msg.text_content(), "hello\nworld");
    }
}
