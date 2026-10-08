# Example Plugin

## Description
A simple example plugin demonstrating the plugin architecture.

## Plugin Manifest
```json
{
  "name": "example-plugin",
  "version": "1.0.0",
  "description": "Example plugin for Agent Harness",
  "entry": "plugin.wasm",
  "permissions": ["read_file", "write_file"]
}
```

## Available Tools
- example_greet(name: string) → string
- example_compute(a: number, b: number, op: string) → number

## Implementation Notes
This plugin demonstrates:
1. WASM sandbox execution
2. Tool registration with harness
3. Permission boundary enforcement
4. Error handling and recovery
