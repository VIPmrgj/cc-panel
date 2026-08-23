import { useEffect, useState } from "react";
import { Activity, CircleStop, LockKeyhole, RotateCcw } from "lucide-react";
import {
  getChatRunState,
  type ChatRunState,
  type ChatState,
} from "../../state/chatReducer";

export function RunCenter({
  chat,
  queuedCount,
  onStop,
  onRetry,
  onOpenPermission,
  onRecover,
}: {
  chat: ChatState;
  queuedCount: number;
  onStop: () => void;
  onRetry: () => void;
  onOpenPermission: () => void;
  onRecover: () => void;
}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!chat.activeTurnId) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [chat.activeTurnId]);
  const runState = getChatRunState(chat);
  const active = [
    "starting",
    "thinking",
    "tool-running",
    "awaiting-permission",
  ].includes(runState);
  const elapsed = active
    ? `已运行 ${formatDuration(Math.max(0, now - startedAt(chat.activeTurnId)))}`
    : "当前没有运行中的回合";
  const status = chat.pendingPermission
    ? "等待你的权限确认"
    : chat.activeTool
      ? `正在执行：${chat.activeTool.toolName || "工具"}`
      : statusLabel(runState);
  return (
    <section className="run-center" aria-labelledby="run-center-title">
      <div className="context-panel__header">
        <div>
          <p className="panel-eyebrow">RUN CENTER</p>
          <h2 id="run-center-title">运行中心</h2>
          <p>当前回合、工具进度、权限等待和排队消息都在这里。</p>
        </div>
        <Activity size={20} aria-hidden="true" />
      </div>
      <div className="run-summary" data-active={active || undefined}>
        <div className="run-summary__status">
          <span
            className="run-status-dot"
            data-status={
              chat.pendingPermission
                ? "permission"
                : ["stalled", "recovering"].includes(runState)
                  ? "warning"
                  : active
                    ? "running"
                    : runState === "failed"
                      ? "error"
                      : "idle"
            }
          />
          <strong>{status}</strong>
          <small>{elapsed}</small>
        </div>
        <div className="run-summary__actions">
          {chat.pendingPermission && (
            <button
              type="button"
              className="button button--secondary"
              onClick={onOpenPermission}
            >
              <LockKeyhole size={14} />
              打开权限中心
            </button>
          )}
          {active && (
            <button
              type="button"
              className="button button--danger"
              onClick={onStop}
            >
              <CircleStop size={14} />
              停止当前回合
            </button>
          )}
          {runState === "stalled" && (
            <button
              type="button"
              className="button button--secondary"
              onClick={onRecover}
            >
              <RotateCcw size={14} />
              检查并恢复会话
            </button>
          )}
          {chat.turnStatus === "failed" && (
            <button
              type="button"
              className="button button--secondary"
              onClick={onRetry}
            >
              <RotateCcw size={14} />
              重试上一轮
            </button>
          )}
        </div>
      </div>
      <div className="run-grid">
        <RunMetric
          label="会话"
          value={chat.sessionId ? chat.sessionId.slice(0, 12) : "未开始"}
        />
        <RunMetric
          label="进程"
          value={chat.runId ? `运行中 · ${chat.runId.slice(0, 8)}` : "未启动"}
        />
        <RunMetric label="排队消息" value={String(queuedCount)} />
        <RunMetric
          label="当前模型"
          value={chat.model ?? "由 Claude Code 决定"}
        />
      </div>
      {chat.activeTool && (
        <div className="active-tool-panel">
          <div>
            <span className="panel-eyebrow">ACTIVE TOOL</span>
            <strong>{chat.activeTool.toolName || "工具执行"}</strong>
          </div>
          <span className="tool-state">{chat.activeTool.state}</span>
          {chat.activeTool.text && <pre>{chat.activeTool.text}</pre>}
        </div>
      )}
      {chat.pendingPermission && (
        <div className="run-permission-hint">
          <LockKeyhole size={16} />
          <div>
            <strong>
              {chat.pendingPermission.toolName || "工具"} 正在等待许可
            </strong>
            <span>
              {chat.pendingPermission.permissionDescription ||
                "请在权限中心选择允许或拒绝。"}
            </span>
          </div>
        </div>
      )}
      {queuedCount > 0 && (
        <p className="run-queue-note">
          有 {queuedCount} 条消息排队中。当前回合结束后会按顺序发送。
        </p>
      )}
    </section>
  );
}

function RunMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="run-metric">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

function statusLabel(runState: ChatRunState) {
  switch (runState) {
    case "starting":
      return "正在启动 Claude Code";
    case "thinking":
      return "Claude 正在思考";
    case "tool-running":
      return "Claude 正在执行工具";
    case "awaiting-permission":
      return "等待你的权限确认";
    case "stopping":
      return "正在停止当前进程";
    case "stalled":
      return "超过 45 秒没有新进度，可能卡住";
    case "recovering":
      return "正在恢复会话";
    case "ended":
      return "当前回合已结束，可继续对话";
    case "failed":
      return "上一回合发生错误，可重试";
    case "disconnected":
      return "连接已断开，可恢复会话";
    case "idle":
    default:
      return "空闲，可发送新消息";
  }
}

function startedAt(turnId: string | null) {
  if (!turnId) return Date.now();
  const match = turnId.match(/(\d{10,})/);
  return match ? Number(match[1]) : Date.now();
}

function formatDuration(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return seconds < 60
    ? `${seconds}s`
    : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}
