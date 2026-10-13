import { trackSteps } from "./paneData";

/** The seven stages as one track, the task's own lit; the owner's own task (`human`, 0022) has
 *  only its three steps, To do · Doing · Done. Clicking another stage is the owner's override
 *  (the core's set_stage), with no confirmation: the pane offers Undo instead. Leaving Building
 *  releases the claim: `releases(to)` names whose (someone else's), so the button says so. A
 *  paused task (0045, `paused` its stamp) isn't moved: the track is off, saying why, as the core
 *  refuses it. */
export default function StageTrack({ stage, busy, onPick, releases = () => "", human = false, paused = "" }: {
  stage: string;
  busy: boolean;
  onPick: (to: string) => void;
  releases?: (to: string) => string;
  human?: boolean;
  paused?: string;
}) {
  return (
    <div className="track-wrap" data-part="track">
      <div className="track-label">
        <span className="lbl">Stage</span>
        <span className="meta small">{paused ? "paused: resume it to move it" : "click any stage to move it there"}</span>
      </div>
      <div className="stage-track" role="group" aria-label="Stage">
        {trackSteps(stage, human).map((s) => {
          const who = s.place === "cur" ? "" : releases(s.id);
          return (
            <button
              key={s.id}
              type="button"
              className={`${s.place} s-${s.id}`}
              aria-pressed={s.place === "cur"}
              disabled={busy || paused !== ""}
              title={s.place === "cur" ? `It's in ${s.label}` : `Put it in ${s.label}${who ? ` (releases ${who}'s claim)` : ""}`}
              onClick={() => {
                if (s.place !== "cur") onPick(s.id);
              }}
            >
              <i aria-hidden="true" />
              <span>{s.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
