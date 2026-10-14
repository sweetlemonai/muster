import type { SessionState } from "./sessionState";

/** An acceptance criterion from the core (0028): `text` without its proof; `owner` when only the
 *  owner verifies it ("— verify: owner"). Its proof (mu-78) is the `— proof: …` tail its ticker
 *  wrote ("" when none), and `proofFiles` the task's own attachments that proof names. Optional,
 *  like the other later fields. */
export type Criterion = { n: number; label: string; text: string; ticked: boolean; owner: boolean; proof?: string; proofFiles?: string[] };

export type Question = {
  n: number | null;
  ticked: boolean;
  asker: string;
  text: string;
  escalated: boolean;
  ownerAnswered: boolean;
  /** Its options, when it was asked with them (0049); absent for a question in words. */
  choices?: Choices;
};

/** A pick-one question's options, (a) first, with the asker's pick and what it does with no
 *  answer as indexes into them; `question` is the question without the option lines. */
export type Choices = { options: string[]; pick: number | null; noAnswer: number | null; question: string };

/** An external link from the task's `links:` line; `title` is "" when it has none. */
export type Link = { title: string; url: string };

/** One comment from the task's `## Comments`, as written. `at` is "YYYY-MM-DD HH:MM". */
export type Comment = { who: string; at: string; text: string };

/** One stage change from the task's log, written by muster. `at` is "YYYY-MM-DD HH:MM". */
export type StageStep = { at: string; who: string; from: string; to: string };

/** A task another is blocked by (0024): its id, stage ("" when there's no such task) and who's on
 *  it (the label, without the session). */
export type BlockedBy = { id: string; stage: string; holder: string };

/** A holder's check-in from the log (0030). `pct` is the session's own estimate, 0–100, or null. */
export type Checkin = { at: string; who: string; pct: number | null; text: string };

export type Task = {
  queue: string;
  id: string;
  /** Its project's prefix (0033), for its key (`mu-12`, taskKey.ts); the core always sends it,
   *  and a task without one shows its id. */
  prefix?: string;
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
  /** Who built it (0047): the last claim's who (Ready → Building), or "". Optional, like the other
   *  later fields, so a task without it reads as built by nobody on record. */
  builtBy?: string;
  /** Who last passed it Building → Testing with the gate's line (0047), a builder too, or "". */
  gatedBy?: string;
  /** Sent back from Testing to Building (mu-66): its latest stage change. Optional, like the other
   *  later fields. */
  sentBack?: boolean;
  /** The id of the task its holder has to fix first (mu-66), or "": this one waits behind it. */
  behind?: string;
  /** Who passed it into Your check most recently: its tester, or the owner's override; or "". */
  testedBy?: string;
  /** When the current stage began (the latest stage line into it), or "". */
  stageSince: string;
  /** Every check-in in the log, oldest first. */
  checkins: Checkin[];
  /** When the session on it was last asked for a check-in (0039), "YYYY-MM-DD HH:MM", or "". The
   *  core always sends it; optional so a task without it reads as never asked. */
  checkinAsked?: string;
  heartbeat: string;
  waitingOn: string;
  /** Why it's blocked (0024): "tasks" (derived from `depends:`), "you", "permission", "question"
   *  or "outside"; "" when nothing blocks it. The core always sends these three; they're optional
   *  so a task without them reads as not blocked. */
  blockedKind?: string;
  /** What the block says (a question's number, what you have to do, …). */
  blockDetail?: string;
  /** The tasks it's blocked by, with their stage and who's on them. */
  blockedBy?: BlockedBy[];
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
  /** When it was blocked: `from`/`to` stamps, `to` empty while it still is (0031), and the
   *  block's kind ("" for a hand move, 0024). */
  blockHistory: { from: string; to: string; kind?: string }[];
  /** Paused by the owner (0045): since when ("YYYY-MM-DD HH:MM"), "" when it isn't, and why ("" for
   *  no reason). It keeps its stage and claim; nobody claims or passes it until it's resumed. The
   *  core always sends these; optional so a task without them reads as not paused. */
  paused?: string;
  pausedWhy?: string;
  /** When it was paused: `from`/`to` stamps, `to` empty while it still is. */
  pauseHistory?: { from: string; to: string }[];
  questions: Question[];
  openQuestions: number;
  ownerCriteria: Criterion[];
  /** Every acceptance criterion, in order, each with its proof (mu-78): Your check's "What you're
   *  accepting" and the card's "All N proven". The core sends it only for a task in Your check,
   *  its one reader; absent elsewhere and from an older core. */
  criteria?: Criterion[];
  /** What was built, in one plain line (mu-78, `summary:`), or "": shown as text, never Markdown. */
  summary?: string;
  /** The vision check (0029): open conflicts, or null before the first check. Over 0, it can't
   *  pass from Shaping to Ready. */
  visionOpen?: number | null;
  /** The open conflicts' ids (V1, V2…), once checked. */
  visionIds?: string[];
  /** A `vision:` value that isn't a count ("" when fine): a problem the core reports, so the task
   *  isn't also listed as unchecked. */
  badVision?: string;
  /** The review of record (0027), from the front matter's `review:`: its last round (null before
   *  the first), the findings its builder still has to answer, and those answered and waiting for
   *  the next round. Optional so a task from an older core reads as not reviewed. */
  reviewRound?: number | null;
  reviewOpen?: number;
  reviewAnswered?: number;
  badReview?: string;
  /** It waits on the owner (the core's needs_owner): Your check, a question escalated to them, or
   *  blocked on the owner. On the board it's a needs-you card, and it counts in `needsYouCount`. */
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
  /** What Claude Code says its holder's session is doing (0048, sessionState.ts); unknown when
   *  absent. "needs-input" also sets `waitingForOwner`. */
  sessionState?: SessionState;
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

/** `counts` is tasks per stage, plus "blocked" (tasks also counted in their stage). `prefix` is
 *  what its tasks' keys start with (0033). `yourCheck` is how many tasks wait in Your check as its
 *  limit counts them (0050: agents' tasks only), and `yourCheckLimit` that limit (projects.json's
 *  `yourCheck` for its folder, check_limit.rs), null when it's off; an older core sends neither,
 *  which reads as within it. */
export type QueueInfo = {
  name: string;
  path: string;
  prefix?: string;
  counts: Record<string, number>;
  yourCheck?: number;
  yourCheckLimit?: number | null;
  /** What the window calls it (0044): its label, else its name. The core always sends these two;
   *  optional so a queue without them is called by its name, under Other. */
  label?: string;
  /** The sidebar heading it goes under (0044): its category, its group, else "Other". */
  category?: string;
  /** Its colour (0057), one of themes.ts PROJECT_COLOURS' ids, "" for none. These three are
   *  optional so an older core reads as no colour and no place. */
  colour?: string;
  /** Its place in the sidebar's Mine order (0057), null when it has none yet. */
  rank?: number | null;
  /** Its heading's place among the sidebar's headings (0057), null when it has none yet. */
  categoryRank?: number | null;
  /** Its heading is folded in the sidebar (0070); optional so an older core reads as open. */
  categoryFolded?: boolean;
  /** How many lessons from reviews wait for the owner's yes (0027); optional so an older core
   *  reads as none. */
  lessonsWaiting?: number;
};

export type State = {
  queues: QueueInfo[];
  tasks: Task[];
  /** Every queue's backlog notes, newest first. */
  backlog: Note[];
  problems: string[];
  activity: Activity[];
  now: number;
  /** This machine's settings the core judged by (0036), in minutes; read with settingsData's
   *  machineOf, which falls back to the defaults. oldPausesOffered: the board offered to convert
   *  old pauses once (mu-59); undefined from a core that doesn't say. */
  settings?: { leaseMinutes: number; liveMinutes: number; oldPausesOffered?: boolean };
};

export type Project = {
  key: string;
  name: string;
  path: string;
  /** Found on its own (`~/playground/<project>/tasks`, or one level deeper), or added by the
   *  owner. A queue that is both is found. */
  source: "found" | "added";
  /** The folder holding a found queue's project one level deeper (`persianUtils` for
   *  `~/playground/persianUtils/colors/tasks`); "" for the rest. */
  group: string;
  hidden: boolean;
  ok: boolean;
  /** The project's gate command (0023), "" when none is set; set with `muster projects set`. */
  gate?: string;
  /** Its run command (0077), "" when none is set; set with `muster projects set`, as the gate is. */
  run?: string;
  /** What its tasks' keys start with (0033): stored with its queue, or its default. The core
   *  always sends it. */
  prefix?: string;
  /** Its label (0044) as set, "" for none (it shows by its name). The core always sends these two. */
  label?: string;
  /** Its category (0044) as set, "" for none (it goes under its group, else Other). */
  category?: string;
  /** Its colour (0057), "" for none. */
  colour?: string;
  /** Its group's display name (0070), "" for none: its heading when it has no category. */
  groupName?: string;
};

/** A lesson from reviews (0027): a kind of mistake that recurred, as a rule for builders, or one
 *  that kept proving false, as something reviewers don't flag. The core's `lessons::Lesson`. */
export type Lesson = {
  id: string;
  kind: string;
  for: "builders" | "reviewers";
  state: "proposed" | "accepted" | "rejected" | "expired";
  rule: string;
  bad: string;
  good: string;
  areas: string[];
  from: string[];
  proposed: string;
  decided: string;
  decidedBy: string;
  why: string;
  /** When the cap expired it, "" otherwise. */
  expired?: string;
};

/** `muster lessons`: the proposals waiting for the owner, and the live ones. */
export type LessonListing = {
  proposed: Lesson[];
  builders: Lesson[];
  reviewers: Lesson[];
  expiredOrRejected: number;
  file: string;
};
