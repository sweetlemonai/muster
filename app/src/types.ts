export type Question = {
  n: number | null;
  ticked: boolean;
  asker: string;
  text: string;
  escalated: boolean;
  ownerAnswered: boolean;
};

export type Task = {
  queue: string;
  id: string;
  title: string;
  kind: string;
  priority: string;
  status: string;
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
  attachments: string[];
  logTail: string[];
  modified: number;
};

export type Activity = { queue: string; at: number; what: string; source: string };

export type QueueInfo = { name: string; path: string; counts: Record<string, number> };

export type State = {
  queues: QueueInfo[];
  tasks: Task[];
  problems: string[];
  activity: Activity[];
  now: number;
};
