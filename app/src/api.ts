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
  moveTask: (queue: string, id: string, status: string, waitingOn: string, stamp: string) =>
    invoke<string>("move_task", { queue, id, status, waitingOn, stamp }),
  setField: (queue: string, id: string, field: string, value: string, stamp: string) =>
    invoke<string>("set_field", { queue, id, field, value, stamp }),
  addNote: (queue: string, id: string, note: string, stamp: string) => invoke<string>("add_note", { queue, id, note, stamp }),
  // Files the window holds (pasted or dropped) go as [name, data URL]; picking uses the core's dialog.
  attachData: (queue: string, id: string, files: [string, string][], stamp: string) =>
    invoke<string[]>("attach_data", { queue, id, files, stamp }),
  attachPick: (queue: string, id: string, stamp: string) => invoke<string[]>("attach_pick", { queue, id, stamp }),
  helperInfo: (queue: string) => invoke<HelperInfo>("helper_info", { queue }),
  helperSetAccount: (account: string) => invoke<void>("helper_set_account", { account }),
  helperReset: (queue: string) => invoke<void>("helper_reset", { queue }),
  helperStop: (queue: string) => invoke<void>("helper_stop", { queue }),
  helperSend: (queue: string, message: string, files: [string, string][], stamp: string) =>
    invoke<void>("helper_send", { queue, message, files, stamp }),
};

export type HelperLine = { role: "owner" | "helper" | "system"; kind: string; text: string; at: number };
export type HelperInfo = { claude: string | null; accounts: string[]; account: string; running: boolean; lines: HelperLine[] };

export type NewTask = { title: string; kind: string; priority: string; body: string };
