import { useEffect, useState } from "react";
import { api, type OldPause } from "./api";
import { failedLine, offerShown, oldPauseLine } from "./oldPauseData";
import { Button } from "./ui";

/** The offer, inline above the board (no dialog): every old pause it would change, then Make them
 *  pauses and Not now. Either is the answer: the board doesn't offer it again. */
export function OldPausesNotice({ list, busy, error, onFix, onLater }: { list: OldPause[]; busy: boolean; error: string; onFix: () => void; onLater: () => void }) {
  const n = list.length;
  return (
    <section className="old-pauses" aria-label="Old pauses">
      <p>
        {n === 1 ? "1 task was" : `${n} tasks were`} paused before Pause existed, so {n === 1 ? "it's" : "they're"} still blocked as paused. Make{" "}
        {n === 1 ? "it a pause" : "them pauses"}? Each keeps its reason and since when, and moves to the Paused lane.
      </p>
      <ul>
        {list.map((p) => (
          <li key={`${p.queue}/${p.id}`}>{oldPauseLine(p)}</li>
        ))}
      </ul>
      <p className="old-pauses-do">
        <Button type="button" variant="primary" disabled={busy} onClick={onFix}>
          Make them pauses
        </Button>
        <Button type="button" disabled={busy} onClick={onLater}>
          Not now
        </Button>
        <span className="meta">
          Either way it isn't offered again; <code>muster check --fix-pauses</code> does it any time.
        </span>
      </p>
      {error && <p className="err small" role="alert">{error}</p>}
    </section>
  );
}

/** Asks the core for old pauses once the state says the board hasn't offered yet, and shows the
 *  offer while there are some. Make them pauses converts them all, then remembers it was offered;
 *  Not now only remembers. Then the board reloads (`onDone`); any it couldn't convert are said in
 *  the offer's place. */
export default function OldPausesOffer({ offered, onDone }: { offered: boolean | undefined; onDone: () => void }) {
  const [list, setList] = useState<OldPause[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [answered, setAnswered] = useState(false);
  const [failed, setFailed] = useState<string[]>([]);
  useEffect(() => {
    if (offered !== false) return;
    let live = true;
    api.oldPauses().then(
      (got) => {
        if (live) setList(got);
      },
      (e: unknown) => {
        if (live) setError(`Can't look for old pauses: ${String(e)}`);
      },
    );
    return () => {
      live = false;
    };
  }, [offered]);
  if (answered) return failed.length ? <p className="err small" role="alert">{failedLine(failed)}</p> : null;
  if (!offerShown(offered, list)) return error && offered === false ? <p className="err small" role="alert">{error}</p> : null;
  const answer = async (fix: boolean) => {
    setBusy(true);
    setError("");
    try {
      const done = fix ? await api.fixOldPauses() : null;
      await api.setOldPausesOffered();
      setFailed(done?.failed ?? []);
      setAnswered(true);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return <OldPausesNotice list={list ?? []} busy={busy} error={error} onFix={() => void answer(true)} onLater={() => void answer(false)} />;
}
