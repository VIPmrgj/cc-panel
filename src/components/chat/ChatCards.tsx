import {
  Check,
  ChevronDown,
  Clock3,
  Loader2,
  ShieldAlert,
  Terminal,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { ActiveTool } from "../../state/chatReducer";
import type { AssistantBlock, ChatMessage } from "../../api/dto";
import { classifyPermissionRisk } from "../../state/permissionRisk";

interface ThinkingCardProps {
  text: string;
  active?: boolean;
}

export function ThinkingCard({ text, active = false }: ThinkingCardProps) {
  // Task #6: thinking card is expanded by default; the summary row stays
  // visible either way, so collapsing is cheap for anyone who wants it gone.
  const [open, setOpen] = useState(true);
  return (
    <details className="chat-card chat-card--thinking" open={open}>
      <summary
        onClick={(event) => {
          event.preventDefault();
          setOpen((value) => !value);
        }}
      >
        <Clock3 size={14} aria-hidden="true" />
        <span>{active ? "正在思考" : "思考摘要"}</span>
        <ChevronDown
          size={14}
          aria-hidden="true"
          className="chat-card__chevron"
        />
      </summary>
      {open && (
        <p className="chat-card__body">{text || "模型正在整理上下文…"}</p>
      )}
    </details>
  );
}

interface ToolCardProps {
  toolUseId: string;
  toolName: string;
  input: unknown;
  result?: { content: string; isError: boolean };
}

export function ToolCard({
  toolUseId,
  toolName,
  input,
  result,
}: ToolCardProps) {
  const [open, setOpen] = useState(!result);
  return (
    <details
      className={`chat-card chat-card--tool ${result?.isError ? "chat-card--error" : ""}`}
      open={open}
    >
      <summary
        onClick={(event) => {
          event.preventDefault();
          setOpen((value) => !value);
        }}
      >
        <Terminal size={14} aria-hidden="true" />
        <strong>{toolName}</strong>
        <code title={toolUseId}>{toolUseId}</code>
        {!result && <span className="chat-card__status">运行中</span>}
        {result && !result.isError && <Check size={14} aria-label="已完成" />}
        {result?.isError && <X size={14} aria-label="工具失败" />}
        <ChevronDown
          size={14}
          aria-hidden="true"
          className="chat-card__chevron"
        />
      </summary>
      {open && (
        <>
          {input !== null && (
            <pre className="chat-card__code">{formatValue(input)}</pre>
          )}
          {result && (
            <pre
              className="chat-card__result"
              data-error={result.isError || undefined}
            >
              {result.content}
            </pre>
          )}
        </>
      )}
    </details>
  );
}

const TOOL_STATE_LABELS: Record<string, string> = {
  in_progress: "运行中",
  completed: "已完成",
  error: "失败",
  cancelled: "已取消",
};

/**
 * Live preview of the tool Claude is currently running, fed by `tool_progress`
 * events. Rendered pinned to the bottom of the transcript so the executing
 * tool is always visible, unlike the per-message ToolCard which only appears
 * once the tool-use block lands in a message.
 */
export function ActiveToolCard({
  toolUseId,
  toolName,
  state,
  subtype,
  text,
}: ActiveTool) {
  const done = state === "completed";
  const failed = state === "error";
  const [open, setOpen] = useState(!done);
  return (
    <details
      className={`chat-card chat-card--tool chat-card--tool-active ${failed ? "chat-card--error" : ""}`}
      open={open}
    >
      <summary
        onClick={(event) => {
          event.preventDefault();
          setOpen((value) => !value);
        }}
      >
        {done ? (
          <Check size={14} aria-label="已完成" />
        ) : failed ? (
          <X size={14} aria-label="工具失败" />
        ) : (
          <Loader2
            size={14}
            className="chat-card__spinner"
            aria-hidden="true"
          />
        )}
        <strong>{toolName || "工具"}</strong>
        <code title={toolUseId}>{toolUseId}</code>
        <span className="chat-card__status">
          {TOOL_STATE_LABELS[state] ?? state}
        </span>
        {subtype === "summary" && (
          <span className="chat-card__status">输出已截断</span>
        )}
        <ChevronDown
          size={14}
          aria-hidden="true"
          className="chat-card__chevron"
        />
      </summary>
      {open && text && (
        <pre className="chat-card__result chat-card__result--live">{text}</pre>
      )}
    </details>
  );
}

export type PermissionDecision =
  "allow" | "session" | "always" | "deny" | "deny-interrupt";

interface PermissionCardProps {
  requestId: string;
  toolName?: string | null;
  input?: unknown;
  expiresAt?: number | null;
  pendingCount?: number;
  busy?: boolean;
  onRespond: (behavior: PermissionDecision) => void;
  onRetry: () => void;
}

export function PermissionCard({
  requestId,
  toolName,
  input,
  expiresAt,
  pendingCount = 1,
  busy = false,
  onRespond,
  onRetry,
}: PermissionCardProps) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, []);

  const remainingMs =
    expiresAt && expiresAt > 0 ? Math.max(0, expiresAt - now) : 0;
  const expired = remainingMs === 0;
  const remainingSeconds = Math.ceil(remainingMs / 1000);
  const permissionDetails = describePermissionInput(input);
  const risk = classifyPermissionRisk({
    id: "permission-" + requestId,
    role: "permission",
    content: "",
    toolName,
    toolInput: input,
  } satisfies ChatMessage);
  const labelledBy = `permission-${requestId}`;

  return (
    <article
      className={`chat-card chat-card--permission${expired ? " chat-card--permission-expired" : ""}`}
      aria-labelledby={labelledBy}
      aria-busy={busy}
    >
      <header className="chat-card__header">
        <ShieldAlert size={15} aria-hidden="true" />
        <strong id={labelledBy}>权限中心</strong>
        <span
          className="permission-risk"
          title={risk.reason}
          data-risk={risk.level}
        >
          {risk.level === "high" ? "高风险，需确认" : "低风险，将自动允许"}
        </span>
        <span className="chat-card__status">
          {expired ? "已失效" : `待处理 ${pendingCount} 项`}
        </span>
      </header>
      <p className="chat-card__permission-summary">
        {toolName ? (
          <>
            Claude Code 请求使用 <code>{toolName}</code>
          </>
        ) : (
          "Claude Code 请求执行一个受保护的操作"
        )}
        {!expired && "，请确认后继续。"}
      </p>
      <dl className="permission-details">
        <div>
          <dt>工具</dt>
          <dd>{toolName || "未知工具"}</dd>
        </div>
        <div>
          <dt>命令</dt>
          <dd title={permissionDetails.command ?? undefined}>
            {permissionDetails.command || "未提供命令"}
          </dd>
        </div>
        <div>
          <dt>工作目录</dt>
          <dd title={permissionDetails.cwd ?? undefined}>
            {permissionDetails.cwd || "未提供"}
          </dd>
        </div>
      </dl>
      {input !== undefined && (
        <details className="permission-details__raw">
          <summary>查看完整请求数据</summary>
          <pre className="chat-card__code">{formatValue(input)}</pre>
        </details>
      )}
      <div className="permission-countdown" role="status" aria-live="polite">
        {expired
          ? "确认已失效，重试后会重新开启 120 秒倒计时。"
          : `本次请求将在 ${remainingSeconds} 秒后失效`}
      </div>
      <div className="chat-card__actions chat-card__actions--permission">
        {expired ? (
          <button
            type="button"
            className="button button--primary"
            disabled={busy}
            onClick={onRetry}
          >
            <Loader2 size={14} aria-hidden="true" />
            失效后重试
          </button>
        ) : (
          <>
            <button
              type="button"
              className="button button--danger"
              disabled={busy}
              onClick={() => onRespond("deny-interrupt")}
            >
              <X size={14} aria-hidden="true" />
              拒绝并中断
            </button>
            <button
              type="button"
              className="button button--muted"
              disabled={busy}
              onClick={() => onRespond("deny")}
            >
              拒绝
            </button>
            <button
              type="button"
              className="button button--muted"
              disabled={busy}
              onClick={() => onRespond("session")}
            >
              本会话允许
            </button>
            <button
              type="button"
              className="button button--muted"
              disabled={busy}
              onClick={() => onRespond("always")}
            >
              永久允许匹配规则
            </button>
            <button
              type="button"
              className="button button--primary"
              disabled={busy}
              onClick={() => onRespond("allow")}
            >
              <Check size={14} aria-hidden="true" />
              允许一次
            </button>
          </>
        )}
      </div>
    </article>
  );
}

function describePermissionInput(input: unknown): {
  command: string | null;
  cwd: string | null;
} {
  if (!input || typeof input !== "object") {
    return { command: null, cwd: null };
  }
  const record = input as Record<string, unknown>;
  const pickString = (...keys: string[]) =>
    keys
      .map((key) => record[key])
      .find(
        (value): value is string =>
          typeof value === "string" && value.trim().length > 0,
      )
      ?.trim() ?? null;
  return {
    command: pickString("command", "cmd", "script"),
    cwd: pickString("cwd", "working_directory", "workingDirectory"),
  };
}

export function BlockCard({ block }: { block: AssistantBlock }) {
  if (block.type === "thinking") return <ThinkingCard text={block.thinking} />;
  if (block.type === "tool-use") {
    return (
      <ToolCard
        toolUseId={block.toolUseId}
        toolName={block.toolName}
        input={block.input}
      />
    );
  }
  if (block.type === "tool-result") {
    return (
      <ToolCard
        toolUseId={block.toolUseId}
        toolName="工具结果"
        input={null}
        result={block}
      />
    );
  }
  return null;
}

function formatValue(value: unknown) {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "[无法显示此数据]";
  }
}
