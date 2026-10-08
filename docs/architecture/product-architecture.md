# Product Architecture

## Overview

Agent Harness is a runtime framework connecting LLMs with execution environments. It drives the Agent main loop (call model → parse output → route tools → execute → record results), manages context, provides sandboxed execution, and supports multi-device access.

## Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    Entrances Layer                      │
│  Web UI │ Desktop GUI │ TUI │ CLI │ ACP Server │ SDK   │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                    Adapter Layer                        │
│  Web Adapter │ Desktop Adapter │ CLI Adapter │ ACP Advdr │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Runtime Layer                         │
│  Core: Coordinator │ SessionOwner │ ToolOwner │ Perm...  │
│  Services: ModelSvc │ FS │ Shell │ Browser │ SubAgent... │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  Plugin Host Layer                      │
│         WASM sandboxed sub-processes                    │
└─────────────────────────────────────────────────────────┘
```

## Core Design Principles

| Principle | Meaning | Source |
|-----------|---------|--------|
| **Everything is a plugin** | Each subsystem is replaceable | DSH Cordis |
| **Session is the single source of truth** | History derived from event log, not stored separately | DSH SessionEvent |
| **Layered owner boundaries** | Each module has one state owner | OpenBitFun |
| **Progressive Skill loading** | Only load skills when needed | DeerFlow |
| **Supervised execution** | Third-party code in isolated subprocess | OpenBitFun |
| **Unified ContentBlock type** | Model output is typed block array, not string | DSH ContentBlockMap |

## Component Map

| Component | Crate/Package | Description |
|-----------|---------------|-------------|
| ContentBlock | `harness-event` | Unified content block type |
| Message | `harness-event` | Message structure with source tracking |
| SessionEvent | `harness-event` | Event log types |
| SessionStore | `harness-session` | Append-only event store |
| AgentLoop | `harness-runtime` | Main loop coordinator |
| ToolRegistry | `harness-tools` | Tool registration and whitelist |
| ProviderAdapter | `harness-model` | OpenAI/Anthropic adapters |
| Compaction | `harness-context` | Context compression |
| SubAgent | `harness-subagent` | Sub-agent orchestration |
| Permission | `harness-permission` | Three-layer permission model |
| MCP | `harness-mcp` | MCP protocol bridge |
| UI Components | `packages/ui-components` | React components for rendering |
| CLI TUI | `apps/cli` | Ink-based terminal UI |

## Data Flow

```
User Input
    ↓
AgentLoop.start_turn()
    ↓
PromptAssembler.assemble_prompt()
    ↓
ModelProvider.chat() → ContentBlock[]
    ↓
ToolPipeline.execute_tool_calls()
    ↓
SessionStore.append() (event log)
    ↓
DerivedState.derive_messages()
    ↓
Turn completes
```
