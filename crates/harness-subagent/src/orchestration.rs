//! SubAgent 编排逻辑

use harness_event::{SessionEvent, AgentId, TurnId};
use harness_session::SessionStore;
use tracing::info;

use super::subagent::{DelegationMode, SubAgentDefinition, SubAgentManager};
use crate::error::SubAgentError;

/// 子 Agent 编排器
#[derive(Debug)]
pub struct SubAgentOrchestrator {
    manager: SubAgentManager,
}

impl SubAgentOrchestrator {
    pub fn new() -> Self {
        Self {
            manager: SubAgentManager::new(),
        }
    }

    /// 注册子 Agent
    pub fn register(&mut self, agent: SubAgentDefinition) {
        self.manager.register(agent);
    }

    /// 创建子 Agent（从父 Agent 派生）
    pub fn create_child(
        &mut self,
        parent_id: &AgentId,
        parent_session: &SessionStore,
        system_prompt: String,
        mode: DelegationMode,
    ) -> AgentId {
        let child_id = AgentId(uuid::Uuid::new_v4());
        let child_agent = SubAgentDefinition::new(
            child_id.clone(),
            parent_id.clone(),
            system_prompt,
            mode,
        );
        self.manager.register(child_agent);
        info!(child_id = %child_id, parent_id = %parent_id, "Created sub-agent");
        child_id
    }

    /// Fork 模式：并行启动多个子 Agent
    pub async fn fork(
        &mut self,
        parent_id: &AgentId,
        parent_session: &SessionStore,
        prompts: Vec<String>,
    ) -> Result<Vec<AgentId>, SubAgentError> {
        let mut child_ids = Vec::new();
        for (i, prompt) in prompts.into_iter().enumerate() {
            let child_id = self.create_child(
                parent_id,
                parent_session,
                format!("({}, 子任务 {})", prompt, i + 1),
                DelegationMode::Fork,
            );
            child_ids.push(child_id);
        }
        Ok(child_ids)
    }

    /// Pipeline 模式：顺序启动子 Agent
    pub async fn pipeline(
        &mut self,
        parent_id: &AgentId,
        parent_session: &SessionStore,
        prompts: Vec<String>,
    ) -> Result<Vec<AgentId>, SubAgentError> {
        let mut child_ids = Vec::new();
        for (i, prompt) in prompts.into_iter().enumerate() {
            let child_id = self.create_child(
                parent_id,
                parent_session,
                format!("Pipeline step {}", i + 1),
                DelegationMode::Pipeline,
            );
            child_ids.push(child_id);
        }
        Ok(child_ids)
    }

    /// 获取所有子 Agent
    pub fn children(&self) -> &[SubAgentDefinition] {
        self.manager.agents()
    }
}

impl Default for SubAgentOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_event::AgentId;

    #[tokio::test]
    async fn test_fork_mode() {
        let mut orchestrator = SubAgentOrchestrator::new();
        let parent_id = AgentId(uuid::Uuid::new_v4());
        let parent_session = SessionStore::new(uuid::Uuid::new_v4());

        let prompts = vec!["Task 1".to_string(), "Task 2".to_string(), "Task 3".to_string()];
        let children = orchestrator.fork(&parent_id, &parent_session, prompts).await.unwrap();

        assert_eq!(children.len(), 3);
        assert_eq!(orchestrator.children().len(), 3);
    }

    #[tokio::test]
    async fn test_pipeline_mode() {
        let mut orchestrator = SubAgentOrchestrator::new();
        let parent_id = AgentId(uuid::Uuid::new_v4());
        let parent_session = SessionStore::new(uuid::Uuid::new_v4());

        let prompts = vec!["Step 1".to_string(), "Step 2".to_string()];
        let children = orchestrator
            .pipeline(&parent_id, &parent_session, prompts)
            .await
            .unwrap();

        assert_eq!(children.len(), 2);
    }
}
