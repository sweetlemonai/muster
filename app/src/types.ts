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
  /** The vision check (0029): open conflicts, or null before the first check. Over 0, it can't
   *  pass from Shaping to Ready. */
  visionOpen?: number | null;
  /** The open conflicts' ids (V1, V2…), once checked. */
  visionIds?: string[];
  /** A `vision:` value that isn't a count ("" when fine): a problem the core reports, so the task
   *  isn't also listed as unchecked. */
  badVision?: string;
  waitingForOwner: boolean;
  tags: string[];
  estTime: string;
  estTokens: string;
  estMinutes: number;
  estTokenCount: number;
  claimedOn: string;
  doneOn: string;
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
