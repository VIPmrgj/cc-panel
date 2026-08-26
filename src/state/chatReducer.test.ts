import { describe, expect, it } from "vitest";
import type { ClaudeRunEnvelope } from "../api/dto";
import { chatReducer, initialChatState } from "./chatReducer";

function envelope(
  sequence: number,
  event: ClaudeRunEnvelope["event"],
  runId = "run-1",
): ClaudeRunEnvelope {
  return { sessionId: "session-1", runId, sequence, event };
}

describe("chatReducer", () => {
  it("scopes turn actions and keeps the active turn until result", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "turn-started",
      sessionId: "session-1",
      runId: "run-1",
      turnId: "turn-1",
      message: {
        id: "user-turn-1",
        role: "user",
        content: "draft",
        status: "running",
      },
    });
    const stale = chatReducer(state, {
      type: "turn-message-committed",
      sessionId: "session-old",
      runId: "run-old",
      turnId: "turn-1",
      message: { id: "user-turn-1", role: "user", content: "stale" },
    });
    expect(stale).toBe(state);

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "result",
        success: true,
        isError: false,
      }),
    });
    expect(state.turnStatus).toBe("idle");
    expect(state.activeTurnId).toBeNull();

    state = chatReducer(state, {
      type: "turn-message-committed",
      sessionId: "session-1",
      runId: "run-1",
      turnId: "turn-1",
      message: {
        id: "user-turn-1",
        role: "user",
        content: "committed after result",
      },
    });
    expect(state.messages[0]).toMatchObject({
      content: "committed after result",
      status: "complete",
    });
  });

  it("gives each null-id stream turn a distinct provisional assistant", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        deltaType: "text",
        delta: "first",
      }),
    });
    const firstId = state.activeAssistantId;
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "result",
        success: true,
        isError: false,
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "stream",
        deltaType: "text",
        delta: "second",
      }),
    });
    expect(firstId).not.toBe(state.activeAssistantId);
    expect(state.messages.map((message) => message.content)).toEqual([
      "first",
      "second",
    ]);
  });

  it("accepts ordered stream deltas and rejects duplicates and stale runs", () => {
    const initial = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    const first = chatReducer(initial, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        messageId: "m-1",
        deltaType: "text",
        delta: "Hello",
      }),
    });
    const second = chatReducer(first, {
      type: "envelope",
      envelope: envelope(2, {
        type: "stream",
        messageId: "m-1",
        deltaType: "text",
        delta: " world",
      }),
    });
    expect(second.messages[0]).toMatchObject({
      id: "m-1",
      content: "Hello world",
      status: "running",
    });

    const duplicate = chatReducer(second, {
      type: "envelope",
      envelope: envelope(2, {
        type: "stream",
        messageId: "m-1",
        deltaType: "text",
        delta: "!",
      }),
    });
    const staleRun = chatReducer(second, {
      type: "envelope",
      envelope: envelope(
        3,
        { type: "stream", messageId: "m-1", deltaType: "text", delta: "!" },
        "run-old",
      ),
    });
    expect(duplicate).toBe(second);
    expect(staleRun).toBe(second);
  });

  it("collects thinking separately and replaces the streamed assistant with final blocks", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        messageId: "m-1",
        deltaType: "thinking",
        delta: "Check",
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "stream",
        messageId: "m-1",
        deltaType: "thinking",
        delta: " files",
      }),
    });
    expect(state.messages[0].blocks).toEqual([
      { type: "thinking", thinking: "Check files" },
    ]);

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "assistant",
        messageId: "m-1",
        blocks: [
          { type: "text", text: "Done" },
          { type: "thinking", thinking: "Checked files" },
        ],
      }),
    });
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toMatchObject({
      content: "Done",
      status: "complete",
    });
  });

  it("removes stale thinking-only bubbles when a new phase starts", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        messageId: "thinking-1",
        deltaType: "thinking",
        delta: "先检查文件",
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "stream",
        messageId: "thinking-2",
        deltaType: "thinking",
        delta: "再执行操作",
      }),
    });
    expect(state.messages.map((message) => message.id)).toEqual(["thinking-2"]);

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "stream",
        messageId: "answer-1",
        deltaType: "text",
        delta: "完成",
      }),
    });
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toMatchObject({
      id: "answer-1",
      content: "完成",
    });
    expect(state.messages[0].blocks ?? []).not.toContainEqual(
      expect.objectContaining({ type: "thinking" }),
    );
  });
  it("does not reset accepted events when session startup resolves", () => {
    let streamed = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    streamed = chatReducer(streamed, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        messageId: "m-1",
        deltaType: "text",
        delta: "early",
      }),
    });
    const started = chatReducer(streamed, {
      type: "session-started",
      sessionId: "session-1",
      runId: "run-1",
    });
    expect(started).toBe(streamed);
    expect(started.lastSequence).toBe(1);
  });

  it("hands an active session to a new generation before a fork starts", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      generation: 3,
      sessionId: "session-parent",
      runId: "run-parent",
    });
    state = chatReducer(state, {
      type: "session-leaving",
      generation: 4,
      sessionId: "session-parent",
      runId: "run-parent",
      reason: "switching-model",
    });
    expect(state).toMatchObject({
      viewGeneration: 4,
      sessionId: "session-parent",
      runId: "run-parent",
      lifecycle: "stopping",
      terminationReason: "switching-model",
    });

    state = chatReducer(state, {
      type: "session-started",
      generation: 4,
      sessionId: "session-fork",
      runId: "run-fork",
    });
    expect(state).toMatchObject({
      viewGeneration: 4,
      sessionId: "session-fork",
      runId: "run-fork",
      lifecycle: "starting",
      processReleased: false,
    });

    state = chatReducer(state, {
      type: "envelope",
      envelope: {
        sessionId: "session-fork",
        runId: "run-fork",
        sequence: 1,
        event: {
          type: "stream",
          messageId: "fork-message",
          deltaType: "text",
          delta: "visible fork",
        },
      },
    });
    expect(state.messages.at(-1)).toMatchObject({
      id: "fork-message",
      content: "visible fork",
    });
  });

  it("keeps additional permission requests queued until each response resolves", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "permission",
        requestId: "request-1",
        toolUseId: "tool-1",
        expiresAt: Date.now() + 120_000,
        toolName: "Bash",
        input: { command: "first" },
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "permission",
        requestId: "request-2",
        toolUseId: "tool-2",
        expiresAt: Date.now() + 120_000,
        toolName: "Bash",
        input: { command: "second" },
      }),
    });
    expect(state.pendingPermission?.requestId).toBe("request-1");
    expect(
      state.messages
        .filter((message) => message.role === "permission")
        .every((message) => (message.permissionExpiresAt ?? 0) > Date.now()),
    ).toBe(true);

    state = chatReducer(state, {
      type: "permission-response",
      sessionId: "session-1",
      runId: "run-1",
      requestId: "request-2",
      behavior: "allow",
    });
    expect(state.pendingPermission?.requestId).toBe("request-1");
    expect(state.turnStatus).toBe("awaiting-permission");

    state = chatReducer(state, {
      type: "permission-response",
      sessionId: "session-1",
      runId: "run-1",
      requestId: "request-1",
      behavior: "deny",
    });
    expect(state.pendingPermission).toBeNull();
    expect(state.turnStatus).toBe("running");
  });
  it("keeps queued permissions pending during non-terminal lifecycle updates", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "permission",
        requestId: "request-1",
        toolName: "Bash",
        input: { command: "first" },
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "permission",
        requestId: "request-2",
        toolName: "Bash",
        input: { command: "second" },
      }),
    });

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, { type: "lifecycle", status: "running" }),
    });

    expect(state.pendingPermission?.requestId).toBe("request-1");
    expect(
      state.messages
        .filter((message) => message.role === "permission")
        .map((message) => message.status),
    ).toEqual(["pending", "pending"]);
  });

  it("tracks permission requests, responses in history, lifecycle, and errors", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "permission",
        requestId: "request-1",
        toolUseId: "tool-1",
        expiresAt: Date.now() + 120_000,
        toolName: "Bash",
        input: { command: "npm test" },
      }),
    });
    expect(state.pendingPermission).toMatchObject({
      requestId: "request-1",
      toolName: "Bash",
    });

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "lifecycle",
        status: "awaiting-permission",
      }),
    });
    expect(state.lifecycle).toBe("awaiting-permission");

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "error",
        code: "PROCESS_EXITED",
        message: "会话已退出",
        retryable: true,
      }),
    });
    expect(state.lifecycle).toBe("awaiting-permission");
    expect(state.turnStatus).toBe("failed");
    expect(state.processReleased).toBe(false);
    expect(state.pendingPermission).toBeNull();
    expect(state.messages.at(-1)).toMatchObject({
      role: "error",
      content: "会话已退出",
    });
  });

  it("finalizes running assistant bubbles when the turn ends", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "stream",
        deltaType: "text",
        delta: "reply",
      }),
    });
    expect(state.messages.at(-1)?.status).toBe("running");
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, { type: "result", success: true, isError: false }),
    });
    const finished = state.messages.find(
      (message) => message.content === "reply",
    );
    expect(finished?.status).toBe("complete");
    expect(
      state.messages.some(
        (message) =>
          message.role === "assistant" && message.status === "running",
      ),
    ).toBe(false);
  });

  it("drops empty streaming placeholders when the turn ends", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, { type: "stream", deltaType: "text", delta: "" }),
    });
    expect(state.messages.some((message) => message.role === "assistant")).toBe(
      true,
    );
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, { type: "result", success: true, isError: false }),
    });
    expect(state.messages.some((message) => message.role === "assistant")).toBe(
      false,
    );
  });

  it("tracks the live tool from tool_progress events", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Bash",
        state: "in_progress",
      }),
    });
    expect(state.activeTool).toMatchObject({
      toolUseId: "tool-1",
      toolName: "Bash",
      state: "in_progress",
    });
    expect(state.messages).toHaveLength(0);
  });

  it("accumulates streamed tool text for the same tool_use_id", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Read",
        state: "in_progress",
        text: "line one\n",
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Read",
        state: "in_progress",
        text: "line two\n",
      }),
    });
    expect(state.activeTool?.text).toBe("line one\nline two\n");
  });

  it("replaces activeTool when a different tool starts and clears on tool-result", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Bash",
        state: "in_progress",
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "tool-progress",
        toolUseId: "tool-2",
        toolName: "Glob",
        state: "in_progress",
      }),
    });
    expect(state.activeTool?.toolUseId).toBe("tool-2");

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "tool-result",
        toolUseId: "tool-2",
        content: "found",
        isError: false,
      }),
    });
    expect(state.activeTool).toBeNull();
    // The result also lands as a block, but not a new top-level message.
    expect(state.messages[0]?.blocks?.at(-1)).toMatchObject({
      type: "tool-result",
      content: "found",
    });
  });

  it("keeps loaded history when a lazily-resumed session starts", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      generation: 5,
      sessionId: "session-1",
    });
    state = chatReducer(state, {
      type: "history",
      generation: 5,
      sessionId: "session-1",
      messages: [{ id: "m-1", role: "assistant", content: "old reply" }],
    });
    state = chatReducer(state, {
      type: "session-loading",
      generation: 6,
      sessionId: "session-1",
    });
    expect(state).toMatchObject({
      viewGeneration: 6,
      sessionId: "session-1",
      lifecycle: "starting",
      processReleased: false,
    });
    expect(state.messages).toEqual([
      { id: "m-1", role: "assistant", content: "old reply" },
    ]);

    state = chatReducer(state, {
      type: "session-started",
      generation: 6,
      sessionId: "session-1",
      runId: "run-1",
    });
    expect(state.messages).toEqual([
      { id: "m-1", role: "assistant", content: "old reply" },
    ]);
    expect(state.runId).toBe("run-1");
  });

  it("caps accumulated tool text and clears activeTool when the turn ends", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Bash",
        state: "in_progress",
        text: "x".repeat(1800),
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "tool-progress",
        toolUseId: "tool-1",
        toolName: "Bash",
        state: "in_progress",
        text: "y".repeat(1800),
      }),
    });
    // Accumulated stream is kept as a prefix, bounded to the cap.
    expect(state.activeTool?.text?.length).toBe(2048);
    expect(state.activeTool?.text?.slice(0, 10)).toBe("x".repeat(10));
    expect(state.activeTool?.text?.slice(-1)).toBe("y");

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, { type: "result", success: true, isError: false }),
    });
    expect(state.activeTool).toBeNull();
  });
  it("associates a permission expiry error with its request", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    for (const [sequence, requestId] of [
      [1, "request-1"],
      [2, "request-2"],
    ] as const) {
      state = chatReducer(state, {
        type: "envelope",
        envelope: envelope(sequence, {
          type: "permission",
          requestId,
          toolName: "Bash",
          input: { command: requestId },
          expiresAt: Date.now() + 120_000,
        }),
      });
    }
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "error",
        code: "PERMISSION_EXPIRED",
        message: "权限确认已失效",
        retryable: true,
        requestId: "request-2",
      }),
    });

    expect(
      state.messages.find((message) => message.requestId === "request-1")
        ?.permissionExpiresAt,
    ).toBeGreaterThan(Date.now());
    expect(
      state.messages.find((message) => message.requestId === "request-2")
        ?.permissionExpiresAt,
    ).toBe(0);
    expect(state.pendingPermission?.requestId).toBe("request-1");
  });
  it("keeps an expired permission actionable until it is retried", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "permission",
        requestId: "request-expired",
        toolName: "Bash",
        input: { command: "npm test", cwd: "E:\\workspace" },
        expiresAt: Date.now() - 1,
      }),
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "error",
        code: "PERMISSION_EXPIRED",
        message: "权限确认已失效",
        retryable: true,
      }),
    });
    expect(state.turnStatus).toBe("awaiting-permission");
    expect(state.pendingPermission?.permissionExpiresAt).toBe(0);

    state = chatReducer(state, {
      type: "permission-retried",
      sessionId: "session-1",
      runId: "run-1",
      requestId: "request-expired",
      expiresAt: Date.now() + 120_000,
    });
    expect(state.pendingPermission?.permissionExpiresAt).toBeGreaterThan(
      Date.now(),
    );
  });
  it("treats deny-and-interrupt as recoverable instead of an error", () => {
    let state = chatReducer(initialChatState, {
      type: "reset",
      sessionId: "session-1",
      runId: "run-1",
    });
    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(1, {
        type: "permission",
        requestId: "request-interrupt",
        toolName: "Bash",
        input: { command: "rm -rf build" },
      }),
    });
    state = chatReducer(state, {
      type: "permission-response",
      sessionId: "session-1",
      runId: "run-1",
      requestId: "request-interrupt",
      behavior: "deny",
      interrupted: true,
    });
    expect(state.turnStatus).toBe("idle");
    expect(state.interruptionRequested).toBe(true);
    expect(state.terminationReason).toBe("interrupted");

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(2, {
        type: "lifecycle",
        status: "interrupted",
        message: "process stopped",
      }),
    });
    expect(state.lifecycle).toBe("interrupted");
    expect(state.turnStatus).toBe("idle");
    expect(state.statusMessage).toContain("Claude 会话已中断");

    state = chatReducer(state, {
      type: "envelope",
      envelope: envelope(3, {
        type: "error",
        code: "PROCESS_EXITED",
        message: "child exited",
        retryable: true,
      }),
    });
    expect(state.turnStatus).toBe("idle");
    expect(state.messages.some((message) => message.role === "error")).toBe(
      false,
    );

    state = chatReducer(state, {
      type: "session-loading",
      generation: 1,
      sessionId: "session-1",
    });
    expect(state.interruptionRequested).toBe(false);
  });
});
