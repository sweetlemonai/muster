import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import commentsTsx from "./Comments.tsx?raw";
import editBodyTsx from "./EditBody.tsx?raw";
import { windowKey } from "./keys";
import { escOwned, OWN_ESC } from "./ownEscape";
import { unreachedHolder, unreachedWords } from "./paneData";
import { NextStep, releaseEscape, ReleaseUnreachedView, watchReleaseEscape, whoIn } from "./Sidebar";
import sidebarTsx from "./Sidebar.tsx?raw";
import type { Task } from "./types";
import type { TaskActions } from "./useTaskActions";

// Release on a claim no session can be reached on (mu-184): beside the disabled Ping, the owner
// lets go of a bare-label claim in Building (back to Ready, worktree and branch kept) or of a bare
// hold in Testing, with a confirm in place.

const task = (over: Partial<Task>): Task => ({
  queue: "demo", id: "0007", title: "T", kind: "task", priority: "normal", status: "doing", stage: "building", blocked: false, stageHistory: [],
  folder: "doing", path: "doing/0007-t.md", claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [],
  attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "building", search: "", ...over,
});
const text = (html: string) => html.replace(/<\/?bdi>/g, "").replace(/<[^>]+>/g, " ").replace(/&#x27;/g, "'").replace(/\s+/g, " ").trim();
/** Each button as `label [aria-label] (disabled)`, in document (so Tab) order. */
const buttons = (html: string) =>
  [...html.matchAll(/<button([^>]*)>([^<]*)<\/button>/g)].map((m) => {
    const aria = /aria-label="([^"]*)"/.exec(m[1])?.[1].replace(/&#x27;/g, "'");
    return `${m[2]}${aria ? ` [${aria}]` : ""}${m[1].includes("disabled") ? " (disabled)" : ""}`;
  });
const STAMPED = "wt-a (wt-0101 · session 7b5e8856 · cc-mg)";
const a = { busy: false, pass: () => {}, release: () => {}, open: () => {}, unblock: () => {}, releaseUnreached: async () => true } as unknown as TaskActions;
const next = (over: Partial<Task>, actions = a, nowSecs = 0) => renderToStaticMarkup(createElement(NextStep, { task: task(over), nowSecs, a: actions }));

describe("which holder Release lets go of (mu-184, as ops::release_unreached)", () => {
  it("a claim in Building or a hold in Testing that names no session", () => {
    expect(unreachedHolder(task({ claimedBy: "Asghar_7" }))).toBe("Asghar_7");
    expect(unreachedHolder(task({ stage: "testing", heldBy: "qa-0007" }))).toBe("qa-0007");
    // A session typed by hand isn't a stamp: still no session to reach.
    expect(unreachedHolder(task({ claimedBy: "capture (drover/main, session 349bc9c3)" }))).toBe("capture (drover/main, session 349bc9c3)");
  });
  it("nothing for a stamped holder, the owner, nobody, or another stage", () => {
    expect(unreachedHolder(task({ claimedBy: STAMPED }))).toBe("");
    expect(unreachedHolder(task({ stage: "testing", heldBy: STAMPED }))).toBe("");
    expect(unreachedHolder(task({ claimedBy: "owner" }))).toBe("");
    expect(unreachedHolder(task({ claimedBy: "" }))).toBe("");
    // Testing's holder is held-by: a claim left over from Building isn't it.
    expect(unreachedHolder(task({ stage: "testing", claimedBy: "Asghar_7" }))).toBe("");
    for (const stage of ["ready", "shaping", "your-check", "done"]) expect(unreachedHolder(task({ stage, claimedBy: "Asghar_7", heldBy: "Asghar_7" }))).toBe("");
  });
});

describe("the pane's Release on a claim no session can be reached on (mu-184)", () => {
  it("shows beside the next step's buttons only when the holder names no session, whatever the lease", () => {
    const html = next({ claimedBy: "Asghar_7", heartbeat: "2026-10-14 10:00" });
    expect(buttons(html)).toEqual(["Release [Release Asghar_7's claim]", "Send back", "Block"]);
    expect(text(html)).toContain("Asghar_7 holds it in Building");
    expect(text(html)).toContain(unreachedWords("building").note);
    // A claim with a session is Ping's: no Release while its lease runs.
    expect(buttons(next({ claimedBy: STAMPED, heartbeat: "2026-10-14 10:00" }))).toEqual(["Send back", "Block"]);
    expect(buttons(next({ claimedBy: "owner" }))).toEqual(["Pass to Testing", "Send back", "Block"]);
  });
  it("one Release when the lease ran out too: this one, which needs no lease", () => {
    // Quiet for 13 days, past any lease.
    const late = new Date(2026, 9, 14, 12, 0).getTime() / 1000;
    const quiet = { heartbeat: "2026-10-01 09:00", modified: 0, stageSince: "2026-10-01 09:00" };
    // The control: a stamped claim this quiet gets the lease's Release, so the lease has run out.
    const stamped = next({ ...quiet, claimedBy: STAMPED }, a, late);
    expect(buttons(stamped).filter((b) => b.startsWith("Release"))).toEqual(["Release"]);
    expect(text(stamped)).toContain("hasn't been heard from");
    const html = next({ ...quiet, claimedBy: "Asghar_7" }, a, late);
    expect(buttons(html).filter((b) => b.startsWith("Release"))).toEqual(["Release [Release Asghar_7's claim]"]);
    expect(text(html)).not.toContain("hasn't been heard from");
  });
  it("in Testing, for a hold that names no session, beside the pass", () => {
    const html = next({ stage: "testing", heldBy: "qa-0007" });
    expect(buttons(html)).toEqual(["Pass to Your check", "Release [Release qa-0007's hold]", "Send back", "Block"]);
    expect(text(html)).toContain(unreachedWords("testing").note);
    expect(buttons(next({ stage: "testing", heldBy: STAMPED })).filter((b) => b.startsWith("Release"))).toEqual([]);
  });
  it("is off while another change runs, and while the task is paused, with why", () => {
    expect(buttons(next({ claimedBy: "Asghar_7" }, { ...a, busy: true } as TaskActions))[0]).toBe("Release [Release Asghar_7's claim] (disabled)");
    const html = next({ claimedBy: "Asghar_7", paused: "2026-10-14 10:00" });
    expect(buttons(html)[0]).toBe("Release [Release Asghar_7's claim] (disabled)");
    // Why it's off is on screen, not only in the hover title.
    expect(text(html)).toContain("Paused: resume it to release it.");
    expect(text(next({ claimedBy: "Asghar_7" }))).not.toContain("Paused:");
  });
});

describe("Release's confirm, in place (mu-184)", () => {
  const view = (over: Partial<Parameters<typeof ReleaseUnreachedView>[0]>) =>
    renderToStaticMarkup(createElement(ReleaseUnreachedView, { stage: "building", who: "Asghar_7", asked: false, busy: false, paused: false, onAsk: () => {}, onConfirm: () => {}, onCancel: () => {}, ...over }));
  it("is a labelled group in the pane, no dialog: the question, then Release and Cancel, both Tab stops", () => {
    const html = view({ asked: true });
    expect(html).toMatch(/^<span class="release-confirm" role="group" aria-label="Confirm Release" data-own-esc="" tabindex="-1">/);
    expect(html).not.toMatch(/role="dialog"|aria-modal/);
    // The group takes the focus only while its buttons are off (the release runs); the buttons
    // themselves are Tab stops.
    expect(html.match(/tabindex="-1"/g)).toHaveLength(1);
    expect(text(html)).toBe("Release Asghar_7's claim? It goes back to Ready; its worktree and branch stay. Release Cancel");
    expect(buttons(html)).toEqual(["Release [Release Asghar_7's claim]", "Cancel"]);
    // Real buttons, so Tab reaches each and Enter or Space presses the one with the focus.
    expect(html.match(/<button type="button"/g)).toHaveLength(2);
    // The focus starts on the confirm's Release, so Enter confirms; never on Cancel.
    const [release, cancel] = [...html.matchAll(/<button([^>]*)>/g)].map((m) => m[1]);
    expect(release).toContain('autofocus=""');
    expect(cancel).not.toContain("autofocus");
    // The question is read with the focused Release.
    const id = /<span class="release-ask" id="([^"]+)"/.exec(html)?.[1];
    expect(id).toBeTruthy();
    expect(release).toContain(`aria-describedby="${id}"`);
  });
  it("puts the holder's name in its own <bdi>, so a Persian label reads right", () => {
    const html = view({ asked: true, who: "اصغر_۷" });
    expect(html).toContain("Release <bdi>اصغر_۷</bdi>&#x27;s claim?");
  });
  it("says what Testing's Release does", () => {
    expect(text(view({ asked: true, stage: "testing", who: "qa-0007" }))).toBe("Release qa-0007's hold? It stays in Testing for the next tester. Release Cancel");
  });
  it("is off while the release runs", () => {
    expect(buttons(view({ asked: true, busy: true }))).toEqual(["Releasing… [Release Asghar_7's claim] (disabled)", "Cancel (disabled)"]);
  });
  it("calls back for each button", () => {
    const calls: string[] = [];
    const el = ReleaseUnreachedView({ stage: "building", who: "Asghar_7", asked: true, busy: false, paused: false, onAsk: () => calls.push("ask"), onConfirm: () => calls.push("confirm"), onCancel: () => calls.push("cancel") });
    const kids = (el.props as { children: { props: { onClick?: () => void } }[] }).children;
    for (const k of kids) k.props.onClick?.();
    expect(calls).toEqual(["confirm", "cancel"]);
    const button = ReleaseUnreachedView({ stage: "building", who: "Asghar_7", asked: false, busy: false, paused: false, onAsk: () => calls.push("ask"), onConfirm: () => {}, onCancel: () => {} });
    (button.props as { onClick: () => void }).onClick();
    expect(calls).toEqual(["confirm", "cancel", "ask"]);
  });
});

describe("Release's words, one string each (review R10, DESIGN.md › Languages)", () => {
  it("the question and the label are whole sentences with a {{who}} placeholder", () => {
    for (const stage of ["building", "testing"]) {
      const w = unreachedWords(stage);
      expect(typeof w.ask).toBe("string");
      expect(w.ask.split("{{who}}")).toHaveLength(2);
      expect(w.label.split("{{who}}")).toHaveLength(2);
    }
    expect(unreachedWords("building").ask).toBe("Release {{who}}'s claim? It goes back to Ready; its worktree and branch stay.");
  });
  it("fills the name in as is, $ patterns and all", () => {
    expect(whoIn("Release {{who}}'s claim", "a$&b")).toBe("Release a$&b's claim");
  });
});

describe("one Esc on Release's confirm is the confirm's alone (review R4)", () => {
  /** An element under (or not under) the confirm, as `closest` sees it. */
  const node = (inConfirm: boolean) => ({ closest: (sel: string) => (inConfirm && sel === `[${OWN_ESC}]` ? {} : null) });
  it("a key pressed inside the confirm's group is owned there; one outside, or with no element, isn't", () => {
    expect(escOwned(node(true) as unknown as EventTarget)).toBe(true);
    expect(escOwned(node(false) as unknown as EventTarget)).toBe(false);
    expect(escOwned(null)).toBe(false);
    expect(escOwned(new EventTarget())).toBe(false);
  });
  it("the confirm's group carries the mark", () => {
    const html = renderToStaticMarkup(createElement(ReleaseUnreachedView, { stage: "building", who: "Asghar_7", asked: true, busy: false, paused: false, onAsk: () => {}, onConfirm: () => {}, onCancel: () => {} }));
    expect(html).toContain(`${OWN_ESC}=""`);
  });
  it("an unsaved write-up's and a typed comment's window Esc leave an owned Esc alone (a source check)", () => {
    for (const [name, src] of [["EditBody.tsx", editBodyTsx], ["Comments.tsx", commentsTsx]]) {
      expect(src, name).toMatch(/if \(e\.key !== "Escape" \|\| escOwned\(e\.target\)\) return;/);
    }
  });
});

describe("Release's confirm on the window, driven by real key events (review R5)", () => {
  const key = (k: string) => Object.assign(new Event("keydown", { cancelable: true }), { key: k, metaKey: false, ctrlKey: false });
  it("Esc in the confirm cancels it, ahead of the window's other listeners, and goes no further", () => {
    const win = new EventTarget();
    const log: string[] = [];
    const stop = watchReleaseEscape(win, () => true, () => false, () => log.push("cancel"));
    win.addEventListener("keydown", () => log.push("window's own Esc"));
    const e = key("Escape");
    win.dispatchEvent(e);
    expect(log).toEqual(["cancel", "window's own Esc"]);
    expect(e.defaultPrevented).toBe(true);
    // Once the confirm closes, Esc is the window's again.
    stop();
    win.dispatchEvent(key("Escape"));
    expect(log).toEqual(["cancel", "window's own Esc", "window's own Esc"]);
  });
  it("does nothing while the release runs, nor for an Esc pressed elsewhere, nor for Enter", () => {
    const win = new EventTarget();
    const log: string[] = [];
    let busy = true;
    let mine = true;
    watchReleaseEscape(win, () => mine, () => busy, () => log.push("cancel"));
    win.dispatchEvent(key("Escape"));
    busy = false;
    mine = false;
    win.dispatchEvent(key("Escape"));
    mine = true;
    win.dispatchEvent(key("Enter"));
    expect(log).toEqual([]);
  });
  it("the pane's Release watches the window while its confirm is open, and puts the focus back (a source check)", () => {
    // The effect that attaches it, keyed on the confirm being open.
    expect(sidebarTsx).toMatch(/if \(!open\) return;\s*return watchReleaseEscape\(window, /);
    // Cancel or Esc: back on Release once it's drawn again; a release that went through: on the
    // Next step, which stays when Release goes with the claim (review R6).
    expect(sidebarTsx).toMatch(/if \(back === "button"\) button\.current\?\.focus\(\);/);
    expect(sidebarTsx).toMatch(/if \(ok\) nextStepOf\(group\.current\)\?\.focus\(\);/);
  });
  it("the Next step can take the focus a release hands it (review R6)", () => {
    const html = next({ claimedBy: "Asghar_7" });
    expect(html).toMatch(/^<div class="next-step" role="group" aria-label="Next step" data-part="next-step" tabindex="-1">/);
  });
});

describe("Release's confirm and the keyboard (mu-184)", () => {
  /** A key pressed while the confirm is open, as the browser runs it: the confirm's capture
   *  listener on the window first, then the window's own shortcuts unless it stopped the key. */
  function press(key: string, busy: boolean, mine = true) {
    const log: string[] = [];
    let stopped = false;
    const e = { key, metaKey: false, ctrlKey: false, stopPropagation: () => (stopped = true), preventDefault: () => {} };
    releaseEscape(e, mine, busy, () => log.push("cancel"));
    if (!stopped) windowKey(e, { closePane: () => log.push("close pane"), newTask: () => log.push("new task"), search: () => log.push("search"), focusMode: () => log.push("focus") });
    return { log, stopped };
  }
  it("Esc cancels the confirm and the pane stays open", () => {
    expect(press("Escape", false)).toEqual({ log: ["cancel"], stopped: true });
  });
  it("Esc while it runs does nothing, and still doesn't close the pane", () => {
    expect(press("Escape", true)).toEqual({ log: [], stopped: true });
  });
  it("an Esc pressed elsewhere (a rename, a field's choices) is that field's, not the confirm's", () => {
    expect(press("Escape", false, false)).toEqual({ log: ["close pane"], stopped: false });
  });
  it("Enter goes on to the focused button, the confirm's Release", () => {
    expect(press("Enter", false)).toEqual({ log: [], stopped: false });
  });
});
