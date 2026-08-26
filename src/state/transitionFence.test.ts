import { describe, expect, it } from "vitest";
import {
  beginTransitionState,
  finishTransitionState,
  isModelSwitchBlocked,
  shouldRestartConversation,
  transitionGenerationMatches,
  transitionIsCurrent,
  type TransitionFenceState,
} from "./transitionFence";

const initial: TransitionFenceState = {
  generation: 0,
  activeGeneration: null,
};

describe("transition fence", () => {
  it("makes only the newest transition current", () => {
    const first = beginTransitionState(initial);
    const second = beginTransitionState(first.state);

    expect(transitionIsCurrent(second.state, first.generation)).toBe(false);
    expect(transitionIsCurrent(second.state, second.generation)).toBe(true);
  });

  it("does not let a stale completion clear a newer transition", () => {
    const first = beginTransitionState(initial);
    const second = beginTransitionState(first.state);

    const afterStaleFinish = finishTransitionState(
      second.state,
      first.generation,
    );
    expect(afterStaleFinish).toBe(second.state);
    expect(transitionIsCurrent(afterStaleFinish, second.generation)).toBe(true);
  });

  it("clears the active fence only for its matching completion", () => {
    const begun = beginTransitionState(initial);
    const finished = finishTransitionState(begun.state, begun.generation);

    expect(finished).toEqual({ generation: 1, activeGeneration: null });
    expect(transitionIsCurrent(finished, begun.generation)).toBe(false);
  });

  it("keeps a completed generation valid for stable-session sends", () => {
    const begun = beginTransitionState(initial);
    const finished = finishTransitionState(begun.state, begun.generation);

    expect(transitionGenerationMatches(finished, begun.generation)).toBe(true);
    expect(transitionIsCurrent(finished, begun.generation)).toBe(false);
  });
});

describe("model switch policy", () => {
  it("blocks switching while Claude is producing the current turn", () => {
    expect(isModelSwitchBlocked("thinking")).toBe(true);
    expect(isModelSwitchBlocked("tool-running")).toBe(true);
    expect(isModelSwitchBlocked("awaiting-permission")).toBe(true);
    expect(isModelSwitchBlocked("stalled")).toBe(true);
  });

  it("restarts a live conversation only after its turn is complete", () => {
    expect(
      shouldRestartConversation(
        {
          sessionId: "session-1",
          runId: "run-1",
          processReleased: false,
          turnStatus: "idle",
          pendingPermission: null,
        },
        "idle",
      ),
    ).toBe(true);
    expect(
      shouldRestartConversation(
        {
          sessionId: "session-1",
          runId: "run-1",
          processReleased: false,
          turnStatus: "running",
          pendingPermission: null,
        },
        "thinking",
      ),
    ).toBe(false);
  });
});
