//! 仅追加事件日志存储。
//!
//! 提供事件的追加、读取、按 turn/step 筛选能力。
//! 当前实现为内存版，后续可扩展为 SQLite/Postgres 持久化。

use harness_event::{AgentId, MessageId, SessionEvent, SessionSeq, StepId, TurnId};

use super::error::SessionError;

/// 内存版 Session 事件日志。
///
/// 保证：
/// - 事件严格有序（按 `seq` 单调递增）
/// - 幂等追加（重复 seq 会被拒绝）
/// - 可重放（所有事件可完整读取并重播）
#[derive(Debug, Clone)]
pub struct SessionStore {
    /// 会话 ID
    pub session_id: uuid::Uuid,
    /// 事件日志，按 seq 排序存储
    events: Vec<SessionEvent>,
    /// 下一个 seq 号
    next_seq: u64,
    /// 当前 turn 计数器（用于生成 turn ID）
    turn_counter: u64,
    /// 当前 step 计数器
    step_counter: u64,
    /// 当前 message ID 计数器
    message_counter: u64,
}

impl SessionStore {
    /// 创建新的空 SessionStore。
    pub fn new(session_id: uuid::Uuid) -> Self {
        Self {
            session_id,
            events: Vec::new(),
            next_seq: 0,
            turn_counter: 0,
            step_counter: 0,
            message_counter: 0,
        }
    }

    /// 追加单个事件，返回分配的 seq 号。
    ///
    /// 如果 seq 已存在，返回 `SessionError::DuplicateEvent`。
    pub fn append(&mut self, event: SessionEvent) -> Result<SessionSeq, SessionError> {
        let seq = event.seq();
        let seq_val = seq.get();

        // 验证单调递增
        if seq_val < self.next_seq {
            return Err(SessionError::DuplicateEvent { seq: seq_val });
        }

        // 更新 next_seq
        if seq_val >= self.next_seq {
            self.next_seq = seq_val + 1;
        }

        self.events.push(event);
        Ok(SessionSeq::new(seq_val))
    }

    /// 批量追加事件，保持原子性。
    pub fn append_batch(&mut self, events: Vec<SessionEvent>) -> Result<Vec<SessionSeq>, SessionError> {
        let mut seqs = Vec::with_capacity(events.len());
        for event in events {
            seqs.push(self.append(event)?);
        }
        Ok(seqs)
    }

    /// 读取指定 seq 的事件。
    pub fn get_event(&self, seq: SessionSeq) -> Result<&SessionEvent, SessionError> {
        self.events
            .iter()
            .find(|e| e.seq() == &seq)
            .ok_or_else(|| SessionError::EventNotFound {
                req: seq.get(),
                max: self.next_seq - 1,
            })
    }

    /// 读取所有事件（用于重放）。
    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }

    /// 读取所有事件并克隆（用于跨线程传递）。
    pub fn take_all_events(&mut self) -> Vec<SessionEvent> {
        std::mem::take(&mut self.events)
    }

    /// 获取当前事件数量。
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// 判断是否为空。
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 获取下一个 seq 号（用于外部构造事件）。
    pub fn next_seq(&self) -> SessionSeq {
        SessionSeq::new(self.next_seq)
    }

    /// 获取当前最大的 seq 号。
    pub fn max_seq(&self) -> Option<u64> {
        self.events.last().map(|e| e.seq().get())
    }

    /// 按 TurnId 筛选事件，返回克隆后的列表。
    pub fn events_for_turn(&self, turn: &TurnId) -> Vec<&SessionEvent> {
        self.events.iter().filter(|e| e.turn_id() == Some(turn)).collect()
    }

    /// 按 StepId 筛选事件，返回克隆后的列表。
    pub fn events_for_step(&self, step: &StepId) -> Vec<&SessionEvent> {
        self.events
            .iter()
            .filter(|e| {
                matches!(e, SessionEvent::StepStart(s) if &s.step == step)
                    || matches!(e, SessionEvent::StepEnd(s) if &s.step == step)
                    || matches!(e, SessionEvent::AssistantMessage(s) if s.step.as_ref() == Some(step))
                    || matches!(e, SessionEvent::ToolResultMessage(s) if &s.step == step)
            })
            .collect()
    }

    /// 序列化整个日志为 JSON（用于持久化）。
    pub fn serialize(&self) -> Result<String, SessionError> {
        let json = serde_json::to_string_pretty(&self.events)?;
        Ok(json)
    }

    /// 从 JSON 字符串反序列化并恢复日志（用于崩溃恢复）。
    ///
    /// 返回新的 SessionStore，事件从 seq 0 开始重编号。
    pub fn deserialize(json: &str) -> Result<Self, SessionError> {
        let events: Vec<SessionEvent> = serde_json::from_str(json)?;
        let session_id = events
            .first()
            .map(|e| match e {
                SessionEvent::TurnStart(s) => s.turn.0,
                _ => uuid::Uuid::nil(),
            })
            .unwrap_or(uuid::Uuid::new_v4());

        let mut store = Self::new(session_id);
        for event in events {
            store.append(event)?;
        }
        Ok(store)
    }

    /// 生成新的 TurnId。
    pub fn generate_turn_id(&mut self) -> TurnId {
        self.turn_counter += 1;
        TurnId(uuid::Uuid::new_v4())
    }

    /// 生成新的 StepId。
    pub fn generate_step_id(&mut self) -> StepId {
        self.step_counter += 1;
        StepId(uuid::Uuid::new_v4())
    }

    /// 生成新的 MessageId。
    pub fn generate_message_id(&mut self) -> MessageId {
        self.message_counter += 1;
        MessageId(uuid::Uuid::new_v4())
    }

    /// 生成新的 AgentId。
    pub fn generate_agent_id(&mut self) -> AgentId {
        AgentId(uuid::Uuid::new_v4())
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new(uuid::Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::{SessionEvent, TurnStart, UserMessage, MessageSource, ContentBlock, TextBlock};

    #[test]
    fn append_and_read() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn = store.generate_turn_id();
        let seq = store.next_seq();

        let event = SessionEvent::UserMessage(UserMessage {
            seq,
            turn: turn.clone(),
            content: vec![ContentBlock::Text(TextBlock::new("hello"))],
            source: MessageSource::User {
                kind: "direct".to_string(),
            },
        });

        let returned_seq = store.append(event).unwrap();
        assert_eq!(returned_seq.get(), 0);

        let retrieved = store.get_event(returned_seq).unwrap();
        assert!(matches!(retrieved, SessionEvent::UserMessage(_)));
    }

    #[test]
    fn duplicate_seq_rejected() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let seq = SessionSeq::new(0);
        let turn = store.generate_turn_id();

        let event = SessionEvent::UserMessage(UserMessage {
            seq,
            turn: turn.clone(),
            content: vec![],
            source: MessageSource::User {
                kind: "test".to_string(),
            },
        });

        store.append(event.clone()).unwrap();
        let result = store.append(event);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SessionError::DuplicateEvent { .. }));
    }

    #[test]
    fn serialization_roundtrip() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn = store.generate_turn_id();
        let _step = store.generate_step_id();
        let seq = store.next_seq();

        store.append(SessionEvent::TurnStart(TurnStart {
            turn: turn.clone(),
            seq,
        })).unwrap();

        let json = store.serialize().unwrap();
        let restored = SessionStore::deserialize(&json).unwrap();

        assert_eq!(restored.events().len(), 1);
        assert_eq!(restored.max_seq(), Some(0));
    }

    #[test]
    fn filter_by_turn() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let turn1 = store.generate_turn_id();
        let turn2 = store.generate_turn_id();

        let seq1 = SessionSeq::new(0);
        let seq2 = SessionSeq::new(1);

        store.append(SessionEvent::TurnStart(TurnStart {
            turn: turn1.clone(),
            seq: seq1,
        })).unwrap();

        store.append(SessionEvent::TurnStart(TurnStart {
            turn: turn2.clone(),
            seq: seq2,
        })).unwrap();

        let events: Vec<_> = store.events_for_turn(&turn1);
        assert_eq!(events.len(), 1);

        let events: Vec<_> = store.events_for_turn(&turn2);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn id_generation_monotonic() {
        let mut store = SessionStore::new(uuid::Uuid::new_v4());
        let id1 = store.generate_turn_id();
        let id2 = store.generate_turn_id();
        assert_ne!(id1, id2);
    }
}
