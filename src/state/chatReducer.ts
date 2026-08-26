import type {
  AssistantBlock,
  ChatMessage,
  ClaudeLifecycleStatus,
  ClaudeRunEnvelope,
  ClaudeRunEvent,
} from "../api/dto";
import { authFailureHint } from "./authHint";

function hintAuthFailure(message: string): string {
  const hint = authFailureHint(message);
  return hint ? `${message}\n\n${hint}` : message;
}

export type TurnStatus = "idle" | "running" | "awaiting-permission" | "failed";

export type ChatRunState =
  | "disconnected"
  | "starting"
  | "thinking"
  | "tool-running"
  | "awaiting-permission"
  | "stopping"
  | "stalled"
  | "recovering"
  | "idle"
  | "ended"
  | "failed";

export type RecoveryStatus = "none" | "suspected" | "recovering";

export type SessionTerminationReason = "switching-model" | "interrupted";

/** Live state of the tool Claude is currently executing, fed by `tool_progress`. */
export interface ActiveTool {
  toolUseId: string;
  toolName: string;
  state: string;
  subtype?: string | null;
  text?: string | null;
}

const ACTIVE_TOOL_TEXT_LIMIT = 2048;

export interface ChatState {
  viewGeneration: number;
  sessionId: string | null;
  runId: string | null;
  lifecycle: ClaudeLifecycleStatus | "disconnected";
  processReleased: boolean;
  interruptionRequested: boolean;
  terminationReason: SessionTerminationReason | null;
  turnStatus: TurnStatus;
  activeTurnId: string | null;
  activeAssistantId: string | null;
  activeTool: ActiveTool | null;
  recoveryStatus: RecoveryStatus;
  lastEventAt: number | null;
  statusMessage: string | null;
  messages: ChatMessage[];
  pendingPermission: ChatMessage | null;
  lastSequence: number;
  model: string | null;
  claudeCodeVersion: string | null;
}

export const initialChatState: ChatState = {
  viewGeneration: 0,
  sessionId: null,
  runId: null,
  lifecycle: "disconnected",
  processReleased: true,
  interruptionRequested: false,
  terminationReason: null,
  turnStatus: "idle",
  activeTurnId: null,
  activeAssistantId: null,
  activeTool: null,
  recoveryStatus: "none",
  lastEventAt: null,
  statusMessage: null,
  messages: [],
  pendingPermission: null,
  lastSequence: -1,
  model: null,
  claudeCodeVersion: null,
};

export type ChatAction =
  | {
      type: "reset";
      generation?: number;
      sessionId?: string | null;
      runId?: string | null;
    }
  | {
      type: "session-leaving";
      generation?: number;
      sessionId: string;
      runId: string;
      reason?: SessionTerminationReason;
    }
  | {
      type: "session-loading";
      generation?: number;
      sessionId: string;
    }
  | {
      type: "session-started";
      generation?: number;
      sessionId: string;
      runId: string;
    }
  | {
      type: "history";
      generation?: number;
      sessionId: string;
      messages: ChatMessage[];
    }
  | {
      type: "turn-started";
      sessionId: string;
      runId: string;
      turnId: string;
      message: ChatMessage;
    }
  | {
      type: "turn-message-committed";
      sessionId: string;
      runId: string;
      turnId: string;
      message: ChatMessage;
    }
  | {
      type: "turn-failed";
      sessionId: string;
      runId: string;
      turnId: string;
      message?: string;
    }
  | {
      type: "permission-response";
      sessionId: string;
      runId: string;
      requestId: string;
      behavior: "allow" | "deny";
      interrupted?: boolean;
    }
  | {
      type: "permission-response-failed";
      sessionId: string;
      runId: string;
      requestId: string;
      code?: string;
    }
  | {
      type: "permission-retried";
      sessionId: string;
      runId: string;
      requestId: string;
      expiresAt: number;
    }
  | { type: "recovery-suspected" }
  | { type: "recovery-started" }
  | { type: "recovery-cleared" }
  | { type: "envelope"; envelope: ClaudeRunEnvelope };

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  if (action.type === "reset") {
    return {
      ...initialChatState,
      viewGeneration: action.generation ?? state.viewGeneration + 1,
      sessionId: action.sessionId ?? null,
      runId: action.runId ?? null,
      processReleased: !action.sessionId,
      interruptionRequested: false,
      terminationReason: null,
    };
  }

  if (action.type === "session-leaving") {
    if (!matchesScope(state, action.sessionId, action.runId)) return state;
    const generation = action.generation ?? state.viewGeneration;
    if (generation < state.viewGeneration) return state;
    return {
      ...state,
      viewGeneration: generation,
      lifecycle: "stopping",
      terminationReason: action.reason ?? state.terminationReason,
      statusMessage:
        action.reason === "switching-model"
          ? "Claude 正在切换模型…"
          : action.reason === "interrupted"
            ? "Claude 会话正在中断…"
            : state.statusMessage,
      ...clearActiveTurn(state.messages),
    };
  }

  if (action.type === "session-loading") {
    const generation = action.generation ?? state.viewGeneration;
    if (generation < state.viewGeneration) return state;
    return {
      ...state,
      viewGeneration: generation,
      sessionId: action.sessionId,
      lifecycle: "starting",
      processReleased: false,
      interruptionRequested: false,
      terminationReason: null,
      ...clearActiveTurn(state.messages),
      statusMessage: null,
    };
  }

  if (action.type === "session-started") {
    const generation = action.generation ?? state.viewGeneration;
    if (generation !== state.viewGeneration) return state;
    if (state.runId === action.runId && state.sessionId === action.sessionId) {
      return state;
    }
    return {
      ...state,
      viewGeneration: generation,
      sessionId: action.sessionId,
      runId: action.runId,
      lifecycle: "starting",
      processReleased: false,
      interruptionRequested: false,
      terminationReason: null,
      ...clearActiveTurn(state.messages),
      statusMessage: null,
      lastSequence: -1,
    };
  }

  if (action.type === "history") {
    if (
      action.generation !== state.viewGeneration ||
      state.sessionId !== action.sessionId
    ) {
      return state;
    }
    const pendingPermission = findPendingPermission(action.messages);
    return {
      ...state,
      sessionId: action.sessionId,
      activeTool: null,
      recoveryStatus: "none",
      terminationReason: null,
      messages: action.messages,
      pendingPermission,
      turnStatus: pendingPermission ? "awaiting-permission" : "idle",
      lastSequence: -1,
    };
  }

  if (action.type === "turn-started") {
    if (!matchesScope(state, action.sessionId, action.runId)) return state;
    return {
      ...state,
      recoveryStatus: "none",
      lastEventAt: Date.now(),
      terminationReason: null,
      turnStatus: "running",
      activeTurnId: action.turnId,
      activeAssistantId: null,
      messages: appendOrReplace(state.messages, {
        ...action.message,
        turnId: action.turnId,
        status: "running",
      }),
    };
  }

  if (action.type === "turn-message-committed") {
    if (!matchesScope(state, action.sessionId, action.runId)) {
      return state;
    }
    const existing = state.messages.find(
      (message) => message.turnId === action.turnId,
    );
    if (!existing) return state;
    return {
      ...state,
      messages: appendOrReplace(state.messages, {
        ...action.message,
        turnId: action.turnId,
        status: "complete",
      }),
    };
  }

  if (action.type === "turn-failed") {
    if (
      !matchesScope(state, action.sessionId, action.runId) ||
      state.activeTurnId !== action.turnId
    ) {
      return state;
    }
    return {
      ...state,
      turnStatus: "failed",
      activeTurnId: null,
      statusMessage: action.message ?? state.statusMessage,
      messages: state.messages.map((message) =>
        message.turnId === action.turnId
          ? { ...message, status: "error" as const }
          : message,
      ),
    };
  }

  if (action.type === "permission-response") {
    if (!matchesScope(state, action.sessionId, action.runId)) return state;
    const pending = state.messages.find(
      (message) =>
        message.role === "permission" &&
        message.requestId === action.requestId &&
        message.status === "pending",
    );
    if (!pending) return state;
    const messages = state.messages.map((message) =>
      message.id === pending.id
        ? { ...message, status: "complete" as const }
        : message,
    );
    const nextPermission = findPendingPermission(messages);
    const interrupted =
      action.interrupted === true && action.behavior === "deny";
    return {
      ...state,
      turnStatus: interrupted
        ? "idle"
        : nextPermission
          ? "awaiting-permission"
          : "running",
      interruptionRequested: interrupted || state.interruptionRequested,
      terminationReason: interrupted ? "interrupted" : state.terminationReason,
      statusMessage: interrupted
        ? "已拒绝权限，Claude 会话已中断，可以继续发送消息。"
        : state.statusMessage,
      pendingPermission: nextPermission,
      messages,
    };
  }

  if (action.type === "permission-response-failed") {
    if (!matchesScope(state, action.sessionId, action.runId)) return state;
    const resolved = state.messages.find(
      (message) =>
        message.role === "permission" &&
        message.requestId === action.requestId &&
        message.status === "complete",
    );
    if (!resolved) return state;
    const restored = {
      ...resolved,
      status: "pending" as const,
      permissionExpiresAt:
        action.code === "PERMISSION_EXPIRED" ? 0 : resolved.permissionExpiresAt,
    };
    const messages = appendOrReplace(state.messages, restored);
    return {
      ...state,
      interruptionRequested: false,
      terminationReason: null,
      turnStatus: "awaiting-permission",
      pendingPermission: findPendingPermission(messages),
      messages,
    };
  }

  if (action.type === "permission-retried") {
    if (!matchesScope(state, action.sessionId, action.runId)) return state;
    const messages = state.messages.map((message) =>
      message.requestId === action.requestId
        ? {
            ...message,
            status: "pending" as const,
            permissionExpiresAt: action.expiresAt,
          }
        : message,
    );
    const pendingPermission = findPendingPermission(messages);
    return {
      ...state,
      turnStatus: pendingPermission ? "awaiting-permission" : state.turnStatus,
      pendingPermission,
      messages,
    };
  }

  if (action.type === "recovery-suspected") {
    if (
      state.recoveryStatus !== "none" ||
      !state.activeTurnId ||
      state.pendingPermission
    ) {
      return state;
    }
    return {
      ...state,
      recoveryStatus: "suspected",
      statusMessage: "超过 45 秒没有收到新的进度，可能需要恢复会话。",
    };
  }

  if (action.type === "recovery-started") {
    return {
      ...state,
      recoveryStatus: "recovering",
      statusMessage: "正在恢复当前会话，请稍候…",
    };
  }

  if (action.type === "recovery-cleared") {
    return {
      ...state,
      recoveryStatus: "none",
    };
  }

  const envelope = action.envelope;
  if (!state.runId || envelope.runId !== state.runId) return state;
  if (
    (state.sessionId && envelope.sessionId !== state.sessionId) ||
    (!state.sessionId && envelope.sessionId)
  ) {
    return state;
  }
  if (envelope.sequence <= state.lastSequence) return state;

  const accepted: ChatState = {
    ...state,
    sessionId: state.sessionId || envelope.sessionId || null,
    runId: state.runId || envelope.runId,
    lastSequence: envelope.sequence,
    lastEventAt: Date.now(),
    recoveryStatus: "none",
  };
  return reduceEvent(accepted, envelope.event);
}

function clearActiveTurn(messages: ChatMessage[]) {
  return {
    turnStatus: "idle" as const,
    activeTurnId: null,
    activeAssistantId: null,
    activeTool: null,
    recoveryStatus: "none" as const,
    lastEventAt: null,
    pendingPermission: null,
    messages: finalizeRunningAssistants(resolvePendingPermissions(messages)),
  };
}

function reduceEvent(state: ChatState, event: ClaudeRunEvent): ChatState {
  switch (event.type) {
    case "lifecycle": {
      const processReleased = [
        "exited",
        "failed",
        "timed-out",
        "disconnected",
      ].includes(event.status);
      return {
        ...state,
        lifecycle: event.status,
        recoveryStatus: ["starting", "running", "awaiting-permission"].includes(
          event.status,
        )
          ? "none"
          : state.recoveryStatus,
        processReleased: state.processReleased || processReleased,
        statusMessage:
          state.terminationReason === "switching-model"
            ? "Claude 正在切换模型…"
            : state.terminationReason === "interrupted"
              ? "Claude 会话已中断，可以继续发送消息。"
              : event.status === "interrupted" && state.interruptionRequested
                ? "当前回合已中断，可以继续发送消息。"
                : (event.message ?? null),
        turnStatus:
          processReleased && state.turnStatus === "running"
            ? "failed"
            : event.status === "awaiting-permission"
              ? "awaiting-permission"
              : state.turnStatus,
        pendingPermission:
          processReleased || event.status === "interrupted"
            ? null
            : state.pendingPermission,
        // A lifecycle update is not a permission response. Claude can emit a
        // running update while several tool requests are still queued, so
        // resolving pending permissions here makes their buttons look dead
        // and loses the queue. Only terminal lifecycle states may close them.
        messages:
          processReleased || event.status === "interrupted"
            ? resolvePendingPermissions(state.messages)
            : state.messages,
        activeTool:
          event.status === "awaiting-permission"
            ? state.activeTool
            : processReleased
              ? null
              : state.activeTool,
      };
    }
    case "init":
      return {
        ...state,
        model: event.model ?? state.model,
        claudeCodeVersion: event.claudeCodeVersion ?? state.claudeCodeVersion,
      };
    case "assistant": {
      const content = event.blocks
        .filter(
          (block): block is Extract<AssistantBlock, { type: "text" }> =>
            block.type === "text",
        )
        .map((block) => block.text)
        .join("");
      const message: ChatMessage = {
        id: event.messageId,
        role: "assistant",
        content,
        blocks: event.blocks,
        status: "complete",
      };
      const promoted = promoteActiveAssistant(state, event.messageId, message);
      const messages = isThinkingOnlyAssistant(message)
        ? removeStaleThinkingMessages(promoted, message.id)
        : settleThinkingMessages(promoted);
      return {
        ...state,
        activeAssistantId: null,
        messages,
      };
    }
    case "stream":
      return reduceStream(state, event);
    case "tool-use": {
      const next = upsertBlock(state, event.toolUseId, {
        type: "tool-use",
        toolUseId: event.toolUseId,
        toolName: event.toolName,
        input: event.input,
      });
      return { ...next, messages: settleThinkingMessages(next.messages) };
    }
    case "tool-progress":
      return reduceToolProgress(state, event);
    case "tool-result": {
      const next = upsertBlock(state, event.toolUseId, {
        type: "tool-result",
        toolUseId: event.toolUseId,
        content: event.content,
        isError: event.isError,
      });
      return {
        ...next,
        activeTool:
          state.activeTool?.toolUseId === event.toolUseId
            ? null
            : state.activeTool,
        messages: settleThinkingMessages(next.messages),
      };
    }
    case "permission": {
      const existing = state.messages.find(
        (message) => message.requestId === event.requestId,
      );
      if (existing && existing.status !== "pending") return state;
      const permission: ChatMessage = {
        id: "permission-" + event.requestId,
        role: "permission",
        content: "",
        requestId: event.requestId,
        toolName: event.toolName ?? null,
        toolInput: event.input,
        permissionExpiresAt: event.expiresAt ?? null,
        status: "pending",
      };
      const messages = settleThinkingMessages(
        appendOrReplace(state.messages, permission),
      );
      return {
        ...state,
        turnStatus: "awaiting-permission",
        pendingPermission: findPendingPermission(messages),
        messages,
      };
    }
    case "compaction":
      return {
        ...state,
        statusMessage:
          event.phase === "starting"
            ? "正在压缩较早的上下文…"
            : "上下文压缩完成。",
      };
    case "result":
      return {
        ...state,
        turnStatus: state.terminationReason
          ? "idle"
          : event.success
            ? "idle"
            : "failed",
        statusMessage:
          state.terminationReason === "switching-model"
            ? "Claude 正在切换模型…"
            : state.terminationReason === "interrupted"
              ? "Claude 会话已中断，可以继续发送消息。"
              : (event.stopReason ?? null),
        activeTurnId: null,
        activeAssistantId: null,
        activeTool: null,
        pendingPermission: null,
        messages: settleThinkingMessages(
          finalizeRunningAssistants(resolvePendingPermissions(state.messages)),
        ),
      };
    case "error": {
      if (event.code === "PERMISSION_EXPIRED") {
        const expiredSource = event.requestId
          ? state.messages.find(
              (message) =>
                message.role === "permission" &&
                message.requestId === event.requestId &&
                message.status === "pending",
            )
          : state.pendingPermission;
        if (expiredSource) {
          const expired = {
            ...expiredSource,
            status: "pending" as const,
            permissionExpiresAt: 0,
          };
          const messages = settleThinkingMessages(
            appendOrReplace(state.messages, expired),
          );
          return {
            ...state,
            statusMessage: event.message,
            turnStatus: "awaiting-permission",
            pendingPermission: findPendingPermission(messages),
            messages,
          };
        }
      }
      if (
        state.terminationReason === "switching-model" ||
        state.terminationReason === "interrupted" ||
        state.interruptionRequested
      ) {
        return {
          ...state,
          statusMessage:
            state.terminationReason === "switching-model"
              ? "Claude 正在切换模型…"
              : "Claude 会话已中断，可以继续发送消息。",
          turnStatus: "idle",
          activeTurnId: null,
          activeAssistantId: null,
          activeTool: null,
          pendingPermission: null,
          messages: settleThinkingMessages(
            finalizeRunningAssistants(
              resolvePendingPermissions(state.messages),
            ),
          ),
        };
      }
      return {
        ...state,
        statusMessage: event.message,
        turnStatus: "failed",
        activeTurnId: null,
        activeTool: null,
        pendingPermission: null,
        messages: appendOrReplace(
          settleThinkingMessages(
            finalizeRunningAssistants(
              resolvePendingPermissions(state.messages),
            ),
          ),
          {
            id:
              "error-" + (state.runId ?? "session") + "-" + state.lastSequence,
            role: "error",
            content: hintAuthFailure(event.message),
            status: "error",
          },
        ),
      };
    }
    case "unknown":
      return state;
  }
}

function reduceStream(
  state: ChatState,
  event: Extract<ClaudeRunEvent, { type: "stream" }>,
): ChatState {
  const requestedId = event.messageId ?? null;
  const id =
    requestedId ??
    state.activeAssistantId ??
    `stream-${state.runId ?? "run"}-${state.lastSequence + 1}`;
  const existing = state.messages.find((message) => message.id === id);
  const base: ChatMessage = existing ?? {
    id,
    role: "assistant",
    content: "",
    blocks: [],
    status: "running",
  };
  const next: ChatMessage = { ...base, status: "running" };
  if (event.deltaType === "text") {
    next.content = `${base.content}${event.delta}`;
  } else if (event.deltaType === "thinking") {
    const blocks = [...(base.blocks ?? [])];
    const last = blocks.at(-1);
    if (last?.type === "thinking") {
      blocks[blocks.length - 1] = {
        ...last,
        thinking: `${last.thinking}${event.delta}`,
      };
    } else {
      blocks.push({ type: "thinking", thinking: event.delta });
    }
    next.blocks = blocks;
  }
  const messages =
    requestedId &&
    state.activeAssistantId &&
    requestedId !== state.activeAssistantId
      ? replaceId(state.messages, state.activeAssistantId, next)
      : appendOrReplace(state.messages, next);
  const visibleMessages =
    event.deltaType === "thinking"
      ? removeStaleThinkingMessages(messages, id)
      : settleThinkingMessages(messages);
  return {
    ...state,
    activeAssistantId: id,
    messages: visibleMessages,
  };
}

function promoteActiveAssistant(
  state: ChatState,
  finalId: string,
  message: ChatMessage,
): ChatMessage[] {
  if (state.activeAssistantId && state.activeAssistantId !== finalId) {
    return replaceId(state.messages, state.activeAssistantId, message);
  }
  return appendOrReplace(state.messages, message);
}

function reduceToolProgress(
  state: ChatState,
  event: Extract<ClaudeRunEvent, { type: "tool-progress" }>,
): ChatState {
  const current = state.activeTool;
  if (!current || current.toolUseId !== event.toolUseId) {
    return { ...state, activeTool: { ...event } };
  }
  const text =
    event.text == null
      ? current.text
      : (current.text ?? "").length >= ACTIVE_TOOL_TEXT_LIMIT
        ? current.text
        : ((current.text ?? "") + event.text).slice(0, ACTIVE_TOOL_TEXT_LIMIT);
  return {
    ...state,
    activeTool: {
      toolUseId: event.toolUseId,
      toolName: event.toolName || current.toolName,
      state: event.state || current.state,
      subtype: event.subtype ?? current.subtype,
      text,
    },
  };
}
function upsertBlock(
  state: ChatState,
  id: string,
  block: AssistantBlock,
): ChatState {
  const existing = state.messages.find((message) => message.id === id);
  const message: ChatMessage = existing ?? {
    id,
    role: "assistant",
    content: "",
    blocks: [],
    status: "running",
  };
  return {
    ...state,
    messages: appendOrReplace(state.messages, {
      ...message,
      blocks: [...(message.blocks ?? []), block],
    }),
  };
}

function matchesScope(state: ChatState, sessionId: string, runId: string) {
  return state.sessionId === sessionId && state.runId === runId;
}

function appendOrReplace(messages: ChatMessage[], message: ChatMessage) {
  const index = messages.findIndex((item) => item.id === message.id);
  if (index === -1) return [...messages, message];
  const next = messages.slice();
  next[index] = message;
  return next;
}

function replaceId(
  messages: ChatMessage[],
  oldId: string,
  message: ChatMessage,
) {
  const index = messages.findIndex((item) => item.id === oldId);
  if (index === -1) return appendOrReplace(messages, message);
  const next = messages.slice();
  next[index] = message;
  return next;
}

function isThinkingOnlyAssistant(message: ChatMessage) {
  return (
    message.role === "assistant" &&
    !message.content.trim() &&
    (message.blocks?.length ?? 0) > 0 &&
    message.blocks?.every((block) => block.type === "thinking") === true
  );
}

function removeStaleThinkingMessages(
  messages: ChatMessage[],
  keepId: string,
): ChatMessage[] {
  return messages.filter(
    (message) => message.id === keepId || !isThinkingOnlyAssistant(message),
  );
}

function settleThinkingMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.flatMap((message) => {
    if (message.role !== "assistant") return [message];
    const visibleBlocks = (message.blocks ?? []).filter(
      (block) => block.type !== "thinking",
    );
    if (!message.content.trim() && visibleBlocks.length === 0) {
      return message.status === "running" ? [message] : [];
    }
    if (visibleBlocks.length === (message.blocks?.length ?? 0)) {
      return [message];
    }
    return [{ ...message, blocks: visibleBlocks }];
  });
}
/**
 * When a turn ends, any assistant message still streaming (`status: "running"`)
 * must be finalized: non-empty ones become complete, empty placeholders are
 * dropped. Without this, a leftover running bubble with a Claude avatar and a
 * green "生成中" badge lingers below the final reply.
 */
function finalizeRunningAssistants(messages: ChatMessage[]): ChatMessage[] {
  return messages
    .map((message) => {
      if (message.role === "assistant" && message.status === "running") {
        const empty = !message.content && !(message.blocks?.length ?? 0);
        return empty ? null : { ...message, status: "complete" };
      }
      return message;
    })
    .filter((message): message is ChatMessage => message !== null);
}

function resolvePendingPermissions(messages: ChatMessage[]) {
  return messages.map((message) =>
    message.role === "permission" && message.status === "pending"
      ? { ...message, status: "complete" as const }
      : message,
  );
}

function findPendingPermission(messages: ChatMessage[]) {
  return (
    messages.find(
      (message) =>
        message.role === "permission" && message.status === "pending",
    ) ?? null
  );
}

export function getChatRunState(state: ChatState): ChatRunState {
  if (state.recoveryStatus === "recovering") return "recovering";
  if (state.recoveryStatus === "suspected") return "stalled";
  if (state.lifecycle === "disconnected") return "disconnected";
  if (state.lifecycle === "starting") return "starting";
  if (state.lifecycle === "stopping") return "stopping";
  if (state.pendingPermission || state.turnStatus === "awaiting-permission") {
    return "awaiting-permission";
  }
  if (
    state.turnStatus === "failed" ||
    state.lifecycle === "failed" ||
    state.lifecycle === "timed-out"
  ) {
    return "failed";
  }
  if (state.activeTool && state.turnStatus === "running") return "tool-running";
  if (state.turnStatus === "running") return "thinking";
  if (
    state.lifecycle === "exited" ||
    state.lifecycle === "interrupted" ||
    state.processReleased
  ) {
    return "ended";
  }
  return "idle";
}
