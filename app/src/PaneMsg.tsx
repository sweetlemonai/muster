import { stageLabel } from "./lib";
import type { Msg, Undo } from "./useTaskActions";
import { Button } from "./ui";

/** What the pane says of its last change, under the sections: a done change in green with its Undo,
 *  the core's refusal as is in red, in the pane (no overlay). A move on the track the core refuses,
 *  like a started task's into Your check before its branch is in main, reads here. */
export default function PaneMsg({ msg, undo, busy, onUndo }: { msg: Msg; undo: Undo | null; busy: boolean; onUndo: (u: Undo) => void }) {
  return (
    <p className={msg.ok ? "ok small pane-msg" : "err small pane-msg"} role="status">
      {msg.text}
      {undo && (
        <Button type="button" className="undo" disabled={busy} onClick={() => onUndo(undo)}>
          Undo: back to {stageLabel(undo.back)}
        </Button>
      )}
    </p>
  );
}
