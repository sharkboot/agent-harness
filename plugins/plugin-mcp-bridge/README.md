# MCP Bridge Plugin

## Description
Plugin that bridges MCP (Model Context Protocol) servers to harness tools.

## Plugin Manifest
```json
{
  "name": "mcp-bridge-plugin",
  "version": "1.0.0",
  "description": "Bridge MCP servers to harness tools",
  "entry": "plugin.wasm",
  "permissions": ["read_file", "http_request"]
}
```

## Configuration
```yaml
mcp_servers:
  - name: filesystem
    command: npx
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/files"]
  - name: wikipedia
    command: npx
    args: ["-y", "@modelcontextprotocol/server-wikipedia"]
```

## Features
1. Auto-discover MCP tools on startup
2. Map MCP tools to harness ToolDefinition
3. Handle MCP protocol serialization/deserialization
4. Timeout and error handling per tool call
5. Permission integration with harness permission system
