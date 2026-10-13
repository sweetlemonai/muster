/** One of the criteria only the owner verifies ("— verify: owner"), from the core (0028). */
export type Criterion = { n: number; label: string; text: string; ticked: boolean; owner: boolean };

export type Question = {
  n: number | null;
  ticked: boolean;
  asker: string;
  text: string;
  escalated: boolean;
  ownerAnswered: boolean;
};

/** An external link from the task's `links:` line; `title` is "" when it has none. */
export type Link = { title: string; url: string };

/** One comment from the task's `## Comments`, as written. `at` is "YYYY-MM-DD HH:MM". */
export type Comment = { who: string; at: string; text: string };

/** One stage change from the task's log, written by muster. `at` is "YYYY-MM-DD HH:MM". */
export type StageStep = { at: string; who: string; from: string; to: string };

/** A holder's check-in from the log (0030). `pct` is the session's own estimate, 0–100, or null. */
export type Checkin = { at: string; who: string; pct: number | null; text: string };

export type Task = {
  queue: string;
  id: string;
  title: string;
  kind: string;
  priority: string;
  /** The old status, kept as a mirror of the stage for older tools. */
  status: string;
  /** One of STAGES (lib.ts); the core maps old files without one. */
  stage: string;
  /** Waiting on something (`waitingOn`), in any stage. */
  blocked: boolean;
  stageHistory: StageStep[];
  folder: string;
  path: string;
  claimedBy: string;
  /** Who holds it in Testing or Your check (`held-by:`), as Building has `claimedBy`. */
  heldBy: string;
  /** When the current stage began (the latest stage line into it), or "". */
  stageSince: string;
  /** Every check-in in the log, oldest first. */
  checkins: Checkin[];
  heartbeat: string;
  waitingOn: string;
  /** The tasks it waits on (`depends:`, same queue, 0019). */
  depends: string[];
  /** Who does it (`for:`, 0022): "human" is the owner's own task, "ai" an agent's (every older one). */
  forWho: string;
  commit: string;
  captured: string;
  /** When it was captured to the minute, when the log knows it (0031); else empty. */
  capturedAt: string;
  /** When it was moved to Done, to the minute, from the log; empty when not known. */
  doneAt: string;
  /** When it was blocked: `from`/`to` stamps, `to` empty while it still is (0031). */
  blockHistory: { from: string; to: string }[];
  questions: Question[];
  openQuestions: number;
  ownerCriteria: Criterion[];
  /** The vision check (0029): open conflicts, or null before the first check. Over 0, it can't
   *  pass from Shaping to Ready. */
  visionOpen?: number | null;
  /** The open conflicts' ids (V1, V2…), once checked. */
  visionIds?: string[];
  /** A `vision:` value that isn't a count ("" when fine): a problem the core reports, so the task
   *  isn't also listed as unchecked. */
  badVision?: string;
  /** It waits on the owner (the core's needs_owner): Your check, a question escalated to them, or
   *  blocked on the owner. An agent's task like this is in the Human board's "From the agents". */
  waitingForOwner: boolean;
  tags: string[];
  /** Estimates: agent time, test time (the QA agent plus the owner's check) and tokens, as written
   *  ("2h", "30m", "400k"), and as numbers for totals. */
  estTime: string;
  estTestTime: string;
  estTokens: string;
  estMinutes: number;
  estTestMinutes: number;
  estTokenCount: number;
  claimedOn: string;
  doneOn: string;
  /** External links, in the order written (0017). */
  links: Link[];
  /** The comment thread, oldest first (0017). */
  comments: Comment[];
  attachments: string[];
  logTail: string[];
  modified: number;
  /** "goal", "feature" or "task" (decision 0003). Only tasks go on the board's stage columns. */
  level: string;
  /** The id of the feature or goal it sits under, in the same queue, or "". */
  parent: string;
  /** A goal's date, "YYYY-MM-DD", or "". */
  due: string;
  /** Goals and features: the ids under them. */
  children: string[];
  /** Goals and features: their tasks' progress and open estimates; null for a task. */
  progress: Progress | null;
  /** The stage to show: a task's own; a goal's or feature's from its tasks ("in-progress" too). */
  shownStage: string;
  /** What search looks in, built by the core when it reads the file (search.rs): the title, tags
   *  and text without the front matter, folded (search.ts `fold`), each word once, no words of
   *  digits only, at most 4 KB. Not in the polled state: the window adds it from the core's
   *  `search_index` while it searches (search.ts `withSearch`). */
  search?: string;
  /** One of the task's sessions changed its transcript in the last 5 minutes (0025). */
  live: boolean;
  /** Where in the repo it works (`touches:`, 0023): folders and globs. */
  touches?: string[];
  /** A task `muster start` began (0023): its git worktree and branch, until the merge clears them. */
  worktree?: string;
  branch?: string;
  /** After the merge: the report from its log ("merged task/… into main as …"), else "". */
  mergeReport?: string;
};

/** Tokens a session used, by kind (0025). */
export type Tokens = { input: number; output: number; cacheRead: number; cacheWrite: number };

/** One session that worked on a task, and what its transcript on this Mac says (0025). Times are
 *  seconds since 1970; `found` says whether the transcript was read. */
export type SessionRow = {
  id: string;
  /** The label it acted under ("muster/wt-0025"). */
  who: string;
  /** Stages it held, in order. */
  stages: string[];
  /** It holds the task now. */
  holds: boolean;
  found: "read" | "missing" | "too-big";
  tokens: Tokens;
  start: number | null;
  end: number | null;
  minutes: number;
  live: boolean;
  skippedLines: number;
  skippedFiles: number;
  /** How many other tasks name this session; its cost counts in full on each. */
  sharedWith: number;
};

/** The task pane's Work section (0025): sessions, totals against the estimates, a progress guess. */
export type Work = {
  rows: SessionRow[];
  totals: { sessions: number; read: number; live: number; shared: number; tokens: Tokens; totalTokens: number; minutes: number };
  /** The estimates as minutes and tokens, and the totals as a share of each (null without one). */
  estimate: { minutes: number; tokens: number; minutesPct: number | null; tokensPct: number | null };
  guess: { pct: number; basis: string };
};

/** A backlog note (0015): a one-liner, or any .md dropped in the queue's backlog folder. */
export type Note = {
  queue: string;
  /** Its file name in the backlog folder: what file, drop and shape take. */
  file: string;
  /** The first non-empty line; "" for an empty note (the window shows "(empty note)"). */
  title: string;
  /** The text, up to 20 KB. */
  text: string;
  /** The file is longer than `text`. */
  truncated: boolean;
  /** When the file last changed, in seconds since 1970. */
  added: number;
};

/** A goal's or feature's tasks: done of total, and the estimates of the open ones, summed. */
export type Progress = { done: number; total: number; estMinutes: number; estTokenCount: number };

export type Activity = { queue: string; at: number; what: string; source: string };

/** `counts` is tasks per stage, plus "blocked" (tasks also counted in their stage). */
export type QueueInfo = { name: string; path: string; counts: Record<string, number> };

export type State = {
  queues: QueueInfo[];
  tasks: Task[];
  /** Every queue's backlog notes, newest first. */
  backlog: Note[];
  problems: string[];
  activity: Activity[];
  now: number;
};

export type Project = {
  key: string;
  name: string;
  path: string;
  source: "found" | "added";
  hidden: boolean;
  ok: boolean;
};
