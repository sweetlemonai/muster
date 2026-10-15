import { useEffect, useState } from "react";
import { api, type OldPause } from "./api";
import { Trans, tn, useT } from "./i18n";
import { failedLine, fixItems, offerShown, oldPauseLine } from "./oldPauseData";
import { Button } from "./ui";

/** The offer, inline above the board (no dialog): every old pause it would change, then Make them
 *  pauses and Not now. Either is the answer: the board doesn't offer it again. */
export function OldPausesNotice({ list, busy, error, onFix, onLater }: { list: OldPause[]; busy: boolean; error: string; onFix: () => void; onLater: () => void }) {
  const t = useT("board");
  const count = list.length;
  return (
    <section className="old-pauses" aria-label={t("oldPauses.aria")}>
      <p>{t("oldPauses.offer", { count })}</p>
      <ul>
        {list.map((p) => (
          <li key={`${p.queue}/${p.id}`}>{oldPauseLine(p)}</li>
        ))}
      </ul>
      <p className="old-pauses-do">
        <Button type="button" variant="primary" disabled={busy} onClick={onFix}>
          {t(count === 1 ? "oldPauses.fixIt" : "oldPauses.fixThem")}
        </Button>
        <Button type="button" disabled={busy} onClick={onLater}>
          {t("oldPauses.later")}
        </Button>
        <span className="meta">
          <Trans ns="board" k="oldPauses.either" components={{ code: <code /> }} />
        </span>
      </p>
      {error && <p className="err small" role="alert">{error}</p>}
    </section>
  );
}

/** Asks the core for old pauses once the state says the board hasn't offered yet, and shows the
 *  offer while there are some. Make them pauses converts the ones listed, then remembers it was offered;
 *  Not now only remembers. Then the board reloads (`onDone`); any it couldn't convert are said in
 *  the offer's place. */
export default function OldPausesOffer({ offered, onDone }: { offered: boolean | undefined; onDone: () => void }) {
  const [list, setList] = useState<OldPause[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [answered, setAnswered] = useState(false);
  const [failed, setFailed] = useState<string[]>([]);
  useT("board");
  useEffect(() => {
    if (offered !== false) return;
    let live = true;
    api.oldPauses().then(
      (got) => {
        if (live) setList(got);
      },
      (e: unknown) => {
        if (live) setError(tn("board", "oldPauses.cantLook", { error: String(e) }));
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
      // Exactly what the offer listed.
      const done = fix ? await api.fixOldPauses(fixItems(list ?? [])) : null;
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
