import { describe, expect, it } from "vitest";
import {
  beginTransitionState,
  finishTransitionState,
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
