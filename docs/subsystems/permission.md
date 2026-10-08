# Permission System Design

## Three-Layer Permission Model

```
┌─────────────────────────────────────────────────────────┐
│ L1: Tool-Level Permissions                              │
│   - Each Skill declares available tools                  │
│   - Undeclared tools invisible to model                  │
│   - Activated only after slash-activate                  │
├─────────────────────────────────────────────────────────┤
│ L2: Execution-Level Permissions                         │
│   - File write: allow overwrite?                         │
│   - Shell commands: block dangerous (rm, sudo)           │
│   - Network: allow outbound HTTP?                        │
│   - Approval gates: some tools require user confirmation │
├─────────────────────────────────────────────────────────┤
│ L3: Process-Level Isolation (Plugin Host)               │
│   - Third-party plugins run in subprocess                │
│   - CPU/memory/time limits                               │
│   - Crash recovery (crash → restart or terminate)        │
│   - No OS hard boundary, don't claim full isolation      │
└─────────────────────────────────────────────────────────┘
```

## Approval Gate

Human-in-the-loop checkpoint before tool execution:

| Risk Level | Tools | Behavior |
|------------|-------|----------|
| High | rm, sudo, network requests | Mandatory approval |
| Medium | File write | Default approval, configurable silent |
| Low | read_file, grep | No approval needed |

### State Machine

```
INPUT_AVAILABLE phase pauses
    → display tool name + params preview
    → user can: approve / edit params / reject
    → reject → tool_result marked "rejected", continue loop
```

## Implementation

```rust
pub struct PermissionPolicy {
    pub allowed_tools: HashSet<String>,
    pub denied_tools: HashSet<String>,
    pub default_level: PermissionLevel,
    pub requires_approval: bool,
}

pub enum PermissionLevel {
    High,    // mandatory approval
    Medium,  // default approval, configurable silent
    Low,     // no approval needed
}
```

## Error Handling

```rust
pub enum PermissionError {
    ApprovalFailed(String),
    PolicyNotFound(String),
    NoApprovalGate,
}
```
