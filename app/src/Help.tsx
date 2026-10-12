import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Command = { name: string; usage: string; what: string; options: [string, string][]; examples: string[] };
type Topic = { title: string; lines: string[] };
type HelpData = { commands: Command[]; common: [string, string][]; session: Topic[]; app: [string, string][] };

/** Every command and how to use it, from the same list `muster help` prints (the core's help.rs). */
export default function Help() {
  const [h, setH] = useState<HelpData | null>(null);
  const [copied, setCopied] = useState("");
  useEffect(() => {
    invoke<HelpData>("help").then(setH, () => {});
  }, []);
  const copy = (t: string) =>
    void navigator.clipboard?.writeText(t).then(() => {
      setCopied(t);
      window.setTimeout(() => setCopied(""), 1200);
    });
  const Cmd = ({ text }: { text: string }) => (
    <code className="cmd" onClick={() => copy(text)} title="Click to copy">
      {text}
      {copied === text && <span className="ok"> copied</span>}
    </code>
  );
  if (!h) return <main className="list"><p className="none">Loading…</p></main>;
  return (
    <main className="list help">
      <nav className="help-toc">
        <a href="#cli">Command line</a> · <a href="#session">Inside a Claude Code session</a> · <a href="#app">In the app</a>
      </nav>

      <h2 id="cli">Command line</h2>
      <p className="meta">
        <code>muster</code> is in <code>~/playground/muster/main/bin</code>: put that folder on your PATH, or link{" "}
        <code>bin/muster</code> into <code>~/.local/bin</code>. It finds the project's queue from the folder you're in. In a
        terminal, <code>muster help</code> prints this list, and <code>muster help new</code> shows one command in detail.
        Click any example to copy it.
      </p>
      {h.commands.map((c) => (
        <section key={c.name} className="help-cmd">
          <h3>
            <code>muster {c.name}</code>
          </h3>
          <p>{c.what}</p>
          <p className="usage"><Cmd text={c.usage} /></p>
          {c.options.length > 0 && (
            <table className="help-opts">
              <tbody>
                {c.options.map(([k, v]) => (
                  <tr key={k}>
                    <td><code>{k}</code></td>
                    <td>{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div className="examples">
            {c.examples.map((e) => <Cmd key={e} text={e} />)}
          </div>
        </section>
      ))}
      <section className="help-cmd">
        <h3>On every command</h3>
        <table className="help-opts">
          <tbody>
            {h.common.map(([k, v]) => (
              <tr key={k}>
                <td><code>{k}</code></td>
                <td>{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <h2 id="session">Inside a Claude Code session</h2>
      {h.session.map((t) => (
        <section key={t.title} className="help-cmd">
          <h3>{t.title}</h3>
          {t.lines.map((l, i) => (l.startsWith("$ ") ? <div key={i} className="examples"><Cmd text={l.slice(2)} /></div> : <p key={i}>{l}</p>))}
        </section>
      ))}

      <h2 id="app">In the app</h2>
      <table className="help-opts">
        <tbody>
          {h.app.map(([k, v]) => (
            <tr key={k}>
              <td><b>{k}</b></td>
              <td>{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
