export interface TransitionFenceState {
  generation: number;
  activeGeneration: number | null;
}

export interface BegunTransition {
  generation: number;
  state: TransitionFenceState;
}

export function beginTransitionState(
  current: TransitionFenceState,
): BegunTransition {
  const generation = current.generation + 1;
  return {
    generation,
    state: {
      generation,
      activeGeneration: generation,
    },
  };
}

export function transitionGenerationMatches(
  current: TransitionFenceState,
  generation: number,
): boolean {
  return current.generation === generation;
}

export function transitionIsCurrent(
  current: TransitionFenceState,
  generation: number,
): boolean {
  return (
    current.activeGeneration === generation &&
    transitionGenerationMatches(current, generation)
  );
}

export function finishTransitionState(
  current: TransitionFenceState,
  generation: number,
): TransitionFenceState {
  if (!transitionIsCurrent(current, generation)) return current;
  return {
    ...current,
    activeGeneration: null,
  };
}

const ACTIVE_MODEL_SWITCH_STATES = new Set([
  "starting",
  "thinking",
  "tool-running",
  "awaiting-permission",
  "stopping",
  "stalled",
  "recovering",
]);

/** Model changes are unsafe while Claude is still producing the current turn. */
export function isModelSwitchBlocked(runState: string): boolean {
  return ACTIVE_MODEL_SWITCH_STATES.has(runState);
}

/** An already-running, completed conversation needs a clean resume with the new model. */
export function shouldRestartConversation(
  state: {
    sessionId: string | null;
    runId: string | null;
    processReleased: boolean;
    turnStatus: string;
    pendingPermission: unknown;
  },
  runState: string,
): boolean {
  return Boolean(
    state.sessionId &&
    state.runId &&
    !state.processReleased &&
    runState === "idle" &&
    state.turnStatus === "idle" &&
    !state.pendingPermission,
  );
}
