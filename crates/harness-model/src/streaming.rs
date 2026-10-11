//! 流式输出映射管线
//!
//! 将 Provider SSE 事件流映射到统一的 ContentBlock：
//!
//! LLM API SSE
//! message_start                -> 初始化 BlockPool（按 index 预留块）
//! content_block_start(index)   -> 创建对应类型块，标记流式状态
//! content_block_delta(index)   -> 按 type 追加到对应块
//!   text_delta        -> TextBlock.text += delta
//!   reasoning_delta   -> ReasoningBlock.text += delta
//!   input_json_delta  -> ToolCallBlock.arguments_json += delta
//! content_block_stop(index)    -> 标记完成（JSON 完整则 parse -> INPUT_AVAILABLE）
//! message_stop                -> 组装 AssistantMessage -> 写入 session 日志

use std::collections::HashMap;
use tracing::{info, warn};

use harness_event::{
    ContentBlock, MessageId, SessionEvent, StepId, TextBlock, ToolCallBlock, TurnId,
};

use crate::error::ModelError;
use crate::provider::ToolSchema;

/// 流式块，表示正在累积中的 ContentBlock。
#[derive(Debug)]
pub struct StreamingBlock {
    /// 块索引
    pub index: usize,
    /// 块类型
    pub block_type: String,
    /// 文本内容累积
    pub text: String,
    /// 工具调用参数累积
    pub arguments_json: String,
    /// 是否已完成
    pub completed: bool,
}

impl StreamingBlock {
    pub fn new(index: usize, block_type: &str) -> Self {
        Self {
            index,
            block_type: block_type.to_string(),
            text: String::new(),
            arguments_json: String::new(),
            completed: false,
        }
    }

    /// 追加文本 delta。
    pub fn append_text(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    /// 追加 JSON 参数 delta。
    pub fn append_arguments(&mut self, delta: &str) {
        self.arguments_json.push_str(delta);
    }

    /// 标记完成。
    pub fn mark_completed(&mut self) {
        self.completed = true;
    }

    /// 尝试解析 JSON 参数。
    pub fn try_parse_arguments(&mut self) -> Result<(), ModelError> {
        if !self.arguments_json.is_empty() {
            let parsed: serde_json::Value = serde_json::from_str(&self.arguments_json)
                .map_err(|e| ModelError::JsonParseError(e.to_string()))?;
            self.text = serde_json::to_string_pretty(&parsed).unwrap_or_default();
        }
        Ok(())
    }

    /// 转换为最终的 ContentBlock。
    pub fn to_content_block(&self, id: harness_event::ToolCallId) -> Option<ContentBlock> {
        if !self.completed {
            return None;
        }

        match self.block_type.as_str() {
            "text" | "text_delta" => {
                Some(ContentBlock::Text(TextBlock::new(self.text.clone())))
            }
            "tool_call" | "input_json" => {
                // 尝试解析 JSON
                if let Ok(parsed) = serde_json::from_str(&self.arguments_json) {
                    Some(ContentBlock::ToolCall(ToolCallBlock {
                        id: id.clone(),
                        name: self.text.clone(),
                        arguments_json: self.arguments_json.clone(),
                        arguments_parsed: Some(parsed),
                    }))
                } else {
                    // JSON 不完整，仍返回未解析版本
                    Some(ContentBlock::ToolCall(ToolCallBlock {
                        id: id.clone(),
                        name: self.text.clone(),
                        arguments_json: self.arguments_json.clone(),
                        arguments_parsed: None,
                    }))
                }
            }
            _ => {
                warn!(block_type = %self.block_type, "Unknown block type");
                None
            }
        }
    }
}

/// BlockPool，管理流式过程中的块集合。
#[derive(Debug, Default)]
pub struct BlockPool {
    blocks: HashMap<usize, StreamingBlock>,
}

impl BlockPool {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    /// 添加新块。
    pub fn add_block(&mut self, index: usize, block_type: &str) {
        if let Some(block) = self.blocks.get_mut(&index) {
            // 块已存在，更新类型
            block.block_type = block_type.to_string();
        } else {
            self.blocks.insert(index, StreamingBlock::new(index, block_type));
        }
    }

    /// 追加文本 delta。
    pub fn append_text(&mut self, index: usize, delta: &str) {
        if let Some(block) = self.blocks.get_mut(&index) {
            block.append_text(delta);
        }
    }

    /// 追加参数 delta。
    pub fn append_arguments(&mut self, index: usize, delta: &str) {
        if let Some(block) = self.blocks.get_mut(&index) {
            block.append_arguments(delta);
        }
    }

    /// 标记块完成。
    pub fn mark_completed(&mut self, index: usize) {
        if let Some(block) = self.blocks.get_mut(&index) {
            block.mark_completed();
        }
    }

    /// 获取所有完成的块。
    pub fn finish_blocks(&mut self) -> Vec<ContentBlock> {
        let mut result = Vec::new();
        let id = harness_event::ToolCallId(uuid::Uuid::new_v4());
        for block in self.blocks.values_mut() {
            if let Some(content) = block.to_content_block(id.clone()) {
                result.push(content);
            }
        }
        self.blocks.clear();
        result
    }

    /// 获取块数量。
    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}

/// 流式处理器，管理 SSE 事件到 ContentBlock 的转换。
#[derive(Debug, Default)]
pub struct StreamingProcessor {
    pool: BlockPool,
}

impl StreamingProcessor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理 OpenAI 风格的 SSE 事件。
    pub fn process_openai_chunk(&mut self, delta: &serde_json::Value) {
        if let Some(choices) = delta.get("choices") {
            if let Some(choice) = choices.as_array().and_then(|c| c.first()) {
                if let Some(delta) = choice.get("delta") {
                    if let Some(index) = choice.get("index").and_then(|i| i.as_u64()) {
                        let idx = index as usize;

                        // 处理内容块
                        if let Some(text) = delta.get("content").and_then(|t| t.as_str()) {
                            self.pool.add_block(idx, "text");
                            self.pool.append_text(idx, text);
                        }

                        // 处理工具调用
                        if let Some(tool_calls) = delta.get("tool_calls") {
                            if let Some(tc_array) = tool_calls.as_array() {
                                for tc in tc_array {
                                    if let Some(index_val) = tc.get("index").and_then(|i| i.as_u64()) {
                                        let tc_idx = index_val as usize;
                                        if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                            // 记录工具调用 ID
                                            self.pool.add_block(tc_idx, "tool_call");
                                            if let Some(name) = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) {
                                                self.pool.append_text(tc_idx, name);
                                            }
                                        }
                                        if let Some(args) = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()) {
                                            self.pool.append_arguments(tc_idx, args);
                                        }
                                    }
                                }
                            }
                        }

                        // 如果 finish_reason 是 "stop"，标记所有块完成
                        if let Some(finish_reason) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                            if finish_reason == "stop" {
                                for block in self.pool.blocks.values_mut() {
                                    block.mark_completed();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// 处理 Anthropic 风格的 SSE 事件。
    pub fn process_anthropic_chunk(&mut self, event_type: &str, data: &serde_json::Value) {
        match event_type {
            "content_block_start" => {
                if let Some(block) = data.get("content_block") {
                    if let Some(index) = block.get("index").and_then(|i| i.as_u64()) {
                        if let Some(type_) = block.get("type").and_then(|t| t.as_str()) {
                            self.pool.add_block(index as usize, type_);
                        }
                    }
                }
            }
            "content_block_delta" => {
                if let Some(block) = data.get("delta") {
                    if let Some(index) = data.get("index").and_then(|i| i.as_u64()) {
                        let idx = index as usize;
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            self.pool.append_text(idx, text);
                        }
                        if let Some(json) = block.get("partial_json").and_then(|j| j.as_str()) {
                            self.pool.append_arguments(idx, json);
                        }
                    }
                }
            }
            "content_block_stop" => {
                if let Some(index) = data.get("index").and_then(|i| i.as_u64()) {
                    self.pool.mark_completed(index as usize);
                }
            }
            _ => {
                info!(event_type = %event_type, "Unknown Anthropic event type");
            }
        }
    }

    /// 获取最终结果。
    pub fn finish(&mut self) -> Vec<ContentBlock> {
        self.pool.finish_blocks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_pool_basic() {
        let mut pool = BlockPool::new();
        pool.add_block(0, "text");
        pool.append_text(0, "Hello");
        pool.append_text(0, " World");
        pool.mark_completed(0);

        let blocks = pool.finish_blocks();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text(tb) => assert_eq!(tb.text, "Hello World"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_processor_openai_format() {
        let mut processor = StreamingProcessor::new();

        // 模拟 OpenAI 流式响应
        processor.process_openai_chunk(&serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"}
            }]
        }));
        processor.process_openai_chunk(&serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {"content": " World"}
            }]
        }));
        // 最后标记完成
        processor.process_openai_chunk(&serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        }));

        let blocks = processor.finish();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text(tb) => assert_eq!(tb.text, "Hello World"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_processor_anthropic_format() {
        let mut processor = StreamingProcessor::new();

        // 模拟 Anthropic 流式响应
        processor.process_anthropic_chunk("content_block_start", &serde_json::json!({
            "content_block": {"index": 0, "type": "text"}
        }));
        processor.process_anthropic_chunk("content_block_delta", &serde_json::json!({
            "index": 0,
            "delta": {"text": "Hello"}
        }));
        processor.process_anthropic_chunk("content_block_delta", &serde_json::json!({
            "index": 0,
            "delta": {"text": " World"}
        }));
        processor.process_anthropic_chunk("content_block_stop", &serde_json::json!({
            "index": 0
        }));

        let blocks = processor.finish();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text(tb) => assert_eq!(tb.text, "Hello World"),
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_tool_call_processing() {
        let mut processor = StreamingProcessor::new();

        // 模拟 OpenAI 工具调用
        processor.process_openai_chunk(&serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_123",
                        "type": "function",
                        "function": {"name": "read_file", "arguments": "{\"path\":\"/test\"}"}
                    }]
                }
            }]
        }));
        // 最后标记完成
        processor.process_openai_chunk(&serde_json::json!({
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        }));

        let blocks = processor.finish();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::ToolCall(tc) => {
                assert_eq!(tc.name, "read_file");
                assert_eq!(tc.arguments_json, "{\"path\":\"/test\"}");
            }
            _ => panic!("Expected ToolCall block"),
        }
    }
}
