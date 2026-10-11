import React, { useState, useRef, useEffect } from 'react';
import {
  ContentBlock,
  ContentBlockRenderer,
  ToolCard,
  ToolCardState,
} from '@agent-harness/ui-components';

/**
 * Agent Harness Web UI
 *
 * Web 端用户交互入口，复用 ui-components 共享组件：
 * - 流式渲染（Markdown）
 * - 工具卡片状态展示
 * - 审批门交互
 */

export function App() {
  const [messages, set_messages] = useState<ContentBlock[]>([]);
  const [input, set_input] = useState('');
  const [tool_state, set_tool_state] = useState<ToolCardState | null>(null);
  const [is_processing, set_is_processing] = useState(false);
  const messages_end_ref = useRef<HTMLDivElement>(null);

  // 自动滚动到底部
  useEffect(() => {
    messages_end_ref.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handle_submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || is_processing) return;

    const user_message: ContentBlock = {
      type: 'text',
      text: input,
    };

    set_messages((prev) => [...prev, user_message]);
    set_input('');
    set_is_processing(true);

    // 模拟 Agent 响应
    setTimeout(async () => {
      // 模拟工具调用
      set_tool_state({
        type: 'INPUT_STREAMING',
        tool_name: 'web_search',
        collected_args: '{"query": ' + JSON.stringify(input) + '}',
      });

      setTimeout(() => {
        set_tool_state({
          type: 'INPUT_AVAILABLE',
          tool_name: 'web_search',
          args: { query: input },
          on_approve: handle_approve,
          on_reject: handle_reject,
          on_edit: handle_edit,
        });
      }, 500);
    }, 500);
  };

  const handle_approve = () => {
    if (!tool_state) return;
    set_tool_state({
      type: 'EXECUTING',
      tool_name: tool_state.tool_name,
      start_time: Date.now(),
      on_cancel: handle_cancel,
    });

    setTimeout(() => {
      set_tool_state({
        type: 'OUTPUT_READY',
        tool_name: 'web_search',
        result: {
          success: true,
          data: ['Result 1', 'Result 2', 'Result 3'],
        },
      });
      set_is_processing(false);
    }, 2000);
  };

  const handle_reject = () => {
    set_tool_state(null);
    set_is_processing(false);
  };

  const handle_edit = (new_args: any) => {
    if (!tool_state) return;
    // 在真实实现中，这里会更新 args 并重新展示
    console.log('Edit args:', new_args);
  };

  const handle_cancel = () => {
    set_tool_state({
      type: 'OUTPUT_ERROR',
      tool_name: 'web_search',
      error: 'Cancelled by user',
    });
    set_is_processing(false);
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        background: '#f5f5f5',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {/* 头部 */}
      <header
        style={{
          background: '#1a1a2e',
          color: '#fff',
          padding: '16px 24px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <h1 style={{ margin: 0, fontSize: '20px' }}>Agent Harness</h1>
          <p style={{ margin: '4px 0 0', fontSize: '12px', color: '#888' }}>
            Web UI v0.1.0
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            onClick={() => {
              set_messages([]);
              set_tool_state(null);
            }}
            style={{
              padding: '6px 12px',
              borderRadius: '4px',
              border: '1px solid #444',
              background: 'transparent',
              color: '#ccc',
              cursor: 'pointer',
            }}
          >
            清空会话
          </button>
        </div>
      </header>

      {/* 消息区域 */}
      <main
        style={{
          flex: 1,
          overflow: 'auto',
          padding: '24px',
          maxWidth: '800px',
          margin: '0 auto',
          width: '100%',
        }}
      >
        {messages.map((msg, idx) => (
          <div key={idx} style={{ marginBottom: '16px' }}>
            <ContentBlockRenderer block={msg} />
          </div>
        ))}

        {/* 工具卡片 */}
        {tool_state && <ToolCard state={tool_state} />}

        {/* 处理中指示器 */}
        {is_processing && !tool_state && (
          <div style={{ color: '#666', fontSize: '14px', marginTop: '16px' }}>
            正在处理...
          </div>
        )}

        <div ref={messages_end_ref} />
      </main>

      {/* 输入区域 */}
      <footer
        style={{
          background: '#fff',
          borderTop: '1px solid #e0e0e0',
          padding: '16px 24px',
        }}
      >
        <form
          onSubmit={handle_submit}
          style={{
            maxWidth: '800px',
            margin: '0 auto',
            display: 'flex',
            gap: '8px',
          }}
        >
          <input
            type="text"
            value={input}
            onChange={(e) => set_input(e.target.value)}
            placeholder="输入消息..."
            disabled={is_processing}
            style={{
              flex: 1,
              padding: '10px 14px',
              borderRadius: '6px',
              border: '1px solid #d0d0d0',
              fontSize: '14px',
              outline: 'none',
            }}
          />
          <button
            type="submit"
            disabled={is_processing || !input.trim()}
            style={{
              padding: '10px 20px',
              borderRadius: '6px',
              border: 'none',
              background: '#0969da',
              color: '#fff',
              cursor: is_processing || !input.trim() ? 'not-allowed' : 'pointer',
              fontSize: '14px',
              opacity: is_processing || !input.trim() ? 0.5 : 1,
            }}
          >
            发送
          </button>
        </form>
      </footer>
    </div>
  );
}
