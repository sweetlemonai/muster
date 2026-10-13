import { useId, useState } from "react";

export const SUGGESTED_TAGS = ["spike", "chore", "tech-debt", "code", "docs", "design", "research", "test", "perf", "security"];

/** Same cleaning as the core's `ops::clean_tags`, so what you see is what's saved. */
export function cleanTag(t: string): string {
  return t
    .trim()
    .toLowerCase()
    .replace(/[ _]+/g, "-")
    .replace(/[^\p{L}\p{N}-]/gu, "")
    .replace(/^-+|-+$/g, "")
    .slice(0, 24);
}

/** At most this many tags on a task, as the core's `ops::MAX_TAGS`. */
export const MAX_TAGS = 8;

/** Suggested tags the task doesn't have yet, offered as completions while typing. */
export function tagSuggestions(tags: string[]): string[] {
  return SUGGESTED_TAGS.filter((t) => !tags.includes(t));
}

/** The task's own tags, each with × to remove it, and a box to add one: Enter or comma adds it,
 *  and the suggested tags complete as you type. Unpicked suggestions stay out of view. */
export default function TagEditor({ tags, onChange }: { tags: string[]; onChange: (tags: string[]) => void }) {
  const [text, setText] = useState("");
  const listId = useId();
  const full = tags.length >= MAX_TAGS;
  const remove = (t: string) => onChange(tags.filter((x) => x !== t));
  const add = () => {
    const t = cleanTag(text);
    setText("");
    if (t && !tags.includes(t)) onChange([...tags, t].slice(0, MAX_TAGS));
  };
  return (
    <div className="tag-editor">
      {tags.map((t) => (
        <button type="button" key={t} className="chip on" onClick={() => remove(t)} title={`Remove #${t}`} aria-label={`Remove tag ${t}`}>
          #{t} ×
        </button>
      ))}
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            add();
          }
        }}
        onBlur={add}
        list={listId}
        disabled={full}
        placeholder={full ? `${MAX_TAGS} tags max` : "+ tag"}
        maxLength={24}
        aria-label="Add a tag"
      />
      <datalist id={listId}>
        {tagSuggestions(tags).map((t) => <option key={t} value={t} />)}
      </datalist>
    </div>
  );
}
