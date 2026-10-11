#!/usr/bin/env node
/**
 * Agent Harness CLI - 交互式终端界面
 *
 * 支持两种模式：
 * - Interactive TUI：Ink 实现的终端交互界面
 * - Headless CLI：脚本化调用入口，输出到 stdout/管道
 */

import React from 'react';
import { run } from 'ink';
import { App } from './app';

// 解析命令行参数
const args = process.argv.slice(2);
const isHeadless = args.includes('--headless') || args.includes('-h');
const input = args.filter((a) => !a.startsWith('--') && !a.startsWith('-'));

if (isHeadless) {
  // Headless 模式：直接输出到 stdout
  console.log('[Headless Mode] Running in headless mode...');
  console.log('Input:', input.join(' '));
  // TODO: 实现 headless 模式的实际逻辑
  process.exit(0);
} else {
  // Interactive TUI 模式
  run(
    React.createElement(App, {
      input: input.join(' '),
    })
  );
}
