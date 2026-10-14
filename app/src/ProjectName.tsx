import type { ReactNode } from "react";
import { keyTitle, labelOf, Labels } from "./labels";
import { ProjectColours } from "./projectColours";
import { taskKey } from "./taskKey";
import type { Task } from "./types";

// Both read the labels (and the colours) through the contexts' Consumers, not a hook, so the rows
// and cards they sit in stay hook-free (their tests call them as functions).

/** Where a task, note or line says which project it's in: the project's label (0044), with its
 *  real name, the one commands take, as the tooltip when the two differ, and its colour (0057) as
 *  a dot before it when it has one. */
export default function ProjectName({ name, className = "q", as: Tag = "span" }: { name: string; className?: string; as?: "span" | "b" | "td" }) {
  return (
    <Labels.Consumer>
      {(labels) => {
        const label = labelOf(labels, name);
        return (
          <ProjectColours.Consumer>
            {(colours) => {
              const colour = colours.get(name);
              return (
                <Tag className={className} title={label !== name ? name : undefined}>
                  {colour && <i className="proj-dot" data-colour={colour} aria-hidden="true" />}
                  {label}
                </Tag>
              );
            }}
          </ProjectColours.Consumer>
        );
      }}
    </Labels.Consumer>
  );
}

/** A task's key where a row or card shows it (`children`: the key, marked by a search), with its
 *  project's label in the tooltip (0044), since the prefix isn't the project's name. */
export function TaskKey({ t, as: Tag = "span", children }: { t: Pick<Task, "queue" | "prefix" | "id">; as?: "span" | "td"; children: ReactNode }) {
  return (
    <Labels.Consumer>
      {(labels) => (
        <Tag className="id" title={keyTitle(labelOf(labels, t.queue), taskKey(t))}>
          {children}
        </Tag>
      )}
    </Labels.Consumer>
  );
}
