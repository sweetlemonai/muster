import { useEffect, useState } from "react";
import { CardLookPicker } from "./CardLook";
import { sentences, useT } from "./i18n";
import { stageLabel } from "./lib";
import { Button, Pill } from "./ui";
import { loadZoom, setZoom, ZOOM_EVENT, ZOOMS } from "./zoom";
import { ACCENTS, apply, buildingColour, cardChoice, type Choice, DEFAULT_CHOICE, load, resolve, save, THEMES, variables } from "./themes";

/** Settings' Appearance group (0036): theme, accent, text size and card look, applied live and
 *  remembered on this Mac (the window's own storage). */
export default function Appearance() {
  const t = useT("shell");
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

  const clash = buildingColour(current.accent).clashes;
  return (
    <div className="settings-sections">
      <section>
        <label className="check">
          <input type="checkbox" checked={c.matchSystem} onChange={(e) => setC({ ...c, matchSystem: e.target.checked, theme: current.theme.id })} />
          {t("appearance.matchMac")}
        </label>
        <p className="meta">{c.matchSystem ? t("appearance.pickTwo", { name: current.theme.name }) : t("appearance.oneTheme")}</p>
      </section>
      <section>
        <h2>{t("appearance.themeHead")}</h2>
        <div className="swatches">
          {THEMES.map((th) => {
            const v = variables(th, current.accent);
            const mode = t(`appearance.mode.${th.mode}`);
            return (
              <button key={th.id} className={selected(th.id) ? "swatch on" : "swatch"} onClick={() => pickTheme(th.id)} title={t("appearance.themeTitle", { name: th.name, mode })}>
                <span className="preview" style={{ background: th.bg, borderColor: th.border }}>
                  <span style={{ background: th.surface, borderColor: th.border }}>
                    <i style={{ background: v["--accent"] }} />
                    <b style={{ background: th.text }} />
                    <b style={{ background: th.muted, width: "60%" }} />
                  </span>
                </span>
                <span className="swatch-name">{th.name}<small>{t("appearance.themeMode", { mode })}</small></span>
              </button>
            );
          })}
        </div>
      </section>
      <section>
        <h2>{t("appearance.accentHead")}</h2>
        <div className="accents">
          {ACCENTS.map((a) => (
            <button key={a.id} className={c.accent === a.id ? "accent on" : "accent"} onClick={() => setC({ ...c, accent: a.id })} title={a.name}>
              <i style={{ background: a[current.theme.mode][0] }} />
              {a.name}
            </button>
          ))}
        </div>
        <p className="meta">{sentences([t("appearance.building"), clash ? t("appearance.clash", { accent: current.accent.name, stage: stageLabel(clash) }) : ""])}</p>
        <p className="meta">{t("appearance.leftOut")}</p>
      </section>
      <section>
        <h2>{t("appearance.textSize")}</h2>
        <TextSize />
        <p className="meta">{t("appearance.textSizeMeta")}</p>
      </section>
      <section>
        <h2>{t("appearance.cards")}</h2>
        <CardLookPicker {...pickerProps(c, setC)} />
        <p className="meta">{t("appearance.cardsMeta")}</p>
      </section>
      <section>
        <Button onClick={() => setC((p) => ({ ...DEFAULT_CHOICE, cards: p.cards, cardColour: p.cardColour }))}>{t("appearance.reset")}</Button>
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
  const t = useT("shell");
  const [size, setSize] = useState(() => initial ?? loadZoom());
  useEffect(() => {
    const on = (e: Event) => setSize((e as CustomEvent<number>).detail);
    window.addEventListener(ZOOM_EVENT, on);
    return () => window.removeEventListener(ZOOM_EVENT, on);
  }, []);
  return (
    <div className="look-colour" role="group" aria-label={t("appearance.textSize")}>
      {ZOOMS.map((z) => (
        <Pill key={z} type="button" on={size === z} aria-pressed={size === z} onClick={() => setSize(setZoom(z))}>
          {t("appearance.percent", { n: Math.round(z * 100) })}
        </Pill>
      ))}
    </div>
  );
}
