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
};
