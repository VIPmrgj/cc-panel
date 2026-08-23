export type AppTheme = "light" | "night";

const THEME_KEY = "cc-panel.theme";

export function readTheme(): AppTheme {
  if (typeof window === "undefined") return "light";
  return window.localStorage.getItem(THEME_KEY) === "night" ? "night" : "light";
}

export function persistTheme(theme: AppTheme) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(THEME_KEY, theme);
}
