/**
 * SDK JSON-RPC Server
 *
 * 向开发者开放集成入口，提供 Agent 创建/恢复、工具注册、事件订阅能力。
 */

import { AgentHarnessSDK } from '@agent-harness/sdk-typescript';

const agents = new Map<string, AgentHarnessSDK>();

// 模拟 JSON-RPC server
async function handle_request(method: string, params: any): Promise<any> {
  switch (method) {
    case 'agent.create': {
      const agent = new AgentHarnessSDK(params);
      agents.set(agent.session_id, agent);
      return { session_id: agent.session_id };
    }
    case 'agent.resume': {
      const id = params.session_id;
      const agent = agents.get(id);
      if (!agent) throw new Error(`Agent not found: ${id}`);
      return { session_id: agent.session_id };
    }
    case 'agent.send': {
      const id = params.session_id;
      const message = params.message;
      const agent = agents.get(id);
      if (!agent) throw new Error(`Agent not found: ${id}`);
      return await agent.send(message);
    }
    case 'agent.dispose': {
      const id = params.session_id;
      agents.delete(id);
      return { success: true };
    }
    default:
      throw new Error(`Unknown method: ${method}`);
  }
}

console.log('Agent Harness SDK Server v0.1.0');
console.log('Supports: agent.create, agent.resume, agent.send, agent.dispose');

// 简单示例
async function main() {
  const agent = new AgentHarnessSDK({
    model: 'claude-3-opus',
    system_prompt: 'You are a helpful assistant.',
  });

  console.log('Created agent:', agent.session_id);

  const event = await agent.send('Hello, world!');
  console.log('Event:', event);

  await agent.dispose();
  console.log('Agent disposed');
}

main().catch(console.error);
