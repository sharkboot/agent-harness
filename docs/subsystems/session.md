# Session Subsystem Design

## Event-Sourced Architecture

The Session subsystem implements event sourcing: all facts are appended to an immutable log, and message history is derived from it.

### Key Design

```
SessionEvent enum {
    // Turn lifecycle
    TurnStart { turn, seq },
    TurnEnd { turn, seq, reason },
    
    // Step lifecycle  
    StepStart { turn, step, seq },
    StepEnd { turn, step, seq },
    
    // Messages
    UserMessage { seq, turn, content, source },
    AssistantMessage { seq, turn, step, content, source },
    ToolResultMessage { seq, turn, step, tool_call_id, content, is_error },
    SystemMessage { seq, turn, content, source },
    DeveloperMessage { seq, turn, content, source },
    
    // Tool changes
    ToolAddition { seq, turn, step, message_id },
    ToolRemoval { seq, turn, step, tool_name },
    
    // Compaction
    CompactionStart { seq, turn, step },
    CompactionSummary { seq, summary },
    CompactionEnd { seq, turn, step },
    
    // Sub-agents
    SubAgentSpawn { seq, sub_agent_id, parent_turn },
    SubAgentResult { seq, sub_agent_id, result },
}
```

## Derivation

Messages are NOT stored separately. They are derived from the event log:

```rust
pub fn derive_state(store: &SessionStore) -> DerivedState {
    // Iterate events in order
    // Create Message objects from UserMessage/AssistantMessage events
    // Track TurnInfo for each turn
    // Return derived state
}
```

This ensures:
- **Single source of truth**: Event log is the canonical state
- **Replay capability**: Crash recovery by replaying events to checkpoint
- **Extensibility**: New event types can be added via declaration merging

## Persistence

Current implementation uses in-memory storage. Production targets:
- SQLite for local deployment
- Postgres for multi-instance deployment

```rust
impl SessionStore {
    pub fn serialize(&self) -> Result<String, SessionError> { ... }
    pub fn deserialize(json: &str) -> Result<Self, SessionError> { ... }
}
```
