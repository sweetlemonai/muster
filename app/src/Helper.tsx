import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type HelperInfo, type HelperLine } from "./api";
import { readFiles, stamp } from "./lib";
import Markdown from "./Markdown";

type Group = { owner?: HelperLine; replies: HelperLine[]; tools: { cmd: string; result?: string }[]; end?: HelperLine; notes: HelperLine[] };

/** Turn the flat transcript into one block per owner message: replies, the commands run, the end. */
function groups(lines: HelperLine[]): Group[] {
  const out: Group[] = [];
  let g: Group = { replies: [], tools: [], notes: [] };
  const flush = () => {
    if (g.owner || g.replies.length || g.tools.length || g.end || g.notes.length) out.push(g);
  };
  for (const l of lines) {
    if (l.role === "owner") {
      flush();
      g = { owner: l, replies: [], tools: [], notes: [] };
    } else if (l.kind === "reply") g.replies.push(l);
    else if (l.kind === "tool") g.tools.push({ cmd: l.text });
    else if (l.kind === "toolResult") {
      const t = g.tools.findLast((x) => x.result === undefined);
      if (t) t.result = l.text;
    } else if (l.kind === "done" || l.kind === "error") g.end = l;
    else g.notes.push(l);
  }
  flush();
  return out;
}

export default function Helper({ queues, initial, onChanged }: { queues: string[]; initial: string; onChanged: () => void }) {
  const [queue, setQueue] = useState(initial);
  const [info, setInfo] = useState<HelperInfo | null>(null);
  const [lines, setLines] = useState<HelperLine[]>([]);
  const [running, setRunning] = useState(false);
  const [text, setText] = useState("");
  const [files, setFiles] = useState<[string, string][]>([]);
  const [error, setError] = useState("");
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let live = true;
    setInfo(null);
    api.helperInfo(queue).then(
      (i) => {
        if (!live) return;
        setInfo(i);
        setLines(i.lines);
        setRunning(i.running);
      },
      (e) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [queue]);

  useEffect(() => {
    const offs = [
      listen<{ queue: string; line: HelperLine }>("helper", (e) => {
        if (e.payload.queue !== queue) return;
        setLines((l) => [...l, e.payload.line]);
        if (e.payload.line.kind === "toolResult") onChanged();
      }),
      listen<string>("helper-idle", (e) => {
        if (e.payload !== queue) return;
        setRunning(false);
        onChanged();
      }),
    ];
    return () => {
      for (const o of offs) void o.then((f) => f());
    };
  }, [queue, onChanged]);

  useEffect(() => end.current?.scrollIntoView({ block: "end" }), [lines.length]);

  const send = async () => {
    if ((!text.trim() && !files.length) || running) return;
    setError("");
    setRunning(true);
    try {
      await api.helperSend(queue, text, files, stamp(new Date()));
      setText("");
      setFiles([]);
    } catch (e) {
      setError(String(e));
      setRunning(false);
    }
  };
  const blocks = useMemo(() => groups(lines), [lines]);

  if (info && !info.claude) {
    return (
      <main className="list helper">
        <p className="err">Couldn't find the <code>claude</code> command. The helper runs your own Claude Code. Install it, then reopen this tab.</p>
      </main>
    );
  }
  if (info && !info.account) {
    return (
      <main className="list helper">
        <h2>Which Claude account should the helper use?</h2>
        <p className="meta">
          The helper runs your own Claude Code with that account's login, like any session you start. What you write here goes to
          Anthropic, the same as any Claude Code session. Pick once; you can change it later.
        </p>
        <div className="accounts">
          {info.accounts.map((a) => (
            <button key={a} onClick={() => void api.helperSetAccount(a).then(() => setInfo({ ...info, account: a }), (e) => setError(String(e)))}>
              {a === ".claude" ? "~/.claude (default)" : `~/${a}`}
            </button>
          ))}
        </div>
        {error && <p className="err">{error}</p>}
      </main>
    );
  }

  return (
    <main className="helper">
      <header className="helper-bar">
        <label>
          Project
          <select value={queue} onChange={(e) => setQueue(e.target.value)}>
            {queues.map((q) => <option key={q}>{q}</option>)}
          </select>
        </label>
        <span className="meta">
          Tell it ideas, bugs and tasks. It files them in {queue}'s queue, checks for duplicates, and asks when something's unclear.
          {info && <> Account: ~/{info.account}.</>}
        </span>
        <span className="spacer" />
        {running && <button onClick={() => void api.helperStop(queue)}>Stop</button>}
        <button onClick={() => void api.helperReset(queue).then(() => api.helperInfo(queue)).then((i) => setLines(i.lines))} disabled={running}>
          New conversation
        </button>
      </header>
      <div className="chat">
        {blocks.length === 0 && <p className="empty">Nothing yet. Try: “The board flickers when I resize the window, and I'd like a dark mode toggle.”</p>}
        {blocks.map((b, i) => (
          <div key={i} className="turn">
            {b.owner && <div className="bubble owner">{b.owner.text}</div>}
            {b.notes.map((n, j) => <p key={j} className="note">{n.text}</p>)}
            {b.tools.length > 0 && (
              <details className="tools">
                <summary>{b.tools.length} step{b.tools.length === 1 ? "" : "s"}{b.tools.some((t) => t.cmd.includes("muster new")) ? " · filed tasks" : ""}</summary>
                <ul>
                  {b.tools.map((t, j) => (
                    <li key={j}>
                      <code>{t.cmd}</code>
                      {t.result && <span className="meta"> → {t.result}</span>}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            {b.replies.map((r, j) => (
              <div key={j} className="bubble helper-reply"><Markdown text={r.text} /></div>
            ))}
            {b.end?.kind === "error" && <p className="err">{b.end.text}</p>}
            {b.end?.kind === "done" && b.end.text && <p className="meta cost">{b.end.text}</p>}
          </div>
        ))}
        {running && <p className="meta working"><i className="dot" /> working…</p>}
        <div ref={end} />
      </div>
      <form
        className="composer"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
        onPaste={(e) => {
          if (e.clipboardData.files.length) {
            e.preventDefault();
            void readFiles(e.clipboardData.files, stamp(new Date())).then((r) => setFiles((f) => [...f, ...r]));
          }
        }}
        onDragOver={(e) => e.preventDefault()}
        onDrop={(e) => {
          e.preventDefault();
          void readFiles(e.dataTransfer.files, stamp(new Date())).then((r) => setFiles((f) => [...f, ...r]));
        }}
      >
        {files.length > 0 && (
          <div className="pending">
            {files.map(([name, url], i) => (
              <span key={name + i} className="pending-file">
                {url.startsWith("data:image/") ? <img src={url} alt="" /> : null}
                {name}
                <button type="button" onClick={() => setFiles((f) => f.filter((_, j) => j !== i))} aria-label={`Remove ${name}`}>×</button>
              </span>
            ))}
          </div>
        )}
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={running ? "It's working on your last message…" : "Ideas, bugs, tasks… paste screenshots here (⌘↩ to send)"}
          rows={3}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void send();
          }}
        />
        <div className="modal-bar">
          {error && <span className="err">{error}</span>}
          <span className="spacer" />
          <button type="submit" className="primary" disabled={running || (!text.trim() && !files.length)}>Send</button>
        </div>
      </form>
    </main>
  );
}
