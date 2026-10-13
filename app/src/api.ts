import { invoke } from "@tauri-apps/api/core";
import type { Project, State } from "./types";

// The only way the window talks to the core. Queues are named, never given as paths.
export const api = {
  state: () => invoke<State>("state"),
  taskBody: (queue: string, id: string) => invoke<string>("task_body", { queue, id }),
  image: (queue: string, id: string, name: string) => invoke<string>("image", { queue, id, name }),
  answer: (queue: string, id: string, n: number, text: string, stamp: string) =>
    invoke<string | null>("answer", { queue, id, n, text, stamp }),
  projects: () => invoke<Project[]>("projects"),
  // Both open the core's folder picker; null when cancelled.
  addProject: () => invoke<string | null>("add_project"),
  createProject: () => invoke<string | null>("create_project"),
  renameProject: (key: string, name: string) => invoke<void>("rename_project", { key, name }),
  hideProject: (key: string, hidden: boolean) => invoke<void>("hide_project", { key, hidden }),
  removeProject: (key: string) => invoke<void>("remove_project", { key }),
  createTask: (queue: string, task: NewTask, files: [string, string][], stamp: string) =>
    invoke<{ id: string; path: string }>("create_task", { queue, task, files, stamp }),
  // Stages: each returns the task's new path. The core refuses what its rules don't allow, with
  // a message to show as is. `from` is the stage the window shows: refused if the task moved on
  // since. `gate` only for Building → Testing, `commit` only for Your check → Done.
  passTask: (queue: string, id: string, from: string, gate: string, commit: string, stamp: string) =>
    invoke<string>("pass_task", { queue, id, from, gate, commit, stamp }),
  // `to` is an earlier stage; null sends it back one.
  returnTask: (queue: string, id: string, why: string, to: string | null, stamp: string) =>
    invoke<string>("return_task", { queue, id, why, to, stamp }),
  blockTask: (queue: string, id: string, on: string, stamp: string) => invoke<string>("block_task", { queue, id, on, stamp }),
  unblockTask: (queue: string, id: string, stamp: string) => invoke<string>("unblock_task", { queue, id, stamp }),
  // A claim in Building whose lease ran out goes back to Ready; refused if it's live again.
  reclaimTask: (queue: string, id: string, stamp: string) => invoke<string>("reclaim_task", { queue, id, stamp }),
  // The owner's override: any stage. `from` is the stage the window shows (refused if the task
  // moved on since); null for the plain override.
  setStage: (queue: string, id: string, stage: string, from: string | null, stamp: string) =>
    invoke<string>("set_stage", { queue, id, stage, from, stamp }),
  setField: (queue: string, id: string, field: string, value: string, stamp: string) =>
    invoke<string>("set_field", { queue, id, field, value, stamp }),
  addNote: (queue: string, id: string, note: string, stamp: string) => invoke<string>("add_note", { queue, id, note, stamp }),
  tickCriterion: (queue: string, id: string, which: string, undo: boolean, stamp: string) =>
    invoke<string>("tick_criterion", { queue, id, which, undo, stamp }),
  // Backlog notes (0015), named by their file name. Add returns the new note's file name; file
  // makes a task in Shaping and moves the note into its attachments; drop sets it aside.
  addBacklogNote: (queue: string, text: string, stamp: string) => invoke<string>("add_backlog_note", { queue, text, stamp }),
  // A filed note is an agent's task on its own: its level, parent, due and who does it are set
  // afterwards (`muster set`, or the task pane).
  fileNote: (queue: string, note: string, task: Omit<NewTask, "level" | "parent" | "due" | "forWho">, stamp: string) =>
    invoke<{ id: string; path: string; attachment: string }>("file_note", { queue, note, task, stamp }),
  dropNote: (queue: string, note: string, why: string, stamp: string) => invoke<string>("drop_note", { queue, note, why, stamp }),
  // The shaping brief as text, for the clipboard.
  shapeNote: (queue: string, note: string) => invoke<string>("shape_note", { queue, note }),
  // The write-up (between the front matter and the Log) for Edit; refused while someone else
  // holds the task in Building.
  editableBody: (queue: string, id: string) => invoke<string>("editable_body", { queue, id }),
  // Save it. `expected` is what Edit opened: refused if the file's write-up changed since. Null
  // when the text is unchanged (nothing written), else the task's path.
  editBody: (queue: string, id: string, body: string, expected: string, stamp: string) =>
    invoke<string | null>("edit_body", { queue, id, body, expected, stamp }),
  // A comment (0017), as the owner, at any stage; returns the task's path.
  addComment: (queue: string, id: string, text: string, stamp: string) => invoke<string>("add_comment", { queue, id, text, stamp }),
  // Opens one of the task's links in the browser: the core opens only an address the task has, and only https.
  openLink: (queue: string, id: string, url: string) => invoke<void>("open_link", { queue, id, url }),
  // Files the window holds (pasted or dropped) go as [name, data URL]; picking uses the core's dialog.
  attachData: (queue: string, id: string, files: [string, string][], stamp: string) =>
    invoke<string[]>("attach_data", { queue, id, files, stamp }),
  attachPick: (queue: string, id: string, stamp: string) => invoke<string[]>("attach_pick", { queue, id, stamp }),
  helperInfo: (queue: string) => invoke<HelperInfo>("helper_info", { queue }),
  helperSetAccount: (account: string) => invoke<void>("helper_set_account", { account }),
  // Opens Terminal on that account's own `claude auth login`.
  helperLogin: (account: string) => invoke<void>("helper_login", { account }),
  helperReset: (queue: string) => invoke<void>("helper_reset", { queue }),
  helperStop: (queue: string) => invoke<void>("helper_stop", { queue }),
  helperSend: (queue: string, message: string, files: [string, string][], stamp: string) =>
    invoke<void>("helper_send", { queue, message, files, stamp }),
};

export type HelperLine = { role: "owner" | "helper" | "system"; kind: string; text: string; at: number };
export type AccountInfo = { name: string; loggedIn: boolean | null; plan: string };
export type HelperInfo = { claude: string | null; accounts: AccountInfo[]; account: string; running: boolean; lines: HelperLine[] };

export type NewTask = { title: string; kind: string; priority: string; body: string; tags: string[]; estTime: string; estTestTime: string; estTokens: string; level: string; parent: string; due: string; forWho: string };
