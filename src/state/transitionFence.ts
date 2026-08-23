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
