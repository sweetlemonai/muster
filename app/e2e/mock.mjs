// Muster's window without its core, for headless checks (mu-143; mu-133 and mu-136 build on it).
// Dependency-free: plain Node, no packages. It turns a read-only snapshot of a queue (what
// `muster list --json --queue <name>` prints: its tasks and backlog notes) into the core's `state`
// answer (app/src/types.ts `State`), and gives a script that stands in for Tauri's IPC
// (`window.__TAURI_INTERNALS__`) inside the page: `state` answers with the snapshot, the window's
// harmless calls answer with nothing, and every command that would change something is refused,
// so a check can never write to a queue.

/** Commands the window makes on its own that only read or set the window up: answered with null. */
const QUIET = new Set(["show_project_title", "plugin:event|listen", "plugin:event|unlisten", "set_old_pauses_offered"]);

/**
 * The core's `state` from a `muster list --json` snapshot.
 * @param {{ tasks: object[], backlog?: object[] }} snap
 * @param {{ queue: string, label?: string, prefix?: string, limit?: number | null, now?: number }} o
 */
export function stateFrom(snap, o) {
  const tasks = (snap.tasks ?? []).map((t) => ({ ...t, queue: o.queue }));
  const counts = {};
  for (const t of tasks) counts[t.stage] = (counts[t.stage] ?? 0) + 1;
  counts.blocked = tasks.filter((t) => t.blocked).length;
  const yourCheck = tasks.filter((t) => t.stage === "your-check" && t.forWho !== "human").length;
  const prefix = o.prefix ?? tasks.find((t) => t.prefix)?.prefix ?? "";
  return {
    queues: [{ name: o.queue, path: `/snapshot/${o.queue}`, prefix, counts, yourCheck, yourCheckLimit: o.limit === undefined ? 5 : o.limit, label: o.label ?? o.queue, category: "Other", colour: "", rank: 0, categoryRank: 0, categoryFolded: false, lessonsWaiting: 0, running: false }],
    tasks,
    backlog: (snap.backlog ?? []).map((n) => ({ ...n, queue: o.queue })),
    problems: [],
    activity: [],
    now: o.now ?? Math.floor(Date.now() / 1000),
    settings: { leaseMinutes: 720, liveMinutes: 15, oldPausesOffered: true, claudeAccounts: ["mg", "hg"] },
  };
}

/**
 * The page script: Tauri's IPC answered from `state`, and `localStorage` seeded with `storage`
 * (the board's layout, the theme…) before the app's first line runs.
 * @param {object} state
 * @param {Record<string, string>} storage
 * @param {{ dir?: "ltr" | "rtl" }} [page]
 */
export function mockScript(state, storage = {}, page = {}) {
  return `(() => {
  const STATE = ${JSON.stringify(state)};
  const QUIET = new Set(${JSON.stringify([...QUIET])});
  const seed = ${JSON.stringify(storage)};
  for (const [k, v] of Object.entries(seed)) localStorage.setItem(k, v);
  ${page.dir === "rtl" ? 'document.addEventListener("readystatechange", () => document.documentElement.setAttribute("dir", "rtl"), { once: true });' : ""}
  const calls = [];
  window.__MOCK_CALLS__ = calls;
  let next = 1;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb, once) {
      const id = next++;
      window["_" + id] = (r) => { if (once) delete window["_" + id]; if (cb) cb(r); };
      return id;
    },
    unregisterCallback(id) { delete window["_" + id]; },
    convertFileSrc: (p) => p,
    async invoke(cmd, args) {
      calls.push(cmd);
      if (cmd === "state") return structuredClone(STATE);
      if (cmd === "search_index") return {};
      if (cmd === "task_body") {
        const t = STATE.tasks.find((x) => x.queue === args.queue && x.id === args.id);
        return t ? "## Vision\\n\\n(" + t.title + ": the task's text isn't in the snapshot.)\\n" : "";
      }
      if (QUIET.has(cmd)) return null;
      throw "Not in the snapshot: " + cmd + " (a headless check reads only)";
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
})();`;
}
