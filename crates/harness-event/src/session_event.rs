//! # SessionEvent — 仅追加事件日志，Session 的唯一真源
//!
//! 所有事实追加到此枚举日志，对话历史由此派生，不单独存储 Message。
//!
//! ## 设计要点
//! - 消息历史从日志派生，不单独存储
//! - 持久化 = 持久化事件日志
//! - 崩溃恢复 = 重放事件日志到检查点
//! - 压缩是可逆的日志操作（新增 summary 事件，不删除旧事件）

use serde::{Deserialize, Serialize};

use crate::{ContentBlock, MessageId, MessageSource, SessionSeq, StepId, ToolCallId, TurnId};
use crate::types::AgentId;

// ── Turn 生命周期 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnStart {
    pub turn: TurnId,
    pub seq: SessionSeq,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnEnd {
    pub turn: TurnId,
    pub seq: SessionSeq,
    pub reason: TurnEndReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnEndReason {
    Completed,
    Rejected,
    BudgetExhausted,
    Cancelled,
    Error,
}

impl std::fmt::Display for TurnEndReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnEndReason::Completed => write!(f, "completed"),
            TurnEndReason::Rejected => write!(f, "rejected"),
            TurnEndReason::BudgetExhausted => write!(f, "budget_exhausted"),
            TurnEndReason::Cancelled => write!(f, "cancelled"),
            TurnEndReason::Error => write!(f, "error"),
        }
    }
}

// ── Step 生命周期 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepStart {
    pub turn: TurnId,
    pub step: StepId,
    pub seq: SessionSeq,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepEnd {
    pub turn: TurnId,
    pub step: StepId,
    pub seq: SessionSeq,
}

// ── 消息事件 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserMessage {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub content: Vec<ContentBlock>,
    pub source: MessageSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssistantMessage {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: Option<StepId>,
    pub content: Vec<ContentBlock>,
    pub source: MessageSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResultMessage {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: StepId,
    pub tool_call_id: ToolCallId,
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemMessage {
    pub seq: SessionSeq,
    pub turn: Option<TurnId>,
    pub content: Vec<ContentBlock>,
    pub source: MessageSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeveloperMessage {
    pub seq: SessionSeq,
    pub turn: Option<TurnId>,
    pub content: Vec<ContentBlock>,
    pub source: MessageSource,
}

// ── 工具变更事件 ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolAddition {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: StepId,
    pub message_id: MessageId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolRemoval {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: StepId,
    pub tool_name: String,
}

// ── 压缩事件 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionStart {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: StepId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionSummary {
    pub seq: SessionSeq,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionEnd {
    pub seq: SessionSeq,
    pub turn: TurnId,
    pub step: StepId,
}

// ── 子 Agent 事件 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentSpawn {
    pub seq: SessionSeq,
    pub sub_agent_id: AgentId,
    pub parent_turn: TurnId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubAgentResult {
    pub seq: SessionSeq,
    pub sub_agent_id: AgentId,
    pub result: SubAgentResultData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SubAgentResultData {
    Completed { content: Vec<ContentBlock> },
    Cancelled,
    Error { message: String },
}

// ── SessionEvent 主枚举 ───────────────────────────────────────────────────────

/// Session 事件，仅追加日志，是 Session 的唯一真源。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SessionEvent {
    // 轮次生命周期
    TurnStart(TurnStart),
    TurnEnd(TurnEnd),

    // 步骤生命周期
    StepStart(StepStart),
    StepEnd(StepEnd),

    // 消息
    UserMessage(UserMessage),
    AssistantMessage(AssistantMessage),
    ToolResultMessage(ToolResultMessage),
    SystemMessage(SystemMessage),
    DeveloperMessage(DeveloperMessage),

    // 工具变更
    ToolAddition(ToolAddition),
    ToolRemoval(ToolRemoval),

    // 上下文压缩
    CompactionStart(CompactionStart),
    CompactionSummary(CompactionSummary),
    CompactionEnd(CompactionEnd),

    // 子 Agent
    SubAgentSpawn(SubAgentSpawn),
    SubAgentResult(SubAgentResult),
}

impl SessionEvent {
    /// 获取事件序列号。
    pub fn seq(&self) -> &SessionSeq {
        match self {
            SessionEvent::TurnStart(e) => &e.seq,
            SessionEvent::TurnEnd(e) => &e.seq,
            SessionEvent::StepStart(e) => &e.seq,
            SessionEvent::StepEnd(e) => &e.seq,
            SessionEvent::UserMessage(e) => &e.seq,
            SessionEvent::AssistantMessage(e) => &e.seq,
            SessionEvent::ToolResultMessage(e) => &e.seq,
            SessionEvent::SystemMessage(e) => &e.seq,
            SessionEvent::DeveloperMessage(e) => &e.seq,
            SessionEvent::ToolAddition(e) => &e.seq,
            SessionEvent::ToolRemoval(e) => &e.seq,
            SessionEvent::CompactionStart(e) => &e.seq,
            SessionEvent::CompactionSummary(e) => &e.seq,
            SessionEvent::CompactionEnd(e) => &e.seq,
            SessionEvent::SubAgentSpawn(e) => &e.seq,
            SessionEvent::SubAgentResult(e) => &e.seq,
        }
    }

    /// 判断此事件是否为消息事件。
    pub fn is_message(&self) -> bool {
        matches!(
            self,
            SessionEvent::UserMessage(_)
                | SessionEvent::AssistantMessage(_)
                | SessionEvent::ToolResultMessage(_)
                | SessionEvent::SystemMessage(_)
                | SessionEvent::DeveloperMessage(_)
        )
    }

    /// 判断此事件是否为工具结果事件。
    pub fn is_tool_result(&self) -> bool {
        matches!(self, SessionEvent::ToolResultMessage(_))
    }

    /// 获取事件所属的 TurnId（如有）。
    pub fn turn_id(&self) -> Option<&TurnId> {
        match self {
            SessionEvent::TurnStart(e)         => Some(&e.turn),
            SessionEvent::TurnEnd(e)           => Some(&e.turn),
            SessionEvent::StepStart(e)         => Some(&e.turn),
            SessionEvent::StepEnd(e)           => Some(&e.turn),
            SessionEvent::UserMessage(e)       => Some(&e.turn),
            SessionEvent::AssistantMessage(e)  => Some(&e.turn),
            SessionEvent::ToolResultMessage(e) => Some(&e.turn),
            SessionEvent::ToolAddition(e)      => Some(&e.turn),
            SessionEvent::ToolRemoval(e)       => Some(&e.turn),
            SessionEvent::CompactionStart(e)   => Some(&e.turn),
            SessionEvent::CompactionEnd(e)     => Some(&e.turn),
            SessionEvent::SubAgentSpawn(e)     => Some(&e.parent_turn),
            SessionEvent::CompactionSummary(_) | SessionEvent::SubAgentResult(_) => None,
            SessionEvent::SystemMessage(e)     => e.turn.as_ref(),
            SessionEvent::DeveloperMessage(e)  => e.turn.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(n: u64) -> SessionSeq {
        SessionSeq::new(n)
    }

    #[test]
    fn event_seq_accessor() {
        let ev = SessionEvent::TurnStart(TurnStart {
            turn: TurnId(uuid::Uuid::new_v4()),
            seq: seq(42),
        });
        assert_eq!(ev.seq().get(), 42);
    }

    #[test]
    fn user_message_roundtrip() {
        let ev = SessionEvent::UserMessage(UserMessage {
            seq: seq(1),
            turn: TurnId(uuid::Uuid::new_v4()),
            content: vec![ContentBlock::Text(crate::TextBlock::new("hello"))],
            source: MessageSource::User {
                kind: "direct".to_string(),
            },
        });
        let json = serde_json::to_string(&ev).unwrap();
        let restored: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, restored);
    }

    #[test]
    fn assistant_message_roundtrip() {
        let step = StepId(uuid::Uuid::new_v4());
        let tc_id = ToolCallId(uuid::Uuid::new_v4());
        let ev = SessionEvent::AssistantMessage(AssistantMessage {
            seq: seq(5),
            turn: TurnId(uuid::Uuid::new_v4()),
            step: Some(step),
            content: vec![
                ContentBlock::Text(crate::TextBlock::new("let me check")),
                ContentBlock::ToolCall(crate::ToolCallBlock::new(
                    tc_id.clone(),
                    "read_file",
                    r#"{"path":"/x"}"#,
                )),
            ],
            source: MessageSource::Model {
                provider: "anthropic".to_string(),
                model: "claude-3".to_string(),
                replay_state: None,
            },
        });
        let json = serde_json::to_string(&ev).unwrap();
        let restored: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, restored);
    }

    #[test]
    fn is_message_variants() {
        let turn = TurnId(uuid::Uuid::new_v4());
        let seq = seq(0);
        assert!(SessionEvent::UserMessage(UserMessage {
            seq, turn: turn.clone(), content: vec![],
            source: MessageSource::User { kind: "d".to_string() },
        }).is_message());
        assert!(!SessionEvent::TurnStart(TurnStart { seq, turn }).is_message());
    }

    #[test]
    fn sub_agent_result_roundtrip() {
        for data in [
            SubAgentResultData::Completed { content: vec![] },
            SubAgentResultData::Cancelled,
            SubAgentResultData::Error { message: "boom".to_string() },
        ] {
            let ev = SessionEvent::SubAgentResult(SubAgentResult {
                seq: seq(10),
                sub_agent_id: AgentId(uuid::Uuid::new_v4()),
                result: data,
            });
            let json = serde_json::to_string(&ev).unwrap();
            let restored: SessionEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(ev, restored);
        }
    }

    #[test]
    fn compaction_events_roundtrip() {
        for ev in [
            SessionEvent::CompactionStart(CompactionStart {
                seq: seq(0), turn: TurnId(uuid::Uuid::new_v4()), step: StepId(uuid::Uuid::new_v4()),
            }),
            SessionEvent::CompactionSummary(CompactionSummary {
                seq: seq(1), summary: "summarized old turns".to_string(),
            }),
            SessionEvent::CompactionEnd(CompactionEnd {
                seq: seq(2), turn: TurnId(uuid::Uuid::new_v4()), step: StepId(uuid::Uuid::new_v4()),
            }),
        ] {
            let json = serde_json::to_string(&ev).unwrap();
            let restored: SessionEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(ev, restored);
        }
    }

    #[test]
    fn tool_result_roundtrip() {
        let ev = SessionEvent::ToolResultMessage(ToolResultMessage {
            seq: seq(7),
            turn: TurnId(uuid::Uuid::new_v4()),
            step: StepId(uuid::Uuid::new_v4()),
            tool_call_id: ToolCallId(uuid::Uuid::new_v4()),
            content: vec![ContentBlock::Text(crate::TextBlock::new("result"))],
            is_error: false,
        });
        let json = serde_json::to_string(&ev).unwrap();
        let restored: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, restored);
    }
}
