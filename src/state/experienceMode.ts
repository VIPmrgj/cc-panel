export type ExperienceMode = "guided" | "complete";

const EXPERIENCE_MODE_KEY = "cc-panel.experience-mode";
const ONBOARDING_COMPLETE_KEY = "cc-panel.onboarding-complete";

export function readExperienceMode(): ExperienceMode {
  if (typeof window === "undefined") return "guided";
  return window.localStorage.getItem(EXPERIENCE_MODE_KEY) === "complete"
    ? "complete"
    : "guided";
}

export function persistExperienceMode(mode: ExperienceMode) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(EXPERIENCE_MODE_KEY, mode);
}

export function readOnboardingComplete(): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(ONBOARDING_COMPLETE_KEY) === "true";
}

export function persistOnboardingComplete() {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(ONBOARDING_COMPLETE_KEY, "true");
}

export function clearOnboardingComplete() {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(ONBOARDING_COMPLETE_KEY);
}
