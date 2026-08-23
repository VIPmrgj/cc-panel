import type { SkillRecord } from "../../api/dto";

export type SkillPanelMode = "basic" | "advanced";

const STORAGE_KEY = "cc-panel.skill-panel-mode";
const DEFAULT_MARKERS = [
  "默认开启",
  "始终开启",
  "always enabled",
  "default enabled",
  "default on",
];

export function readSkillPanelMode(): SkillPanelMode {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "advanced"
      ? "advanced"
      : "basic";
  } catch {
    return "basic";
  }
}

export function persistSkillPanelMode(mode: SkillPanelMode): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // The UI still works when localStorage is unavailable.
  }
}

/**
 * Basic mode starts with only the Skills that explicitly describe themselves as
 * default-on, or that the user explicitly forced on in advanced mode.
 */
export function getBasicDefaultSkills(skills: SkillRecord[]): SkillRecord[] {
  return skills.filter((skill) => {
    if (skill.overrideState === "off" || !skill.manifestHash) return false;
    if (skill.overrideState === "on") return true;
    const description = skill.description.toLocaleLowerCase();
    return DEFAULT_MARKERS.some((marker) =>
      description.includes(marker.toLocaleLowerCase()),
    );
  });
}
