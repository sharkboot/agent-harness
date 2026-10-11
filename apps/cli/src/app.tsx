import React, { useState, useEffect } from 'react';
import { Text, Box, Input, useInput, useApp } from 'ink';
import { ToolCard, ToolCardState } from '@agent-harness/ui-components';

// ── 类型定义 ────────────────────────────────────────────────────────────────────

interface AppState {
  mode: 'idle' | 'running' | 'paused' | 'error';
  message: string;
  tool_card?: ToolCardState;
  error?: string;
}

// ── 主应用组件 ──────────────────────────────────────────────────────────────────

export function App({ input }: { input: string }) {
  const [state, setState] = useState<AppState>({
    mode: 'idle',
    message: 'Agent Harness CLI v0.1.0',
  });
  const { exit } = useApp();

  // 键盘快捷键
  useInput((input, key) => {
    if (input === 'q') {
      exit();
    }
    if (key.enter && state.mode === 'idle') {
      handleSendMessage(input);
    }
  });

  const handleSendMessage = async (message: string) => {
    if (!message.trim()) return;

    setState((prev) => ({
      ...prev,
      mode: 'running',
      message: '',
    }));

    // TODO: 连接到 Rust Runtime
    // 这里使用模拟响应作为占位
    setTimeout(() => {
      setState((prev) => ({
        ...prev,
        mode: 'idle',
        message: '模拟响应: ' + message,
      }));
    }, 1000);
  };

  const handleToolApproval = async (approved: boolean) => {
    if (!state.tool_card) return;

    if (approved) {
      setState((prev) => ({
        ...prev,
        tool_card: {
          ...prev.tool_card,
          type: 'EXECUTING',
          start_time: Date.now(),
        },
      }));

      // 模拟工具执行完成
      setTimeout(() => {
        setState((prev) => ({
          ...prev,
          tool_card: {
            type: 'OUTPUT_READY',
            tool_name: prev.tool_card?.tool_name || 'tool',
            result: { success: true, output: '工具执行完成' },
          },
        }));
      }, 2000);
    } else {
      setState((prev) => ({
        ...prev,
        tool_card: {
          type: 'OUTPUT_ERROR',
          tool_name: prev.tool_card?.tool_name || 'tool',
          error: '用户拒绝',
        },
      }));
    }
  };

  const handleCancel = () => {
    setState((prev) => ({
      ...prev,
      mode: 'idle',
      tool_card: undefined,
      message: '操作已取消',
    }));
  };

  // ── 渲染 ──────────────────────────────────────────────────────────────────────

  return (
    <Box flexDirection="column" height={process.stdout.innerHeight}>
      {/* 标题栏 */}
      <Box borderColor="cyan" borderStyle="single">
        <Text bold color="cyan"> Agent Harness CLI</Text>
        <Text dimColor> v0.1.0</Text>
      </Box>

      <Box marginTop={1} marginBottom={1}>
        <Text bold>快捷键:</Text>
        <Text> Enter=发送</Text>
        <Text> q=退出</Text>
        <Text> a=批准</Text>
        <Text> r=拒绝</Text>
      </Box>

      {/* 消息显示区 */}
      <Box marginTop={1} borderColor="gray" borderStyle="single">
        <Text color="yellow">{state.message}</Text>
      </Box>

      {/* 工具卡片 */}
      {state.tool_card && (
        <Box marginTop={1}>
          <ToolCard
            state={{
              ...state.tool_card,
              on_approve: () => handleToolApproval(true),
              on_reject: () => handleToolApproval(false),
              on_edit: () => {},
              on_cancel: handleCancel,
            }}
          />
        </Box>
      )}

      {/* 输入框 */}
      {state.mode === 'idle' && (
        <Box marginTop={1} flexDirection="row">
          <Text color="green"> {'>'} </Text>
          <Input
            onSubmit={handleSendMessage}
            placeholder="输入消息..."
          />
        </Box>
      )}

      {/* 状态指示 */}
      {state.mode === 'running' && (
        <Box marginTop={1}>
          <Text color="blue"> 正在处理...</Text>
        </Box>
      )}

      {state.error && (
        <Box marginTop={1}>
          <Text color="red"> 错误: {state.error}</Text>
        </Box>
      )}
    </Box>
  );
}
