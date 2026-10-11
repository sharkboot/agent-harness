/**
 * Agent Harness TypeScript SDK
 *
 * 面向应用开发者的公开 API：
 * - Agent 创建/恢复
 * - 工具注册
 * - 事件订阅
 */

export interface AgentOptions {
  session_id?: string;
  model: string;
  system_prompt?: string;
  tools?: ToolDefinition[];
}

export interface ToolDefinition {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

export interface Message {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export interface AgentEvent {
  type: 'message' | 'tool_call' | 'approval' | 'error' | 'complete';
  data: any;
}

export class AgentHarnessSDK {
  private session_id: string;
  private messages: Message[] = [];
  private on_event?: (event: AgentEvent) => void;

  constructor(options: AgentOptions) {
    this.session_id = options.session_id || crypto.randomUUID();
    if (options.system_prompt) {
      this.messages.push({ role: 'system', content: options.system_prompt });
    }
  }

  on(event: 'event', callback: (event: AgentEvent) => void): void {
    this.on_event = callback;
  }

  async send(message: string): Promise<AgentEvent> {
    this.messages.push({ role: 'user', content: message });

    const event: AgentEvent = {
      type: 'message',
      data: { message, role: 'assistant' },
    };

    if (this.on_event) {
      this.on_event(event);
    }

    return event;
  }

  async dispose(): Promise<void> {
    this.messages = [];
  }

  get session_id(): string {
    return this.session_id;
  }
}

export default AgentHarnessSDK;
