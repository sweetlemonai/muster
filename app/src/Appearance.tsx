import { useEffect, useState } from "react";
import { ACCENTS, apply, type Choice, load, resolve, save, THEMES, variables } from "./themes";

/** Theme and accent, applied live while the board stays in view beside it. */
export default function Appearance({ onClose }: { onClose: () => void }) {
  const [c, setC] = useState<Choice>(load);
  useEffect(() => {
    apply(c);
    save(c);
  }, [c]);
  const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const current = resolve(c, dark);
  const pickTheme = (id: string) => {
    const t = THEMES.find((x) => x.id === id)!;
    setC((p) => (p.matchSystem ? { ...p, [t.mode]: id } : { ...p, theme: id }));
  };
  const selected = (id: string) => (c.matchSystem ? c.light === id || c.dark === id : c.theme === id);

  return (
    <div className="pane-body">
      <header className="pane-head">
        <h1>Appearance</h1>
        <button className="close" onClick={onClose} aria-label="Close">×</button>
      </header>
      <section>
        <label className="check">
          <input type="checkbox" checked={c.matchSystem} onChange={(e) => setC({ ...c, matchSystem: e.target.checked, theme: current.theme.id })} />
          Match the Mac's light or dark
        </label>
        <p className="meta">
          {c.matchSystem
            ? `Pick a dark and a light theme. Now showing ${current.theme.name}.`
            : "One theme, whatever the Mac uses."}
        </p>
      </section>
      <section>
        <h2>Theme</h2>
        <div className="swatches">
          {THEMES.map((t) => {
            const v = variables(t, current.accent);
            return (
              <button key={t.id} className={selected(t.id) ? "swatch on" : "swatch"} onClick={() => pickTheme(t.id)} title={`${t.name} (${t.mode})`}>
                <span className="preview" style={{ background: t.bg, borderColor: t.border }}>
                  <span style={{ background: t.surface, borderColor: t.border }}>
                    <i style={{ background: v["--accent"] }} />
                    <b style={{ background: t.text }} />
                    <b style={{ background: t.muted, width: "60%" }} />
                  </span>
                </span>
                <span className="swatch-name">{t.name}<small> · {t.mode}</small></span>
              </button>
            );
          })}
        </div>
      </section>
      <section>
        <h2>Accent</h2>
        <div className="accents">
          {ACCENTS.map((a) => (
            <button key={a.id} className={c.accent === a.id ? "accent on" : "accent"} onClick={() => setC({ ...c, accent: a.id })} title={a.name}>
              <i style={{ background: a[current.theme.mode][0] }} />
              {a.name}
            </button>
          ))}
        </div>
        <p className="meta">Blue and orange are left out: they belong to 127, Drover and Braindump. Every theme and accent pair passes WCAG AA.</p>
      </section>
      <section>
        <button onClick={() => setC({ theme: "violet-night", accent: "violet", matchSystem: true, light: "violet-day", dark: "violet-night" })}>Back to Muster's violet</button>
      </section>
    </div>
  );
}
