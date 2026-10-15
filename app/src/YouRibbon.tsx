/** The gold "You" ribbon (0022): the owner's own task, wherever it sits among agents' tasks. The
 *  word is on it, so colour isn't the only cue. On a card it sits in the top-right corner (top-left
 *  right to left), and on an agent's task the owner holds, or a note the owner shapes, it says so
 *  (0097, never beside the holder ribbon): `spoken` goes before "You" for a screen reader, as the
 *  holder ribbon's does. A note the owner shapes whose lease ran out says "quiet" too, in words. */
export default function YouRibbon({ title = "Your own task: only you do it, agents never take it", spoken, quiet = false }: { title?: string; spoken?: string; quiet?: boolean }) {
  return <span className="you-ribbon" title={title}>{spoken && <span className="sr-only">{`${spoken} `}</span>}You{quiet && " · quiet"}</span>;
}
