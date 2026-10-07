# Agent Harness

连接 LLM 与执行环境的自研运行框架，参考 DeepSeek Harness / OpenBitFun 设计。

## 项目书

完整的开发方案见 [agent-harness-项目开发书.md](agent-harness-项目开发书.md)，包含：

- 项目定位与目标
- 架构总览
- 核心数据结构（ContentBlock / Message / SessionEvent）
- Agent 主循环设计
- 工具系统架构
- 流式渲染管线
- 上下文管理 / 多 Agent 编排 / 安全权限
- 技术选型
- 模块拆分与开发里程碑

## 仓库说明

本仓库当前为项目规划阶段，包含：

- 项目开发书（设计文档）
- 已拆解为 11 个跟踪 issue（见仓库 Issues，标签 `project-tracking`）
- 后续将按 4 阶段里程碑（核心 Runtime → 渲染交互 → 高级能力 → 产品化）推进

## 相关资源

- [项目书全文](agent-harness-项目开发书.md)
- [项目跟踪 issue](https://github.com/sharkboot/agent-harness/issues/1)
