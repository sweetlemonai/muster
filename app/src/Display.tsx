import { ELEMENTS, MODES, type DisplayMode } from "./modes";

/** Settings' Display group (0038): the three modes as chips, the current one pressed, and every
 *  element that differs between them, side by side, the current mode's column marked. A pick
 *  applies at once and is remembered on this Mac. */
export function DisplayModes({ mode, onMode }: { mode: DisplayMode; onMode: (m: DisplayMode) => void }) {
  return (
    <div className="settings-sections">
      <section>
        <h2>Mode</h2>
        <div className="modes" role="group" aria-label="Display mode">
          {MODES.map((m) => (
            <button key={m.id} type="button" className={m.id === mode ? "mode on" : "mode"} aria-pressed={m.id === mode} onClick={() => onMode(m.id)}>
              <b>{m.label}</b>
              <small>{m.says}</small>
            </button>
          ))}
        </div>
        <p className="meta">
          ⌘. turns Focus on from any mode, and off again back to the mode you were in. The header's mode chip opens this page. In every mode, what waits
          on you keeps its counts and its badge, and everything a mode folds is one click or key away.
        </p>
      </section>
      <section>
        <h2>What each mode shows</h2>
        <table className="mode-table">
          <thead>
            <tr>
              <td />
              {MODES.map((m) => (
                <th key={m.id} scope="col" className={m.id === mode ? "on" : undefined}>{m.label}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {Object.entries(ELEMENTS).map(([id, e]) => (
              <tr key={id}>
                <th scope="row">{e.label}</th>
                {MODES.map((m) => (
                  <td key={m.id} className={m.id === mode ? "on" : undefined}>
                    {e[m.id].says}
                    {e[m.id].reach && <small>{e[m.id].reach}</small>}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        <p className="meta">The colour code, the docked panes and where every control sits are the same in all three.</p>
      </section>
    </div>
  );
}
