import { useState } from "react";

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

/** Toggle the suggested tags, or type your own (Enter or comma adds it). */
export default function TagEditor({ tags, onChange }: { tags: string[]; onChange: (tags: string[]) => void }) {
  const [text, setText] = useState("");
  const all = [...SUGGESTED_TAGS, ...tags.filter((t) => !SUGGESTED_TAGS.includes(t))];
  const toggle = (t: string) => onChange(tags.includes(t) ? tags.filter((x) => x !== t) : [...tags, t].slice(0, 8));
  const add = () => {
    const t = cleanTag(text);
    setText("");
    if (t && !tags.includes(t)) onChange([...tags, t].slice(0, 8));
  };
  return (
    <div className="tag-editor">
      {all.map((t) => (
        <button type="button" key={t} className={tags.includes(t) ? "chip on" : "chip"} onClick={() => toggle(t)}>
          #{t}
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
        placeholder="+ tag"
        maxLength={24}
        aria-label="Add a tag"
      />
    </div>
  );
}
