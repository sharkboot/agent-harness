//! MCP Client 实现（简化版）

use async_trait::async_trait;
use harness_tools::{ToolDefinition, ToolOutputDefinition};
use tracing::info;

use crate::error::McpError;

/// MCP Client trait
#[async_trait]
pub trait McpClient: Send + Sync + 'static {
    /// 连接 MCP Server
    async fn connect(&mut self, uri: &str) -> Result<(), McpError>;

    /// 列出可用工具
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError>;

    /// 调用 MCP 工具
    async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<McpToolResult, McpError>;

    /// 断开连接
    async fn disconnect(&mut self) -> Result<(), McpError>;
}

/// MCP 工具定义
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP 工具调用结果
#[derive(Debug, Clone)]
pub struct McpToolResult {
    pub content: Vec<McpContentBlock>,
    pub is_error: bool,
}

/// MCP 内容块
#[derive(Debug, Clone)]
pub enum McpContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    EmbeddedResource {
        type_: String,
        resource: serde_json::Value,
    },
}

/// MCP Client 实现（模拟）
#[derive(Debug, Clone)]
pub struct MockMcpClient {
    connected: bool,
    tools: Vec<McpTool>,
}

impl MockMcpClient {
    pub fn new() -> Self {
        Self {
            connected: false,
            tools: Vec::new(),
        }
    }
}

#[async_trait]
impl McpClient for MockMcpClient {
    async fn connect(&mut self, _uri: &str) -> Result<(), McpError> {
        self.connected = true;
        info!("Connected to mock MCP server");
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        if !self.connected {
            return Err(McpError::NotConnected);
        }
        // 返回一些示例工具
        Ok(vec![
            McpTool {
                name: "web_search".to_string(),
                description: "Search the web".to_string(),
                input_schema: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
            },
            McpTool {
                name: "file_read".to_string(),
                description: "Read a file".to_string(),
                input_schema: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            },
        ])
    }

    async fn call_tool(&self, name: &str, _arguments: serde_json::Value) -> Result<McpToolResult, McpError> {
        if !self.connected {
            return Err(McpError::NotConnected);
        }
        Ok(McpToolResult {
            content: vec![McpContentBlock::Text {
                text: format!("Result from {}", name),
            }],
            is_error: false,
        })
    }

    async fn disconnect(&mut self) -> Result<(), McpError> {
        self.connected = false;
        Ok(())
    }
}

/// 将 MCP 工具转换为 ToolDefinition
pub fn mcp_tool_to_tool_definition(tool: &McpTool) -> ToolDefinition {
    use harness_tools::ToolOutputDefinition;
    use std::sync::Arc;

    ToolDefinition::new(
        &tool.name,
        &tool.description,
        tool.input_schema.clone(),
        Arc::new(McpToolExecutor {
            name: tool.name.clone(),
        }),
        ToolOutputDefinition {
            schema: serde_json::json!({}),
            render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| {
                vec![]
            }),
            presentation_meta: None,
        },
    )
}

/// MCP 工具执行器
struct McpToolExecutor {
    name: String,
}

#[async_trait]
impl harness_tools::ToolExecutor for McpToolExecutor {
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _signal: &std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::Semaphore>>>,
    ) -> Result<serde_json::Value, harness_tools::definitions::ToolError> {
        Ok(serde_json::json!({
            "executor": self.name.clone(),
            "result": "executed"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_mcp_client() {
        let mut client = MockMcpClient::new();

        // 测试连接
        client.connect("mock://test").await.unwrap();
        assert!(client.connected);

        // 测试列出工具
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "web_search");
        assert_eq!(tools[1].name, "file_read");

        // 测试调用工具
        let result = client.call_tool("web_search", serde_json::json!({"query": "test"})).await.unwrap();
        assert!(!result.is_error);

        // 测试断开连接
        client.disconnect().await.unwrap();
        assert!(!client.connected);
    }

    #[test]
    fn test_mcp_tool_to_definition() {
        let tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };

        let definition = mcp_tool_to_tool_definition(&tool);
        assert_eq!(definition.name(), "test_tool");
        assert_eq!(definition.description(), "A test tool");
    }
}
