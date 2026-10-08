//! MCP Server 实现（简化版）

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::McpError;

/// MCP 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpMessage {
    #[serde(rename = "request")]
    Request(McpRequest),
    #[serde(rename = "response")]
    Response(McpResponse),
    #[serde(rename = "notification")]
    Notification(McpNotification),
}

/// MCP 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

/// MCP 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: String,
    pub result: serde_json::Value,
}

/// MCP 通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    pub params: serde_json::Value,
}

/// MCP Server trait
#[async_trait::async_trait]
pub trait McpServer: Send + Sync + 'static {
    /// 处理初始化请求
    async fn initialize(&mut self, params: &serde_json::Value) -> Result<McpResponse, McpError>;

    /// 处理工具列表请求
    async fn list_tools(&self, params: &serde_json::Value) -> Result<McpResponse, McpError>;

    /// 处理工具调用请求
    async fn call_tool(&self, params: &serde_json::Value) -> Result<McpResponse, McpError>;

    /// 获取服务器版本信息
    fn version(&self) -> (&str, &str);
}

/// Mock MCP Server 实现
#[derive(Debug, Clone)]
pub struct MockMcpServer {
    pub name: String,
    pub version: String,
}

impl MockMcpServer {
    pub fn new() -> Self {
        Self {
            name: "mock-mcp-server".to_string(),
            version: "1.0.0".to_string(),
        }
    }
}

impl Default for MockMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl McpServer for MockMcpServer {
    async fn initialize(&mut self, _params: &serde_json::Value) -> Result<McpResponse, McpError> {
        info!("Initializing mock MCP server");
        Ok(McpResponse {
            id: "1".to_string(),
            result: serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": self.name.clone(),
                    "version": self.version.clone()
                }
            }),
        })
    }

    async fn list_tools(&self, _params: &serde_json::Value) -> Result<McpResponse, McpError> {
        Ok(McpResponse {
            id: "2".to_string(),
            result: serde_json::json!({
                "tools": [
                    {
                        "name": "mock_tool",
                        "description": "A mock tool for testing",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": {"type": "string"}
                            }
                        }
                    }
                ]
            }),
        })
    }

    async fn call_tool(&self, params: &serde_json::Value) -> Result<McpResponse, McpError> {
        let name = params.get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing tool name".to_string()))?;

        Ok(McpResponse {
            id: "3".to_string(),
            result: serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("Result from {}", name)
                    }
                ]
            }),
        })
    }

    fn version(&self) -> (&str, &str) {
        (&self.name, &self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_initialize() {
        let mut server = MockMcpServer::new();
        let result = server.initialize(&serde_json::json!({})).await.unwrap();
        assert_eq!(result.id, "1");
        assert_eq!(result.result["serverInfo"]["name"], "mock-mcp-server");
    }

    #[tokio::test]
    async fn test_mock_server_list_tools() {
        let server = MockMcpServer::new();
        let result = server.list_tools(&serde_json::json!({})).await.unwrap();
        assert_eq!(result.id, "2");
        let tools = result.result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "mock_tool");
    }

    #[tokio::test]
    async fn test_mock_server_call_tool() {
        let server = MockMcpServer::new();
        let result = server
            .call_tool(&serde_json::json!({"name": "mock_tool", "arguments": {"input": "test"}}))
            .await
            .unwrap();
        assert_eq!(result.id, "3");
        let content = result.result["content"][0]["text"].as_str().unwrap();
        assert!(content.contains("mock_tool"));
    }
}
