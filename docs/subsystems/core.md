# Core Runtime Design

## ContentBlock Type System

All provider outputs (OpenAI, Anthropic, Grok, DeepSeek) map to unified `ContentBlock` type:

```rust
pub enum ContentBlock {
    Text(TextBlock),
    Reasoning(ReasoningBlock),
    Image(ImageBlock),
    File(FileBlock),
    ToolCall(ToolCallBlock),
    ToolAddition(ToolAdditionBlock),
    ToolRemoval(ToolRemovalBlock),
}
```

### Design Decisions

- **Text vs Reasoning separation**: Different rendering strategies (reasoning is collapsed by default)
- **Token budget tracking**: Reasoning blocks track token usage separately from context window
- **Security isolation**: Reasoning content never injected into system prompt (prevents prompt injection via thinking)

## Message Structure

```rust
pub struct Message {
    pub role: MessageRole,        // user / assistant / tool / system / developer
    pub content: Vec<ContentBlock>,
    pub source: MessageSource,    // tracks origin: user/model/tool/system/developer
    pub metadata: MessageMetadata,
    pub id: MessageId,
}
```

### MessageSource Variants

| Variant | Description |
|---------|-------------|
| `User { kind }` | Direct user input |
| `Model { provider, model }` | LLM output |
| `Tool { tool_name, is_error }` | Tool execution result |
| `SystemPrompt { route, node_index }` | System prompt injection |
| `Developer { kind }` | Injected messages (skills, cron, file changes) |

## Agent Main Loop State Machine

```
turn/start → claim queued input → assemble prompt
    → pre-step hook (reject/enter)
    → step/start → model.chat() streaming
    → aggregate ContentBlock by index
    → has tool_call? → execute tool sequentially
    → check budget/interrupt signal → continue or end
    → turn/end (completed/rejected/budget_exhausted/cancelled/error)
```

## Ownership Model

- `AgentHandle` holds ownership and can `dispose()`
- `dispose()` stops loop → waits exit → deregisters agent → deletes session
- `resume()` restores from event log
- Cancel signal interrupts ongoing steps
