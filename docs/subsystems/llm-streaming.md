# LLM Streaming Design

## SSE to ContentBlock Conversion

```
LLM API SSE events:
├─ message_start                → Initialize BlockPool (reserve blocks by index)
├─ content_block_start(index)   → Create typed block, mark streaming
├─ content_block_delta(index)   → Append to block by type
│     text_delta        → TextBlock.text += delta
│     reasoning_delta   → ReasoningBlock.text += delta
│     input_json_delta  → ToolCallBlock.arguments_json += delta
├─ content_block_stop(index)    → Mark complete
│     ToolCallBlock: JSON complete → parse → INPUT_AVAILABLE
│     TextBlock: complete text → trigger markdown incremental parse
└─ message_stop                 → Assemble AssistantMessage → write session log
```

## Markdown Incremental Rendering Strategy

**Problem**: Markdown is context-dependent; unclosed code fences consume subsequent content.

**Solution**:
```
Receive chunk → append to raw_buffer
    ↓ throttle 100ms → reparse entire buffer
    ↓ unclosed fence → render immediately as <pre><code> (don't wait for close)
    ↓ fence closed → apply full syntax highlighting (Shiki)
    ↓ message complete → final render (stable version)
```

## Wide Character Support

CJK and emoji alignment requires special handling:
- Use `unicode-width` crate for accurate width calculation
- Account for surrogate pairs in UTF-16 vs UTF-8
- Terminal rendering must handle double-width characters

## Tool Card State Machine

```
INPUT_STREAMING  → ⏳ Skeleton (tool name + collecting params)
       ↓
INPUT_AVAILABLE  → 🔧 Params form + "Confirm" button (HIThrottle hook)
       ↓ (user approves)
EXECUTING        → ⚙️ Spinner + elapsed time display
       ↓
OUTPUT_READY     → ✅ Call tool.output.render() to display result
       ↓
OUTPUT_ERROR     → ❌ Red error + retry option

Special handling:
- Long execution (>5s) → show ETA or progress bar
- Interrupt/cancel → show "cancelled", no error
- Timeout → OUTPUT_ERROR, error code distinguishes "timeout" vs "execution failure"
```
