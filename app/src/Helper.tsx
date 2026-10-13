import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type AccountInfo, type HelperInfo, type HelperLine } from "./api";
import { readFiles, stamp } from "./lib";
import Markdown from "./Markdown";
import { HelpButton } from "./HelpButton";

type Group = { owner?: HelperLine; replies: HelperLine[]; tools: { cmd: string; result?: string }[]; end?: HelperLine; notes: HelperLine[] };

/** Turn the flat transcript into one block per owner message: replies, the commands run, the end. */
function groups(lines: HelperLine[]): Group[] {
  const out: Group[] = [];
  let g: Group = { replies: [], tools: [], notes: [] };
  const flush = () => {
    if (g.owner || g.replies.length || g.tools.length || g.end || g.notes.length) out.push(g);
  };
  for (const l of lines) {
    if (l.role === "helper" && l.kind === "reply" && /^not logged in/i.test(l.text)) continue;
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
  const [picking, setPicking] = useState(false);

  const reload = () => api.helperInfo(queue).then(setInfo, (e) => setError(String(e)));

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
  if (info && (!info.account || picking)) {
    return (
      <main className="list helper">
        <AccountPicker
          info={info}
          onPicked={(a) => {
            setInfo({ ...info, account: a });
            setPicking(false);
          }}
          onRefresh={() => void reload()}
          onCancel={info.account ? () => setPicking(false) : undefined}
        />
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
        <button onClick={() => { setPicking(true); void reload(); }} disabled={running}>Change account</button>
        {running && <button onClick={() => void api.helperStop(queue)}>Stop</button>}
        <button onClick={() => void api.helperReset(queue).then(() => api.helperInfo(queue)).then((i) => setLines(i.lines))} disabled={running}>
          New conversation
        </button>
        <HelpButton topic="helper" />
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
            {b.end?.kind === "login" && info && (
              <div className="login-card">
                <p>
                  <b>The account ~/{info.account} isn't logged in on its own.</b> The helper uses an account's own login and never
                  reads tokens, so an account that only works through a shell command with a token won't work here. Pick an account
                  that's logged in, or log this one in (it opens Terminal on Claude's own login).
                </p>
                <div className="accounts">
                  <button onClick={() => { setPicking(true); void reload(); }}>Choose another account</button>
                  <button onClick={() => void api.helperLogin(info.account).catch((e) => setError(String(e)))}>Log in ~/{info.account}…</button>
                </div>
              </div>
            )}
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

function AccountPicker({ info, onPicked, onRefresh, onCancel }: {
  info: HelperInfo;
  onPicked: (a: string) => void;
  onRefresh: () => void;
  onCancel?: () => void;
}) {
  const [error, setError] = useState("");
  const pick = (a: AccountInfo) =>
    void api.helperSetAccount(a.name).then(() => onPicked(a.name), (e) => setError(String(e)));
  const label = (a: AccountInfo) => (a.name === ".claude" ? "~/.claude (default)" : `~/${a.name}`);
  return (
    <>
      <h2>Which Claude account should the helper use?</h2>
      <p className="meta">
        The helper runs your own Claude Code with that account's own login, like any session you start, and never reads tokens.
        What you write here goes to Anthropic, the same as any Claude Code session. You can change it any time.
      </p>
      <table className="projects-table">
        <tbody>
          {info.accounts.map((a) => (
            <tr key={a.name}>
              <td><b>{label(a)}</b>{info.account === a.name && <span className="meta"> · in use</span>}</td>
              <td>
                {a.loggedIn === true && <span className="ok">logged in{a.plan ? ` · ${a.plan}` : ""}</span>}
                {a.loggedIn === false && <span className="err">not logged in</span>}
                {a.loggedIn === null && <span className="meta">couldn't check</span>}
              </td>
              <td className="actions">
                {a.loggedIn !== true && <button onClick={() => void api.helperLogin(a.name).catch((e) => setError(String(e)))}>Log in…</button>}
                <button onClick={() => pick(a)} disabled={a.loggedIn === false}>Use this one</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="accounts">
        <button onClick={onRefresh}>Check again</button>
        {onCancel && <button onClick={onCancel}>Cancel</button>}
      </div>
      {error && <p className="err">{error}</p>}
    </>
  );
}
