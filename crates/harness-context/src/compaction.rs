//! Compaction 实现 — 上下文压缩

use harness_event::{SessionEvent, TurnId};
use harness_session::SessionStore;
use thiserror::Error;

use crate::error::ContextError;

/// 压缩策略配置
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// 触发阈值（token 占比，默认 80%）
    pub threshold_percent: f32,
    /// 保留最近 N 条消息
    pub keep_recent: usize,
    /// 最大 token 数（用于计算阈值）
    pub max_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 80.0,
            keep_recent: 10,
            max_tokens: 100_000,
        }
    }
}

/// 压缩结果
#[derive(Debug)]
pub struct CompactionResult {
    /// 是否执行了压缩
    pub compacted: bool,
    /// 压缩摘要（如果有）
    pub summary: Option<String>,
    /// 压缩后剩余的事件数量
    pub remaining_events: usize,
}

/// 上下文管理器，负责 prompt 组装和压缩。
#[derive(Debug)]
pub struct ContextManager {
    config: CompactionConfig,
}

impl ContextManager {
    /// 创建新的上下文管理器。
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// 创建默认配置的上下文管理器。
    pub fn default_config() -> Self {
        Self::new(CompactionConfig::default())
    }

    /// 检查是否需要压缩。
    ///
    /// 基于当前事件数量和配置的阈值判断。
    pub fn needs_compaction(&self, store: &SessionStore) -> bool {
        let current_tokens = self.estimate_tokens(store);
        let threshold = (self.config.max_tokens as f32 * self.config.threshold_percent / 100.0) as usize;
        current_tokens > threshold
    }

    /// 估算当前会话的 token 数（简化估算）。
    pub fn estimate_tokens(&self, store: &SessionStore) -> usize {
        // 简化估算：每个事件约 100 tokens
        store.events().len() * 100
    }

    /// 执行压缩。
    ///
    /// 策略：
    /// 1. 保留最近 N 条消息
    /// 2. 对早期消息生成摘要（在真实实现中会调用 LLM）
    /// 3. 将摘要写入 compaction/summary 事件
    /// 4. 原始事件保留用于重放，但不再派生到 prompt
    pub fn compact(
        &mut self,
        store: &mut SessionStore,
        turn_id: &TurnId,
    ) -> Result<CompactionResult, ContextError> {
        // 检查是否需要压缩
        if !self.needs_compaction(store) {
            return Ok(CompactionResult {
                compacted: false,
                summary: None,
                remaining_events: store.events().len(),
            });
        }

        // 获取要压缩的事件（排除最近 keep_recent 条）
        let events = store.events();
        if events.len() <= self.config.keep_recent {
            return Ok(CompactionResult {
                compacted: false,
                summary: None,
                remaining_events: events.len(),
            });
        }

        // 找到目标 turn 的事件
        let turn_events: Vec<_> = events
            .iter()
            .filter(|e| e.turn_id() == Some(turn_id))
            .collect();

        if turn_events.is_empty() {
            return Ok(CompactionResult {
                compacted: false,
                summary: None,
                remaining_events: events.len(),
            });
        }

        // 生成摘要（简化实现，真实场景应调用 LLM）
        let summary = self.generate_summary(&turn_events);

        // 写入压缩摘要事件
        let seq = store.next_seq();
        store
            .append(harness_event::SessionEvent::CompactionStart(
                harness_event::CompactionStart {
                    seq,
                    turn: turn_id.clone(),
                    step: harness_event::StepId(uuid::Uuid::new_v4()),
                },
            ))
            .map_err(ContextError::from)?;

        let seq = store.next_seq();
        store
            .append(harness_event::SessionEvent::CompactionSummary(
                harness_event::CompactionSummary {
                    seq,
                    summary: summary.clone(),
                },
            ))
            .map_err(ContextError::from)?;

        let seq = store.next_seq();
        store
            .append(harness_event::SessionEvent::CompactionEnd(
                harness_event::CompactionEnd {
                    seq,
                    turn: turn_id.clone(),
                    step: harness_event::StepId(uuid::Uuid::new_v4()),
                },
            ))
            .map_err(ContextError::from)?;

        Ok(CompactionResult {
            compacted: true,
            summary: Some(summary),
            remaining_events: store.events().len(),
        })
    }

    /// 生成消息摘要（简化实现）。
    fn generate_summary(&self, events: &[&harness_event::SessionEvent]) -> String {
        let mut summary = String::from("压缩摘要：\n");

        for event in events.iter().take(10) {
            match event {
                harness_event::SessionEvent::UserMessage(msg) => {
                    for block in &msg.content {
                        if let harness_event::ContentBlock::Text(tb) = block {
                            summary.push_str(&format!("[用户] {}\n", &tb.text[..tb.text.len().min(100)]));
                        }
                    }
                }
                harness_event::SessionEvent::AssistantMessage(msg) => {
                    for block in &msg.content {
                        if let harness_event::ContentBlock::Text(tb) = block {
                            summary.push_str(&format!("[助手] {}\n", &tb.text[..tb.text.len().min(100)]));
                        }
                    }
                }
                _ => {}
            }
        }

        summary.push_str(&format!(
            "\n...（共 {} 条事件被压缩）",
            events.len()
        ));

        summary
    }

    /// 获取压缩后的 prompt 内容。
    ///
    /// 返回不包含已压缩事件的提示词内容。
    pub fn get_prompt_content(
        &self,
        store: &SessionStore,
    ) -> Vec<harness_event::ContentBlock> {
        // 简化实现：返回所有文本块
        store
            .events()
            .iter()
            .flat_map(|event| match event {
                harness_event::SessionEvent::UserMessage(msg) => msg.content.clone(),
                harness_event::SessionEvent::AssistantMessage(msg) => msg.content.clone(),
                _ => vec![],
            })
            .filter(|block| !matches!(block, harness_event::ContentBlock::ToolCall(_)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{
        ContentBlock, MessageSource, SessionEvent, StepId, TextBlock, TurnEndReason,
        TurnStart, UserMessage,
    };

    #[test]
    fn test_needs_compaction() {
        let manager = ContextManager::default_config();
        let mut store = SessionStore::new(uuid::Uuid::new_v4());

        // 添加少量事件，不应触发压缩
        assert!(!manager.needs_compaction(&store));

        // 添加大量事件（模拟 token 超限）
        // 每个 turn 有 3 个事件（TurnStart, UserMessage, TurnEnd）
        // 需要约 270 个事件才能超过 80% 阈值（80,000 tokens）
        for i in 0..300 {
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
                    turn: turn.clone(),
                    content: vec![ContentBlock::Text(TextBlock::new(format!(
                        "Message {}",
                        i
                    )))],
                    source: MessageSource::User {
                        kind: "test".to_string(),
                    },
                }))
                .unwrap();
            store
                .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                    turn,
                    seq: store.next_seq(),
                    reason: TurnEndReason::Completed,
                }))
                .unwrap();
        }

        // 现在应该触发压缩
        assert!(manager.needs_compaction(&store));
    }

    #[test]
    fn test_compaction_no_op_when_below_threshold() {
        let mut manager = ContextManager::default_config();
        let mut store = SessionStore::new(uuid::Uuid::new_v4());

        let turn = store.generate_turn_id();
        store
            .append(SessionEvent::TurnStart(TurnStart {
                turn: turn.clone(),
                seq: store.next_seq(),
            }))
            .unwrap();

        let result = manager.compact(&mut store, &turn).unwrap();
        assert!(!result.compacted);
        assert!(result.summary.is_none());
    }

    #[test]
    fn test_compaction_generates_summary() {
        // 降低阈值以便测试能快速触发
        let mut manager = ContextManager::new(CompactionConfig {
            threshold_percent: 10.0, // 10% 阈值 = 10,000 tokens
            keep_recent: 5,
            max_tokens: 100_000,
        });
        let mut store = SessionStore::new(uuid::Uuid::new_v4());

        let turn = store.generate_turn_id();

        // 添加足够多的事件以触发压缩
        // 每个 turn 有 3 个事件，每个事件估算 100 tokens
        // 需要超过 10,000 tokens (100 个事件) 才能触发压缩
        for i in 0..50 {
            store
                .append(SessionEvent::TurnStart(TurnStart {
                    turn: turn.clone(),
                    seq: store.next_seq(),
                }))
                .unwrap();
            store
                .append(SessionEvent::UserMessage(UserMessage {
                    seq: store.next_seq(),
                    turn: turn.clone(),
                    content: vec![ContentBlock::Text(TextBlock::new(
                        format!("Test message {}", i),
                    ))],
                    source: MessageSource::User {
                        kind: "test".to_string(),
                    },
                }))
                .unwrap();
            store
                .append(SessionEvent::TurnEnd(harness_event::TurnEnd {
                    turn: turn.clone(),
                    seq: store.next_seq(),
                    reason: TurnEndReason::Completed,
                }))
                .unwrap();
        }

        let result = manager.compact(&mut store, &turn).unwrap();
        assert!(result.compacted);
        assert!(result.summary.is_some());
        assert!(result.summary.as_ref().unwrap().contains("压缩摘要"));
    }
}
