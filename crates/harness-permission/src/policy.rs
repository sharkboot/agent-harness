//! 权限策略定义

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 权限级别
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// 高风险：强制审批
    High,
    /// 中风险：默认审批，可配置静默
    Medium,
    /// 低风险：免审批
    Low,
}

/// 权限策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// 策略 ID
    pub id: String,
    /// 策略名称
    pub name: String,
    /// 允许的工具列表
    pub allowed_tools: HashSet<String>,
    /// 拒绝的工具列表
    pub denied_tools: HashSet<String>,
    /// 默认权限级别
    pub default_level: PermissionLevel,
    /// 是否需要人工审批
    pub requires_approval: bool,
}

impl PermissionPolicy {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            allowed_tools: HashSet::new(),
            denied_tools: HashSet::new(),
            default_level: PermissionLevel::Medium,
            requires_approval: false,
        }
    }

    /// 添加工具到允许列表
    pub fn allow_tool(mut self, tool: impl Into<String>) -> Self {
        self.allowed_tools.insert(tool.into());
        self
    }

    /// 添加工具到拒绝列表
    pub fn deny_tool(mut self, tool: impl Into<String>) -> Self {
        self.denied_tools.insert(tool.into());
        self
    }

    /// 设置默认权限级别
    pub fn with_default_level(mut self, level: PermissionLevel) -> Self {
        self.default_level = level;
        self
    }

    /// 设置是否需要人工审批
    pub fn with_approval_required(mut self, required: bool) -> Self {
        self.requires_approval = required;
        self
    }

    /// 检查工具是否被允许
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if self.denied_tools.contains(tool_name) {
            return false;
        }
        if self.allowed_tools.is_empty() {
            return true; // 如果允许列表为空，默认允许
        }
        self.allowed_tools.contains(tool_name)
    }

    /// 获取工具的权限级别
    pub fn get_tool_level(&self, tool_name: &str) -> PermissionLevel {
        if self.denied_tools.contains(tool_name) {
            return PermissionLevel::High;
        }
        match tool_name {
            // 高风险工具
            "rm" | "sudo" | "exec" | "shell" => PermissionLevel::High,
            // 中风险工具
            "write_file" | "edit_file" | "http_request" => PermissionLevel::Medium,
            // 低风险工具
            _ => PermissionLevel::Low,
        }
    }

    /// 检查是否需要审批
    pub fn needs_approval(&self, tool_name: &str) -> bool {
        self.requires_approval
            || self.get_tool_level(tool_name) == PermissionLevel::High
    }
}

/// 权限评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResult {
    pub allowed: bool,
    pub level: PermissionLevel,
    pub requires_approval: bool,
    pub reason: Option<String>,
}

/// 权限评估器
#[derive(Debug, Clone)]
pub struct PermissionEvaluator {
    policies: Vec<PermissionPolicy>,
}

impl Default for PermissionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionEvaluator {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// 添加权限策略
    pub fn add_policy(&mut self, policy: PermissionPolicy) {
        self.policies.push(policy);
    }

    /// 评估工具调用权限
    pub fn evaluate(&self, tool_name: &str) -> PermissionResult {
        // 检查所有策略
        for policy in &self.policies {
            if !policy.is_tool_allowed(tool_name) {
                return PermissionResult {
                    allowed: false,
                    level: PermissionLevel::High,
                    requires_approval: false,
                    reason: Some(format!("Tool '{}' is denied by policy '{}'", tool_name, policy.name)),
                };
            }
        }

        // 获取权限级别
        let level = PermissionLevel::Medium;
        let requires_approval = matches!(level, PermissionLevel::High);

        PermissionResult {
            allowed: true,
            level,
            requires_approval,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_policy() {
        let policy = PermissionPolicy::new("test", "Test Policy")
            .allow_tool("read_file")
            .deny_tool("rm")
            .with_default_level(PermissionLevel::Medium)
            .with_approval_required(true);

        assert!(policy.is_tool_allowed("read_file"));
        assert!(!policy.is_tool_allowed("rm"));
        assert_eq!(policy.get_tool_level("read_file"), PermissionLevel::Low);
        assert_eq!(policy.get_tool_level("rm"), PermissionLevel::High);
        assert!(policy.needs_approval("rm"));
    }

    #[test]
    fn test_permission_evaluator() {
        let mut evaluator = PermissionEvaluator::new();
        evaluator.add_policy(
            PermissionPolicy::new("restrict", "Restrictive Policy")
                .deny_tool("dangerous_tool"),
        );

        let result = evaluator.evaluate("dangerous_tool");
        assert!(!result.allowed);
        assert_eq!(result.level, PermissionLevel::High);

        let result = evaluator.evaluate("safe_tool");
        assert!(result.allowed);
    }
}
