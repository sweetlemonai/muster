import { afterEach, describe, expect, it, vi } from "vitest";
import arrangeRs from "../src-tauri/src/arrange.rs?raw";
import css from "./styles.css?raw";
import { ACCENTS, apply, buildingColour, OLD_TOKEN_NAMES, PROJECT_COLOURS, CARD_LOOKS, colourDistance, SAME_AT_A_GLANCE, CARD_TINT, cardChoice, contrast, DEFAULT_CHOICE, edgeColour, HUMAN_COLOUR, HUMAN_HATCH, mix, NEEDS_COLOUR, PRIORITY_COLOURS, resolve, STAGE_COLOURS, THEMES, variables } from "./themes";

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
        expect(contrast(v["--accent-ink"], v["--accent"])).toBeGreaterThanOrEqual(4.5);
        // Text on the soft accent (selected chips).
        expect(contrast(t.text, v["--selected"])).toBeGreaterThanOrEqual(4.5);
      });
    }
    it(`${t.name}: coloured status words`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const k of ["--bad", "--ok", "--gold-text", "--warn"]) expect(contrast(v[k], t.surface), k).toBeGreaterThanOrEqual(4.5);
      // Needs-you words (the Your check limit, 0050) sit on the board, the sidebar and Settings too.
      for (const surface of [t.bg, t.popover]) expect(contrast(v["--warn"], surface)).toBeGreaterThanOrEqual(4.5);
      // Still the needs-you orange: the same hue family, the colour itself where it reads.
      if (t.mode === "dark") expect(v["--warn"]).toBe(NEEDS_COLOUR);
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
    // Ready is cyan: far from Building's violet (owner, 2026-10-13: "ready and building colors are too close").
    expect(hue("ready")).toBeGreaterThan(180);
    expect(hue("ready")).toBeLessThan(195);
    expect(colourDistance(STAGE_COLOURS.ready, STAGE_COLOURS.building)).toBeGreaterThanOrEqual(60);
    for (const a of ACCENTS) expect(colourDistance(STAGE_COLOURS.ready, buildingColour(a).colour), a.id).toBeGreaterThanOrEqual(60);
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

describe("the owner's own tasks' gold (0022)", () => {
  const hsl = (hex: string) => {
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(1 + i, 3 + i), 16) / 255);
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
    return { h: d === 0 ? 0 : max === r ? 60 * (((g - b) / d + 6) % 6) : max === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4), s: d / (1 - Math.abs(max + min - 1)) };
  };
  it("is a gold", () => {
    const { h, s } = hsl(HUMAN_COLOUR);
    expect(h).toBeGreaterThan(38);
    expect(h).toBeLessThan(56);
    expect(s).toBeGreaterThan(0.5);
  });
  it("is never a stage colour: it reads apart from every stage, blocked and needs you at a glance", () => {
    for (const [s, c] of [...Object.entries(STAGE_COLOURS), ["needs", NEEDS_COLOUR]]) expect(colourDistance(HUMAN_COLOUR, c), s).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
  });
  it("is a hue apart from blocked red and needs-you orange, as well as at a glance", () => {
    const apart = (a: string, b: string) => {
      const d = Math.abs(hsl(a).h - hsl(b).h) % 360;
      return Math.min(d, 360 - d);
    };
    expect(apart(HUMAN_COLOUR, STAGE_COLOURS.blocked)).toBeGreaterThanOrEqual(40);
    expect(apart(HUMAN_COLOUR, NEEDS_COLOUR)).toBeGreaterThanOrEqual(20);
  });
  for (const t of THEMES) {
    it(`${t.name}: the You ribbon's word reads on it, and it stands out on the card`, () => {
      const v = variables(t, ACCENTS[0]);
      expect(v["--st-human"]).toBe(HUMAN_COLOUR);
      expect(contrast(v["--on-status"], v["--st-human"])).toBeGreaterThanOrEqual(4.5);
      // Its border is the edge shade: 3:1 on the plain card and on a tinted one.
      expect(contrast(v["--edge-human"], t.surface)).toBeGreaterThanOrEqual(3);
    });
  }
});

describe("the owner's own task's hatch (0022, owner 2026-10-13: one board, human cards patterned)", () => {
  // A stripe is the gold mixed by HUMAN_HATCH into the card's own background: the surface, or in
  // the Tint look the card washed in its colour (blocked, needs you), which stays under the stripes.
  const stripe = (t: (typeof THEMES)[number], under = t.surface) => mix(HUMAN_COLOUR, under, HUMAN_HATCH);
  for (const t of THEMES) {
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: every word a card shows keeps its contrast on a stripe`, () => {
        const v = variables(t, a);
        // The title and other text at 7:1 (AAA), as on the plain card.
        expect(contrast(v["--text"], stripe(t))).toBeGreaterThanOrEqual(7);
        // Secondary words: meta (muted), the project name (accent-text), warnings (bad).
        for (const w of ["--muted", "--human-accent-text", "--human-bad"]) expect(contrast(v[w], stripe(t)), w).toBeGreaterThanOrEqual(4.5);
        // The You ribbon's border still stands out on it.
        expect(contrast(v["--edge-human"], stripe(t))).toBeGreaterThanOrEqual(3);
      });
      it(`${t.name} × ${a.name}: every word keeps its contrast on a stripe over whatever the Tint look washes the card in (4.5:1, 7:1 for the title)`, () => {
        const v = variables(t, a);
        // The wash is the card's colour: its stage, its priority, blocked or needs you (the Tint
        // look under any card colour); the stripe is the gold mixed into that wash.
        const colours = [...Object.keys(STAGE_COLOURS).map((s) => `--st-${s}`), "--st-needs", ...Object.keys(PRIORITY_COLOURS).map((p) => `--pri-${p}`)];
        for (const k of colours) {
          const under = mix(v[k], t.surface, CARD_TINT[t.mode]);
          expect(contrast(v["--text"], stripe(t, under)), `--text on ${k}`).toBeGreaterThanOrEqual(7);
          // Meta (muted), then the project name and the quiet and vision flags: .card.human shades
          // accent-text and bad to --human-accent-text and --human-bad (styles.css).
          for (const w of ["--muted", "--human-accent-text", "--human-bad"]) expect(contrast(v[w], stripe(t, under)), `${w} on ${k}`).toBeGreaterThanOrEqual(4.5);
        }
      });
    }
    it(`${t.name}: the hatch can be seen, and the CSS gets the tested amount`, () => {
      // Faint, but clearly there: a CIE76 distance of 2.3 is just noticeable.
      expect(colourDistance(stripe(t), t.surface)).toBeGreaterThanOrEqual(8);
      expect(variables(t, ACCENTS[0])["--human-hatch"]).toBe(`${Math.round(HUMAN_HATCH * 100)}%`);
    });
  }
});

describe("card looks (0026)", () => {
  // Every colour a card can take: each stage, blocked, needs you, the owner's gold, and each priority.
  const fills: [string, string][] = [
    ...Object.entries(STAGE_COLOURS),
    ["needs", NEEDS_COLOUR],
    ["human", HUMAN_COLOUR],
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
          // Title and meta (text, muted), the project name (.q, accent-text), a bug's kind (.kind.bug, bad).
          for (const w of ["--text", "--muted", "--accent-text", "--bad"]) expect(contrast(v[w], tinted), `${w} on ${k}`).toBeGreaterThanOrEqual(4.5);
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
    expect(css).toContain("--card-bg: color-mix(in srgb, var(--card-fill) var(--card-tint, 8%), var(--panel));");
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
  it("the You ribbon is the owner's gold with the status word colour, edged, in every look (0022)", () => {
    expect(css).toContain(`--st-human: ${HUMAN_COLOUR};`);
    const body = rule(".you-ribbon").body;
    expect(body).toContain("background: var(--st-human); color: var(--on-status);");
    expect(body).toContain("border: 1px solid var(--edge-human, var(--st-human));");
    // It's the card's own element, not a look's variable: no look hides or recolours it.
    for (const l of ["tint", "top", "dot", "outline"]) expect(rule(`:root[data-cards="${l}"] .board .card, .look-${l} .card`).body).not.toContain("human");
  });
  it("the owner's own card is hatched in every look: diagonal gold stripes over the card, CSS only (0022)", () => {
    expect(css).toContain(`--human-hatch: ${Math.round(HUMAN_HATCH * 100)}%;`);
    const body = rule(".card.human").body;
    // Stripes of the gold mixed into the card's own background (the tested colours), so a Tint wash stays under them.
    expect(body).toContain("background-image: repeating-linear-gradient(135deg, color-mix(in srgb, var(--st-human) var(--human-hatch), var(--card-bg, var(--panel))) 0 5px, transparent 5px 11px);");
    expect(body).not.toMatch(/url\(/);
    // It's the card's own rule, after the base card and the states, and no look, state or hover
    // sets a background image over it: blocked and needs you keep their colour, the hatch its pattern.
    expect(body.length).toBeGreaterThan(0);
    expect(rule(".card.human").at).toBeGreaterThan(rule(".card.blocked").at);
    const others = css.replace(body, "");
    expect(others.match(/\.card[^{]*\{[^}]*background-image/g) ?? []).toEqual([]);
    for (const sel of [".card.blocked", ".card.needs-you"]) expect(rule(sel).body).not.toContain("gradient");
  });
  it("on the owner's own card the accent and bad words take the --human-* shades, and the page has them before apply() (light and dark)", () => {
    expect(rule(".card.human").body).not.toContain("--bad");
    expect(css).toContain(".card.human { --accent-text: var(--human-accent-text); --bad: var(--human-bad); }");
    const block = (from: number) => css.slice(from, css.indexOf("}", from));
    const root = block(css.indexOf(":root {"));
    const night = block(css.indexOf(":root {", css.indexOf("prefers-color-scheme: dark")));
    for (const [b, t] of [[root, THEMES.find((x) => x.id === "violet-day")!], [night, THEMES.find((x) => x.id === "violet-night")!]] as const) {
      const v = variables(t, ACCENTS[0]);
      for (const k of ["--human-accent-text", "--human-bad"]) expect(b, `${k} on ${t.name}`).toContain(`${k}: ${v[k]};`);
    }
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

describe("one colour, one meaning (owner, 2026-10-13)", () => {
  it("priority's colours never pass for a stage, blocked or needs you", () => {
    for (const [p, c] of Object.entries(PRIORITY_COLOURS))
      for (const [s, sc] of Object.entries({ ...STAGE_COLOURS, needs: NEEDS_COLOUR }))
        if (p === "high") expect(colourDistance(c, sc), `${p} vs ${s}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
    // Normal and low are neutral greys, so they never read as a coloured stage either.
    for (const s of ["ready", "building", "testing", "your-check", "done", "blocked"])
      for (const p of ["normal", "low"]) expect(colourDistance(PRIORITY_COLOURS[p], STAGE_COLOURS[s]), `${p} vs ${s}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
  });
  it("the CSS gives priority, tags and types their own colours, not a stage's or the accent", () => {
    const rule = (sel: string) => css.slice(css.indexOf(`${sel} {`), css.indexOf("}", css.indexOf(`${sel} {`)));
    // A colour family by its names, Drover's and the old ones (0053), as a prefix: --accent is
    // --accent-strong and --accent-text too.
    const family = (...now: string[]) => new RegExp([...now, ...Object.keys(OLD_TOKEN_NAMES).filter((old) => now.includes(OLD_TOKEN_NAMES[old])), "--st-"].join("|"));
    expect(rule(".pri-high")).toContain("var(--pri-high");
    expect(rule(".pri-high")).not.toContain("--st-");
    expect(rule(".ui-pill.ui-pill--on.pchip-high")).toContain("var(--pri-high");
    expect(rule(".ui-pill.ui-pill--on.pchip-high")).not.toContain("--st-");
    expect(family("--bad").source).toBe("--bad|--err|--st-");
    expect(family("--accent", "--selected", "--focus").source).toBe("--accent|--selected|--focus|--accent-soft|--st-");
    expect(rule(".tag")).not.toMatch(family("--accent", "--selected", "--focus"));
    expect(rule(".kind.bug")).not.toMatch(family("--bad"));
  });
});

describe("Drover's token names (0053)", () => {
  const drover = ["--bg", "--panel", "--line", "--text", "--muted", "--accent", "--accent-ink", "--hover", "--selected", "--ok", "--bad", "--warn", "--focus"];
  const noComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const aliases = /\/\* Old names[\s\S]*?\*\/([^}]*)/.exec(css)?.[1] ?? "";
  it("every theme sets Drover's names, with Muster's own values", () => {
    for (const t of THEMES)
      for (const a of ACCENTS) {
        const v = variables(t, a);
        for (const k of drover) expect(v[k], `${t.id} ${k}`).toMatch(/^#[0-9A-F]{6}$/i);
        expect([v["--bg"], v["--panel"], v["--line"], v["--hover"], v["--text"], v["--muted"]]).toEqual([t.bg, t.surface, t.border, t.popover, t.text, t.muted]);
        expect(v["--accent"]).toBe(a[t.mode][0]);
        expect(v["--focus"]).toBe(v["--accent"]);
        expect(v["--warn"]).toBe(t.mode === "dark" ? NEEDS_COLOUR : "#A84300");
      }
  });
  it("and none of the old ones: styles.css aliases those", () => {
    const v = variables(THEMES[0], ACCENTS[0]);
    for (const [old, now] of Object.entries(OLD_TOKEN_NAMES)) {
      expect(v[old], old).toBeUndefined();
      expect(drover).toContain(now);
      expect(aliases).toContain(`${old}: var(${now});`);
    }
  });
  it("styles.css uses only the new names, outside the alias block", () => {
    const rest = noComments.replace(aliases, "");
    for (const old of Object.keys(OLD_TOKEN_NAMES)) expect(rest.match(new RegExp(`var\\(${old}[,)]`, "g")) ?? [], old).toEqual([]);
    for (const k of drover) expect(noComments, k).toContain(`var(${k}`);
  });
  it("the page has every Drover name before apply(), light and dark", () => {
    const block = (from: number) => noComments.slice(from, noComments.indexOf("}", from));
    const root = block(noComments.indexOf(":root {"));
    const night = block(noComments.indexOf(":root {", noComments.indexOf("prefers-color-scheme: dark")));
    for (const k of drover) {
      expect(root, k).toMatch(new RegExp(`\\n  ${k}: #[0-9A-F]{6};`));
      expect(night, k).toMatch(new RegExp(`\\n    ${k}: #[0-9A-F]{6};`));
    }
  });
});

describe("a block's look (0060): the owner's in needs-you orange, the rest neutral, each with its red mark", () => {
  const rule = (selector: string) => {
    const i = css.indexOf(`${selector} {`);
    expect(i, selector).toBeGreaterThanOrEqual(0);
    return css.slice(i, css.indexOf("}", i));
  };
  it("the CSS paints each look with the tokens checked below", () => {
    expect(rule(".ui-badge.block-why.for-you, .ui-status-pill.block-why.for-you")).toContain("background: var(--st-needs); color: var(--on-status);");
    expect(rule(".ui-badge.block-why.not-you, .ui-status-pill.block-why.not-you")).toContain("background: var(--panel); color: var(--text);");
    expect(rule(".block-why.plain.for-you")).toContain("color: var(--warn);");
    expect(rule(".block-why.plain.not-you")).toContain("color: var(--text);");
    expect(rule(".block-why::before")).toContain("background: var(--bad);");
    // On the orange, the red mark is ringed in the words' colour so it still reads.
    expect(rule(".block-why.for-you::before")).toContain("box-shadow: 0 0 0 1px var(--on-status);");
  });
  for (const t of THEMES) {
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name} (${t.mode}): every look's words read at 4.5:1, and its mark at 3:1`, () => {
        const v = variables(t, a);
        // A pill in the pane, a badge on the card.
        expect(contrast(v["--on-status"], v["--st-needs"]), "for you").toBeGreaterThanOrEqual(4.5);
        expect(contrast(v["--text"], v["--panel"]), "not you").toBeGreaterThanOrEqual(4.5);
        // Words alone, in the table, on the board or the pane.
        for (const s of [t.bg, t.surface, t.popover]) {
          expect(contrast(v["--warn"], s), "for you, plain").toBeGreaterThanOrEqual(4.5);
          expect(contrast(v["--text"], s), "not you, plain").toBeGreaterThanOrEqual(4.5);
          expect(contrast(v["--bad"], s), "the red mark").toBeGreaterThanOrEqual(3);
        }
        expect(contrast(v["--bad"], v["--panel"]), "the red mark on a neutral badge").toBeGreaterThanOrEqual(3);
        expect(contrast(v["--on-status"], v["--st-needs"]), "the mark's ring on orange").toBeGreaterThanOrEqual(3);
      });
    }
  }
});

describe("project colours (0057): a dot where a project is named, never read as a stage or a priority", () => {
  // Every colour that means something on a card or row, as it shows in a theme with an accent:
  // the stage, blocked and needs-you fills and their edges, the owner's gold and the priorities.
  const meanings = (t: (typeof THEMES)[number], a: (typeof ACCENTS)[number]) => {
    const v = variables(t, a);
    return [...Object.entries(v).filter(([k]) => /^--(st|edge|pri|edge-pri)-/.test(k)), ["--st-human", HUMAN_COLOUR] as const];
  };
  it("are a handful, each with a name, a shade per mode and the id projects.json keeps (arrange.rs COLOURS)", () => {
    expect(PROJECT_COLOURS.map((c) => c.id)).toEqual(["lime", "orchid", "pink", "tan", "mint"]);
    const rust = /pub const COLOURS: \[&str; \d+\] = \[([^\]]*)\]/.exec(arrangeRs)?.[1] ?? "";
    expect([...rust.matchAll(/"([a-z]+)"/g)].map((m) => m[1])).toEqual(PROJECT_COLOURS.map((c) => c.id));
    for (const c of PROJECT_COLOURS) {
      expect(c.name, c.id).toMatch(/^[A-Z][a-z]+$/);
      expect([c.dark, c.light], c.id).toEqual([expect.stringMatching(/^#[0-9A-F]{6}$/), expect.stringMatching(/^#[0-9A-F]{6}$/)]);
    }
  });
  for (const t of THEMES) {
    for (const a of ACCENTS) {
      it(`${t.name} × ${a.name}: each reads apart from every stage, blocked, needs-you, gold and priority colour at a glance`, () => {
        const v = variables(t, a);
        for (const c of PROJECT_COLOURS) {
          const shown = v[`--proj-${c.id}`];
          expect(shown, c.id).toBe(c[t.mode]);
          for (const [k, m] of meanings(t, a)) expect(colourDistance(shown, m), `${c.id} vs ${k}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
        }
      });
    }
    it(`${t.name}: each stands out at 3:1 where a dot sits, and reads apart from the others`, () => {
      const v = variables(t, ACCENTS[0]);
      for (const c of PROJECT_COLOURS) {
        for (const s of [t.bg, t.surface, t.popover]) expect(contrast(c[t.mode], s), `${c.id} on ${s}`).toBeGreaterThanOrEqual(3);
        for (const d of PROJECT_COLOURS) if (d !== c) expect(colourDistance(c[t.mode], d[t.mode]), `${c.id} vs ${d.id}`).toBeGreaterThanOrEqual(SAME_AT_A_GLANCE);
      }
      expect(Object.keys(v).filter((k) => k.startsWith("--proj-"))).toHaveLength(PROJECT_COLOURS.length);
    });
  }
  it("the CSS gives each its own rule, from the theme's variable", () => {
    for (const c of PROJECT_COLOURS) expect(css, c.id).toContain(`[data-colour="${c.id}"] { --proj: var(--proj-${c.id}); }`);
  });
});
