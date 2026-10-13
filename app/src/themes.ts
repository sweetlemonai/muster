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

// Stage colours (and the blocked flag's), the same in every theme: column strips, card edges,
// pills and timeline bars. Words on them use --on-status. Colours people know from boards (0034):
// grey backlog, blue shaping, cyan ready (well apart from Building: owner, 2026-10-13), Building the accent (violet unless the accent would
// pass for another stage, see buildingColour), amber testing, orange for the owner, green done,
// red blocked.
export const STAGE_COLOURS: Record<string, string> = {
  backlog: "#8E8AA6",
  shaping: "#64A2F2",
  ready: "#22C3D6",
  building: "#8B6CF0",
  testing: "#F7C948",
  "your-check": "#FB8C3C",
  done: "#4CC38A",
  blocked: "#F2556F",
};
const ON_STATUS = "#1A1730";
/** "Needs you" (Your check, escalations): the Your check orange, as styles.css's --st-needs. */
export const NEEDS_COLOUR = STAGE_COLOURS["your-check"];
/** The owner's own tasks (0022): an old gold for the "You" ribbon, in the needs-you family but
 *  never a stage colour (a brighter gold would pass for Testing's amber), as styles.css's --st-human. */
export const HUMAN_COLOUR = "#A08428";
/** How much of the gold a hatch stripe on the owner's own card takes, mixed into the card's
 *  surface (owner, 2026-10-13: one board, human cards patterned). Faint enough that every word on
 *  a card keeps its contrast in every theme, strong enough to see (tested in themes.test.ts). */
export const HUMAN_HATCH = 0.14;
// Priority colours, for cards coloured by priority: magenta for high (orange means "needs you" and
// red means blocked), violet for normal, grey for low.
// Priority has its own colours, never a stage's (owner, 2026-10-13: "a solid color code"):
// magenta for high, a light slate for normal, grey for low.
export const PRIORITY_COLOURS: Record<string, string> = { high: "#E879C9", normal: "#B4B9C8", low: "#8E8AA6" };

/** How a card shows its colour (0026). Picked in Appearance; applied as CSS only. */
export type CardLook = "stripe" | "tint" | "top" | "dot" | "outline";
export const CARD_LOOKS: { id: CardLook; name: string; hint: string }[] = [
  { id: "stripe", name: "Stripe", hint: "A bar down the left edge" },
  { id: "tint", name: "Tint", hint: "The whole card washed in the colour" },
  { id: "top", name: "Top bar", hint: "A band across the top that names it" },
  { id: "dot", name: "Dot", hint: "A quiet card with a dot by the id" },
  { id: "outline", name: "Outline", hint: "The border in the colour" },
];
/** What the card colour means: the stage it's in, or its priority. */
export type CardColour = "stage" | "priority";
/** How much of the colour a tinted card takes, per mode (tested for text contrast). */
export const CARD_TINT: Record<Mode, number> = { dark: 0.08, light: 0.12 };

/** The colour as an edge (stripe, border, dot) on the given backgrounds: itself when it stands
 *  out at 3:1 on every one, else darkened (light themes) or lightened (dark themes) until it does.
 *  Mixing toward black or white keeps the hue. */
export function edgeColour(c: string, backgrounds: string | string[], mode: Mode): string {
  const on = typeof backgrounds === "string" ? [backgrounds] : backgrounds;
  const toward = mode === "light" ? "#000000" : "#FFFFFF";
  for (let t = 1; t > 0; t -= 0.05) {
    const e = mix(c, toward, t);
    if (on.every((b) => contrast(e, b) >= 3)) return e;
  }
  return toward;
}

export function luminance(hex: string): number {
  const h = hex.replace("#", "");
  const c = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
/** CIE76 colour distance (in Lab): under about 20, two colours read as the same at a glance. */
export function colourDistance(a: string, b: string): number {
  const lab = (hex: string) => {
    const lin = [0, 2, 4]
      .map((i) => parseInt(hex.replace("#", "").slice(i, i + 2), 16) / 255)
      .map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
    const [x, y, z] = [
      (0.4124 * lin[0] + 0.3576 * lin[1] + 0.1805 * lin[2]) / 0.95047,
      0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2],
      (0.0193 * lin[0] + 0.1192 * lin[1] + 0.9505 * lin[2]) / 1.08883,
    ].map((v) => (v > 0.008856 ? Math.cbrt(v) : 7.787 * v + 16 / 116));
    return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
  };
  const [p, q] = [lab(a), lab(b)];
  return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
}
/** Below this distance, a Building accent would pass for another stage. */
export const SAME_AT_A_GLANCE = 30;

/** Building's colour for an accent: the accent's bright shade, unless it would pass for another
 *  stage, blocked or "needs you" (then violet, and `clashes` names what it looked like). */
export function buildingColour(accent: Accent): { colour: string; clashes: string } {
  const candidate = accent.dark[0];
  const others = Object.entries(STAGE_COLOURS).filter(([s]) => s !== "building");
  const near = others.find(([, c]) => colourDistance(candidate, c) < SAME_AT_A_GLANCE);
  return near ? { colour: STAGE_COLOURS.building, clashes: near[0] } : { colour: candidate, clashes: "" };
}

/** a mixed into b by t (0..1), as hex. */
export function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [0, 2, 4].map((i) => parseInt(h.replace("#", "").slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return "#" + x.map((v, i) => Math.round(v * t + y[i] * (1 - t)).toString(16).padStart(2, "0")).join("").toUpperCase();
}

export type Choice = { theme: string; accent: string; matchSystem: boolean; light: string; dark: string; cards: CardLook; cardColour: CardColour };
export const DEFAULT_CHOICE: Choice = { theme: "violet-night", accent: "violet", matchSystem: true, light: "violet-day", dark: "violet-night", cards: "stripe", cardColour: "stage" };

/** The card look and colour, with anything unknown (an old or hand-edited setting) falling back. */
export function cardChoice(choice: Partial<Choice>): { cards: CardLook; cardColour: CardColour } {
  const cards = CARD_LOOKS.some((l) => l.id === choice.cards) ? (choice.cards as CardLook) : DEFAULT_CHOICE.cards;
  const cardColour = choice.cardColour === "priority" ? "priority" : "stage";
  return { cards, cardColour };
}

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
  const edge = (c: string) => edgeColour(c, [theme.surface, mix(c, theme.surface, CARD_TINT[theme.mode])], theme.mode);
  const stages: Record<string, string> = { ...STAGE_COLOURS, building: buildingColour(accent).colour };
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
    ...Object.fromEntries(Object.entries(stages).map(([s, c]) => [`--st-${s}`, c])),
    "--on-status": ON_STATUS,
    // Card looks (0026): edges at 3:1 on the card, priority colours, and the tint amount.
    // An edge sits on the plain card and, in the Tint look, on the card washed in its own colour.
    ...Object.fromEntries(Object.entries(stages).map(([s, c]) => [`--edge-${s}`, edge(c)])),
    "--st-needs": NEEDS_COLOUR,
    "--edge-needs": edge(NEEDS_COLOUR),
    "--st-human": HUMAN_COLOUR,
    "--edge-human": edge(HUMAN_COLOUR),
    "--human-hatch": `${Math.round(HUMAN_HATCH * 100)}%`,
    ...Object.fromEntries(Object.entries(PRIORITY_COLOURS).map(([p, c]) => [`--pri-${p}`, c])),
    ...Object.fromEntries(Object.entries(PRIORITY_COLOURS).map(([p, c]) => [`--edge-pri-${p}`, edge(c)])),
    "--card-tint": `${Math.round(CARD_TINT[theme.mode] * 100)}%`,
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
  const { cards, cardColour } = cardChoice(c);
  root.dataset.cards = cards;
  root.dataset.cardColour = cardColour;
}
