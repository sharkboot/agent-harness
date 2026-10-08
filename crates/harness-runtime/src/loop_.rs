//! 主循环实现：turn → step → model → tool 的流程控制。

use std::sync::Arc;

use harness_event::{
    AssistantMessage, ContentBlock, Message, MessageRole, MessageSource, SessionEvent, StepEnd,
    StepStart, TextBlock, ToolCallBlock, ToolResultMessage, TurnEndReason,
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::error::RuntimeError;
use super::traits::{AgentConfig, ModelService, ToolExecutor, ToolRegistry};
use super::AgentInner;

/// 步骤执行器，封装单步的 model call + tool execution 逻辑。
pub struct StepExecutor<'a> {
    inner: &'a mut AgentInner,
    tools: &'a ToolRegistry,
    model: &'a dyn ModelService,
}

impl<'a> StepExecutor<'a> {
    pub fn new(
        inner: &'a mut AgentInner,
        tools: &'a ToolRegistry,
        model: &'a dyn ModelService,
    ) -> Self {
        Self { inner, tools, model }
    }

    /// 执行单个 step：模型调用 + 工具执行。
    pub async fn execute_step(
        &mut self,
        turn_id: &harness_event::TurnId,
        step_id: harness_event::StepId,
    ) -> Result<(), RuntimeError> {
        // 写入 StepStart
        self.inner
            .store
            .append(SessionEvent::StepStart(StepStart {
                turn: turn_id.clone(),
                step: step_id.clone(),
                seq: self.inner.store.next_seq(),
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        // 获取当前消息历史
        let messages = self.current_messages();

        // 调用模型
        let signal = Arc::new(Mutex::new(None));
        let content_blocks = self.model.chat(&messages, self.tools.schemas(), &signal).await?;

        // 检查是否有 tool calls
        let tool_calls: Vec<_> = content_blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolCall(tc) => Some(tc.clone()),
                _ => None,
            })
            .collect();

        if tool_calls.is_empty() {
            // 无工具调用，直接写入 assistant message
            self.write_assistant_message(turn_id, Some(step_id.clone()), content_blocks)?;
        } else {
            // 执行工具调用
            self.execute_tool_calls(turn_id, &step_id, content_blocks, tool_calls)
                .await?;
        }

        // 写入 StepEnd
        self.inner
            .store
            .append(SessionEvent::StepEnd(StepEnd {
                turn: turn_id.clone(),
                step: step_id,
                seq: self.inner.store.next_seq(),
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        Ok(())
    }

    /// 获取当前消息历史。
    fn current_messages(&self) -> Vec<Message> {
        let state = self.inner.derive_state();
        harness_session::derive_messages(&state)
    }

    /// 写入助手消息。
    fn write_assistant_message(
        &mut self,
        turn_id: &harness_event::TurnId,
        step_id: Option<harness_event::StepId>,
        content: Vec<ContentBlock>,
    ) -> Result<(), RuntimeError> {
        self.inner
            .store
            .append(SessionEvent::AssistantMessage(AssistantMessage {
                seq: self.inner.store.next_seq(),
                turn: turn_id.clone(),
                step: step_id,
                content,
                source: MessageSource::Model {
                    provider: "local".to_string(),
                    model: "mock".to_string(),
                    replay_state: None,
                },
            }))
            .map_err(|e| RuntimeError::Session(e))?;
        Ok(())
    }

    /// 执行工具调用链。
    async fn execute_tool_calls(
        &mut self,
        turn_id: &harness_event::TurnId,
        step_id: &harness_event::StepId,
        content_blocks: Vec<ContentBlock>,
        tool_calls: Vec<ToolCallBlock>,
    ) -> Result<(), RuntimeError> {
        for tc in tool_calls {
            // 执行工具
            let result = if let Some(executor) = self.tools.get(&tc.name) {
                let args = tc
                    .arguments_parsed
                    .as_ref()
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                executor
                    .execute(
                        &tc.name,
                        &args,
                        &Arc::new(Mutex::new(None)),
                    )
                    .await
            } else {
                warn!(tool_name = %tc.name, "Tool not found");
                Err(RuntimeError::ToolExecutionError {
                    tool_name: tc.name.clone(),
                    reason: "Tool not registered".to_string(),
                })
            };

            // 写入工具结果
            let (result_content, is_error) = match result {
                Ok(content) => (content, false),
                Err(e) => (
                    vec![ContentBlock::Text(TextBlock::new(format!(
                        "Error: {}",
                        e
                    )))],
                    true,
                ),
            };

            self.inner
                .store
                .append(SessionEvent::ToolResultMessage(ToolResultMessage {
                    seq: self.inner.store.next_seq(),
                    turn: turn_id.clone(),
                    step: step_id.clone(),
                    tool_call_id: tc.id.clone(),
                    content: result_content,
                    is_error,
                }))
                .map_err(|e| RuntimeError::Session(e))?;
        }

        // 写入合并后的 assistant message
        self.write_assistant_message(turn_id, Some(step_id.clone()), content_blocks)?;

        Ok(())
    }
}

/// 主循环：驱动 turn 和 step 的执行。
pub struct AgentLoop<'a> {
    inner: &'a mut AgentInner,
    tools: &'a ToolRegistry,
    model: &'a dyn ModelService,
    config: &'a AgentConfig,
}

impl<'a> AgentLoop<'a> {
    pub fn new(
        inner: &'a mut AgentInner,
        tools: &'a ToolRegistry,
        model: &'a dyn ModelService,
        config: &'a AgentConfig,
    ) -> Self {
        Self {
            inner,
            tools,
            model,
            config,
        }
    }

    /// 执行单个轮次的主循环。
    ///
    /// 状态机：
    /// turn/start -> claim 排队输入 -> 组装 prompt
    /// -> agent/pre-step hook（reject / enter）
    /// -> step/start -> model.chat() 流式调用
    /// -> 按 index 聚合 ContentBlock
    /// -> 有 tool_call? 逐工具 execute() / 否则 turn/end(completed)
    /// -> 检查 budget / 中断信号 -> 继续或结束
    pub async fn run_turn(
        &mut self,
        user_input: Vec<ContentBlock>,
    ) -> Result<Vec<ContentBlock>, RuntimeError> {
        // 1. turn/start
        let turn_id = self.inner.store.generate_turn_id();
        self.inner
            .store
            .append(SessionEvent::TurnStart(harness_event::TurnStart {
                turn: turn_id.clone(),
                seq: self.inner.store.next_seq(),
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        // 2. 写入用户消息
        self.inner
            .store
            .append(SessionEvent::UserMessage(harness_event::UserMessage {
                seq: self.inner.store.next_seq(),
                turn: turn_id.clone(),
                content: user_input,
                source: MessageSource::User {
                    kind: "direct".to_string(),
                },
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        // 3. pre-step hook（简化实现，暂不拒绝）
        // TODO: 实现真实的 pre-step hook

        let mut last_output = Vec::new();

        // 4. 步骤循环
        for step_num in 0..self.config.max_steps {
            // 检查取消信号
            if self.inner.is_cancelled().await {
                self.inner
                    .store
                    .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                        turn: turn_id.clone(),
                        seq: self.inner.store.next_seq(),
                        reason: TurnEndReason::Cancelled,
                    }))
                    .map_err(|e| RuntimeError::Session(e))?;
                return Err(RuntimeError::Cancelled);
            }

            let step_id = self.inner.store.generate_step_id();
            info!(turn = %turn_id, step = step_num, "Executing step");

            let mut step_executor = StepExecutor::new(self.inner, self.tools, self.model);
            step_executor
                .execute_step(&turn_id, step_id.clone())
                .await?;

            // 获取最新 assistant 消息的输出
            let state = self.inner.derive_state();
            let messages = harness_session::derive_messages(&state);
            if let Some(last_msg) = messages.last() {
                if last_msg.role == MessageRole::Assistant {
                    last_output = last_msg.content.clone();
                }
            }
        }

        // 5. turn/end (completed)
        self.inner
            .store
            .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                turn: turn_id,
                seq: self.inner.store.next_seq(),
                reason: TurnEndReason::Completed,
            }))
            .map_err(|e| RuntimeError::Session(e))?;

        Ok(last_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{ContentBlock, TextBlock, UserMessage, MessageSource};

    // Mock model service
    struct MockModel;

    #[async_trait::async_trait]
    impl ModelService for MockModel {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: &[super::super::traits::ToolSchema],
            _signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
        ) -> Result<Vec<ContentBlock>, RuntimeError> {
            // 返回一个简单的文本响应
            Ok(vec![ContentBlock::Text(TextBlock::new("Mock response"))])
        }
    }

    // Mock tool executor
    struct MockTool;

    #[async_trait::async_trait]
    impl ToolExecutor for MockTool {
        async fn execute(
            &self,
            _name: &str,
            _args: &serde_json::Value,
            _signal: &Arc<Mutex<Option<tokio::sync::Semaphore>>>,
        ) -> Result<Vec<ContentBlock>, RuntimeError> {
            Ok(vec![ContentBlock::Text(TextBlock::new("Tool result"))])
        }
    }

    #[tokio::test]
    async fn test_run_turn_basic() {
        let mut inner = AgentInner::new(
            harness_event::AgentId(uuid::Uuid::new_v4()),
            uuid::Uuid::new_v4(),
            AgentConfig::default(),
        );

        let model = MockModel;
        let tools = ToolRegistry::new();
        let config = AgentConfig::default();

        let mut loop_ = AgentLoop::new(&mut inner, &tools, &model, &config);

        let result = loop_
            .run_turn(vec![ContentBlock::Text(TextBlock::new("Hello"))])
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.len(), 1);
    }

    #[tokio::test]
    async fn test_turn_end_reasons() {
        let mut inner = AgentInner::new(
            harness_event::AgentId(uuid::Uuid::new_v4()),
            uuid::Uuid::new_v4(),
            AgentConfig::default(),
        );

        // Test completed
        let turn1 = inner.store.generate_turn_id();
        inner
            .store
            .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                turn: turn1,
                seq: inner.store.next_seq(),
                reason: TurnEndReason::Completed,
            }))
            .unwrap();

        // Test cancelled
        let turn2 = inner.store.generate_turn_id();
        inner
            .store
            .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                turn: turn2,
                seq: inner.store.next_seq(),
                reason: TurnEndReason::Cancelled,
            }))
            .unwrap();

        assert_eq!(inner.store.len(), 2);
    }
}
