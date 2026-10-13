import { STAGES } from "./lib";
import { trackPlace } from "./paneData";

/** The seven stages as one track, the task's own lit. Clicking another stage is the owner's
 *  override (the core's set_stage), with no confirmation: the pane offers Undo instead. Leaving
 *  Building releases the claim. */
export default function StageTrack({ stage, busy, onPick }: { stage: string; busy: boolean; onPick: (to: string) => void }) {
  return (
    <div className="track-wrap" data-part="track">
      <div className="track-label">
        <span className="lbl">Stage</span>
        <span className="meta small">click any stage to move it there</span>
      </div>
      <div className="stage-track" role="group" aria-label="Stage">
        {STAGES.map((s) => {
          const place = trackPlace(s.id, stage);
          return (
            <button
              key={s.id}
              type="button"
              className={`${place} s-${s.id}`}
              aria-pressed={place === "cur"}
              disabled={busy}
              title={place === "cur" ? `It's in ${s.label}` : `Put it in ${s.label}`}
              onClick={() => {
                if (place !== "cur") onPick(s.id);
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
