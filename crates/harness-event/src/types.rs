//! 标识符类型：SessionSeq、TurnId、StepId、MessageId、ToolCallId、AgentId、SessionId

use serde::{Deserialize, Serialize};

/// 事件日志中的单调递增序列号，保证事件严格有序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionSeq(pub u64);

impl SessionSeq {
    pub fn new(v: u64) -> Self {
        Self(v)
    }
    pub fn get(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for SessionSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "seq{}", self.0)
    }
}

/// 轮次 ID，标识一次用户输入到完成的所有步骤。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct TurnId(pub uuid::Uuid);

impl TryFrom<String> for TurnId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(TurnId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<TurnId> for String {
    fn from(id: TurnId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for TurnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "t-{}", self.0)
    }
}

/// 步骤 ID，标识一次模型调用 + 其所有工具执行。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct StepId(pub uuid::Uuid);

impl TryFrom<String> for StepId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(StepId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<StepId> for String {
    fn from(id: StepId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "s-{}", self.0)
    }
}

/// 消息 ID，唯一标识一条 Message。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct MessageId(pub uuid::Uuid);

impl TryFrom<String> for MessageId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(MessageId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<MessageId> for String {
    fn from(id: MessageId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "m-{}", self.0)
    }
}

/// 工具调用 ID，对应模型输出中一个 tool_call 的唯一标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct ToolCallId(pub uuid::Uuid);

impl TryFrom<String> for ToolCallId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(ToolCallId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<ToolCallId> for String {
    fn from(id: ToolCallId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for ToolCallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tc-{}", self.0)
    }
}

/// 会话（Session）ID，标识一次独立对话。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(pub uuid::Uuid);

impl TryFrom<String> for SessionId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(SessionId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<SessionId> for String {
    fn from(id: SessionId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sess-{}", self.0)
    }
}

/// Agent ID，标识一个 Agent 实例（含子 Agent）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(try_from = "String", into = "String")]
pub struct AgentId(pub uuid::Uuid);

impl TryFrom<String> for AgentId {
    type Error = uuid::Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(AgentId(uuid::Uuid::parse_str(&s)?))
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> Self {
        id.0.to_string()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "agent-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_monotonic() {
        let a = SessionSeq::new(1);
        let b = SessionSeq::new(2);
        assert!(a < b);
    }

    #[test]
    fn turn_id_roundtrip() {
        let original = TurnId(uuid::Uuid::new_v4());
        let json = serde_json::to_string(&original).unwrap();
        let restored: TurnId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn step_id_roundtrip() {
        let original = StepId(uuid::Uuid::new_v4());
        let json = serde_json::to_string(&original).unwrap();
        let restored: StepId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn message_id_roundtrip() {
        let original = MessageId(uuid::Uuid::new_v4());
        let json = serde_json::to_string(&original).unwrap();
        let restored: MessageId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn agent_id_roundtrip() {
        let original = AgentId(uuid::Uuid::new_v4());
        let json = serde_json::to_string(&original).unwrap();
        let restored: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }
}
