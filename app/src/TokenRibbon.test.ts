import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import Card from "./Card";
import { setLocale } from "./i18n/runtime";
import { Mode, MODES, type DisplayMode } from "./modes";
import css from "./styles.css?raw";
import enBoard from "./i18n/locales/en/board.json";
import enCommon from "./i18n/locales/en/common.json";
import faCommon from "./i18n/locales/fa/common.json";
import TokenRibbon from "./TokenRibbon";
import source from "./TokenRibbon.tsx?raw";
import type { Task } from "./types";

const ASGHAR = "Asghar_7 (wt-0172 · session a82f2534 · cc-mg)";
const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0172", prefix: "mu", title: "Token ribbon", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0172-t.md", claimedBy: ASGHAR, heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, tokensUsed: 210_412, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "600k", estMinutes: 0, estTestMinutes: 0, estTokenCount: 600_000,
  claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});

const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
const ribbonIn = (t: Task, mode: DisplayMode = "adhd", active = false) => inMode(mode, createElement(TokenRibbon, { t, active }));
const card = (t: Task, mode: DisplayMode = "adhd", active = false) => inMode(mode, createElement(Card, { t, now: 0, multi: false, parent: "", onOpen: () => {}, active, words: [] }));
/** The token ribbon's markup in a card; "" for none. */
const ribbon = (html: string) => /<span class="token-ribbon[^"]*"[\s\S]*?(?=<\/button>)/.exec(html)?.[0] ?? "";
/** The card button alone, without the Ask and Pause strip under it. */
const button = (html: string) => html.slice(0, html.indexOf("</button>") + "</button>".length);
const LRI = "\u2066", PDI = "\u2069";
const strip = (h: string) => h.replace(/[\u2066-\u2069]/g, "").replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();
/** What's read out: the text without aria-hidden parts. */
const spoken = (html: string) => strip(html.replace(/<(\w+)[^>]*aria-hidden="true"[^>]*>[^<]*<\/\1>/g, "").replace(/<i [^>]*aria-hidden="true"[^>]*><\/i>/g, ""));
/** What's seen: the text without the screen-reader-only words. */
const seen = (html: string) => strip(html.replace(/<span class="sr-only">[^<]*<\/span>/g, ""));
const classOf = (html: string) => /^<button class="([^"]*)"/.exec(html)?.[1].split(" ") ?? [];
const topRow = (html: string) => /<span class="card-top">.*?(?=<span class="title">)/.exec(html)?.[0] ?? "";
const rule = (selector: string) => {
  const i = css.indexOf(`\n${selector} {`);
  expect(i, selector).toBeGreaterThanOrEqual(0);
  return css.slice(i, css.indexOf("}", i));
};

afterEach(async () => {
  await setLocale("en");
});

describe("the token ribbon (mu-172)", () => {
  it("says used / estimated, with the counts in its tooltip and the words read out", () => {
    const html = ribbonIn(task({}));
    expect(html).toMatch(/^<span class="token-ribbon" title="210,412 of 600,000 tokens \(35%\): input, output and cache writes">/);
    expect(seen(html)).toBe("210k / 600k");
    expect(spoken(html)).toBe("210k of 600k tokens used");
    // Not a control: no button, link or tab stop of its own; the card is the button.
    expect(html).not.toMatch(/<button|<a |tabindex|role="button"/);
  });

  it("says over in a word of its own, at weight 800, never in an attention colour", () => {
    const html = ribbonIn(task({ tokensUsed: 720_000 }));
    expect(seen(html)).toBe("720k / 600k · over");
    expect(html).toContain('<b aria-hidden="true" class="token-over token-end">over</b>');
    expect(spoken(html)).toBe("720k of 600k tokens used, over the estimate");
    expect(rule(".token-over")).toContain("font-weight: 800;");
    for (const sel of [".token-ribbon", ".token-over"]) expect(rule(sel), sel).not.toMatch(/--warn|--bad|--st-|--status|--card-|--edge|--accent|--pri/);
  });

  it("carries the live dot while a session spends tokens, said once in words", () => {
    const html = ribbonIn(task({ live: true }));
    expect(html).toContain('class="dot live-dot"');
    // The dot is a shape beside the words; the words say it, so the dot isn't read out twice.
    expect(html).toMatch(/<i class="dot live-dot" aria-hidden="true" title="[^"]+"><\/i>/);
    expect(spoken(html)).toBe("210k of 600k tokens used, being used now");
    expect(ribbonIn(task({}))).not.toContain("live-dot");
  });

  it("says a shared session isn't counted: a word after the figures, and in the spoken words (review a11y-1)", () => {
    const html = ribbonIn(task({ tokensShared: true }));
    expect(seen(html)).toBe("210k / 600k · shared");
    expect(html).toContain('<span aria-hidden="true" class="token-shared">· shared</span>');
    expect(spoken(html)).toBe("210k of 600k tokens used, a session shared with other tasks not counted");
    expect(html).toContain("title=\"210,412 of 600,000 tokens (35%): input, output and cache writes; a session shared with other tasks isn&#x27;t counted\"");
    expect(seen(ribbonIn(task({ tokensShared: true, tokensUsed: 720_000 })))).toBe("720k / 600k · over · shared");
  });

  it("says no estimate and not read in words", () => {
    expect(seen(ribbonIn(task({ estTokenCount: 0 })))).toBe("210k · no est");
    expect(seen(ribbonIn(task({ tokensUsed: null })))).toBe("600k est · not read");
  });

  it("shows in ADHD on every held card; in Standard live, over or selected; in Focus live or over", () => {
    const quiet = task({});
    for (const mode of MODES.map((m) => m.id)) {
      expect(ribbonIn(task({ live: true }), mode), mode).toContain("token-ribbon");
      expect(ribbonIn(task({ tokensUsed: 720_000 }), mode), mode).toContain("token-ribbon");
    }
    expect(ribbonIn(quiet, "adhd")).toContain("token-ribbon");
    expect(ribbonIn(quiet, "standard")).toBe("");
    expect(ribbonIn(quiet, "standard", true)).toContain("token-ribbon");
    expect(ribbonIn(quiet, "focus")).toBe("");
    expect(ribbonIn(quiet, "focus", true)).toBe("");
  });

  it("speaks Persian in Persian, with Persian digits, the figures left to right", async () => {
    await setLocale("fa");
    const html = ribbonIn(task({ tokensUsed: 720_000 }));
    // «used از estimated», each figure in its own left-to-right isolate, no slash beside ٫.
    for (const fig of ["۷۲۰k", "۶۰۰k"]) expect(html).toContain(LRI + fig + PDI);
    expect(seen(html)).toMatch(/^۷۲۰k از ۶۰۰k /);
    expect(html).not.toContain(" / ");
    // The pair isn't one left-to-right run: از keeps its place right to left.
    expect(html).not.toMatch(/\u2066[^\u2069]*از/);
    expect(spoken(html)).not.toMatch(/[a-z]{2,}/i);
    expect(seen(html)).not.toMatch(/[a-z]{2,}/i);
  });
});

describe("the token ribbon on a card (mu-172)", () => {
  it("comes on a card someone holds in Shaping, Building or Testing, and on no other (round 2, F1)", () => {
    for (const stage of ["shaping", "building", "testing"]) {
      const held = stage === "testing" ? { heldBy: ASGHAR } : { claimedBy: ASGHAR };
      expect(ribbon(card(task({ stage, ...held }))), stage).toContain("210k");
      // Nobody holds it: no ribbon, so never "600k est · not read" on a card nobody works.
      const free = card(task({ stage, claimedBy: "", heldBy: "", tokensUsed: null }));
      expect(free, stage).not.toContain("token-ribbon");
      expect(free, stage).not.toContain("not read");
      // Its room at the bottom stays, so a claim arriving moves nothing.
      expect(classOf(free), stage).toContain("holdable");
    }
    for (const stage of ["backlog", "ready", "your-check", "done"]) expect(card(task({ stage })), stage).not.toContain("token-ribbon");
    // The owner's own task is theirs, not an agent's spend.
    expect(card(task({ forWho: "human" }))).not.toContain("token-ribbon");
    // An agent's task the owner holds still shows what it used.
    expect(ribbon(card(task({ claimedBy: "owner" })))).toContain("210k");
  });

  it("keeps the live dot in the ribbon's corner on a card nobody holds yet, alone and read out", () => {
    const html = card(task({ claimedBy: "", live: true }));
    expect(html.match(/live-dot/g)).toHaveLength(1);
    expect(topRow(html)).not.toContain("live-dot");
    expect(ribbon(html)).toMatch(/^<span class="token-ribbon token-dot"><span class="token-live token-end"><i class="dot live-dot" role="img" aria-label="live"/);
    expect(seen(ribbon(html))).toBe("");
    for (const mode of ["standard", "focus"] as const) expect(ribbon(card(task({ claimedBy: "", live: true }), mode)), mode).toContain("live-dot");
    expect(card(task({ claimedBy: "" }))).not.toContain("token-ribbon");
  });

  it("has its words in the shared namespace, there before the first paint (round 2, F2)", () => {
    expect(Object.keys(enCommon.tokenRibbon)).toContain("over");
    expect(Object.keys(faCommon.tokenRibbon).sort()).toEqual(Object.keys(enCommon.tokenRibbon).sort());
    expect(enBoard).not.toHaveProperty("tokenRibbon");
    expect(source).not.toMatch(/useT\("board"\)/);
  });

  it("is the card's last element, read out after the rest of the card", () => {
    const html = button(card(task({ tokensUsed: 720_000 })));
    expect(html).toMatch(/<span class="token-ribbon"[^>]*>[\s\S]*<\/span><\/button>$/);
    expect(spoken(html)).toMatch(/now 720k of 600k tokens used, over the estimate$/);
  });

  it("keeps its strip at the card's bottom in every mode, so a first count or a claim moves nothing", () => {
    for (const mode of MODES.map((m) => m.id)) {
      expect(classOf(card(task({}), mode)), mode).toContain("holdable");
      expect(classOf(card(task({ claimedBy: "", tokensUsed: null }), mode)), mode).toContain("holdable");
    }
    expect(rule(".card.holdable")).toContain("padding-block-end: 17px;");
    // Everything but the ribbon is the same before the first count and after it.
    const without = (html: string) => html.replace(/<span class="token-ribbon"[\s\S]*(?=<\/button>)/, "");
    expect(without(card(task({ tokensUsed: null })))).toBe(without(card(task({ tokensUsed: 900_000 }))));
  });

  it("holds the one live dot of a card that has it: not the holder ribbon, not the top row", () => {
    const html = card(task({ live: true }));
    expect(html.match(/live-dot/g)).toHaveLength(1);
    expect(ribbon(html)).toContain("live-dot");
    expect(topRow(html)).not.toContain("live-dot");
    expect(html).not.toMatch(/<span class="holder-ribbon[^"]*"[^>]*>(?:(?!<span class="card-band").)*live-dot/);
    // In Standard and Focus the ribbon shows while live, so the dot is never lost.
    for (const mode of ["standard", "focus"] as const) expect(ribbon(card(task({ live: true }), mode)), mode).toContain("live-dot");
    // A card without the ribbon's spot keeps its dot in the top row, as before.
    expect(topRow(card(task({ stage: "ready", live: true })))).toContain("live-dot");
    expect(topRow(card(task({ forWho: "human", live: true })))).toContain("live-dot");
  });

  it("drops the dot while Claude Code says the holder's session waits for input or has ended (mu-171)", () => {
    for (const sessionState of ["needs-input", "ended"] as const) {
      expect(card(task({ live: true, tokensUsed: 210_000, sessionState })), sessionState).not.toContain("live-dot");
    }
    expect(ribbon(card(task({ live: true, tokensUsed: 210_000, sessionState: "working" })))).toContain("live-dot");
    expect(ribbon(card(task({ live: true, tokensUsed: 210_000, sessionState: "unknown" })))).toContain("live-dot");
  });

  it("says 'being used now' only when it draws the dot: Claude Code's word wins in the spoken words and the busy check too (review R1)", () => {
    for (const sessionState of ["needs-input", "ended"] as const) {
      const t = task({ live: true, tokensUsed: 210_000, sessionState });
      expect(spoken(ribbonIn(t)), sessionState).toBe("210k of 600k tokens used");
      // Standard and Focus show the ribbon while it's being spent; a waiting or ended session isn't.
      for (const mode of ["standard", "focus"] as const) expect(ribbonIn(t, mode), `${sessionState} ${mode}`).toBe("");
    }
    const working = task({ live: true, tokensUsed: 210_000, sessionState: "working" });
    expect(spoken(ribbonIn(working))).toBe("210k of 600k tokens used, being used now");
    for (const mode of ["standard", "focus"] as const) expect(ribbonIn(working, mode), mode).toContain("live-dot");
  });
});

describe("the token ribbon's CSS (mu-172)", () => {
  /** Each part's own text on the card, as the flex layout shows it: the spaces between the parts
   *  are only for copied text, the layout drops them and the parts' padding spaces them. */
  const parts = (html: string) => [...html.matchAll(/<(?:span|b) aria-hidden="true" class="token-[^"]*">([^<]*)<\/(?:span|b)>/g)].map((m) => m[1]);
  const cases = [
    task({}), task({ tokensUsed: 720_000 }), task({ tokensUsed: 720_000, tokensShared: true, live: true }), task({ estTokenCount: 0, tokensShared: true }),
    task({ tokensUsed: null }), task({ tokensUsed: null, estTokenCount: 0 }), task({ tokensUsed: 9_900_000, estTokenCount: 999_000, tokensShared: true, live: true }),
  ];

  it("sits in the card's bottom corner on the side the text ends, mirrored right to left", () => {
    const corner = rule(".card .token-ribbon");
    expect(corner).toContain("position: absolute; inset-block-end: 0; inset-inline-end: 0;");
    expect(corner).not.toMatch(/\bright:|\bleft:|\bbottom:/);
    // The first part shown rounds the corner the ribbon turns.
    expect(rule(".token-ribbon > :first-child")).toContain("border-inline-start: 1px solid var(--line); border-start-start-radius: 6px;");
  });

  it("is neutral, as the holder ribbon: each part on the panel, under the line, in the text colour", () => {
    expect(rule(".token-ribbon > :not(.sr-only)")).toContain("background: var(--panel); color: var(--text); border-block-start: 1px solid var(--line);");
  });

  it("gives way a whole part at a time, never part of one: one line high, a part that doesn't fit wraps out of sight (re-shoot 2)", () => {
    const body = rule(".token-ribbon");
    expect(body).toContain("display: flex; flex-wrap: wrap; justify-content: flex-end; align-content: flex-start;");
    expect(body).toContain("max-width: 100%; height: calc(1.5em + 1px); overflow: hidden;");
    expect(body).toContain("white-space: nowrap;");
    // No part shrinks, so none is ever shown cut; no ellipsis anywhere in the ribbon.
    expect(rule(".token-ribbon > :not(.sr-only)")).toContain("flex: none;");
    const ribbonRules = [...css.matchAll(/\n([^{}\n]*\.token-[^{}\n]*)\{([^}]*)\}/g)];
    expect(ribbonRules.length).toBeGreaterThan(5);
    expect(ribbonRules.filter((m) => /text-overflow|min-width: 0|flex-shrink|flex: 0 \d/.test(m[2])).map((m) => m[1].trim())).toEqual([]);
  });

  it("orders the parts by what must stay: the dot, the figures, over and the note first; shared, which goes first, last", () => {
    const order = (html: string) => [...html.matchAll(/class="(token-[a-z]+|sr-only)[^"]*"/g)].map((m) => m[1]).filter((c) => c !== "token-ribbon");
    expect(order(ribbonIn(task({ tokensUsed: 720_000, tokensShared: true, live: true })))).toEqual(["token-live", "token-figures", "token-sep", "token-over", "token-shared", "sr-only"]);
    expect(order(ribbonIn(task({ tokensUsed: null, tokensShared: true })))).toEqual(["token-figures", "token-shared", "sr-only"]);
    // The note is the whole message when nothing was read: it never gives way.
    expect(css).not.toMatch(/\.token-note[^{]*\{[^}]*display: none/);
  });

  it("when nothing was read and a session is live, glues the note to the figures as one part and puts the dot after it, so the dot is what gives way (re-shoot 3)", () => {
    const order = (html: string) => [...html.matchAll(/class="(token-[a-z]+|sr-only)[^"]*"/g)].map((m) => m[1]).filter((c) => c !== "token-ribbon");
    const html = ribbonIn(task({ tokensUsed: null, live: true, tokensShared: true }));
    expect(order(html)).toEqual(["token-figures", "token-live", "token-shared", "sr-only"]);
    expect(html).toContain('<span aria-hidden="true" class="token-figures token-note token-end">' + LRI + "600k" + PDI + ' est · not read</span>');
    expect(seen(html)).toBe("600k est · not read · shared");
    // The words still say a session is spending now.
    expect(spoken(html)).toBe("600k tokens estimated, none read yet, a session shared with other tasks not counted, being used now");
    // Nothing read and no estimate: the note alone, the dot after it too.
    expect(order(ribbonIn(task({ tokensUsed: null, estTokenCount: 0, live: true })))).toEqual(["token-figures", "token-live", "sr-only"]);
    // Once something was read the dot leads again.
    expect(order(ribbonIn(task({ live: true })))).toEqual(["token-live", "token-figures", "sr-only"]);
  });

  it("keeps the · before over (AC3's \"720k / 600k · over\") whole on every card: nothing hides or squeezes it", () => {
    expect(css).not.toMatch(/\.token-sep[^{]*\{[^}]*(display: none|flex: 0)/);
    expect(seen(ribbonIn(task({ tokensUsed: 9_900_000, estTokenCount: 999_000, live: true })))).toBe("9.9M / 999k · over");
    expect(rule(".token-over")).toContain("padding-inline-start: 2px;");
  });

  it("shows no stray space, dot or ellipsis: each part's text is whole and trimmed, the spacing is the parts' padding", async () => {
    for (const lang of ["en", "fa"] as const) {
      await setLocale(lang);
      for (const t of cases) {
        const ps = parts(ribbonIn(t));
        expect(ps.length, lang).toBeGreaterThan(0);
        for (const p of ps) {
          expect(p, `${lang} ${p}`).toBe(p.trim());
          expect(p, `${lang} ${p}`).not.toMatch(/…|\s{2}/);
        }
      }
    }
  });

  it("ends with its own padding on whichever part is last: the last that never goes, and shared", () => {
    expect(rule(".token-ribbon > .token-end")).toContain("padding-inline-end: 2px;");
    expect(rule(".token-ribbon > .token-shared")).toContain("padding-inline: 0 2px;");
    expect(ribbonIn(task({ tokensUsed: 720_000 }))).toContain('<b aria-hidden="true" class="token-over token-end">over</b>');
    expect(ribbonIn(task({}))).toContain('class="token-figures token-end"');
    expect(ribbonIn(task({ tokensUsed: null }))).toContain('class="token-figures token-note token-end"');
    expect(ribbonIn(task({ tokensShared: true }))).toContain('<span aria-hidden="true" class="token-shared">· shared</span>');
  });

  it("fits the dot, the figures, the · and over within a 100px card in every look: a width budget from the system font", () => {
    // The system font's advance widths at weight 600 in em (a digit 0.62, M 0.85, k 0.55, the
    // point and the · 0.27, slash 0.33, space 0.28; "over" at 800 2.3em). Checked against the
    // shots: at 106px (a 100px padding box) the round-3 ribbon fit its "9.9M / 999k · over", the
    // dot and a stray point of "shared", which this model puts at about 99.5px; at 100px (94px)
    // it fit them less the "·" but for its space, about 93px. Each part's 2px padding, the 6px
    // dot, the 1px border; nothing else.
    expect(rule(".token-ribbon")).toContain("font-size: 9px;");
    expect(rule(".token-ribbon")).not.toMatch(/padding|gap/);
    expect(rule(".token-ribbon > :not(.sr-only)")).toContain("padding-inline-start: 2px;");
    expect(rule(".token-ribbon .live-dot")).toContain("width: 6px; height: 6px;");
    const em: Record<string, number> = { M: 0.85, k: 0.55, ".": 0.27, "·": 0.27, "/": 0.33, " ": 0.28 };
    const w = (s: string) => [...s].reduce((n, c) => n + (em[c] ?? 0.62) * 9, 0);
    // dot, figures, ·, over (and its end padding), the corner's border.
    const width = 2 + 6 + (2 + w("9.9M / 999k")) + (2 + w("·")) + (2 + 2.3 * 9 + 2) + 1;
    // A 100px card's padding box: less the right border's 1px and the left accent's 5px (the
    // widest look's).
    expect(width).toBeLessThanOrEqual(94);
  });

  it("keeps the live dot still: no animation, and none under reduced motion either", () => {
    // No rule animates the dot…
    const animated = [...css.matchAll(/([^{}]*live-dot[^{}]*)\{([^}]*)\}/g)].filter((m) => /animation(?!: none)|transition(?!: none)/.test(m[2]));
    expect(animated.map((m) => m[1].trim())).toEqual([]);
    // …and reduced motion holds it still whatever a later rule adds.
    const reduce = /@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)\n\}/.exec(css)?.[1] ?? "";
    expect(reduce).toMatch(/\.live-dot[^{]*\{[^}]*animation: none;/);
  });
});
