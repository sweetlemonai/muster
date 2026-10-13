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
  heartbeat: string;
  waitingOn: string;
  commit: string;
  captured: string;
  questions: Question[];
  openQuestions: number;
  ownerCriteria: Criterion[];
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

export type Activity ={ queue: string; at: number; what: string; source: string };

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
