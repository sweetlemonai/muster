import { useEffect, useState } from "react";
import { CardLookPicker } from "./CardLook";
import { HelpButton } from "./HelpButton";
import { stageLabel } from "./lib";
import { loadZoom, setZoom, ZOOM_EVENT, ZOOMS } from "./zoom";
import { ACCENTS, apply, buildingColour, cardChoice, type Choice, DEFAULT_CHOICE, load, resolve, save, THEMES, variables } from "./themes";

/** Theme, accent and card look, applied live while the board stays in view beside it. */
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
        <h1>Appearance <HelpButton topic="appearance" /></h1>
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
        <p className="meta">
          Building shows in the accent colour.
          {buildingColour(current.accent).clashes &&
            ` ${current.accent.name} would look like ${stageLabel(buildingColour(current.accent).clashes)}, so Building stays violet.`}
        </p>
        <p className="meta">Blue and orange are left out of the accents: they belong to 127, Drover and Braindump. Every theme and accent pair passes WCAG AA.</p>
      </section>
      <section>
        <h2>Text size</h2>
        <TextSize />
        <p className="meta">Everything in the window grows or shrinks, the manual too. ⌘+ and ⌘− step through the sizes, ⌘0 goes back to 100%.</p>
      </section>
      <section>
        <h2>Cards</h2>
        <CardLookPicker {...pickerProps(c, setC)} />
        <p className="meta">Blocked and "needs you" take over the colour in every look, and keep their words on the card.</p>
      </section>
      <section>
        <button onClick={() => setC((p) => ({ ...DEFAULT_CHOICE, cards: p.cards, cardColour: p.cardColour }))}>Back to Muster's violet</button>
      </section>
    </div>
  );
}

function pickerProps(c: Choice, setC: (f: (p: Choice) => Choice) => void) {
  const { cards, cardColour } = cardChoice(c);
  return {
    look: cards,
    colour: cardColour,
    onLook: (l: Choice["cards"]) => setC((p) => ({ ...p, cards: l })),
    onColour: (k: Choice["cardColour"]) => setC((p) => ({ ...p, cardColour: k })),
  };
}

/** The text size steps as chips, the current one pressed (zoom.ts). Follows ⌘+ ⌘− ⌘0 too. */
export function TextSize({ initial }: { initial?: number }) {
  const [size, setSize] = useState(() => initial ?? loadZoom());
  useEffect(() => {
    const on = (e: Event) => setSize((e as CustomEvent<number>).detail);
    window.addEventListener(ZOOM_EVENT, on);
    return () => window.removeEventListener(ZOOM_EVENT, on);
  }, []);
  return (
    <div className="look-colour" role="group" aria-label="Text size">
      {ZOOMS.map((z) => (
        <button key={z} type="button" className={size === z ? "chip on" : "chip"} aria-pressed={size === z} onClick={() => setSize(setZoom(z))}>
          {Math.round(z * 100)}%
        </button>
      ))}
    </div>
  );
}
