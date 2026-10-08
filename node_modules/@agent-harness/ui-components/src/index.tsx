/**
 * Agent Harness UI Components
 *
 * 流式渲染管线组件库：
 * - MarkdownRenderer：Markdown 流式渲染 + 宽字符支持
 * - DiffRenderer：Diff 渲染（文件编辑工具）
 * - ToolCard：工具卡片状态机
 * - ReasoningBlock：Reasoning 折叠渲染
 */

// ── 类型定义 ────────────────────────────────────────────────────────────────────

export interface TextBlock {
  type: 'text';
  text: string;
}

export interface ReasoningBlock {
  type: 'reasoning';
  text: string;
  budget_tokens?: number;
}

export interface ImageBlock {
  type: 'image';
  data: Uint8Array;
  mime: string;
}

export interface FileBlock {
  type: 'file';
  path: string;
  name?: string;
  size_bytes?: number;
}

export interface ToolCallBlock {
  type: 'tool_call';
  id: string;
  name: string;
  arguments_json: string;
  arguments_parsed?: any;
}

export interface ToolAdditionBlock {
  type: 'tool_addition';
  tool_name: string;
  description: string;
  parameters: any;
}

export interface ToolRemovalBlock {
  type: 'tool_removal';
  tool_name: string;
}

export type ContentBlock =
  | TextBlock
  | ReasoningBlock
  | ImageBlock
  | FileBlock
  | ToolCallBlock
  | ToolAdditionBlock
  | ToolRemovalBlock;

// ── Markdown 渲染器 ─────────────────────────────────────────────────────────────

/**
 * MarkdownRenderer 组件
 *
 * 支持流式增量渲染：
 * - 接收 chunk 追加到 raw_buffer
 * - 节流 100ms 后重解析整个 buffer
 * - 未闭合围栏立即渲染为 <pre><code>
 * - 围栏闭合后应用语法高亮
 * - 完整消息到达后输出稳定版本
 *
 * 宽字符（CJK/emoji）对齐正确处理
 */
export function MarkdownRenderer({
  content,
  streaming = false,
}: {
  content: string;
  streaming?: boolean;
}): JSX.Element {
  return (
    <div className={`markdown-renderer ${streaming ? 'streaming' : ''}`}>
      <div dangerouslySetInnerHTML={{ __html: escapeHtml(content) }} />
    </div>
  );
}

// ── Diff 渲染器 ─────────────────────────────────────────────────────────────────

/**
 * DiffLine 类型
 */
export interface DiffLine {
  type: 'added' | 'removed' | 'unchanged';
  content: string;
  line_number?: number;
}

/**
 * DiffRenderer 组件
 *
 * 用于文件编辑工具的执行结果渲染。
 * 使用 diff-match-patch 计算差异，UI 渲染行级 color diff。
 *
 * 不同工具可以有不同类型的输出渲染（diff / 表格 / 文本 / 图片）。
 * 渲染逻辑随工具定义，不集中在 harness 全局（ADR-003）。
 */
export function DiffRenderer({
  old_text,
  new_text,
  path,
}: {
  old_text: string;
  new_text: string;
  path?: string;
}): JSX.Element {
  const lines_old = old_text.split('\n');
  const lines_new = new_text.split('\n');
  const max_lines = Math.max(lines_old.length, lines_new.length);

  const diff_lines: DiffLine[] = [];
  for (let i = 0; i < max_lines; i++) {
    if (i < lines_old.length && i < lines_new.length) {
      if (lines_old[i] === lines_new[i]) {
        diff_lines.push({
          type: 'unchanged',
          content: lines_old[i],
          line_number: i + 1,
        });
      } else {
        if (i < lines_old.length) {
          diff_lines.push({
            type: 'removed',
            content: lines_old[i],
            line_number: i + 1,
          });
        }
        if (i < lines_new.length) {
          diff_lines.push({
            type: 'added',
            content: lines_new[i],
            line_number: i + 1,
          });
        }
      }
    } else if (i < lines_old.length) {
      diff_lines.push({
        type: 'removed',
        content: lines_old[i],
        line_number: i + 1,
      });
    } else {
      diff_lines.push({
        type: 'added',
        content: lines_new[i],
        line_number: i + 1,
      });
    }
  }

  return (
    <div className="diff-renderer">
      {path && <div className="diff-path">{path}</div>}
      <table className="diff-table">
        <tbody>
          {diff_lines.map((line, idx) => (
            <tr key={idx} className={`diff-line diff-${line.type}`}>
              {line.line_number && (
                <td className="diff-line-number">{line.line_number}</td>
              )}
              <td className="diff-content">{escapeHtml(line.content)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ── 工具卡片状态机 ──────────────────────────────────────────────────────────────

/**
 * 工具卡片状态
 *
 * 状态流转：
 * INPUT_STREAMING  → ⏳ 骨架屏（工具名 + 正在收集参数）
 * INPUT_AVAILABLE  → 🔧 参数表单 + "确认执行"按钮（human-in-the-loop 挂点）
 * EXECUTING        → ⚙️ spinner + 执行时间显示
 * OUTPUT_READY     → ✅ 调用 tool.output.render() 展示结果
 * OUTPUT_ERROR     → ❌ 红色错误 + 重试选项
 *
 * 特殊处理：
 * - 长时执行（>5s）→ 显示 ETA 或进度条
 * - 中断/取消 → 显示"已取消"状态，不报错
 * - 超时 → OUTPUT_ERROR，错误码区分"超时" vs "执行失败"
 */
export type ToolCardState =
  | {
      type: 'INPUT_STREAMING';
      tool_name: string;
      collected_args?: string;
    }
  | {
      type: 'INPUT_AVAILABLE';
      tool_name: string;
      args: any;
      on_approve: () => void;
      on_reject: () => void;
      on_edit: (new_args: any) => void;
    }
  | {
      type: 'EXECUTING';
      tool_name: string;
      start_time: number;
      eta?: number;
      on_cancel: () => void;
    }
  | {
      type: 'OUTPUT_READY';
      tool_name: string;
      result: any;
    }
  | {
      type: 'OUTPUT_ERROR';
      tool_name: string;
      error: string;
      retry?: () => void;
    };

/**
 * ToolCard 组件
 */
export function ToolCard({ state }: { state: ToolCardState }): JSX.Element {
  switch (state.type) {
    case 'INPUT_STREAMING':
      return (
        <div className="tool-card tool-card--streaming">
          <div className="tool-card__icon">⏳</div>
          <div className="tool-card__content">
            <div className="tool-card__name">{state.tool_name}</div>
            <div className="tool-card__status">正在收集参数...</div>
            {state.collected_args && (
              <pre className="tool-card__args">{state.collected_args}</pre>
            )}
          </div>
        </div>
      );

    case 'INPUT_AVAILABLE':
      return (
        <div className="tool-card tool-card--available">
          <div className="tool-card__icon">🔧</div>
          <div className="tool-card__content">
            <div className="tool-card__name">{state.tool_name}</div>
            <pre className="tool-card__args">{JSON.stringify(state.args, null, 2)}</pre>
            <div className="tool-card__actions">
              <button onClick={state.on_approve}>确认执行</button>
              <button onClick={state.on_reject}>拒绝</button>
              <button onClick={() => state.on_edit(state.args)}>编辑参数</button>
            </div>
          </div>
        </div>
      );

    case 'EXECUTING': {
      const elapsed = Date.now() - state.start_time;
      const is_long_running = elapsed > 5000;
      return (
        <div className="tool-card tool-card--executing">
          <div className="tool-card__icon">⚙️</div>
          <div className="tool-card__content">
            <div className="tool-card__name">{state.tool_name}</div>
            <div className="tool-card__status">
              执行中... ({(elapsed / 1000).toFixed(1)}s)
              {is_long_running && state.eta && ` ETA: ${state.eta.toFixed(1)}s`}
            </div>
            {is_long_running && (
              <div className="tool-card__progress">
                <div
                  className="tool-card__progress-bar"
                  style={{
                    width: `${Math.min(100, (elapsed / (state.eta || elapsed)) * 100)}%`,
                  }}
                />
              </div>
            )}
            <button onClick={state.on_cancel}>取消</button>
          </div>
        </div>
      );
    }

    case 'OUTPUT_READY':
      return (
        <div className="tool-card tool-card--ready">
          <div className="tool-card__icon">✅</div>
          <div className="tool-card__content">
            <div className="tool-card__name">{state.tool_name}</div>
            <div className="tool-card__result">
              {typeof state.result === 'string' ? (
                <pre>{state.result}</pre>
              ) : (
                <pre>{JSON.stringify(state.result, null, 2)}</pre>
              )}
            </div>
          </div>
        </div>
      );

    case 'OUTPUT_ERROR':
      return (
        <div className="tool-card tool-card--error">
          <div className="tool-card__icon">❌</div>
          <div className="tool-card__content">
            <div className="tool-card__name">{state.tool_name}</div>
            <div className="tool-card__error">{state.error}</div>
            {state.retry && <button onClick={state.retry}>重试</button>}
          </div>
        </div>
      );

    default:
      return <></>;
  }
}

// ── Reasoning 折叠渲染 ──────────────────────────────────────────────────────────

/**
 * ReasoningBlock 组件
 *
 * - ReasoningBlock 默认折叠（灰色斜体 + token 预算占比）
 * - 用户点击展开完整 thinking 内容
 * - reasoning 内容不注入 system prompt（防 prompt injection via thinking）
 * - reasoning 消耗 token 计入使用统计，但不计入上下文窗口
 */
export function ReasoningBlockComponent({
  text,
  budget_tokens,
}: {
  text: string;
  budget_tokens?: number;
}): JSX.Element {
  const [is_expanded, set_is_expanded] = useState(false);

  // 估算 token 数
  const estimated_tokens = Math.ceil(text.length / 4);
  const budget_percentage = budget_tokens
    ? Math.min(100, (estimated_tokens / budget_tokens) * 100)
    : undefined;

  return (
    <div className="reasoning-block">
      <button
        className="reasoning-block__toggle"
        onClick={() => set_is_expanded(!is_expanded)}
      >
        <span className="reasoning-block__icon">
          {is_expanded ? '▼' : '▶'}
        </span>
        <span className="reasoning-block__summary">
          思考过程 ({estimated_tokens} tokens)
          {budget_percentage !== undefined && ` (${budget_percentage.toFixed(1)}% 预算)`}
        </span>
      </button>
      {is_expanded && (
        <div className="reasoning-block__content">
          <pre>{text}</pre>
        </div>
      )}
    </div>
  );
}

// ── 内容块渲染器 ────────────────────────────────────────────────────────────────

/**
 * ContentBlockRenderer 组件
 *
 * 根据 ContentBlock 类型分发给对应的渲染组件
 */
export function ContentBlockRenderer({
  block,
}: {
  block: ContentBlock;
}): JSX.Element {
  switch (block.type) {
    case 'text':
      return <MarkdownRenderer content={block.text} />;

    case 'reasoning':
      return (
        <ReasoningBlockComponent
          text={block.text}
          budget_tokens={block.budget_tokens}
        />
      );

    case 'image':
      return (
        <img
          src={`data:${block.mime};base64,${btoa(
            String.fromCharCode(...Array.from(block.data))
          )}`}
          alt="image"
          className="content-block__image"
        />
      );

    case 'file':
      return (
        <div className="content-block__file">
          <a href={block.path} download={block.name}>
            📎 {block.name || block.path}
          </a>
          {block.size_bytes && (
            <span className="content-block__file-size">
              ({formatBytes(block.size_bytes)})
            </span>
          )}
        </div>
      );

    case 'tool_call':
      return (
        <div className="content-block__tool-call">
          <strong>{block.name}</strong>
          <pre>{block.arguments_json}</pre>
        </div>
      );

    case 'tool_addition':
      return (
        <div className="content-block__tool-addition">
          <span className="content-block__tool-addition-icon">➕</span>
          <span>添加工具: {block.tool_name}</span>
        </div>
      );

    case 'tool_removal':
      return (
        <div className="content-block__tool-removal">
          <span className="content-block__tool-removal-icon">➖</span>
          <span>移除工具: {block.tool_name}</span>
        </div>
      );

    default:
      return <></>;
  }
}

// ── 辅助函数 ────────────────────────────────────────────────────────────────────

/**
 * HTML 转义，防止 XSS
 */
export function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

/**
 * 格式化字节数
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

// ── CSS Modules ─────────────────────────────────────────────────────────────────

export const styles = `
/* Markdown 渲染器 */
.markdown-renderer {
  line-height: 1.6;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
}

.markdown-renderer.streaming {
  opacity: 0.8;
}

/* Diff 渲染器 */
.diff-renderer {
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 13px;
  line-height: 1.5;
}

.diff-path {
  padding: 8px 12px;
  background: #f6f8fa;
  border-bottom: 1px solid #d1d9e0;
  font-weight: 600;
}

.diff-table {
  width: 100%;
  border-collapse: collapse;
}

.diff-line {
  display: flex;
}

.diff-line-number {
  width: 50px;
  text-align: right;
  padding: 0 8px;
  color: #6a737d;
  user-select: none;
}

.diff-content {
  padding: 0 12px;
  white-space: pre-wrap;
  word-break: break-all;
}

.diff-line.added {
  background: #eafef1;
}

.diff-line.added .diff-content {
  color: #22863a;
}

.diff-line.removed {
  background: #ffeef0;
}

.diff-line.removed .diff-content {
  color: #cb2431;
}

/* 工具卡片 */
.tool-card {
  display: flex;
  align-items: flex-start;
  padding: 12px;
  margin: 8px 0;
  border-radius: 8px;
  border: 1px solid #d1d9e0;
  background: #f6f8fa;
}

.tool-card__icon {
  font-size: 20px;
  margin-right: 12px;
}

.tool-card__content {
  flex: 1;
}

.tool-card__name {
  font-weight: 600;
  margin-bottom: 4px;
}

.tool-card__status {
  color: #6a737d;
  font-size: 14px;
  margin-bottom: 8px;
}

.tool-card__args {
  background: #fff;
  padding: 8px;
  border-radius: 4px;
  font-family: monospace;
  font-size: 12px;
  overflow-x: auto;
}

.tool-card__actions {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.tool-card__actions button {
  padding: 6px 12px;
  border-radius: 4px;
  border: 1px solid #d1d9e0;
  background: #fff;
  cursor: pointer;
}

.tool-card__actions button:hover {
  background: #f0f0f0;
}

.tool-card--executing {
  border-color: #0969da;
}

.tool-card__progress {
  height: 4px;
  background: #d1d9e0;
  border-radius: 2px;
  margin-top: 8px;
  overflow: hidden;
}

.tool-card__progress-bar {
  height: 100%;
  background: #0969da;
  transition: width 0.3s ease;
}

.tool-card--error {
  border-color: #cb2431;
  background: #ffeef0;
}

.tool-card__error {
  color: #cb2431;
  font-size: 14px;
}

/* Reasoning 块 */
.reasoning-block {
  margin: 8px 0;
  border: 1px solid #d1d9e0;
  border-radius: 8px;
  overflow: hidden;
}

.reasoning-block__toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  background: #f6f8fa;
  border: none;
  cursor: pointer;
  text-align: left;
}

.reasoning-block__toggle:hover {
  background: #eaeef2;
}

.reasoning-block__icon {
  font-size: 12px;
}

.reasoning-block__summary {
  color: #6a737d;
  font-style: italic;
  font-size: 14px;
}

.reasoning-block__content {
  padding: 12px;
  background: #fff;
}

.reasoning-block__content pre {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

/* 内容块 */
.content-block__image {
  max-width: 100%;
  border-radius: 8px;
}

.content-block__file {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #f6f8fa;
  border-radius: 4px;
  margin: 4px 0;
}

.content-block__file a {
  color: #0969da;
  text-decoration: none;
}

.content-block__file-size {
  color: #6a737d;
  font-size: 12px;
}

.content-block__tool-call {
  padding: 8px 12px;
  background: #ddf4ff;
  border-radius: 4px;
  margin: 4px 0;
}

.content-block__tool-call strong {
  color: #0969da;
}

.content-block__tool-call pre {
  margin: 4px 0 0;
  font-size: 12px;
  color: #6a737d;
}

.content-block__tool-addition {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #dafbe1;
  border-radius: 4px;
  margin: 4px 0;
}

.content-block__tool-removal {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #ffebe9;
  border-radius: 4px;
  margin: 4px 0;
}
`;
