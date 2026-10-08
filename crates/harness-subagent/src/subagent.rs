//! 子 Agent 定义

use harness_event::AgentId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::SubAgentError;

/// 子 Agent 委派模式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegationMode {
    /// 并行研究：多个子 Agent 同时运行
    Fork,
    /// 流水线：A 完成后自动触发 B
    Pipeline,
    /// 内联委托：Lead Agent 内联委托专家
    Delegation,
    /// 角色切换：Agent 之间转移对话权
    Handoff,
}

/// 子 Agent 状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubAgentState {
    /// 待启动
    Pending,
    /// 正在运行
    Running,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 出错
    Error(String),
}

/// 子 Agent 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    pub sub_agent_id: AgentId,
    pub status: SubAgentState,
    /// 子 Agent 的输出内容
    pub output: Option<String>,
    /// 错误信息（如果有）
    pub error: Option<String>,
}

/// 子 Agent 定义
#[derive(Debug, Clone)]
pub struct SubAgentDefinition {
    /// 子 Agent ID
    pub id: AgentId,
    /// 委派模式
    pub mode: DelegationMode,
    /// 父 Agent ID
    pub parent_agent_id: AgentId,
    /// 子 Agent 的系统提示
    pub system_prompt: String,
    /// 工具白名单（继承自父 Agent）
    pub tool_allowlist: Vec<String>,
    /// 可选的 seed 前缀
    pub seed_prefix: Option<String>,
}

impl SubAgentDefinition {
    pub fn new(
        id: AgentId,
        parent_agent_id: AgentId,
        system_prompt: String,
        mode: DelegationMode,
    ) -> Self {
        Self {
            id,
            mode,
            parent_agent_id,
            system_prompt,
            tool_allowlist: Vec::new(),
            seed_prefix: None,
        }
    }

    /// 设置工具白名单
    pub fn with_tool_allowlist(mut self, tools: Vec<String>) -> Self {
        self.tool_allowlist = tools;
        self
    }

    /// 设置 seed 前缀
    pub fn with_seed_prefix(mut self, prefix: String) -> Self {
        self.seed_prefix = Some(prefix);
        self
    }
}

/// 子 Agent 管理器
#[derive(Debug)]
pub struct SubAgentManager {
    agents: Vec<SubAgentDefinition>,
    states: std::collections::HashMap<AgentId, SubAgentState>,
    results: std::collections::HashMap<AgentId, SubAgentResult>,
}

impl Default for SubAgentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubAgentManager {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            states: std::collections::HashMap::new(),
            results: std::collections::HashMap::new(),
        }
    }

    /// 注册子 Agent
    pub fn register(&mut self, agent: SubAgentDefinition) {
        self.agents.push(agent);
        self.states.insert(agent.id.clone(), SubAgentState::Pending);
    }

    /// 启动子 Agent
    pub async fn spawn(&mut self, agent_id: &AgentId) -> Result<(), SubAgentError> {
        if let Some(state) = self.states.get_mut(agent_id) {
            *state = SubAgentState::Running;
            Ok(())
        } else {
            Err(SubAgentError::NotFound(agent_id.to_string()))
        }
    }

    /// 完成子 Agent
    pub async fn complete(
        &mut self,
        agent_id: &AgentId,
        output: String,
    ) -> Result<SubAgentResult, SubAgentError> {
        if let Some(state) = self.states.get_mut(agent_id) {
            *state = SubAgentState::Completed;
            let result = SubAgentResult {
                sub_agent_id: agent_id.clone(),
                status: SubAgentState::Completed,
                output: Some(output),
                error: None,
            };
            self.results.insert(agent_id.clone(), result.clone());
            Ok(result)
        } else {
            Err(SubAgentError::NotFound(agent_id.to_string()))
        }
    }

    /// 取消子 Agent
    pub async fn cancel(&mut self, agent_id: &AgentId) -> Result<(), SubAgentError> {
        if let Some(state) = self.states.get_mut(agent_id) {
            *state = SubAgentState::Cancelled;
            Ok(())
        } else {
            Err(SubAgentError::NotFound(agent_id.to_string()))
        }
    }

    /// 获取子 Agent 状态
    pub fn get_state(&self, agent_id: &AgentId) -> Option<&SubAgentState> {
        self.states.get(agent_id)
    }

    /// 获取子 Agent 结果
    pub fn get_result(&self, agent_id: &AgentId) -> Option<&SubAgentResult> {
        self.results.get(agent_id)
    }

    /// 获取所有子 Agent 定义
    pub fn agents(&self) -> &[SubAgentDefinition] {
        &self.agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_lifecycle() {
        let mut manager = SubAgentManager::new();
        let agent_id = AgentId(uuid::Uuid::new_v4());
        let parent_id = AgentId(uuid::Uuid::new_v4());

        let agent = SubAgentDefinition::new(
            agent_id.clone(),
            parent_id,
            "Test system prompt".to_string(),
            DelegationMode::Delegation,
        );

        manager.register(agent);
        assert_eq!(manager.get_state(&agent_id), Some(&SubAgentState::Pending));

        // 启动
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                manager.spawn(&agent_id).await.unwrap();
                assert_eq!(
                    manager.get_state(&agent_id),
                    Some(&SubAgentState::Running)
                );

                // 完成
                let result = manager
                    .complete(&agent_id, "Test output".to_string())
                    .await
                    .unwrap();
                assert_eq!(result.status, SubAgentState::Completed);
                assert_eq!(result.output, Some("Test output".to_string()));
            });
    }

    #[test]
    fn test_subagent_cancel() {
        let mut manager = SubAgentManager::new();
        let agent_id = AgentId(uuid::Uuid::new_v4());
        let parent_id = AgentId(uuid::Uuid::new_v4());

        let agent = SubAgentDefinition::new(
            agent_id.clone(),
            parent_id,
            "Test".to_string(),
            DelegationMode::Fork,
        );

        manager.register(agent);

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                manager.spawn(&agent_id).await.unwrap();
                manager.cancel(&agent_id).await.unwrap();
                assert_eq!(
                    manager.get_state(&agent_id),
                    Some(&SubAgentState::Cancelled)
                );
            });
    }
}
