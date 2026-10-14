// A colour per project (task 0057): picked from themes.ts PROJECT_COLOURS by clicking the dot at
// the left of its sidebar row (or in Settings › Projects), kept in projects.json by the core
// (arrange.rs), and shown as a dot wherever the project is named (ProjectName).
import { createContext } from "react";
import { PROJECT_COLOURS } from "./themes";
import type { QueueInfo } from "./types";

/** A colour the palette has, by its id. */
export const isColour = (id: string): boolean => PROJECT_COLOURS.some((c) => c.id === id);

/** A colour's name ("Lime"); "" for none or one the palette doesn't have. */
export const colourName = (id: string): string => PROJECT_COLOURS.find((c) => c.id === id)?.name ?? "";

/** Each queue's name to its colour; a queue without one, or with one the palette doesn't have
 *  (an older or newer core), isn't in it. */
export function coloursOf(queues: Pick<QueueInfo, "name" | "colour">[]): Map<string, string> {
  const out = new Map<string, string>();
  for (const q of queues) if (q.colour && isColour(q.colour)) out.set(q.name, q.colour);
  return out;
}

/** The colours of the projects the window shows, by name (`coloursOf`). Without a provider, no
 *  project has one. */
export const ProjectColours = createContext<ReadonlyMap<string, string>>(new Map());
