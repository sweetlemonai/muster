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
};

/** A goal's or feature's tasks: done of total, and the estimates of the open ones, summed. */
export type Progress = { done: number; total: number; estMinutes: number; estTokenCount: number };

export type Activity = { queue: string; at: number; what: string; source: string };

/** `counts` is tasks per stage, plus "blocked" (tasks also counted in their stage). */
export type QueueInfo = { name: string; path: string; counts: Record<string, number> };

export type State = {
  queues: QueueInfo[];
  tasks: Task[];
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
