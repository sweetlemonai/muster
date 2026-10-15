import { fmt } from "./i18n";
import { cx } from "./ui/cx";

type Count = { n: number; className?: string; title?: string; quiet?: boolean };

// A count in the header's toolbar (Waiting for you, Problems and its quiet unchecked-visions count;
// mu-202). It always takes room for three digits, in tabular figures, and at zero it's there but
// empty and hidden, so a poll that changes it never changes where the toolbar wraps or how tall it is.
// A quiet one is the outlined, unweighted count beside a main one.
export function HeadCount({ n, className, title, quiet }: Count) {
  const cls = cx("count", "count-reserve", quiet && "quiet", className);
  const Tag = quiet ? "small" : "b";
  return n ? <Tag className={cls} title={title}>{fmt.num(n)}</Tag> : <Tag className={cls} aria-hidden="true" />;
}

// A pill's counts, the shown ones first and the empty reserved slots after them, so a hidden slot
// never sits between the label and a shown count (mu-202 review R2). The width is the same either way.
export function HeadCounts({ counts }: { counts: (Count & { id: string })[] }) {
  const shown = [...counts.filter((c) => c.n), ...counts.filter((c) => !c.n)];
  return <>{shown.map(({ id, ...c }) => <HeadCount key={id} {...c} />)}</>;
}
