import { tl } from "./i18n";

/** The gold "You" ribbon (0022): the owner's own task, wherever it sits among agents' tasks. The
 *  word is on it, so colour isn't the only cue. On a card it sits in the top-right corner (top-left
 *  right to left), and on an agent's task the owner holds, or a note the owner shapes, it says so
 *  (0097, never beside the holder ribbon): `spoken` is then the whole sentence a screen reader
 *  hears ("held by you", one key per case, mu-147), and the shown word is hidden from it, as the
 *  holder ribbon's name is. A note the owner shapes whose lease ran out says "quiet" too, in words. */
export default function YouRibbon({ title, spoken, quiet = false }: { title?: string; spoken?: string; quiet?: boolean }) {
  const shown = quiet ? `${tl("youRibbon.label")} · ${tl("youRibbon.quiet")}` : tl("youRibbon.label");
  return (
    <span className="you-ribbon" title={title ?? tl("youRibbon.title")}>
      {spoken ? (
        <>
          <span className="sr-only">{spoken}</span>
          <span aria-hidden="true">{shown}</span>
        </>
      ) : (
        shown
      )}
    </span>
  );
}
