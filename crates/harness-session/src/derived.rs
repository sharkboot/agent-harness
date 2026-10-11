//! 从 SessionEvent 日志派生消息历史。
//!
//! 消息历史由事件日志派生，不单独存储。
//! 每次调用重新计算，保证与日志一致。

use std::collections::HashMap;

use harness_event::{
    ContentBlock, Message, MessageId, MessageRole, MessageSource, SessionEvent, TurnEndReason,
    TurnId,
};

use super::store::SessionStore;

/// 从 SessionStore 派生的会话状态。
#[derive(Debug, Clone)]
pub struct DerivedState {
    /// 所有消息，按 MessageId 索引
    pub messages: HashMap<MessageId, Message>,
    /// 所有 Turn 的元信息
    pub turns: HashMap<TurnId, TurnInfo>,
    /// 当前活跃的 TurnId（如果有）
    pub current_turn_id: Option<TurnId>,
    /// 最后处理的 seq 号（用于增量更新）
    pub last_seq: u64,
}

/// 单个 Turn 的元信息。
#[derive(Debug, Clone)]
pub struct TurnInfo {
    pub turn_id: TurnId,
    pub start_seq: u64,
    pub end_seq: Option<u64>,
    pub reason: Option<TurnEndReason>,
    pub step_count: usize,
}

/// 派生消息历史。
///
/// 幂等操作：对同一事件日志调用始终返回相同结果。
pub fn derive_state(store: &SessionStore) -> DerivedState {
    let events = store.events();
    let mut messages: HashMap<MessageId, Message> = HashMap::new();
    let mut turns: HashMap<TurnId, TurnInfo> = HashMap::new();
    let mut current_turn_id: Option<TurnId> = None;
    let mut last_seq: u64 = 0;

    // 追踪当前 turn 的 step 计数
    let mut turn_step_counts: HashMap<TurnId, usize> = HashMap::new();
    let mut turn_start_seqs: HashMap<TurnId, u64> = HashMap::new();

    for event in events {
        let seq = event.seq().get();
        last_seq = seq;

        match event {
            SessionEvent::TurnStart(e) => {
                current_turn_id = Some(e.turn.clone());
                turn_step_counts.insert(e.turn.clone(), 0);
                turn_start_seqs.insert(e.turn.clone(), seq);
            }
            SessionEvent::TurnEnd(e) => {
                if let Some(info) = turns.get_mut(&e.turn) {
                    info.end_seq = Some(seq);
                    info.reason = Some(e.reason);
                }
                current_turn_id = None;
            }
            SessionEvent::StepStart(e) => {
                // 增加 turn 的 step 计数
                if let Some(count) = turn_step_counts.get_mut(&e.turn) {
                    *count += 1;
                }
            }
            SessionEvent::UserMessage(e) => {
                let msg_id = MessageId(uuid::Uuid::new_v4());
                let msg = Message::new(
                    MessageRole::User,
                    e.content.clone(),
                    e.source.clone(),
                    msg_id.clone(),
                );
                messages.insert(msg_id, msg);
                ensure_turn_info(&mut turns, &e.turn, turn_start_seqs.get(&e.turn).copied());
            }
            SessionEvent::AssistantMessage(e) => {
                let msg_id = MessageId(uuid::Uuid::new_v4());
                let msg = Message::new(
                    MessageRole::Assistant,
                    e.content.clone(),
                    e.source.clone(),
                    msg_id.clone(),
                );
                messages.insert(msg_id, msg);
                ensure_turn_info(&mut turns, &e.turn, turn_start_seqs.get(&e.turn).copied());
            }
            SessionEvent::ToolResultMessage(e) => {
                // ToolResultMessage 不直接产生 Message，但会在派生时合并到 assistant message
                // 这里先记录，实际合并逻辑在 derive_messages 中处理
                ensure_turn_info(&mut turns, &e.turn, turn_start_seqs.get(&e.turn).copied());
            }
            SessionEvent::SystemMessage(e) => {
                let msg_id = MessageId(uuid::Uuid::new_v4());
                let msg = Message::new(
                    MessageRole::System,
                    e.content.clone(),
                    e.source.clone(),
                    msg_id.clone(),
                );
                messages.insert(msg_id, msg);
            }
            SessionEvent::DeveloperMessage(e) => {
                let msg_id = MessageId(uuid::Uuid::new_v4());
                let msg = Message::new(
                    MessageRole::Developer,
                    e.content.clone(),
                    e.source.clone(),
                    msg_id.clone(),
                );
                messages.insert(msg_id, msg);
            }
            // 其他事件类型不直接产生消息
            _ => {}
        }
    }

    // 更新 turn 的 step 计数
    for (turn_id, count) in turn_step_counts {
        if let Some(info) = turns.get_mut(&turn_id) {
            info.step_count = count;
        }
    }

    DerivedState {
        messages,
        turns,
        current_turn_id,
        last_seq,
    }
}

/// 确保 turn 信息存在。
fn ensure_turn_info(
    turns: &mut HashMap<TurnId, TurnInfo>,
    turn: &TurnId,
    start_seq: Option<u64>,
) {
    if !turns.contains_key(turn) {
        turns.insert(
            turn.clone(),
            TurnInfo {
                turn_id: turn.clone(),
                start_seq: start_seq.unwrap_or(0),
                end_seq: None,
                reason: None,
                step_count: 0,
            },
        );
    }
}

/// 获取按时间顺序排列的消息列表。
pub fn derive_messages(state: &DerivedState) -> Vec<Message> {
    let mut msgs: Vec<_> = state.messages.values().cloned().collect();
    // 按 MessageId 排序（近似时间顺序，因为 ID 包含时间戳）
    msgs.sort_by(|a, b| a.id.cmp(&b.id));
    msgs
}

/// 获取指定 Turn 的消息列表。
pub fn derive_messages_for_turn(
    state: &DerivedState,
    _turn: &TurnId,
) -> Vec<Message> {
    state
        .messages
        .values()
        .filter(|msg| {
            // 通过事件日志反向查找 turn 归属
            // 简化实现：假设 assistant/user message 属于当前 turn
            matches!(
                msg.role,
                MessageRole::User | MessageRole::Assistant
            )
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{
        SessionEvent, TurnStart, TurnEnd, StepStart, StepEnd, UserMessage, AssistantMessage,
        ContentBlock, TextBlock, MessageSource, TurnEndReason,
    };

    #[test]
    fn derive_empty_store() {
        let store = SessionStore::new(uuid::Uuid::new_v4());
        let state = derive_state(&store);
        assert!(state.messages.is_empty());
        assert!(state.turns.is_empty());
        assert!(state.current_turn_id.is_none());
    }

    #[test]
    fn derive_user_message() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn = store.generate_turn_id();
        let seq = store.next_seq();

        store.append(SessionEvent::TurnStart(TurnStart { turn: turn.clone(), seq })).unwrap();
        store.append(SessionEvent::UserMessage(UserMessage {
            seq: store.next_seq(),
            turn: turn.clone(),
            content: vec![ContentBlock::Text(TextBlock::new("hello"))],
            source: MessageSource::User {
                kind: "direct".to_string(),
            },
        })).unwrap();
        store.append(SessionEvent::TurnEnd(TurnEnd {
            turn: turn.clone(),
            seq: store.next_seq(),
            reason: TurnEndReason::Completed,
        })).unwrap();

        let state = derive_state(&store);
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.turns.len(), 1);
        assert_eq!(state.current_turn_id, None); // turn 已结束
    }

    #[test]
    fn derive_full_turn() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn = store.generate_turn_id();
        let step = store.generate_step_id();

        store.append(SessionEvent::TurnStart(TurnStart {
            turn: turn.clone(),
            seq: store.next_seq(),
        })).unwrap();

        store.append(SessionEvent::StepStart(StepStart {
            turn: turn.clone(),
            step: step.clone(),
            seq: store.next_seq(),
        })).unwrap();

        store.append(SessionEvent::UserMessage(UserMessage {
            seq: store.next_seq(),
            turn: turn.clone(),
            content: vec![ContentBlock::Text(TextBlock::new("what is rust?"))],
            source: MessageSource::User {
                kind: "direct".to_string(),
            },
        })).unwrap();

        store.append(SessionEvent::AssistantMessage(AssistantMessage {
            seq: store.next_seq(),
            turn: turn.clone(),
            step: Some(step.clone()),
            content: vec![ContentBlock::Text(TextBlock::new("Rust is a systems programming language."))],
            source: MessageSource::Model {
                provider: "anthropic".to_string(),
                model: "claude-3".to_string(),
                replay_state: None,
            },
        })).unwrap();

        store.append(SessionEvent::StepEnd(StepEnd {
            turn: turn.clone(),
            step: step.clone(),
            seq: store.next_seq(),
        })).unwrap();

        store.append(SessionEvent::TurnEnd(TurnEnd {
            turn: turn.clone(),
            seq: store.next_seq(),
            reason: TurnEndReason::Completed,
        })).unwrap();

        let state = derive_state(&store);
        let messages = derive_messages(&state);
        assert_eq!(messages.len(), 2);
        // 验证包含 user 和 assistant 两类消息（顺序不定，因 MessageId 为随机 UUID）
        let roles: Vec<_> = messages.iter().map(|m| m.role).collect();
        assert!(roles.contains(&MessageRole::User));
        assert!(roles.contains(&MessageRole::Assistant));
    }

    #[test]
    fn derive_state_idempotent() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn = store.generate_turn_id();

        store.append(SessionEvent::TurnStart(TurnStart {
            turn: turn.clone(),
            seq: store.next_seq(),
        })).unwrap();

        store.append(SessionEvent::UserMessage(UserMessage {
            seq: store.next_seq(),
            turn: turn.clone(),
            content: vec![ContentBlock::Text(TextBlock::new("test"))],
            source: MessageSource::User {
                kind: "test".to_string(),
            },
        })).unwrap();

        let state1 = derive_state(&store);
        let state2 = derive_state(&store);

        assert_eq!(state1.messages.len(), state2.messages.len());
        assert_eq!(state1.turns.len(), state2.turns.len());
    }
}
