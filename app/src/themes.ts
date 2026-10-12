// Themes (backgrounds, surfaces, borders, text) and accents (the accent colour and its shades).
// Blue and orange stay out: they belong to 127 and to Drover and Braindump. themes.test.ts checks
// every theme and accent pair for contrast (WCAG AA).

export type Mode = "dark" | "light";
export type Theme = {
  id: string;
  name: string;
  mode: Mode;
  bg: string;
  surface: string;
  popover: string;
  border: string;
  text: string;
  muted: string;
  faint: string;
};
export type Accent = { id: string; name: string; dark: [string, string]; light: [string, string] };

export const THEMES: Theme[] = [
  { id: "violet-night", name: "Violet night", mode: "dark", bg: "#14121F", surface: "#1C1930", popover: "#262241", border: "#34304F", text: "#ECEAF5", muted: "#A8A3C2", faint: "#6F6A8A" },
  { id: "violet-day", name: "Violet day", mode: "light", bg: "#FAF9FD", surface: "#FFFFFF", popover: "#F3F1FA", border: "#E4E1F0", text: "#1A1730", muted: "#5E5978", faint: "#8E8AA6" },
  { id: "graphite", name: "Graphite", mode: "dark", bg: "#141517", surface: "#1C1D20", popover: "#26272B", border: "#34363B", text: "#ECECEE", muted: "#A6A8AE", faint: "#6E7076" },
  { id: "midnight", name: "Midnight", mode: "dark", bg: "#0B0B10", surface: "#131319", popover: "#1C1C24", border: "#2A2A35", text: "#F1F0F6", muted: "#A9A7B8", faint: "#6C6A7C" },
  { id: "paper", name: "Paper", mode: "light", bg: "#FBF8F1", surface: "#FFFDF8", popover: "#F4EFE4", border: "#E6DFD0", text: "#211D16", muted: "#5F5848", faint: "#8F8775" },
  { id: "snow", name: "Snow", mode: "light", bg: "#F7F7F8", surface: "#FFFFFF", popover: "#F0F0F2", border: "#E2E2E6", text: "#18181B", muted: "#55555C", faint: "#8A8A92" },
  { id: "contrast", name: "High contrast", mode: "dark", bg: "#000000", surface: "#0C0C0C", popover: "#161616", border: "#5A5A5A", text: "#FFFFFF", muted: "#D2D2D2", faint: "#9A9A9A" },
];

// [accent, strong] per mode. Light-mode values are darker so they read on light backgrounds.
export const ACCENTS: Accent[] = [
  { id: "violet", name: "Violet", dark: ["#8B6CF0", "#A08AF5"], light: ["#6D4AE0", "#5A38C9"] },
  { id: "teal", name: "Teal", dark: ["#2EC4B6", "#5AD6CB"], light: ["#0E7C73", "#0A6159"] },
  { id: "rose", name: "Rose", dark: ["#F0647D", "#F48A9C"], light: ["#C2334F", "#A1253F"] },
  { id: "gold", name: "Gold", dark: ["#F5C04A", "#F8D27A"], light: ["#8A6A00", "#6E5400"] },
  { id: "green", name: "Green", dark: ["#4CC38A", "#74D3A6"], light: ["#177A4B", "#11603B"] },
  { id: "graphite", name: "Graphite", dark: ["#B4B2C2", "#CFCDDA"], light: ["#4F4C5E", "#3A3846"] },
];

// Status colours: fills in both modes; in light themes coloured words use the darker *-text shades.
const STATUS_TEXT: Record<Mode, { err: string; ok: string; gold: string }> = {
  dark: { err: "#F0647D", ok: "#4CC38A", gold: "#F5C04A" },
  light: { err: "#C2334F", ok: "#177A4B", gold: "#8A6A00" },
};

export function luminance(hex: string): number {
  const h = hex.replace("#", "");
  const c = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
/** a mixed into b by t (0..1), as hex. */
export function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [0, 2, 4].map((i) => parseInt(h.replace("#", "").slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return "#" + x.map((v, i) => Math.round(v * t + y[i] * (1 - t)).toString(16).padStart(2, "0")).join("").toUpperCase();
}

export type Choice = { theme: string; accent: string; matchSystem: boolean; light: string; dark: string };
export const DEFAULT_CHOICE: Choice = { theme: "violet-night", accent: "violet", matchSystem: true, light: "violet-day", dark: "violet-night" };

export function resolve(choice: Choice, systemDark: boolean): { theme: Theme; accent: Accent } {
  const id = choice.matchSystem ? (systemDark ? choice.dark : choice.light) : choice.theme;
  const theme = THEMES.find((t) => t.id === id) ?? THEMES[0];
  const accent = ACCENTS.find((a) => a.id === choice.accent) ?? ACCENTS[0];
  return { theme, accent };
}

/** CSS variables for a theme and accent. Text on the accent is whichever of the theme's text or
 *  background reads better on it. */
export function variables(theme: Theme, accent: Accent): Record<string, string> {
  const [acc, strong] = accent[theme.mode];
  const onAccent = contrast(theme.text, acc) >= contrast(theme.bg, acc) ? theme.text : theme.bg;
  const st = STATUS_TEXT[theme.mode];
  return {
    "--bg": theme.bg,
    "--surface": theme.surface,
    "--popover": theme.popover,
    "--tint": theme.popover,
    "--border": theme.border,
    "--text": theme.text,
    "--muted": theme.muted,
    "--faint": theme.faint,
    "--accent": acc,
    "--accent-strong": strong,
    // Accent-coloured words (links, project names) use the strong shade: it reads at 4.5:1 on
    // every surface, where the accent itself is for fills, borders and selection (3:1).
    "--accent-text": strong,
    "--accent-soft": mix(acc, theme.bg, theme.mode === "dark" ? 0.2 : 0.12),
    "--on-accent": onAccent,
    "--err": st.err,
    "--ok": st.ok,
    "--gold-text": st.gold,
  };
}

const KEY = "muster.appearance";
export function load(): Choice {
  try {
    return { ...DEFAULT_CHOICE, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return DEFAULT_CHOICE;
  }
}
export function save(c: Choice) {
  localStorage.setItem(KEY, JSON.stringify(c));
}
export function apply(c: Choice) {
  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const { theme, accent } = resolve(c, dark);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(variables(theme, accent))) root.style.setProperty(k, v);
  root.style.colorScheme = theme.mode;
}
