import { describe, expect, it } from "vitest";
import {
  acceptAllRun, byEffort, criterionEffort, cutText, EFFORTS, justAccept, namedViews, nextStep, refusalText, staleResult, stripEffort, taskEffort, tryText,
} from "./yourCheck";
import type { Criterion, Question, Task } from "./types";

const q = (n: number, text: string, over: Partial<Question> = {}): Question => ({ n, ticked: false, asker: "wt-a", text, escalated: true, ownerAnswered: false, ...over });

const crit = (n: number, ticked: boolean, words: string): Criterion => ({ n, label: `AC${n}`, text: `AC${n}: ${words}`, ticked, owner: true });
const task = (id: string, over: Partial<Task> = {}): Task => ({
  queue: "muster", id, title: `Task ${id}`, kind: "code", priority: "normal", status: "doing", stage: "your-check", blocked: false, stageHistory: [],
  folder: "doing", path: `doing/${id}-t.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "", blockHistory: [], questions: [],
  openQuestions: 0, live: false, ownerCriteria: [], waitingForOwner: true, tags: [], estTime: "", estTestTime: "", estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "", due: "", children: [], progress: null, shownStage: over.stage ?? "your-check", ...over,
});

describe("a criterion's effort (0043)", () => {
  it("is a session when it mentions a session, a terminal or a command, else one look", () => {
    expect(criterionEffort("AC1: every row routes to a working command. — verify: owner (tries each in a session)")).toBe("session");
    expect(criterionEffort("AC2: the owner runs it in a Terminal — verify: owner")).toBe("session");
    expect(criterionEffort("AC3: the slash commands answer — verify: owner")).toBe("session");
    expect(criterionEffort("AC4: View shows a session's transcript — verify: owner")).toBe("session");
    expect(criterionEffort("AC7: the owner groups the board by Feature — verify: owner")).toBe("look");
    // Words that only contain one of them don't count.
    expect(criterionEffort("AC1: commandeers nothing; the sessionless view and terminally fine — verify: owner")).toBe("look");
  });
  it("can be overridden with — effort: look|session, before or after the verify part", () => {
    expect(criterionEffort("AC4: View shows a session's transcript — verify: owner — effort: look")).toBe("look");
    expect(criterionEffort("AC1: opens INDEX.md — effort: session — verify: owner")).toBe("session");
    expect(criterionEffort("AC1: a command — verify: owner — Effort: LOOK")).toBe("look");
    // Anything else after effort: isn't an override.
    expect(criterionEffort("AC1: a command — verify: owner — effort: lots")).toBe("session");
  });
  it("drops the override from the words", () => {
    expect(stripEffort("AC1: opens it — effort: session — verify: owner")).toBe("AC1: opens it — verify: owner");
    expect(stripEffort("AC1: opens it — verify: owner — effort: look")).toBe("AC1: opens it — verify: owner");
    expect(stripEffort("no tag")).toBe("no tag");
  });
});

describe("a task's effort and the groups", () => {
  it("is Just accept with no open owner criteria, Needs a session when any open one does, else One look", () => {
    expect(taskEffort(task("0001"))).toBe("accept");
    expect(taskEffort(task("0001", { ownerCriteria: [crit(1, true, "run a command — verify: owner")] }))).toBe("accept");
    expect(taskEffort(task("0001", { ownerCriteria: [crit(1, false, "look at it — verify: owner")] }))).toBe("look");
    expect(taskEffort(task("0001", { ownerCriteria: [crit(1, false, "look at it — verify: owner"), crit(2, false, "run it in a terminal — verify: owner")] }))).toBe("session");
    expect(taskEffort(task("0001", { ownerCriteria: [crit(1, false, "run it in a terminal — verify: owner — effort: look")] }))).toBe("look");
  });
  it("groups in order Just accept, One look, Needs a session, keeping each one's order", () => {
    expect(EFFORTS.map((e) => e.label)).toEqual(["Just accept", "One look", "Needs a session"]);
    const ts = [
      task("0005", { ownerCriteria: [crit(1, false, "in a session — verify: owner")] }),
      task("0003"),
      task("0004", { ownerCriteria: [crit(1, false, "look — verify: owner")] }),
      task("0002"),
    ];
    const g = byEffort(ts);
    expect(Object.fromEntries(Object.entries(g).map(([k, v]) => [k, v.map((t) => t.id)]))).toEqual({ accept: ["0003", "0002"], look: ["0004"], session: ["0005"] });
  });
  it("Accept all takes exactly the Just-accept tasks in Your check that aren't blocked", () => {
    const ts = [
      task("0001"),
      task("0002", { ownerCriteria: [crit(1, true, "look — verify: owner")] }),
      task("0003", { ownerCriteria: [crit(1, false, "look — verify: owner")] }),
      task("0004", { blocked: true, waitingOn: "owner: pick", blockedKind: "you" }),
      task("0005", { stage: "testing", waitingForOwner: false }),
      task("0006", { stage: "done", waitingForOwner: false }),
      // A goal or feature put in Your check by hand: a single Accept never offers it (review, round 1).
      task("0007", { level: "goal" }),
      task("0008", { level: "feature" }),
      // An open question escalated to the owner is the owner's work; an answered one, or one only for an agent, isn't.
      task("0009", { questions: [q(1, "Which colour?")] }),
      task("0010", { questions: [q(1, "Which colour?", { ticked: true })] }),
      task("0011", { questions: [q(1, "Which colour?", { escalated: false })] }),
    ];
    expect(justAccept(ts).map((t) => t.id)).toEqual(["0001", "0002", "0010", "0011"]);
  });
  it("counts an open escalated question or a block on the owner as owner work: at least One look", () => {
    expect(taskEffort(task("0001", { questions: [q(1, "Which colour?")] }))).toBe("look");
    expect(taskEffort(task("0001", { blocked: true, blockedKind: "you", blockDetail: "pick" }))).toBe("look");
    expect(taskEffort(task("0001", { blocked: true, blockedKind: "permission", blockDetail: "repo write" }))).toBe("look");
    expect(taskEffort(task("0001", { questions: [q(1, "x")], ownerCriteria: [crit(1, false, "run it in a terminal")] }))).toBe("session");
    // A block by tasks or by something outside isn't the owner's to act on here.
    expect(taskEffort(task("0001", { blocked: true, blockedKind: "outside", blockDetail: "Apple" }))).toBe("accept");
  });
});

describe("the next step when the owner has to answer or unblock", () => {
  it("says Answer Qn with its first line, before any criterion", () => {
    const t = task("0001", { questions: [q(2, "\nChild webview or a window?\nMore."), q(3, "Later?")], ownerCriteria: [crit(1, false, "look")] });
    expect(nextStep(t)).toEqual({ kind: "question", text: "Answer Q2: Child webview or a window?", full: "Answer Q2: Child webview or a window?", more: 2 });
  });
  it("says Unblock with what it waits on, when blocked on the owner or by a permission", () => {
    expect(nextStep(task("0001", { blocked: true, blockedKind: "you", blockDetail: "pick a colour" }))).toEqual({ kind: "block", text: "Unblock: pick a colour", full: "Unblock: pick a colour", more: 0 });
    expect(nextStep(task("0001", { blocked: true, blockedKind: "permission", blockDetail: "", ownerCriteria: [crit(1, false, "look")] })).text).toBe("Unblock: Permission");
  });
  it("ignores answered questions and ones only for an agent", () => {
    expect(nextStep(task("0001", { questions: [q(1, "x", { ticked: true }), q(2, "y", { escalated: false })] })).kind).toBe("accept");
  });
});

describe("the core's refusals, as the app says them", () => {
  it("drops the leading id and the command-line hints", () => {
    expect(refusalText("0003", "0003: 1 of the owner's criteria aren't ticked; tick each once you've checked it (muster tick 0003 <AC>), then pass it to Done")).toBe(
      "1 of the owner's criteria aren't ticked; tick its criteria first",
    );
    expect(refusalText("0004", "0004 is now in Testing, not Your check; it moved on since you looked")).toBe("it is now in Testing, not Your check; it moved on since you looked");
    expect(refusalText("0003", "0003 has a question for the owner (Q1); answer it first")).toBe("it has a question for the owner (Q1); answer it first");
    expect(refusalText("0005", "no queue named \"x\"")).toBe("no queue named \"x\"");
  });
});

describe("when Accept all's result line goes", () => {
  const r = (over: Partial<Parameters<typeof staleResult>[0]> = {}) => ({ ok: true, text: "Accepted 2.", refused: [], known: ["muster/0001", "muster/0002", "muster/0003"], at: 100, ...over });
  it("stays while the tasks it accepted leave and nothing new comes in, so it outlives an emptied column", () => {
    expect(staleResult(r(), [task("0003")])).toBe(false);
    expect(staleResult(r(), [])).toBe(false);
  });
  it("goes on the next change: a task new to Your check here", () => {
    expect(staleResult(r(), [task("0003"), task("0004")])).toBe(true);
  });
  it("goes once a task it refused has changed since (a criterion ticked, a question answered, unblocked)", () => {
    const refused = r({ ok: false, refused: ["muster/0003"] });
    expect(staleResult(refused, [task("0003", { modified: 90 })])).toBe(false);
    expect(staleResult(refused, [task("0003", { modified: 101 })])).toBe(true);
    // One that had already moved on when it was refused: the line says why, until the next change.
    expect(staleResult(refused, [])).toBe(false);
  });
});

describe("the next step a card names", () => {
  it("says Nothing to try: Accept when no owner criterion is open", () => {
    expect(nextStep(task("0001"))).toEqual({ kind: "accept", text: "Nothing to try: Accept", full: "Nothing to try: Accept", more: 0 });
    expect(nextStep(task("0001", { ownerCriteria: [crit(1, true, "x — verify: owner")] })).text).toBe("Nothing to try: Accept");
  });
  it("turns the first open criterion into an instruction, and counts the rest", () => {
    const t = task("0016", { ownerCriteria: [crit(6, true, "done — verify: owner"), crit(7, false, "the owner groups the board by Feature and checks a goal's progress. — verify: owner"), crit(8, false, "x — verify: owner")] });
    expect(nextStep(t)).toEqual({ kind: "criterion", text: "Try: group the board by Feature and check a goal's progress", full: "Try: group the board by Feature and check a goal's progress", more: 1 });
    expect(nextStep(task("0020", { ownerCriteria: [crit(4, false, "a comment added in the app shows in the thread. — verify: owner")] })).text).toBe("Check: a comment added in the app shows in the thread");
  });
  it("cuts a long one on the card and keeps it whole in full", () => {
    const words = "the owner looks at the sidebar on the real queue: the working dots, the last activity and the order chips read right, in every theme there is";
    const s = nextStep(task("0037", { ownerCriteria: [crit(4, false, `${words}. — verify: owner`)] }));
    expect(s.full).toBe(`Try: look at the sidebar on the real queue: the working dots, the last activity and the order chips read right, in every theme there is`);
    expect(s.text.length).toBeLessThanOrEqual(90);
    expect(s.text.endsWith("…")).toBe(true);
    expect(s.full.startsWith(s.text.slice(0, -1))).toBe(true);
  });
});

describe("tryText", () => {
  it("makes the owner's sentence an instruction", () => {
    expect(tryText("the owner adds a one-liner in the app and files it.")).toBe("Try: add a one-liner in the app and file it");
    expect(tryText("The owner opens INDEX.md and finds it useful.")).toBe("Try: open INDEX.md and find it useful");
    expect(tryText("the owner tries each and pushes the fix")).toBe("Try: try each and push the fix");
    expect(tryText("the owner adds a comment and a link and opens the link")).toBe("Try: add a comment and a link and open the link");
    // Not every word after "and" is a verb.
    expect(tryText("the owner groups it and its parts and this")).toBe("Try: group it and its parts and this");
    expect(tryText("the owner's screenshots pass the check")).toBe("Check: the owner's screenshots pass the check");
    expect(tryText("")).toBe("Check: (no words)");
  });
  it("cutText keeps short text and cuts long text on a word", () => {
    expect(cutText("short", 10)).toBe("short");
    expect(cutText("one two three four", 10)).toBe("one two…");
    expect(cutText("abcdefghijklmnop", 10)).toBe("abcdefghi…");
  });
});

describe("the views a criterion names", () => {
  it("finds each view by name, once, in the app's order", () => {
    expect(namedViews("the Graph tab shows the chain and the Timeline too, Graph again")).toEqual(["timeline", "graph"]);
    expect(namedViews("the owner groups the board by Feature")).toEqual(["board"]);
    expect(namedViews("Appearance shows the card looks; Help opens the manual")).toEqual(["help", "appearance"]);
    expect(namedViews("Waiting for you lists it, and Problems and Activity and Projects and the Helper and the Table")).toEqual([
      "table", "helper", "waiting", "activity", "problems", "projects",
    ]);
  });
  it("finds Settings (0036) by its name, after Help, and not the everyday word", () => {
    expect(namedViews("Settings shows the lease; Help opens the manual")).toEqual(["help", "settings"]);
    expect(namedViews("the settings of a camera")).toEqual([]);
  });
  it("ignores everyday words that aren't the view's name", () => {
    expect(namedViews("it helps the projects; activity and problems show; a tabled motion; graphics")).toEqual([]);
  });
});

describe("Accept all, across projects", () => {
  it("asks the core once per project with exactly the ids shown, and sums what came back", async () => {
    const calls: [string, string[]][] = [];
    const ts = [task("0001"), task("0002", { queue: "drover" }), task("0003")];
    const r = await acceptAllRun(ts, async (queue, ids) => {
      calls.push([queue, ids]);
      return queue === "muster" ? { accepted: ["0001"], refused: [["0003", "0003 is now in Done, not Your check; it moved on since you looked"]] } : { accepted: ids, refused: [] as [string, string][] };
    });
    expect(calls).toEqual([["muster", ["0001", "0003"]], ["drover", ["0002"]]]);
    expect(r).toEqual({ ok: false, text: "Accepted 2. Not accepted: muster 0003: it is now in Done, not Your check; it moved on since you looked.", refused: ["muster/0003"] });
  });
  it("says how many when all went through, and a project the core can't reach without stopping the rest", async () => {
    expect(await acceptAllRun([task("0001"), task("0002")], async (_q, ids) => ({ accepted: ids, refused: [] }))).toEqual({ ok: true, text: "Accepted 2: they're done.", refused: [] });
    expect(await acceptAllRun([task("0001")], async (_q, ids) => ({ accepted: ids, refused: [] }))).toEqual({ ok: true, text: "Accepted 1: it's done.", refused: [] });
    const r = await acceptAllRun([task("0001", { queue: "gone" }), task("0002")], async (q, ids) => {
      if (q === "gone") throw "no queue named \"gone\"";
      return { accepted: ids, refused: [] };
    });
    expect(r).toEqual({ ok: false, text: "Accepted 1. Not accepted: gone: no queue named \"gone\".", refused: ["gone/0001"] });
  });
});
