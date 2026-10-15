import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { NoteCard } from "./Backlog";
import Card from "./Card";
import { cardRibbon, noteQuiet, ribbonHolder, ribbonNames } from "./HolderRibbon";
import { Mode, MODES, type DisplayMode } from "./modes";
import { Machine } from "./settingsData";
import css from "./styles.css?raw";
import type { Note, Task } from "./types";

const task = (over: Partial<Task>): Task => ({
  queue: "muster", id: "0097", prefix: "mu", title: "Shaping is a claim", kind: "code", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0097-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "2026-10-14 18:55", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0,
  level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", ...over,
});
const note = (over: Partial<Note>): Note => ({ queue: "muster", file: "2026-10-14-1600-x.md", title: "Backlog notes show who", text: "Backlog notes show who", truncated: false, added: 0, heldBy: "", heldSince: "", heartbeat: "", ...over });

/** The wall clock here reading `s`, as the state's `now` (seconds since 1970). */
const at = (s: string) => {
  const [y, mo, d, h, mi] = s.split(/[- :]/).map(Number);
  return new Date(y, mo - 1, d, h, mi).getTime() / 1000;
};
const NOW = at("2026-10-14 19:00");
const SHAPOUR = "shapour (main · session 1137d98f · cc-mg)";
const ASGHAR = "Asghar_7 (wt-0097 · session a82f2534 · cc-mg)";

const inMode = (mode: DisplayMode, el: ReactElement) => renderToStaticMarkup(createElement(Mode.Provider, { value: mode }, el));
const card = (t: Task, mode: DisplayMode = "adhd") => inMode(mode, createElement(Card, { t, now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] }));
const noteCard = (n: Note, mode: DisplayMode = "adhd") => inMode(mode, createElement(NoteCard, { n, now: NOW, multi: false, active: false, onOpen: () => {}, words: [] }));
/** The ribbon's markup, up to what follows it (a card's band, a note's title); "" for none. */
const RIBBON = /<span class="holder-ribbon.*?(?=<span class="card-band"|<span class="title")/;
const ribbon = (html: string) => RIBBON.exec(html)?.[0] ?? "";
/** What's read out: the text without tags and without aria-hidden parts. */
const spoken = (html: string) =>
  html.replace(/<(\w+)[^>]*aria-hidden="true"[^>]*>[^<]*<\/\1>/g, "").replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();
/** What's seen: the text without tags and without the screen-reader-only words. */
const seen = (html: string) => html.replace(/<span class="sr-only">[^<]*<\/span>/g, "").replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim();
const topRow = (html: string) => /<span class="card-top">.*?(?=<span class="title">)/.exec(html)?.[0] ?? "";
const classOf = (html: string) => /^<button class="([^"]*)"/.exec(html)?.[1].split(" ") ?? [];
const MODE_IDS = MODES.map((m) => m.id);

describe("who holds a card, for its ribbon (0097)", () => {
  it("is the claim in Shaping and Building, held-by in Testing, nobody elsewhere", () => {
    expect(ribbonHolder(task({ stage: "shaping", claimedBy: SHAPOUR }))).toBe(SHAPOUR);
    expect(ribbonHolder(task({ stage: "building", claimedBy: ASGHAR }))).toBe(ASGHAR);
    expect(ribbonHolder(task({ stage: "testing", heldBy: "qa-0097", claimedBy: ASGHAR }))).toBe("qa-0097");
    // Ready's leftover claim, Your check's holder and Done wear no holder ribbon.
    for (const stage of ["backlog", "ready", "your-check", "done"]) expect(ribbonHolder(task({ stage, claimedBy: ASGHAR, heldBy: ASGHAR })), stage).toBe("");
    expect(ribbonHolder(task({ stage: "building", claimedBy: "  " }))).toBe("");
  });
  it("wears one ribbon: You for the owner's own task and one the owner holds, the holder's otherwise", () => {
    expect(cardRibbon(task({ claimedBy: ASGHAR }))).toBe("holder");
    expect(cardRibbon(task({ stage: "shaping", claimedBy: SHAPOUR }))).toBe("holder");
    expect(cardRibbon(task({ claimedBy: "owner" }))).toBe("you");
    expect(cardRibbon(task({ stage: "testing", heldBy: "owner (main · session 1137d98f · cc-mg)" }))).toBe("you");
    expect(cardRibbon(task({ forWho: "human" }))).toBe("you");
    expect(cardRibbon(task({ forWho: "human", stage: "ready" }))).toBe("you");
    expect(cardRibbon(task({}))).toBeNull();
    expect(cardRibbon(task({ stage: "ready", claimedBy: ASGHAR }))).toBeNull();
  });
  it("says whether the ribbon names the holder, so the who line leaves the name out", () => {
    expect(ribbonNames(task({ claimedBy: ASGHAR }))).toBe(true);
    expect(ribbonNames(task({ claimedBy: "owner" }))).toBe(true);
    expect(ribbonNames(task({ stage: "ready", claimedBy: ASGHAR }))).toBe(false);
    expect(ribbonNames(task({}))).toBe(false);
    // The owner's own task wears You for being theirs: a name in its claim isn't on it.
    expect(ribbonNames(task({ forWho: "human", claimedBy: "wt-b" }))).toBe(false);
  });
});

describe("a held card wears its holder's ribbon (0097)", () => {
  const held: [string, Partial<Task>, string][] = [
    ["shaping", { stage: "shaping", claimedBy: SHAPOUR }, "shapour"],
    ["building", { stage: "building", claimedBy: ASGHAR }, "Asghar_7"],
    ["testing", { stage: "testing", heldBy: "qa-0097 (wt-0097 · session 99999999 · cc-hg)" }, "qa-0097"],
  ];
  for (const mode of MODE_IDS) {
    for (const [stage, over, name] of held) {
      it(`in ${stage}, in ${mode}: the short name in the corner, the label whole in the tooltip`, () => {
        const html = card(task(over), mode);
        const r = ribbon(html);
        expect(r, html).not.toBe("");
        expect(r).toContain(`<span class="holder-name" dir="auto">${name}</span>`);
        expect(r).toContain(`title="Held by ${(over.claimedBy || over.heldBy)!}"`);
        // The ribbon is the card's own first element, like the You ribbon, and never beside it.
        expect(html).toMatch(/^<button class="[^"]*"><span class="holder-ribbon/);
        expect(html).not.toContain("you-ribbon");
        // Read out with the card: "held by shapour".
        expect(spoken(html)).toContain(`held by ${name}`);
        // Not a control: no button, link or tab stop of its own.
        expect(r).not.toMatch(/<button|<a |tabindex|role="button"/);
      });
    }
  }
  it("shows only the You ribbon on the owner's own task, and on one the owner holds", () => {
    for (const t of [
      task({ forWho: "human", stage: "building", claimedBy: "owner" }),
      task({ forWho: "human", stage: "ready" }),
      task({ stage: "building", claimedBy: "owner" }),
      task({ stage: "testing", heldBy: "owner" }),
    ]) {
      const html = card(t);
      expect(html.match(/you-ribbon/g), t.stage).toHaveLength(1);
      expect(html, t.stage).not.toContain("holder-ribbon");
    }
    expect(card(task({ stage: "testing", heldBy: "owner" }))).toContain('<span class="you-ribbon" title="You hold it"><span class="sr-only">held by </span>You</span>');
  });
  it("reads out who holds it on an agent's task the owner holds, as the holder ribbon does; the owner's own task is just You", () => {
    expect(spoken(card(task({ stage: "building", claimedBy: "owner" })))).toContain("held by You");
    const own = card(task({ forWho: "human", stage: "ready" }));
    expect(own).toContain('<span class="you-ribbon" title="Your own task: only you do it, agents never take it">You</span>');
    expect(spoken(own)).not.toContain("held by");
  });
  it("wears nothing when nobody holds it, or in a stage with no ribbon", () => {
    expect(card(task({}))).not.toContain("ribbon");
    expect(card(task({ stage: "ready", claimedBy: ASGHAR }))).not.toContain("ribbon");
    expect(card(task({ stage: "your-check", heldBy: ASGHAR }))).not.toContain("holder-ribbon");
  });
  it("keeps the ribbon's room in the top row held or not, so a claim arriving moves nothing", () => {
    const unheld = card(task({}));
    const heldCard = card(task({ claimedBy: ASGHAR }));
    expect(classOf(unheld)).toContain("holdable");
    expect(classOf(heldCard)).toContain("holdable");
    expect(topRow(heldCard)).not.toBe("");
    expect(topRow(heldCard)).toBe(topRow(unheld));
    for (const stage of ["shaping", "testing"]) expect(classOf(card(task({ stage }))), stage).toContain("holdable");
    // Where no ribbon comes, nothing is kept; the owner's own card keeps the You ribbon's room.
    for (const stage of ["backlog", "ready", "your-check", "done"]) expect(classOf(card(task({ stage }))), stage).not.toContain("holdable");
    expect(classOf(card(task({ forWho: "human" })))).not.toContain("holdable");
  });
  it("carries the live dot beside the name, not in the top row, so a session starting moves nothing either", () => {
    const html = card(task({ claimedBy: ASGHAR, live: true }));
    expect(ribbon(html)).toContain('class="dot live-dot" role="img" aria-label="live"');
    expect(topRow(html)).not.toContain("live-dot");
    expect(html.match(/live-dot/g)).toHaveLength(1);
    expect(topRow(html)).toBe(topRow(card(task({}))));
    // Without a holder ribbon the dot stays where it was.
    expect(topRow(card(task({ live: true })))).toContain("live-dot");
    expect(topRow(card(task({ forWho: "human", live: true })))).toContain("live-dot");
  });
  it("turns quiet, in words, once the lease runs out, as the card's flag does", () => {
    const quiet = { claimedBy: ASGHAR, heartbeat: "2026-10-14 06:00", stageSince: "2026-10-14 05:00" };
    const r = ribbon(card(task(quiet)));
    expect(r).toMatch(/^<span class="holder-ribbon quiet"/);
    expect(seen(r)).toBe("Asghar_7 · quiet");
    expect(r).toContain('<span class="holder-quiet"> · quiet</span>');
    expect(r).toContain(`title="Held by ${ASGHAR}. Quiet: no heartbeat for over 12 h, its lease ran out"`);
    expect(seen(card(task(quiet)))).toContain("quiet 12 h+");
    // Shaping's claim has the same lease.
    expect(ribbon(card(task({ ...quiet, stage: "shaping" })))).toMatch(/^<span class="holder-ribbon quiet"/);
    // Within the lease, blocked or paused: not quiet. This machine's lease decides.
    expect(ribbon(card(task({ ...quiet, heartbeat: "2026-10-14 08:00" })))).toMatch(/^<span class="holder-ribbon"/);
    expect(ribbon(card(task({ ...quiet, blocked: true, waitingOn: "owner", blockedKind: "you" })))).toMatch(/^<span class="holder-ribbon"/);
    expect(ribbon(card(task({ ...quiet, paused: "2026-10-14 07:00" })))).toMatch(/^<span class="holder-ribbon"/);
    const shortLease = renderToStaticMarkup(
      createElement(Machine.Provider, { value: { leaseMinutes: 60, liveMinutes: 5 } }, createElement(Card, { t: task({ ...quiet, heartbeat: "2026-10-14 17:30" }), now: NOW, multi: false, parent: "", onOpen: () => {}, active: false, words: [] })),
    );
    expect(ribbon(shortLease)).toMatch(/^<span class="holder-ribbon quiet"/);
  });
  it("keeps a long name whole for CSS to end in …, with the whole label in the tooltip", () => {
    const long = "a-very-long-session-name-that-will-never-fit-the-corner (main · session 1137d98f · cc-mg)";
    const r = ribbon(card(task({ claimedBy: long })));
    expect(r).toContain('<span class="holder-name" dir="auto">a-very-long-session-name-that-will-never-fit-the-corner</span>');
    expect(r).toContain(`title="Held by ${long}"`);
  });
  it("isolates a right-to-left name and shows it as text, never markup", () => {
    const r = ribbon(card(task({ claimedBy: "شاپور (main · session 1137d98f · cc-mg)" })));
    expect(r).toContain('<span class="holder-name" dir="auto">شاپور</span>');
    expect(ribbon(card(task({ claimedBy: "<b>x</b>" })))).toContain("&lt;b&gt;x&lt;/b&gt;");
  });
});

describe("the who line beside the ribbon (0097)", () => {
  const lineOf = (html: string) => /<span class="card-who[^"]*"[^>]*>([^<]*)<\/span>/.exec(html)?.[1] ?? null;
  it("keeps the session and since when, and leaves the name to the ribbon", () => {
    const t = task({ claimedBy: ASGHAR, stageSince: "2026-10-14 17:00" });
    expect(lineOf(card(t))).toBe("wt-0097 · a82f2534 · started 2 h ago");
    expect(lineOf(card(task({ stage: "testing", heldBy: "qa-0097 (session 99999999)", stageSince: "2026-10-14 18:00" })))).toBe("99999999 · started 1 h ago");
    // The owner's: You says who.
    expect(lineOf(card(task({ claimedBy: "owner", stageSince: "2026-10-14 17:00" })))).toBe("started 2 h ago");
  });
  it("leaves out an empty line rather than saying nobody on a held card", () => {
    const html = card(task({ stage: "testing", heldBy: "qa" }));
    expect(lineOf(html)).toBeNull();
    expect(html).not.toContain("nobody on it");
  });
  it("keeps the name where no ribbon names it: Ready's claim, nobody", () => {
    expect(lineOf(card(task({ stage: "ready", claimedBy: "wt-b" })))).toBe("wt-b");
    expect(lineOf(card(task({})))).toBe("nobody on it");
  });
  it("in Standard's brief card, the ribbon is the one place the holder shows", () => {
    const html = card(task({ claimedBy: ASGHAR, stageSince: "2026-10-14 17:00" }), "standard");
    expect(lineOf(html)).toBeNull();
    expect(seen(ribbon(html))).toBe("Asghar_7");
  });
});

describe("a backlog note being shaped (0097)", () => {
  const shaped = { heldBy: SHAPOUR, heldSince: "2026-10-14 18:40", heartbeat: "2026-10-14 18:50" };
  for (const mode of MODE_IDS) {
    it(`says "shaping · shapour" in its corner, in ${mode}`, () => {
      const html = noteCard(note(shaped), mode);
      const r = ribbon(html);
      expect(seen(r)).toBe("shaping · shapour");
      expect(r).toContain('<span class="holder-name" dir="auto">shapour</span>');
      expect(r).toContain(`title="Being shaped by ${SHAPOUR}, taken 20 min ago"`);
      expect(spoken(html)).toContain("being shaped by shapour");
      expect(spoken(html)).not.toContain("shaping ·");
      // "shaping" gives way first; the "·" stays, so what's left never reads as two names.
      expect(r).toContain('<span aria-hidden="true" class="holder-prefix">shaping</span><span aria-hidden="true" class="holder-sep"> · </span>');
      expect(html).toMatch(/^<button class="note-card"[^>]*><span class="holder-ribbon/);
    });
  }
  it("says in its tooltip how long ago it was taken, when the claim says", () => {
    expect(ribbon(noteCard(note({ ...shaped, heldSince: "2026-10-14 19:00" })))).toContain(`title="Being shaped by ${SHAPOUR}, taken just now"`);
    expect(ribbon(noteCard(note({ ...shaped, heldSince: "" })))).toContain(`title="Being shaped by ${SHAPOUR}"`);
  });
  it("wears the gold You ribbon when the owner shapes it, never the holder's", () => {
    const html = noteCard(note({ ...shaped, heldBy: "owner" }));
    expect(html).not.toContain("holder-ribbon");
    expect(html).toMatch(/^<button class="note-card"[^>]*><span class="you-ribbon" title="You&#x27;re shaping it, taken 20 min ago"><span class="sr-only">being shaped by <\/span>You<\/span>/);
    expect(spoken(html)).toContain("being shaped by You");
    // An owner's label with a where still reads as the owner.
    expect(noteCard(note({ ...shaped, heldBy: "owner (main · session 1137d98f · cc-mg)" }))).toContain('<span class="you-ribbon"');
  });
  it("says quiet on the owner's You ribbon too once the lease ran out, as reclaim will end it", () => {
    const owner = "owner (main · session 1137d98f · cc-mg)";
    const live = noteCard(note({ ...shaped, heldBy: owner }));
    expect(seen(live)).not.toContain("quiet");
    const html = noteCard(note({ ...shaped, heldBy: owner, heartbeat: "2026-10-14 06:00" }));
    // In words, on the gold: the attention colour wouldn't read on it.
    expect(html).toContain('<span class="you-ribbon" title="You&#x27;re shaping it, taken 20 min ago. Quiet: no heartbeat for over 12 h, its lease ran out">');
    expect(seen(html)).toMatch(/^You · quiet/);
    expect(spoken(html)).toMatch(/^being shaped by You · quiet/);
  });
  it("wears nothing when nobody is shaping it", () => {
    expect(noteCard(note({}))).not.toContain("ribbon");
  });
  it("keeps everything else where it was when a shaper takes it", () => {
    const strip = (html: string) => html.replace(RIBBON, "");
    expect(strip(noteCard(note(shaped)))).toBe(noteCard(note({})));
  });
  it("turns quiet once its heartbeat is older than the lease", () => {
    expect(noteQuiet(note(shaped), NOW, 720)).toBe(false);
    expect(noteQuiet(note({ ...shaped, heartbeat: "2026-10-14 06:59" }), NOW, 720)).toBe(true);
    expect(noteQuiet(note({ ...shaped, heartbeat: "2026-10-14 07:00" }), NOW, 720)).toBe(false);
    expect(noteQuiet(note({ ...shaped, heartbeat: "2026-10-14 18:30" }), NOW, 20)).toBe(true);
    // Not held, or no heartbeat to judge by: not quiet.
    expect(noteQuiet(note({ heartbeat: "2026-10-01 00:00" }), NOW, 720)).toBe(false);
    expect(noteQuiet(note({ ...shaped, heartbeat: "" }), NOW, 720)).toBe(false);
    expect(noteQuiet(note({ ...shaped, heartbeat: "soon" }), NOW, 720)).toBe(false);
    const r = ribbon(noteCard(note({ ...shaped, heartbeat: "2026-10-14 06:00" })));
    expect(r).toMatch(/^<span class="holder-ribbon quiet"/);
    expect(seen(r)).toBe("shaping · shapour · quiet");
  });
});

describe("the ribbons' CSS (0097)", () => {
  /** The rule whose selector starts its line, as written. */
  const rule = (selector: string) => {
    const i = css.indexOf(`\n${selector} {`);
    expect(i, selector).toBeGreaterThanOrEqual(0);
    return css.slice(i, css.indexOf("}", i));
  };
  it("sits in the corner the text starts from the other side of, mirrored right to left, for both ribbons", () => {
    const corner = rule(".card .you-ribbon, .card .holder-ribbon, .note-card .you-ribbon, .note-card .holder-ribbon");
    expect(corner).toContain("position: absolute; inset-block-start: 0; inset-inline-end: 0;");
    expect(corner).toContain("border-end-start-radius: 6px;");
    expect(corner).not.toMatch(/\bright:|\bleft:/);
    expect(rule(".card.human .card-top")).toContain("padding-inline-end: 34px;");
    // A card clips its ribbon to its rounded corner; a note doesn't clip, so its ribbon rounds that
    // corner itself, inside the note's 1px dashed border.
    expect(rule(".card")).toContain("overflow: clip;");
    expect(rule(".note-card .you-ribbon, .note-card .holder-ribbon")).toContain("border-start-end-radius: calc(var(--radius) - 1px);");
  });
  it("runs nearly the card's width (half beside the stage band), clipped there, and the name keeps its share: the shaping prefix gives way first", () => {
    const ribbon = rule(".holder-ribbon");
    expect(ribbon).toContain("max-width: calc(100% - 12px);");
    expect(rule(':root[data-cards="top"] .board .card .holder-ribbon, .look-top .card .holder-ribbon')).toContain("max-width: 50%;");
    expect(ribbon).toContain("overflow: hidden;");
    // The name grows into what's free and never shrinks below about four characters.
    expect(rule(".holder-name")).toContain("flex: 1 1 auto; min-width: 4ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;");
    // The prefix shrinks first, down to nothing, ending in …: at a hundred thousand times the
    // name's rate the name's share of the squeeze rounds to nothing, so WebKit never cuts it early.
    expect(rule(".holder-prefix")).toContain("flex: 0 100000 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis;");
    // Its "·" never shrinks.
    expect(rule(".holder-sep")).toContain("flex: none;");
    // Quiet is a state: its words never shrink.
    expect(rule(".holder-quiet")).toContain("flex: none;");
    // Nor does the live dot: beside a long name it stays round.
    expect(rule(".holder-ribbon .live-dot")).toContain("flex: none;");
    for (const sel of [".card", ".note-card"]) expect(rule(sel), sel).toContain("position: relative;");
  });
  it("keeps a strip the ribbon's height above the top row, held or not, so the top row runs full width", () => {
    // No inline-end room in the top row or a note's title any more: key, kind and priority keep the card's width.
    expect(css).not.toContain(".card.holdable .card-top");
    expect(css).not.toContain(".note-card .title::before");
    // The ribbon is 9.5px × 1.4 plus its 1px border, about 14.3px: the strip is 17px from the card's edge.
    expect(rule(".card.holdable")).toContain("padding-block-start: 17px;");
    // The Top bar look's band still starts at the card's edge, under the ribbon, as on every card.
    expect(rule(".card.holdable .card-band")).toContain("margin-block-start: -17px;");
    // Every note keeps the same strip above its title.
    expect(rule(".note-card")).toContain("padding: 16px 10px 7px;");
  });
  it("has the You ribbon's size and weight, without its capitals", () => {
    const you = rule(".you-ribbon");
    const holder = rule(".holder-ribbon");
    for (const d of ["font-size: 9.5px;", "font-weight: 800;", "line-height: 1.4;"]) {
      expect(you).toContain(d);
      expect(holder).toContain(d);
    }
    expect(holder).not.toContain("uppercase");
  });
});
