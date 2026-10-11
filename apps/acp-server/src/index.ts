/**
 * ACP Server - 编辑器集成入口
 *
 * 基于 ACP (Agent Control Protocol) 与编辑器通信：
 * - VSCode / Zed / JetBrains
 * 复用同一 Runtime，适配层只做映射（不建立平行业务路径）
 */

import { AgentHarnessSDK } from '@agent-harness/sdk-typescript';

interface ACPRequest {
  id: string;
  method: string;
  params: any;
}

interface ACPResponse {
  id: string;
  result?: any;
  error?: { code: number; message: string };
}

class ACPProtocol {
  private agents = new Map<string, AgentHarnessSDK>();
  private handlers = new Map<string, (params: any) => Promise<any>>();

  constructor() {
    this.register_handlers();
  }

  private register_handlers() {
    // Agent 生命周期
    this.handlers.set('agent/create', async (params) => {
      const agent = new AgentHarnessSDK(params);
      this.agents.set(agent.session_id, agent);
      return { session_id: agent.session_id };
    });

    this.handlers.set('agent/resume', async (params) => {
      const agent = this.agents.get(params.session_id);
      if (!agent) throw new Error('Agent not found');
      return { session_id: agent.session_id };
    });

    this.handlers.set('agent/dispose', async (params) => {
      this.agents.delete(params.session_id);
      return { success: true };
    });

    // 消息处理
    this.handlers.set('agent/send', async (params) => {
      const agent = this.agents.get(params.session_id);
      if (!agent) throw new Error('Agent not found');
      return await agent.send(params.message);
    });

    // 工具调用
    this.handlers.set('tool/call', async (params) => {
      return { result: 'Tool call handled' };
    });
  }

  async handle_request(request: ACPRequest): Promise<ACPResponse> {
    const handler = this.handlers.get(request.method);
    if (!handler) {
      return {
        id: request.id,
        error: { code: -32601, message: `Method not found: ${request.method}` },
      };
    }

    try {
      const result = await handler(request.params);
      return { id: request.id, result };
    } catch (error) {
      return {
        id: request.id,
        error: { code: -32000, message: (error as Error).message },
      };
    }
  }
}

console.log('Agent Harness ACP Server v0.1.0');
console.log('Waiting for editor connections...');

// 简单示例
async function main() {
  const protocol = new ACPProtocol();

  const response = await protocol.handle_request({
    id: '1',
    method: 'agent/create',
    params: { model: 'claude-3-opus' },
  });
  console.log('Response:', response);
}

main().catch(console.error);
