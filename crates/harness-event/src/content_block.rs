//! # ContentBlock — 模型输出的统一类型
//!
//! 所有 Provider（OpenAI / Anthropic / Grok / DeepSeek）的输出最终映射到这个联合类型。
//! 对应 DeepSeek Harness ContentBlockMap 设计，类型化块数组，而非字符串。

use serde::{Deserialize, Serialize};

/// 内容块联合类型。
///
/// 设计要点：
/// - `Text`：普通对话文本，流式累积后最终不可变
/// - `Reasoning`：思考链，与 Text 分开以便独立折叠渲染
/// - `Image`：图片数据，携带 MIME 类型
/// - `File`：文件附件（路径 + 元信息）
/// - `ToolCall`：工具调用，含流式累积的原始 JSON 参数
/// - `ToolAddition` / `ToolRemoval`：动态添加工具定义（来自开发者消息）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text(TextBlock),
    Reasoning(ReasoningBlock),
    Image(ImageBlock),
    File(FileBlock),
    ToolCall(ToolCallBlock),
    ToolAddition(ToolAdditionBlock),
    ToolRemoval(ToolRemovalBlock),
}

// ── TextBlock ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextBlock {
    /// 流式累积文本，完成后为完整不可变内容。
    pub text: String,
}

impl TextBlock {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
    pub fn empty() -> Self {
        Self { text: String::new() }
    }
}

// ── ReasoningBlock ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningBlock {
    /// 思考内容文本。
    pub text: String,
    /// 思考预算标记（token 限制），None 表示无显式预算。
    pub budget_tokens: Option<u32>,
}

impl ReasoningBlock {
    pub fn new(text: impl Into<String>, budget_tokens: Option<u32>) -> Self {
        Self {
            text: text.into(),
            budget_tokens,
        }
    }
}

// ── ImageBlock ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageBlock {
    /// 原始二进制数据（如 PNG / JPEG 字节）。
    pub data: Vec<u8>,
    /// MIME 类型，如 `"image/png"`、`"image/jpeg"`。
    pub mime: String,
}

impl ImageBlock {
    pub fn new(data: Vec<u8>, mime: impl Into<String>) -> Self {
        Self {
            data,
            mime: mime.into(),
        }
    }
}

// ── FileBlock ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileBlock {
    /// 文件路径或 URL。
    pub path: String,
    /// 可选的文件名（用于展示）。
    pub name: Option<String>,
    /// 文件大小（字节），None 表示未知。
    pub size_bytes: Option<u64>,
}

impl FileBlock {
    pub fn new(path: impl Into<String>, name: Option<String>, size_bytes: Option<u64>) -> Self {
        Self {
            path: path.into(),
            name,
            size_bytes,
        }
    }
}

// ── ToolCallBlock ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallBlock {
    /// 本次 tool_call 的唯一 ID，供后续 tool_result 引用。
    pub id: super::ToolCallId,
    /// 工具名称。
    pub name: String,
    /// 原始 JSON 字符串，流式累积，最终包含完整参数。
    pub arguments_json: String,
    /// 解析后的参数，仅在流式完成后填充（`Some`）。
    pub arguments_parsed: Option<serde_json::Value>,
}

impl ToolCallBlock {
    pub fn new(id: super::ToolCallId, name: impl Into<String>, arguments_json: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            arguments_json: arguments_json.into(),
            arguments_parsed: None,
        }
    }

    /// 尝试从已累积的 arguments_json 中解析参数。
    ///
    /// 流式过程中若 JSON 不完整则返回解析错误；调用方可据此判断是否仍在流式中。
    pub fn try_parse_arguments(&mut self) -> Result<(), serde_json::Error> {
        self.arguments_parsed = Some(serde_json::from_str(&self.arguments_json)?);
        Ok(())
    }

    /// 将已解析的参数设为给定值。
    pub fn set_parsed_arguments(&mut self, value: serde_json::Value) {
        self.arguments_parsed = Some(value);
    }
}

// ── ToolAdditionBlock ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolAdditionBlock {
    /// 工具名称（追加后对模型可见）。
    pub tool_name: String,
    /// 工具描述（进入 system prompt 的最大 1024 字符）。
    pub description: String,
    /// 工具的 JSON Schema 参数定义。
    pub parameters: serde_json::Value,
}

impl ToolAdditionBlock {
    pub fn new(
        tool_name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        // 截断 description 至 1024 字符，符合 OpenAI 限制
        let mut desc = description.into();
        if desc.len() > 1024 {
            desc.truncate(1024);
        }
        Self {
            tool_name: tool_name.into(),
            description: desc,
            parameters,
        }
    }
}

// ── ToolRemovalBlock ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRemovalBlock {
    /// 要移除的工具名称。
    pub tool_name: String,
}

impl ToolRemovalBlock {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// 判断 ContentBlock 是否为工具调用块。
pub fn is_tool_call(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::ToolCall(_))
}

/// 判断 ContentBlock 是否为文本块。
pub fn is_text(block: &ContentBlock) -> bool {
    matches!(block, ContentBlock::Text(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_block_serialize() {
        let block = ContentBlock::Text(TextBlock::new("hello"));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains("hello"));
    }

    #[test]
    fn tool_call_block_roundtrip() {
        let id = crate::ToolCallId(uuid::Uuid::new_v4());
        let block = ContentBlock::ToolCall(ToolCallBlock::new(id.clone(), "read_file", r#"{"path":"/x"}"#));
        let json = serde_json::to_string(&block).unwrap();
        let restored: ContentBlock = serde_json::from_str(&json).unwrap();
        match restored {
            ContentBlock::ToolCall(tc) => {
                assert_eq!(tc.id, id);
                assert_eq!(tc.name, "read_file");
            }
            _ => panic!("expected ToolCall"),
        }
    }

    #[test]
    fn tool_call_parse_arguments() {
        let id = crate::ToolCallId(uuid::Uuid::new_v4());
        let mut block = ToolCallBlock::new(id, "calc", r#"{"a":1,"b":2}"#);
        assert!(block.arguments_parsed.is_none());
        block.try_parse_arguments().unwrap();
        assert!(block.arguments_parsed.is_some());
    }

    #[test]
    fn tool_addition_truncates_description() {
        let long_desc = "x".repeat(2000);
        let block = ToolAdditionBlock::new("my_tool", long_desc, serde_json::json!({}));
        assert!(block.description.len() <= 1024);
    }

    #[test]
    fn reasoning_block_with_budget() {
        let block = ContentBlock::Reasoning(ReasoningBlock::new("thinking...", Some(1000)));
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("reasoning"));
        assert!(json.contains("1000"));
    }

    #[test]
    fn image_block_roundtrip() {
        let block = ContentBlock::Image(ImageBlock::new(vec![1, 2, 3], "image/png"));
        let json = serde_json::to_string(&block).unwrap();
        let restored: ContentBlock = serde_json::from_str(&json).unwrap();
        match restored {
            ContentBlock::Image(img) => {
                assert_eq!(img.mime, "image/png");
                assert_eq!(img.data, vec![1, 2, 3]);
            }
            _ => panic!("expected Image"),
        }
    }
}
