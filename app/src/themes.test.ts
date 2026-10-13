import { afterEach, describe, expect, it, vi } from "vitest";
import css from "./styles.css?raw";
import { ACCENTS, apply, buildingColour, CARD_LOOKS, colourDistance, SAME_AT_A_GLANCE, CARD_TINT, cardChoice, contrast, DEFAULT_CHOICE, edgeColour, mix, NEEDS_COLOUR, PRIORITY_COLOURS, resolve, STAGE_COLOURS, THEMES, variables } from "./themes";

describe("every theme and accent pair is readable (WCAG AA)", () => {
  for (const t of THEMES) {
    it(`${t.name}: text and secondary text`, () => {
      for (const surface of [t.bg, t.surface, t.popover]) {
        expect(contrast(t.text, surface)).toBeGreaterThanOrEqual(7);
        expect(contrast(t.muted, surface)).toBeGreaterThanOrEqual(4.5);
      }
    });
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: accent and text on it`, () => {
        const v = variables(t, a);
        // The accent is for fills, borders and selection: 3:1 against what's around it.
        expect(contrast(v["--accent"], t.bg)).toBeGreaterThanOrEqual(3);
        expect(contrast(v["--accent"], t.surface)).toBeGreaterThanOrEqual(3);
        // Accent-coloured words use --accent-text: 4.5:1 on every surface.
        for (const surface of [t.bg, t.surface, t.popover]) expect(contrast(v["--accent-text"], surface)).toBeGreaterThanOrEqual(4.5);
        // Button text on the accent.
        expect(contrast(v["--on-accent"], v["--accent"])).toBeGreaterThanOrEqual(4.5);
        // Text on the soft accent (selected chips).
        expect(contrast(t.text, v["--accent-soft"])).toBeGreaterThanOrEqual(4.5);
      });
    }
    it(`${t.name}: coloured status words`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const k of ["--err", "--ok", "--gold-text"]) expect(contrast(v[k], t.surface)).toBeGreaterThanOrEqual(4.5);
    });
    it(`${t.name}: words on stage colours (pills, badges)`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const s of Object.keys(STAGE_COLOURS)) expect(contrast(v["--on-status"], v[`--st-${s}`]), s).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("stage colours", () => {
  const hsl = (hex: string) => {
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(1 + i, 3 + i), 16) / 255);
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min, l = (max + min) / 2;
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
    const h = d === 0 ? 0 : max === r ? 60 * (((g - b) / d + 6) % 6) : max === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
    return { h, s };
  };
  it("cover every stage and blocked, each its own colour", () => {
    expect(Object.keys(STAGE_COLOURS)).toEqual(["backlog", "shaping", "ready", "building", "testing", "your-check", "done", "blocked"]);
    expect(new Set(Object.values(STAGE_COLOURS)).size).toBe(8);
  });
  it("are the colours people know: blue ready, amber testing, orange for you, green done, red blocked", () => {
    const hue = (s: string) => hsl(STAGE_COLOURS[s]).h;
    expect(hue("ready")).toBeGreaterThan(195);
    expect(hue("ready")).toBeLessThan(225);
    expect(hue("testing")).toBeGreaterThan(38);
    expect(hue("testing")).toBeLessThan(50);
    expect(hue("your-check")).toBeGreaterThan(18);
    expect(hue("your-check")).toBeLessThan(32);
    expect(hue("done")).toBeGreaterThan(130);
    expect(hue("done")).toBeLessThan(165);
    expect(hue("blocked") > 340 || hue("blocked") < 10).toBe(true);
    expect(hsl(STAGE_COLOURS.backlog).s).toBeLessThan(0.2);
    expect(NEEDS_COLOUR).toBe(STAGE_COLOURS["your-check"]);
  });
  it("no two read as the same at a glance", () => {
    const e = Object.entries(STAGE_COLOURS);
    for (let i = 0; i < e.length; i++)
      for (let j = i + 1; j < e.length; j++) expect(colourDistance(e[i][1], e[j][1]), `${e[i][0]} ~ ${e[j][0]}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
  });
  it("Building takes the accent, unless the accent would pass for another stage", () => {
    const by = Object.fromEntries(ACCENTS.map((a) => [a.id, buildingColour(a)]));
    expect(by.violet).toEqual({ colour: ACCENTS.find((a) => a.id === "violet")!.dark[0], clashes: "" });
    expect(by.rose.clashes).toBe("blocked");
    expect(by.gold.clashes).toBe("testing");
    expect(by.green.clashes).toBe("done");
    expect(by.graphite.clashes).toBe("backlog");
    for (const a of ACCENTS) {
      const b = by[a.id];
      // Whatever Building ends up as, it's distinct from every other stage.
      for (const [s, c] of Object.entries(STAGE_COLOURS)) if (s !== "building") expect(colourDistance(b.colour, c), `${a.id} vs ${s}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
      expect(b.clashes ? b.colour : a.dark[0]).toBe(b.colour);
      expect(variables(THEMES[0], a)["--st-building"]).toBe(b.colour);
    }
  });
});

describe("choices", () => {
  it("defaults to Muster's violet, following the Mac", () => {
    expect(resolve(DEFAULT_CHOICE, true).theme.id).toBe("violet-night");
    expect(resolve(DEFAULT_CHOICE, false).theme.id).toBe("violet-day");
    expect(resolve(DEFAULT_CHOICE, true).accent.id).toBe("violet");
  });
  it("a fixed theme ignores the Mac", () => {
    expect(resolve({ ...DEFAULT_CHOICE, matchSystem: false, theme: "paper" }, true).theme.id).toBe("paper");
  });
  it("unknown ids fall back", () => {
    const r = resolve({ ...DEFAULT_CHOICE, matchSystem: false, theme: "nope", accent: "nope" }, true);
    expect([r.theme.id, r.accent.id]).toEqual(["violet-night", "violet"]);
  });
  it("mix", () => {
    expect(mix("#FFFFFF", "#000000", 0.5)).toBe("#808080");
  });
});

describe("card looks (0026)", () => {
  // Every colour a card can take: each stage, blocked, needs you, and each priority.
  const fills: [string, string][] = [
    ...Object.entries(STAGE_COLOURS),
    ["needs", NEEDS_COLOUR],
    ...Object.entries(PRIORITY_COLOURS).map(([p, c]): [string, string] => [`pri-${p}`, c]),
  ];
  for (const t of THEMES) {
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: edges stand out at 3:1 on the card (stripe, outline, dot)`, () => {
        const v = variables(t, a);
        for (const [k] of fills) {
          const edge = v[`--edge-${k}`];
          expect(edge, k).toMatch(/^#[0-9A-F]{6}$/);
          expect(contrast(edge, t.surface), `${k} on ${t.name}`).toBeGreaterThanOrEqual(3);
        }
      });
    }
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: every word a card shows reads on every tinted card (AA)`, () => {
        const v = variables(t, a);
        for (const [k, c] of fills) {
          const tinted = mix(c, t.surface, CARD_TINT[t.mode]);
          // Title and meta (text, muted), the project name (.q, accent-text), a bug's kind (.kind.bug, err).
          for (const w of ["--text", "--muted", "--accent-text", "--err"]) expect(contrast(v[w], tinted), `${w} on ${k}`).toBeGreaterThanOrEqual(4.5);
          // The Tint look's border is the edge colour: 3:1 on the tinted card too.
          expect(contrast(v[`--edge-${k}`], tinted), `edge on tinted ${k}`).toBeGreaterThanOrEqual(3);
        }
      });
    }
    it(`${t.name}: the tint amount and "needs you" colour the CSS uses are the tested ones`, () => {
      const v = variables(t, ACCENTS[0]);
      expect(v["--card-tint"]).toBe(`${Math.round(CARD_TINT[t.mode] * 100)}%`);
      expect(v["--st-needs"]).toBe(NEEDS_COLOUR);
    });
  }
  it("words on the Top bar band read on every colour it can take", () => {
    for (const [k, c] of fills) expect(contrast("#1A1730", c), k).toBeGreaterThanOrEqual(4.5);
  });
  it("priority colours are distinct, and apart from blocked and needs you", () => {
    const all = Object.values(PRIORITY_COLOURS);
    expect(new Set(all).size).toBe(3);
    expect(all).not.toContain(STAGE_COLOURS.blocked);
    expect(all).not.toContain(NEEDS_COLOUR);
  });
  it("an edge keeps a colour that already stands out, and keeps its hue when it has to change", () => {
    expect(edgeColour("#8B6CF0", "#1C1930", "dark")).toBe("#8B6CF0");
    const gold = edgeColour("#F5C04A", "#FFFFFF", "light");
    expect(gold).not.toBe("#F5C04A");
    expect(contrast(gold, "#FFFFFF")).toBeGreaterThanOrEqual(3);
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(gold.slice(i, i + 2), 16));
    expect(r).toBeGreaterThan(g);
    expect(g).toBeGreaterThan(b);
  });
  it("five looks, stripe by default, coloured by stage", () => {
    expect(CARD_LOOKS.map((l) => l.id)).toEqual(["stripe", "tint", "top", "dot", "outline"]);
    expect([DEFAULT_CHOICE.cards, DEFAULT_CHOICE.cardColour]).toEqual(["stripe", "stage"]);
  });
  it("an old or hand-edited setting falls back to the defaults", () => {
    expect(cardChoice({})).toEqual({ cards: "stripe", cardColour: "stage" });
    expect(cardChoice({ cards: "neon" as never, cardColour: "mood" as never })).toEqual({ cards: "stripe", cardColour: "stage" });
    expect(cardChoice({ cards: "dot", cardColour: "priority" })).toEqual({ cards: "dot", cardColour: "priority" });
  });
});

describe("the card CSS uses what the tests check (0026)", () => {
  const rule = (selector: string) => {
    const i = css.indexOf(`${selector} {`);
    expect(i, selector).toBeGreaterThanOrEqual(0);
    return { at: i, body: css.slice(i, css.indexOf("}", i)) };
  };
  it("the Tint look washes the card in its fill by --card-tint, over the surface (as mix() in the tests)", () => {
    expect(css).toContain("--card-bg: color-mix(in srgb, var(--card-fill) var(--card-tint, 8%), var(--surface));");
  });
  it("the Tint look's border is the edge colour", () => {
    expect(css).toContain("--card-tint-border: var(--card-c);");
  });
  it("the band is the fill with the status word colour", () => {
    expect(rule(".card-band").body).toContain("background: var(--card-fill); color: var(--on-status);");
  });
  it("the page's own needs-you colour before apply() matches", () => {
    expect(css).toContain(`--st-needs: ${NEEDS_COLOUR};`);
  });
  it("blocked takes over from needs you, and both from the stage or priority colour", () => {
    const needs = rule(".card.needs-you");
    const blocked = rule(".card.blocked");
    expect(blocked.at).toBeGreaterThan(needs.at);
    expect(blocked.body).toContain("--card-fill: var(--st-blocked); --card-c: var(--edge-blocked");
    expect(needs.body).toContain("--card-fill: var(--st-needs); --card-c: var(--edge-needs");
    expect(rule(".card").body).toBeTruthy();
    expect(css).toContain(".card { --card-fill: var(--base-fill); --card-c: var(--base-edge); }");
  });
  it("each look only sets variables, never the stage colour directly", () => {
    for (const l of ["tint", "top", "dot", "outline"]) {
      const body = rule(`:root[data-cards="${l}"] .board .card, .look-${l} .card`).body;
      expect(body, l).not.toContain("--status");
      for (const decl of body.slice(body.indexOf("{") + 1).split(";").map((d: string) => d.trim()).filter(Boolean)) expect(decl.startsWith("--"), `${l}: ${decl}`).toBe(true);
    }
  });
});

describe("the task pane's CSS (0021) follows the theme", () => {
  const start = css.indexOf("/* Task pane (0021");
  const block = css.slice(start);
  it("is there", () => {
    expect(start).toBeGreaterThan(0);
    expect(block).toContain(".stage-track");
  });
  it("names no colour of its own: every colour is a theme variable every theme sets (or the stage's)", () => {
    expect(block).not.toMatch(/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i);
    const themed = new Set([...Object.keys(variables(THEMES[0], ACCENTS[0])), "--status"]);
    const used = new Set([...block.matchAll(/var\((--[\w-]+)/g)].map((m) => m[1]));
    expect(used.size).toBeGreaterThan(5);
    for (const v of used) expect(themed.has(v), v).toBe(true);
  });
  it("its gold \"waiting on you\" words, live badge and track reads are the tested pairs", () => {
    expect(block).toContain(".task-pane .needs { color: var(--gold-text);");
    expect(block).toContain(".live-badge { font-size: 9.5px; font-weight: 800; letter-spacing: 0.6px; color: var(--on-status); background: var(--st-building);");
    // Words on the track and the sidebar's labels use text or muted, never faint (placeholders only).
    expect(block).not.toContain("var(--faint)");
  });
});

describe("apply() puts the card look on the page (0026)", () => {
  afterEach(() => vi.unstubAllGlobals());
  const page = () => {
    const root = { style: { setProperty: () => {}, colorScheme: "" }, dataset: {} as Record<string, string> };
    vi.stubGlobal("document", { documentElement: root });
    vi.stubGlobal("window", { matchMedia: () => ({ matches: true }) });
    return root;
  };
  it("as the attributes the board CSS selects on", () => {
    const root = page();
    apply({ ...DEFAULT_CHOICE, cards: "tint", cardColour: "priority" });
    expect(root.dataset).toEqual({ cards: "tint", cardColour: "priority" });
  });
  it("falls back for unknown values", () => {
    const root = page();
    apply({ ...DEFAULT_CHOICE, cards: "neon" as never, cardColour: "mood" as never });
    expect(root.dataset).toEqual({ cards: "stripe", cardColour: "stage" });
  });
});
