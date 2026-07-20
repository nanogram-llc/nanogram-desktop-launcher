import type { Settings } from "./types";

export type AccentName = "blue" | "green" | "purple" | "amber" | "rose" | "custom";

/** Preset accents. `hover` is the darker shade used for button hover states. */
export const ACCENT_PRESETS: Record<
  Exclude<AccentName, "custom">,
  { label: string; color: string; hover: string }
> = {
  blue: { label: "ブルー", color: "#3b82f6", hover: "#2563eb" },
  green: { label: "グリーン", color: "#10b981", hover: "#059669" },
  purple: { label: "パープル", color: "#8b5cf6", hover: "#7c3aed" },
  amber: { label: "アンバー", color: "#f59e0b", hover: "#d97706" },
  rose: { label: "ローズ", color: "#f43f5e", hover: "#e11d48" },
};

/** Darken a hex colour by `amount` (0-1), used to derive a hover shade. */
function darken(hex: string, amount: number): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  const f = 1 - amount;
  const r = Math.round(((n >> 16) & 0xff) * f);
  const g = Math.round(((n >> 8) & 0xff) * f);
  const b = Math.round((n & 0xff) * f);
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, "0")}`;
}

function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

export function resolveAccent(settings: Pick<Settings, "accent_color" | "accent_custom">): {
  color: string;
  hover: string;
} {
  if (settings.accent_color === "custom") {
    const color = settings.accent_custom || "#3b82f6";
    return { color, hover: darken(color, 0.15) };
  }
  const preset = ACCENT_PRESETS[settings.accent_color as Exclude<AccentName, "custom">];
  return preset
    ? { color: preset.color, hover: preset.hover }
    : { color: ACCENT_PRESETS.blue.color, hover: ACCENT_PRESETS.blue.hover };
}

export function isDarkTheme(theme: string): boolean {
  return (
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches)
  );
}

/** Resolve the launcher text colours for the current settings. */
export function resolveTextColors(
  settings: Pick<Settings, "text_color" | "text_custom">
): { main: string; sub: string } | null {
  switch (settings.text_color) {
    case "light":
      return { main: "#f5f5f5", sub: "rgba(245, 245, 245, 0.65)" };
    case "dark":
      return { main: "#1a1a1a", sub: "rgba(26, 26, 26, 0.6)" };
    case "custom": {
      const main = settings.text_custom || "#ffffff";
      const rgb = hexToRgb(main);
      return {
        main,
        sub: rgb ? `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, 0.65)` : main,
      };
    }
    // "auto" — leave the light/dark theme colours alone.
    default:
      return null;
  }
}

/**
 * Apply light/dark class plus the accent, opacity and text CSS variables.
 * Called on load, on settings change, and when the OS theme flips.
 */
export function applyTheme(settings: Settings): void {
  const root = document.documentElement;

  root.classList.toggle("dark", isDarkTheme(settings.theme));

  const { color, hover } = resolveAccent(settings);
  const rgb = hexToRgb(color);
  root.style.setProperty("--accent", color);
  root.style.setProperty("--accent-hover", hover);
  root.style.setProperty(
    "--accent-soft",
    rgb ? `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, 0.2)` : "rgba(59, 130, 246, 0.2)"
  );

  // 100 = fully opaque (0.92 keeps a hint of the Mica/Acrylic backdrop, matching
  // the original look).
  //
  // The alpha must never reach 0. The window is created with `transparent: true`,
  // and a fully transparent surface stops registering mouse input — clicks and
  // even hover fall straight through to whatever is behind the launcher. A
  // background image is painted by a layer *above* this surface, so there is no
  // need to clear the base to show it.
  const opacity = Math.min(100, Math.max(50, settings.window_opacity ?? 100)) / 100;
  root.style.setProperty("--surface-alpha", String(opacity * 0.92));

  const text = resolveTextColors(settings);
  if (text) {
    root.style.setProperty("--text-main", text.main);
    root.style.setProperty("--text-sub", text.sub);
    root.classList.add("custom-text");
  } else {
    root.classList.remove("custom-text");
  }
}
