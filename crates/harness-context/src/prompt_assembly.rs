//! System prompt 组装

use harness_event::{ContentBlock, Message, MessageRole, TextBlock};
use harness_session::SessionStore;

/// System prompt 组装器。
///
/// 组装格式：
///
/// ```ignore
/// System Prompt = 固定片段 + 动态片段 + 工具 Schema
///
/// 固定片段（来自 system-prompt 插件）：
///   - 角色定义
///   - 行为规范
///   - 安全约束
///
/// 动态片段（每次 step 前计算）：
///   - 当前任务上下文（当前目录、AGENTS.md、skill 内容）
///   - 文件变更通知（如果上次 step 后有文件修改）
///   - Skill 加载内容（渐进式，仅加载需要的 skill）
///
/// 工具 Schema（来自 tool registry，白名单过滤）：
///   - 只暴露当前 agent 有权使用的工具
///   - 每个工具的 description 最长 1024 字符（OpenAI 限制）
/// ```
#[derive(Debug, Clone)]
pub struct PromptAssembler {
    /// 固定系统提示
    pub system_template: String,
    /// 动态上下文
    pub context: PromptContext,
}

/// 动态上下文信息
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    /// 当前工作目录
    pub current_directory: Option<String>,
    /// AGENTS.md 内容（如有）
    pub agents_md: Option<String>,
    /// 加载的 skills
    pub skills: Vec<String>,
    /// 文件变更通知
    pub file_changes: Vec<String>,
}

impl PromptAssembler {
    /// 创建新的 prompt 组装器。
    pub fn new(system_template: String) -> Self {
        Self {
            system_template,
            context: PromptContext::default(),
        }
    }

    /// 从 SessionStore 组装 prompt。
    ///
    /// 返回组装后的消息列表（system + history）。
    pub fn assemble_prompt(
        &self,
        store: &SessionStore,
        tools: &[ToolSchema],
    ) -> Vec<Message> {
        let mut messages = Vec::new();

        // 1. 添加 system message
        messages.push(Message::new(
            MessageRole::System,
            vec![ContentBlock::Text(TextBlock::new(
                self.system_template.clone(),
            ))],
            harness_event::MessageSource::SystemPrompt {
                route: "default".to_string(),
                node_index: 0,
            },
            harness_event::MessageId(uuid::Uuid::new_v4()),
        ));

        // 2. 添加动态上下文
        if let Some(ref dir) = self.context.current_directory {
            messages.push(Message::new(
                MessageRole::Developer,
                vec![ContentBlock::Text(TextBlock::new(format!(
                    "当前工作目录: {}",
                    dir
                )))],
                harness_event::MessageSource::Developer {
                    kind: "context".to_string(),
                },
                harness_event::MessageId(uuid::Uuid::new_v4()),
            ));
        }

        if let Some(ref agents_md) = self.context.agents_md {
            messages.push(Message::new(
                MessageRole::Developer,
                vec![ContentBlock::Text(TextBlock::new(format!(
                    "AGENTS.md 内容:\n{}",
                    agents_md
                )))],
                harness_event::MessageSource::Developer {
                    kind: "agents_md".to_string(),
                },
                harness_event::MessageId(uuid::Uuid::new_v4()),
            ));
        }

        // 3. 添加工具 schema
        if !tools.is_empty() {
            let tool_schema_text = tools
                .iter()
                .map(|t| format!("{}: {}\n参数: {}", t.name, t.description, t.parameters))
                .collect::<Vec<_>>()
                .join("\n\n");

            messages.push(Message::new(
                MessageRole::System,
                vec![ContentBlock::Text(TextBlock::new(format!(
                    "可用工具:\n{}",
                    tool_schema_text
                )))],
                harness_event::MessageSource::SystemPrompt {
                    route: "tools".to_string(),
                    node_index: 0,
                },
                harness_event::MessageId(uuid::Uuid::new_v4()),
            ));
        }

        // 4. 从事件日志派生历史消息
        let derived = harness_session::derive_state(store);
        let history = harness_session::derive_messages(&derived);

        // 过滤掉已压缩的消息（简化实现）
        let active_messages: Vec<_> = history
            .into_iter()
            .filter(|msg| !matches!(msg.role, MessageRole::Developer))
            .collect();

        messages.extend(active_messages);

        messages
    }

    /// 更新动态上下文。
    pub fn update_context(&mut self, context: PromptContext) {
        self.context = context;
    }
}

/// 工具 schema，进入 prompt。
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

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{
        ContentBlock, MessageRole, MessageSource, SessionEvent, TextBlock, TurnStart,
        UserMessage,
    };

    #[test]
    fn test_assemble_prompt_basic() {
        let assembler = PromptAssembler::new("You are a helpful assistant.".to_string());
        let mut store = SessionStore::new(uuid::Uuid::new_v4());

        let turn = store.generate_turn_id();
        store
            .append(SessionEvent::TurnStart(TurnStart {
                turn: turn.clone(),
                seq: store.next_seq(),
            }))
            .unwrap();
        store
            .append(SessionEvent::UserMessage(UserMessage {
                seq: store.next_seq(),
                turn,
                content: vec![ContentBlock::Text(TextBlock::new("Hello"))],
                source: MessageSource::User {
                    kind: "direct".to_string(),
                },
            }))
            .unwrap();

        let messages = assembler.assemble_prompt(&store, &[]);
        assert!(!messages.is_empty());
        assert_eq!(messages[0].role, MessageRole::System);
    }

    #[test]
    fn test_assemble_prompt_with_tools() {
        let assembler = PromptAssembler::new("Test".to_string());
        let store = SessionStore::new(uuid::Uuid::new_v4());

        let tools = vec![ToolSchema::new(
            "test_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
        )];

        let messages = assembler.assemble_prompt(&store, &tools);
        assert!(!messages.is_empty());
    }
}
