/**
 * Web Adapter - 将 Runtime 事件映射到 Web 组件
 */

import { ContentBlock } from '@agent-harness/ui-components';

export interface WebAdapterEvent {
  type: 'message' | 'tool_call' | 'approval_required' | 'error';
  data: any;
}

export class WebAdapter {
  private events: WebAdapterEvent[] = [];
  private on_event?: (event: WebAdapterEvent) => void;

  constructor() {
    this.events = [];
  }

  on(event: 'event', callback: (event: WebAdapterEvent) => void): void {
    this.on_event = callback;
  }

  send_event(event_type: string, data: any): void {
    const web_event: WebAdapterEvent = {
      type: event_type,
      data,
    };
    this.events.push(web_event);
    if (this.on_event) {
      this.on_event(web_event);
    }
  }

  send_message(block: ContentBlock): void {
    this.send_event('message', block);
  }

  send_tool_call(tool_name: string, args: any, call_id: string): void {
    this.send_event('tool_call', { tool_name, args, call_id });
  }

  send_approval_required(tool_name: string, args: any, call_id: string): void {
    this.send_event('approval_required', { tool_name, args, call_id });
  }

  send_error(error: string): void {
    this.send_event('error', { error });
  }

  get_events(): WebAdapterEvent[] {
    return this.events;
  }

  clear_events(): void {
    this.events = [];
  }
}
