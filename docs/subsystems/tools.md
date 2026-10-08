# Tools Subsystem Design

## ToolDefinition Contract

Each tool declares its complete lifecycle contract:

```rust
pub struct ToolDefinition {
    // Model-facing fields (enter system prompt)
    pub name: String,
    pub description: String,          // max 1024 chars (OpenAI limit)
    pub parameters: JsonSchemaNode,
    
    // Execution-facing fields (not exposed to model)
    pub execute: Arc<dyn ToolExecutor>,
    pub timeout_ms: Option<u64>,
    pub is_concurrency_safe: bool,
    
    // Output rendering contract
    pub output: ToolOutputDefinition,
    
    // Lifecycle hooks
    pub project_content: Option<ProjectContentFn>,
    pub finalize_content: Option<FinalizeContentFn>,
}
```

## Execution Pipeline

```
Model outputs tool_call list
    ↓
[Approval Gate: human-in-the-loop]
    ↓
foreach tool_call:
    ↓ execute(args, signal) → canonical_value
    ↓ finalize_content?() final transformation
    ↓ write ToolResultMessage to session log
    ↓ derive assistant message (with tool_result)
    ↓ check budget/interrupt signal → continue or end
```

## Diff Rendering Design Position

Diff is NOT an LLM output type. It is the rendered projection of file edit tool execution results through `render()`:

```
FileEditTool.execute() → returns { old: str, new: str, path: str }
    ↓
ToolOutputDefinition.render(args, value)
    ↓ compute diff with diff-match-patch
    ↓ return Vec<ContentBlock> { type: "file_diff", path, hunk_list }
    ↓
UI receives file_diff block → renders as line-level color diff
```

## Tool Registry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
    whitelist: Option<Vec<String>>,  // None = no filter
}
```

### Features
- Register/unregister tools
- Whitelist filtering per agent permissions
- JSON Schema parameter validation
- Concurrency control

## Approval Gate

Three-tier permission model:

| Level | Scope |
|-------|-------|
| L1 Tool | Each Skill declares available tools; undeclared tools invisible to model |
| L2 Execution | File write (overwrite) / Shell dangerous commands (rm, sudo) / Outbound HTTP / Approval gates |
| L3 Process | Third-party plugins run in isolated subprocess with CPU/memory/time limits, crash recovery |
