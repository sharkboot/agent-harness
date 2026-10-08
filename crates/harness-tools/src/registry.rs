//! 工具注册表 — 管理工具的注册/注销/白名单过滤

use std::collections::HashMap;
use std::sync::Arc;

use crate::definitions::ToolDefinition;
use crate::error::ToolError;

/// 工具注册表，按名称索引所有可用工具。
#[derive(Clone)]
pub struct ToolRegistry {
    /// 工具定义映射
    tools: HashMap<String, ToolDefinition>,
    /// 白名单（None 表示不过滤）
    whitelist: Option<Vec<String>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// 创建空的工具注册表。
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            whitelist: None,
        }
    }

    /// 注册一个工具。
    ///
    /// 如果已存在同名工具，会覆盖旧定义。
    pub fn register(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// 注销一个工具。
    ///
    /// 返回是否成功注销。
    pub fn unregister(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }

    /// 获取工具定义（只读引用）。
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        if let Some(ref whitelist) = self.whitelist {
            if !whitelist.contains(&name.to_string()) {
                return None;
            }
        }
        self.tools.get(name)
    }

    /// 获取工具定义（可变引用，用于内联修改）。
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ToolDefinition> {
        if let Some(ref whitelist) = self.whitelist {
            if !whitelist.contains(&name.to_string()) {
                return None;
            }
        }
        self.tools.get_mut(name)
    }

    /// 获取所有工具名称（受白名单过滤）。
    pub fn names(&self) -> Vec<String> {
        self.tools
            .keys()
            .filter(|k| {
                if let Some(ref whitelist) = self.whitelist {
                    whitelist.contains(k)
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    /// 获取所有工具定义（受白名单过滤）。
    pub fn all_tools(&self) -> Vec<&ToolDefinition> {
        self.tools
            .values()
            .filter(|t| {
                if let Some(ref whitelist) = self.whitelist {
                    whitelist.contains(&t.name)
                } else {
                    true
                }
            })
            .collect()
    }

    /// 设置白名单。
    ///
    /// 仅允许白名单中的工具被访问。
    pub fn set_whitelist(&mut self, whitelist: Vec<String>) {
        self.whitelist = Some(whitelist);
    }

    /// 清除白名单（允许所有工具）。
    pub fn clear_whitelist(&mut self) {
        self.whitelist = None;
    }

    /// 判断注册表是否为空。
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 获取工具数量。
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 执行参数验证（使用 JSON Schema）。
    ///
    /// 注意：这是一个简化实现，实际应使用专门的 JSON Schema 验证库。
    pub fn validate_arguments(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<(), ToolError> {
        let tool = self
            .get(tool_name)
            .ok_or_else(|| ToolError::NotFound(tool_name.to_string()))?;

        // 简化验证：检查 parameters 是否存在 required 字段
        if let Some(schema) = tool.parameters().get("required") {
            if let Some(required) = schema.as_array() {
                for field in required {
                    if let Some(field_name) = field.as_str() {
                        if !args.is_object() || !args.get(field_name).is_some() {
                            return Err(ToolError::ValidationError(format!(
                                "Missing required field: {}",
                                field_name
                            )));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{ToolDefinition, ToolOutputDefinition};

    fn make_test_tool(name: &str) -> ToolDefinition {
        ToolDefinition::new(
            name,
            format!("Test tool: {}", name),
            serde_json::json!({"type": "object"}),
            Arc::new(crate::definitions::MockExecutor),
            ToolOutputDefinition {
                schema: serde_json::json!({}),
                render: Arc::new(|_: &serde_json::Value, _: &serde_json::Value| vec![]),
                presentation_meta: None,
            },
        )
    }

    #[test]
    fn register_and_get() {
        let mut registry = ToolRegistry::new();
        let tool = make_test_tool("test_tool");
        registry.register(tool);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("test_tool").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn unregister() {
        let mut registry = ToolRegistry::new();
        let tool = make_test_tool("test_tool");
        registry.register(tool);

        assert!(registry.unregister("test_tool"));
        assert_eq!(registry.len(), 0);
        assert!(!registry.unregister("test_tool")); // already removed
    }

    #[test]
    fn whitelist_filtering() {
        let mut registry = ToolRegistry::new();
        registry.register(make_test_tool("tool_a"));
        registry.register(make_test_tool("tool_b"));
        registry.register(make_test_tool("tool_c"));

        // 无白名单：所有工具可见
        assert_eq!(registry.names().len(), 3);

        // 设置白名单
        registry.set_whitelist(vec!["tool_a".to_string(), "tool_b".to_string()]);

        // 只有白名单内的工具可见
        let names = registry.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"tool_a".to_string()));
        assert!(names.contains(&"tool_b".to_string()));
        assert!(!names.contains(&"tool_c".to_string()));
    }

    #[test]
    fn whitelist_prevents_get() {
        let mut registry = ToolRegistry::new();
        registry.register(make_test_tool("allowed"));
        registry.register(make_test_tool("blocked"));
        registry.set_whitelist(vec!["allowed".to_string()]);

        assert!(registry.get("allowed").is_some());
        assert!(registry.get("blocked").is_none());
    }

    #[test]
    fn clear_whitelist() {
        let mut registry = ToolRegistry::new();
        registry.register(make_test_tool("tool_a"));
        registry.register(make_test_tool("tool_b"));
        registry.set_whitelist(vec!["tool_a".to_string()]);
        registry.clear_whitelist();

        assert_eq!(registry.names().len(), 2);
    }
}
