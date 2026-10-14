import { invoke } from "@tauri-apps/api/core";
import type { SearchIndex } from "./search";
import type { SettingsInfo } from "./settingsData";
import type { LessonListing, Project, State, Work } from "./types";
import type { AcceptResult } from "./yourCheck";

// The only way the window talks to the core. Queues are named, never given as paths.
export const api = {
  state: () => invoke<State>("state"),
  // Each task's search text by "queue/id", for one queue or every queue (null). Not in `state`:
  // the window asks only while it searches (search.ts `needsIndex`).
  searchIndex: (queue: string | null) => invoke<SearchIndex>("search_index", { queue }),
  taskBody: (queue: string, id: string) => invoke<string>("task_body", { queue, id }),
  image: (queue: string, id: string, name: string) => invoke<string>("image", { queue, id, name }),
  answer: (queue: string, id: string, n: number, text: string, stamp: string) =>
    invoke<string | null>("answer", { queue, id, n, text, stamp }),
  // A pick-one question (0049) answered with an option's letter, plus any words to add after it.
  answerPick: (queue: string, id: string, n: number, pick: string, extra: string, stamp: string) =>
    invoke<string | null>("answer_pick", { queue, id, n, pick, extra, stamp }),
  projects: () => invoke<Project[]>("projects"),
  // Both open the core's folder picker; null when cancelled.
  addProject: () => invoke<string | null>("add_project"),
  createProject: () => invoke<string | null>("create_project"),
  // What the window calls a project and the sidebar heading it goes under (0044), by its key: the
  // core strips control characters and keeps 40 and 24 characters; "" goes back to the name, and
  // to the group or Other. Commands keep the name.
  setProjectLabel: (key: string, label: string) => invoke<void>("set_project_label", { key, label }),
  setProjectCategory: (key: string, category: string) => invoke<void>("set_project_category", { key, category }),
  // The window's title for the project shown, by its name ("all" for All projects).
  showProjectTitle: (name: string) => invoke<void>("show_project_title", { name }),
  // What its tasks' keys start with (0033), 2 to 4 lowercase letters; refused when it isn't one
  // or another project has it, with a message to show as is.
  setProjectPrefix: (key: string, prefix: string) => invoke<void>("set_project_prefix", { key, prefix }),
  hideProject: (key: string, hidden: boolean) => invoke<void>("hide_project", { key, hidden }),
  removeProject: (key: string) => invoke<void>("remove_project", { key }),
  // This machine's settings (0036) with their bounds, and what Settings' About shows. Set one by
  // name (`lease` or `live`) to a span like 12h or 5m, empty for the default, or `notify` (0052)
  // to on or off; the core checks it.
  settingsInfo: () => invoke<SettingsInfo>("settings_info"),
  setSetting: (name: "lease" | "live" | "notify", value: string) => invoke<SettingsInfo>("set_setting", { name, value }),
  // A project's Your check limit (0050), by its key (queue folder): 1 to 50, "off", or "" for the default (5).
  setYourCheckLimit: (key: string, value: string) => invoke<void>("set_your_check_limit", { key, value }),
  createTask: (queue: string, task: NewTask, files: [string, string][], stamp: string) =>
    invoke<{ id: string; path: string }>("create_task", { queue, task, files, stamp }),
  // Stages: each returns the task's new path. The core refuses what its rules don't allow, with
  // a message to show as is. `from` is the stage the window shows: refused if the task moved on
  // since. `gate` only for Building → Testing, `commit` only for Your check → Done.
  passTask: (queue: string, id: string, from: string, gate: string, commit: string, stamp: string) =>
    invoke<string>("pass_task", { queue, id, from, gate, commit, stamp }),
  // Accept all (0043): each id passed from Your check to Done as Accept does, in order; what the
  // core refused comes back with why, and the rest still go.
  acceptAll: (queue: string, ids: string[], stamp: string) => invoke<AcceptResult>("accept_all", { queue, ids, stamp }),
  // `to` is an earlier stage; null sends it back one.
  returnTask: (queue: string, id: string, why: string, to: string | null, stamp: string) =>
    invoke<string>("return_task", { queue, id, why, to, stamp }),
  // Why (0024): kind is tasks, you, permission, question or outside; detail what it waits on.
  blockTask: (queue: string, id: string, kind: string, detail: string, stamp: string) => invoke<string>("block_task", { queue, id, kind, detail, stamp }),
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
  // Who worked on a task and what it cost (0025), from the session transcripts on this Mac; and
  // one of those sessions' transcripts as folded Markdown (only a session the task names).
  taskSessions: (queue: string, id: string) => invoke<Work>("task_sessions", { queue, id }),
  sessionTranscript: (queue: string, id: string, session: string) => invoke<string>("session_transcript", { queue, id, session }),
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
  // The vision check (0029): the brief as text, for the clipboard; and resolving one conflict
  // (V2) with how, which returns how many are still open.
  visionBrief: (queue: string, id: string) => invoke<string>("vision_brief", { queue, id }),
  resolveVision: (queue: string, id: string, which: string, how: string, stamp: string) =>
    invoke<number>("resolve_vision", { queue, id, which, how, stamp }),
  // The review of record (0027): the next round's brief as text, for the clipboard (a started
  // task only); the queue's lessons; and the owner's batch on the proposals.
  reviewBrief: (queue: string, id: string, stamp: string) => invoke<string>("review_brief", { queue, id, stamp }),
  lessons: (queue: string, stamp: string) => invoke<LessonListing>("lessons", { queue, stamp }),
  decideLessons: (queue: string, accept: string[], all: boolean, reject: string[], why: string, stamp: string) =>
    invoke<{ accepted: string[]; rejected: string[]; expired: string[] }>("decide_lessons", { queue, accept, all, reject, why, stamp }),
  // The write-up (between the front matter and the Log) for Edit; refused while someone else
  // holds the task in Building.
  editableBody: (queue: string, id: string) => invoke<string>("editable_body", { queue, id }),
  // Save it. `expected` is what Edit opened: refused if the file's write-up changed since. Null
  // when the text is unchanged (nothing written), else the task's path.
  editBody: (queue: string, id: string, body: string, expected: string, stamp: string) =>
    invoke<string | null>("edit_body", { queue, id, body, expected, stamp }),
  // A comment (0017), as the owner, at any stage; returns the task's path.
  addComment: (queue: string, id: string, text: string, stamp: string) => invoke<string>("add_comment", { queue, id, text, stamp }),
  // Reach the session holding a task (0039), as the owner, through its mailbox: ask it for a
  // check-in (Building), or send it a message (Building or Testing). Refused, with why, when the
  // holder's claim has no session stamp.
  askCheckin: (queue: string, id: string, stamp: string) => invoke<Pinged>("ask_checkin", { queue, id, stamp }),
  messageSession: (queue: string, id: string, text: string, stamp: string) => invoke<Pinged>("message_session", { queue, id, text, stamp }),
  // Pause or resume a task (0045), as the owner: it keeps its stage and claim; in Building or
  // Testing its session is asked to stop, or to carry on. `why` may be "".
  pauseTask: (queue: string, id: string, why: string, stamp: string) => invoke<Paused>("pause_task", { queue, id, why, stamp }),
  resumeTask: (queue: string, id: string, stamp: string) => invoke<Paused>("resume_task", { queue, id, stamp }),
  // Opens one of the task's links in the browser: the core opens only an address the task has, and only https.
  openLink: (queue: string, id: string, url: string) => invoke<void>("open_link", { queue, id, url }),
  // Files the window holds (pasted or dropped) go as [name, data URL]; picking uses the core's dialog.
  attachData: (queue: string, id: string, files: [string, string][], stamp: string) =>
    invoke<string[]>("attach_data", { queue, id, files, stamp }),
  attachPick: (queue: string, id: string, stamp: string) => invoke<string[]>("attach_pick", { queue, id, stamp }),
  // The help `muster help` prints; the Help tab shows its Commands page (Markdown, from help.rs).
  help: () => invoke<{ commandsPage?: unknown }>("help"),
  helperInfo: (queue: string) => invoke<HelperInfo>("helper_info", { queue }),
  helperSetAccount: (account: string) => invoke<void>("helper_set_account", { account }),
  // One of HelperInfo's `models`, from the helper's next turn (0036).
  helperSetModel: (model: string) => invoke<void>("helper_set_model", { model }),
  // Opens Terminal on that account's own `claude auth login`.
  helperLogin: (account: string) => invoke<void>("helper_login", { account }),
  helperReset: (queue: string) => invoke<void>("helper_reset", { queue }),
  helperStop: (queue: string) => invoke<void>("helper_stop", { queue }),
  helperSend: (queue: string, message: string, files: [string, string][], stamp: string) =>
    invoke<void>("helper_send", { queue, message, files, stamp }),
};

/** What a ping reached: the session's short id, and the task's path. */
export type Pinged = { session: string; path: string };
/** What a pause or resume did: the session it told ("" for none), or why none could be ("" when
 *  nobody works on it in its stage). */
export type Paused = { path: string; session: string; unreached: string };

export type HelperLine = { role: "owner" | "helper" | "system"; kind: string; text: string; at: number };
export type AccountInfo = { name: string; loggedIn: boolean | null; plan: string };
export type HelperInfo = { claude: string | null; accounts: AccountInfo[]; account: string; model: string; models: string[]; running: boolean; lines: HelperLine[] };

export type NewTask = { title: string; kind: string; priority: string; body: string; tags: string[]; estTime: string; estTestTime: string; estTokens: string; level: string; parent: string; due: string; forWho: string };
