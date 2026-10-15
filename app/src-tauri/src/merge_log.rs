//! The work log's merge driver (task 0067, decision 0009): `muster merge-log %O %A %B`, which git
//! runs for `docs/WORKLOG.md` (`.gitattributes`: `docs/WORKLOG.md merge=muster-log`). Every task
//! puts its entry at the top of the log, or under a later-dated one that landed on main meanwhile,
//! and its review rounds' entries on top of that, so two branches always clash; this keeps every
//! new entry whole, newest first, so a merge or a rebase doesn't stop for them (0190: under an
//! entry, not only on top; 0196: at any number of points). An older entry changed or removed on
//! one side merges too, entry by entry (0209). Anything else is a plain three-way merge, conflict
//! markers and all, and so is a result that would lose an entry neither side touched (0218).
//! Git finds the driver in config: `muster start`
//! and `muster init` write it to the repo's own config when it's missing (`configure`), and
//! Muster's merge passes it with `git -c` (`config_args`), so it never depends on that.

use crate::parallel::{git, git_output};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The driver's name in `.gitattributes` (`merge=muster-log`) and in git config.
pub const DRIVER: &str = "muster-log";
/// What git says the driver is (`merge.muster-log.name`).
const NAME: &str = "muster work log: entries stay whole, newest on top";
/// A larger file is no work log: git's own merge-file merges it (`run`).
const MAX_BYTES: u64 = 64 << 20;

/// The driver git runs, through `sh`, with `program` (a word for the shell, `%` doubled) as the
/// muster to run. When that muster can't start or gives up (exit 2 and up: not on the PATH, as
/// for a GUI git client; killed; its own fallback failed), git's merge-file writes the conflict
/// markers in its place, so a log git can't merge always shows both sides, never ours alone.
pub(crate) fn driver_command(program: &str) -> String {
    format!(
        "{program} merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; git merge-file -L ours -L base -L theirs %A %O %B"
    )
}

/// A log split at its `## ` headings: what comes before the first one, then each entry, from its
/// heading to the next. Every byte is in one part, so the parts put back together are the file.
struct Log<'a> {
    preamble: &'a [u8],
    entries: Vec<&'a [u8]>,
}

/// A fence line (CommonMark): up to 3 spaces, then 3 or more backticks or tildes. Its character,
/// how many, and what follows them.
fn fence(line: &[u8]) -> Option<(u8, usize, &[u8])> {
    let indent = line.iter().take_while(|&&b| b == b' ').count();
    let rest = line.get(indent..).filter(|_| indent <= 3)?;
    let c = *rest.first().filter(|c| matches!(c, b'`' | b'~'))?;
    let n = rest.iter().take_while(|&&b| b == c).count();
    (n >= 3).then(|| (c, n, &rest[n..]))
}

/// Fenced code blocks, followed line by line (CommonMark): a fence closes on the same character,
/// at least as many, and nothing after them but spaces; one never closed runs to the end. Also
/// what the lenses' block reader skips (0076), so both read fences alike.
#[derive(Default)]
pub(crate) struct Fences {
    open: Option<(u8, usize)>,
}

impl Fences {
    /// Is a fence open after the lines read so far?
    pub(crate) fn open(&self) -> bool {
        self.open.is_some()
    }

    /// Is `line`, the next line, code: a fence line itself, or inside an open fence?
    pub(crate) fn fenced(&mut self, line: &[u8]) -> bool {
        match (self.open, fence(line)) {
            (Some((c, n)), Some((c2, n2, rest)))
                if c2 == c && n2 >= n && rest.iter().all(|b| b" \t\r\n".contains(b)) =>
            {
                self.open = None;
                true
            }
            (Some(_), _) => true,
            // A backtick fence's info string can't hold a backtick (then it's inline code).
            (None, Some((c, n, rest))) if c == b'~' || !rest.contains(&b'`') => {
                self.open = Some((c, n));
                true
            }
            (None, _) => false,
        }
    }
}

/// Split at every `## ` line outside a fenced code block (`Fences`), so a heading quoted in an
/// entry's code stays in that entry.
fn split(text: &[u8]) -> Log<'_> {
    let mut starts = Vec::new();
    let mut at = 0;
    let mut fences = Fences::default();
    for line in text.split_inclusive(|&b| b == b'\n') {
        if !fences.fenced(line) && line.starts_with(b"## ") {
            starts.push(at);
        }
        at += line.len();
    }
    let first = starts.first().copied().unwrap_or(text.len());
    let ends = starts.iter().skip(1).copied().chain([text.len()]);
    Log {
        preamble: &text[..first],
        entries: starts.iter().zip(ends).map(|(&s, e)| &text[s..e]).collect(),
    }
}

/// The date an entry's heading starts with (`## 2026-10-14 — …`), as `YYYY-MM-DD`.
fn date(entry: &[u8]) -> Option<&[u8]> {
    let d = entry.get(3..13)?;
    let digits = |r: std::ops::Range<usize>| d[r].iter().all(u8::is_ascii_digit);
    let dated = digits(0..4) && d[4] == b'-' && digits(5..7) && d[7] == b'-' && digits(8..10);
    let ends = entry.get(13).is_none_or(|b| !b.is_ascii_digit());
    (dated && ends).then_some(d)
}

/// An entry's identity (0209): its `## ` heading line, without its line end.
fn heading(entry: &[u8]) -> &[u8] {
    let line = entry.split(|&b| b == b'\n').next().unwrap_or(entry);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// One run of new entries a side put in among the base's (0190, 0196): the points in the base's
/// entries it can be at (0 is on top; more than one when the run repeats the base entries below
/// it, or when the side removed the base entries beside it, 0209), where it starts in the side's
/// entries at the first of them, and how many entries it has. Up to point `turn` the run is the
/// side's entries `at..at + len`; at `turn + j` it's `at + j..at + j + len`: a different turn of
/// the same entries, so it's sliced at the point chosen, never moved there from another.
struct Run {
    points: std::ops::RangeInclusive<usize>,
    turn: usize,
    at: usize,
    len: usize,
}

impl Run {
    /// The run's entries in `side` when it's at `point`, one of its points.
    fn at<'a, 'l>(&self, side: &'l Log<'a>, point: usize) -> &'l [&'a [u8]] {
        let k = self.at + point.saturating_sub(self.turn);
        &side.entries[k..k + self.len]
    }
}

/// The runs of new entries a side put in among the base's, top to bottom, when that's all it
/// changed (the same preamble, the base's entries untouched and in order around them) and each
/// new entry has a date (0196: at any number of points, not only one).
///
/// A side that reads as one run is read as mu-190 read it. Otherwise the base's entries are
/// matched from the bottom, each to the lowest side entry it can be, so
/// every run is as high as it can go: one ordered pass, no search. Then each run is let down
/// over the base entries below it that it repeats, as far as the next run: the other points it
/// can be at, all giving the side's file.
fn inserted(base: &Log, side: &Log) -> Option<Vec<Run>> {
    let (b, s) = (&base.entries, &side.entries);
    if side.preamble != base.preamble || s.len() < b.len() {
        return None;
    }
    let dated = |run: &[&[u8]]| run.iter().all(|e| date(e).is_some());
    // One run first, as mu-190 read it: at point k the side is b[..k], the run, b[k..], k no
    // further down than the entries the same from the top, and no higher than the entries the
    // same from the bottom allow. A run holding a copy of a base entry can also read as several
    // runs below, so this keeps mu-190's bytes for every side it took.
    let same_above = b.iter().zip(s.iter()).take_while(|(x, y)| x == y).count();
    let same_below = b
        .iter()
        .rev()
        .zip(s.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (from, len) = (b.len() - same_below, s.len() - b.len());
    if len == 0 {
        return (s == b).then(Vec::new);
    }
    if from <= same_above {
        return dated(&s[from..from + len]).then(|| {
            vec![Run {
                points: from..=same_above,
                turn: from,
                at: from,
                len,
            }]
        });
    }
    // pos[i]: where base entry i is in the side.
    let mut pos = vec![0; b.len()];
    let mut j = s.len();
    for (i, e) in b.iter().enumerate().rev() {
        j = s[..j].iter().rposition(|x| x == e)?;
        pos[i] = j;
    }
    // Gap g (point g: above base entry g, or under the last at g == b.len()) is the side's
    // entries start(g)..end(g).
    let start = |g: usize| if g == 0 { 0 } else { pos[g - 1] + 1 };
    let end = |g: usize| pos.get(g).copied().unwrap_or(s.len());
    let mut runs = Vec::new();
    let mut g = 0;
    while g <= b.len() {
        let (at, len) = (start(g), end(g) - start(g));
        if len == 0 {
            g += 1;
            continue;
        }
        if !dated(&s[at..at + len]) {
            return None;
        }
        // Down one point, past base entry p: the run's first entry at that point is p itself,
        // and the gap under p is empty, so the next base entry still follows the run.
        let mut p = g;
        while p < b.len() && s[at + p - g] == b[p] && start(p + 1) == end(p + 1) {
            p += 1;
        }
        runs.push(Run {
            points: g..=p,
            turn: g,
            at,
            len,
        });
        g = p + 1;
    }
    Some(runs)
}

/// What a side did to one base entry (0209): kept it byte for byte, changed it (a side entry with
/// its heading, in its place), or removed it.
#[derive(Clone, Copy, PartialEq)]
enum Fate<'a> {
    Kept,
    Changed(&'a [u8]),
    Removed,
}

/// A side read against the base (0209): what it did to each base entry, and its runs of new
/// entries, top to bottom.
struct Reading<'a> {
    fates: Vec<Fate<'a>>,
    runs: Vec<Run>,
}

/// Read a side against the base. A side that only added runs is read as `inserted` reads it, so
/// mu-196's bytes stand for every side it took. Otherwise each base entry is matched to a side
/// entry in order (`matched`): byte-equal is kept, the same heading is changed, none is removed;
/// every other side entry is new and must be dated, in a run between the matched entries around
/// it. Such a run can be at any point among the base entries removed beside it, and is let down
/// over kept base entries below it that it repeats, as `inserted` does. None when the preamble
/// changed or a new entry has no date.
fn read_side<'a>(base: &Log<'a>, side: &Log<'a>) -> Option<Reading<'a>> {
    let (b, s) = (&base.entries, &side.entries);
    if let Some(runs) = inserted(base, side) {
        let fates = vec![Fate::Kept; b.len()];
        return Some(Reading { fates, runs });
    }
    if side.preamble != base.preamble {
        return None;
    }
    let pos = matched(b, s);
    let fates: Vec<Fate> = pos
        .iter()
        .zip(b.iter())
        .map(|(p, e)| match *p {
            None => Fate::Removed,
            Some(k) if s[k] == *e => Fate::Kept,
            Some(k) => Fate::Changed(s[k]),
        })
        .collect();
    // (base index, side index) of every matched entry, in order on both.
    let pairs: Vec<(usize, usize)> = pos
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.map(|k| (i, k)))
        .collect();
    let mut is_new = vec![true; s.len()];
    for &(_, k) in &pairs {
        is_new[k] = false;
    }
    let mut runs = Vec::new();
    let mut at = 0;
    while at < s.len() {
        if !is_new[at] {
            at += 1;
            continue;
        }
        let len = is_new[at..].iter().take_while(|&&n| n).count();
        if s[at..at + len].iter().any(|e| date(e).is_none()) {
            return None;
        }
        // The matched base entries above and below it; the ones between are removed.
        let next = pairs.partition_point(|&(_, k)| k < at);
        let start = next.checked_sub(1).map_or(0, |p| pairs[p].0 + 1);
        let turn = pairs.get(next).map_or(b.len(), |&(i, _)| i);
        // Down one point, past kept base entry p, as `inserted` lets a run down: the run's first
        // entry at that point is p itself, and nothing new is under p.
        let mut p = turn;
        while p < b.len()
            && fates[p] == Fate::Kept
            && s[at + p - turn] == b[p]
            && pos[p] == Some(at + len + p - turn)
            && if p + 1 < b.len() {
                pos[p + 1] == Some(at + len + p - turn + 1)
            } else {
                at + len + p - turn + 1 == s.len()
            }
        {
            p += 1;
        }
        runs.push(Run {
            points: start..=p,
            turn,
            at,
            len,
        });
        at += len;
    }
    Some(Reading { fates, runs })
}

/// Where each base entry is in the side, in order on both, or None (0209). First the entries
/// that are in each once, the longest run of them in the same order (patience diff's anchors), so
/// a removed copy of an entry the base has twice doesn't pull the others out of place. Then,
/// between matched entries, the same bytes, then the same heading, each base entry from the
/// bottom to the lowest side entry it can be. Every step is one ordered pass with a hash map.
fn matched(b: &[&[u8]], s: &[&[u8]]) -> Vec<Option<usize>> {
    use std::collections::HashMap;
    let mut seen: HashMap<&[u8], (usize, usize, usize, usize)> = HashMap::new();
    for (i, e) in b.iter().enumerate() {
        let c = seen.entry(e).or_insert((0, i, 0, 0));
        c.0 += 1;
    }
    for (k, e) in s.iter().enumerate() {
        if let Some(c) = seen.get_mut(e) {
            c.2 += 1;
            c.3 = k;
        }
    }
    let mut once: Vec<(usize, usize)> = seen
        .values()
        .filter(|c| c.0 == 1 && c.2 == 1)
        .map(|c| (c.1, c.3))
        .collect();
    once.sort_unstable();
    // The longest increasing run of side indices (patience sorting, with links back).
    let mut tails: Vec<usize> = Vec::new();
    let mut back = vec![usize::MAX; once.len()];
    for (n, &(_, k)) in once.iter().enumerate() {
        let t = tails.partition_point(|&m| once[m].1 < k);
        if t > 0 {
            back[n] = tails[t - 1];
        }
        if t == tails.len() {
            tails.push(n);
        } else {
            tails[t] = n;
        }
    }
    let mut pos = vec![None; b.len()];
    let mut n = tails.last().copied().unwrap_or(usize::MAX);
    while n != usize::MAX {
        pos[once[n].0] = Some(once[n].1);
        n = back[n];
    }
    match_gaps(b, s, &mut pos, |e| e);
    match_gaps(b, s, &mut pos, heading);
    pos
}

/// Between each two matched entries, match the base entries left there to side entries left
/// there with the same `key`, from the bottom, each to the lowest it can be.
fn match_gaps(b: &[&[u8]], s: &[&[u8]], pos: &mut [Option<usize>], key: fn(&[u8]) -> &[u8]) {
    let (mut i, mut j) = (b.len(), s.len());
    loop {
        let mut lo = i;
        while lo > 0 && pos[lo - 1].is_none() {
            lo -= 1;
        }
        let above = lo.checked_sub(1).and_then(|m| pos[m]);
        let from = above.map_or(0, |k| k + 1);
        if lo < i && from < j {
            let mut by_key: std::collections::HashMap<&[u8], Vec<usize>> =
                std::collections::HashMap::new();
            for (k, e) in s.iter().enumerate().take(j).skip(from) {
                by_key.entry(key(e)).or_default().push(k);
            }
            let mut bound = j;
            for m in (lo..i).rev() {
                if let Some(list) = by_key.get_mut(key(b[m])) {
                    while list.last().is_some_and(|&k| k >= bound) {
                        list.pop();
                    }
                    if let Some(k) = list.pop() {
                        pos[m] = Some(k);
                        bound = k;
                    }
                }
            }
        }
        match above {
            Some(k) => (i, j) = (lo - 1, k),
            None => break,
        }
    }
}

/// The entry-wise three-way merge (0196, 0209). Each side is read against the base
/// (`read_side`). Each base entry is kept when both kept it, else the version of the side that
/// changed or removed it; both changed it the same way, or both removed it, is that once. The
/// new entries go in at their points: one side's run as it is, or both sides' runs merged there
/// newest date first and, on the same date, theirs (the incoming side: the commit a rebase
/// replays, the branch a merge brings in) above ours; each side's entries keep their own order.
/// An entry both sides added is kept once, at ours' point (what the target branch already
/// shows). None (the conflict markers) when a side can't be read (a changed preamble, an undated
/// new entry), when the sides changed one entry differently or one changed what the other
/// removed, when a new entry has the heading of a base entry either side changed or removed
/// (an entry moved, or a copy of the one changed: which is which can't be told), when both
/// removed an entry but put different new entries where it was (an edited heading against a
/// removal or another edit), when one side's new entries are among base entries it removed and
/// the other side's run starts between two of those (which new entry was above it can't be
/// told), or when the result doesn't split back into the entries put in.
///
/// A run that fits several points goes where mu-190 put it: at a point the other side's run can
/// be at too, the topmost such, else its own topmost. A run over base entries its side removed
/// (an edited heading reads so) is at the first of them, where the old entry was: it shares that
/// point only with a run of the other side that starts there or above, whose entries go first on
/// the old entry's date or before, so what the other side put under the old entry stays under
/// the new one and what it put above stays above. The runs on
/// one side are at disjoint ranges of points, in order, so one pass over both pairs them.
fn entry_wise(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let b = split(base);
    let (o_log, t_log) = (split(ours), split(theirs));
    let (o_read, t_read) = (read_side(&b, &o_log)?, read_side(&b, &t_log)?);
    let mut touched = std::collections::HashSet::new();
    let mut slots: Vec<Option<&[u8]>> = Vec::with_capacity(b.entries.len());
    for ((e, of), tf) in b.entries.iter().zip(&o_read.fates).zip(&t_read.fates) {
        let slot = match (*of, *tf) {
            (Fate::Kept, Fate::Kept) => Some(*e),
            (Fate::Changed(x), Fate::Kept) | (Fate::Kept, Fate::Changed(x)) => Some(x),
            (Fate::Changed(x), Fate::Changed(y)) if x == y => Some(x),
            (Fate::Removed, Fate::Kept | Fate::Removed) | (Fate::Kept, Fate::Removed) => None,
            _ => return None,
        };
        if (*of, *tf) != (Fate::Kept, Fate::Kept) {
            touched.insert(heading(e));
        }
        slots.push(slot);
    }
    // An entry both sides removed, with new entries where it was: an edited heading reads so, and
    // can't be told from a plain removal beside an insert. Only the same new entries there on
    // both sides (the same edit) merge; else one side's edit would win over the other's removal
    // or edit unseen. A copy of a base entry there is no edited heading (one of a changed or
    // removed entry is refused below).
    // Each run's new entries, sorted, made once; and the run covering k, if any (a side's runs
    // are at disjoint ranges, in order, so one at most). Each pair of covering runs is compared
    // once, so this stays linear but for the sorts.
    let in_base: std::collections::HashSet<&[u8]> = b.entries.iter().copied().collect();
    let sorted = |read: &Reading, log: &Log<'_>| -> Vec<Vec<Vec<u8>>> {
        read.runs
            .iter()
            .map(|r| {
                let mut new: Vec<Vec<u8>> = r
                    .at(log, r.turn)
                    .iter()
                    .filter(|e| !in_base.contains(*e))
                    .map(|e| e.to_vec())
                    .collect();
                new.sort_unstable();
                new
            })
            .collect()
    };
    let covering = |read: &Reading, k: usize| {
        let next = read.runs.partition_point(|r| r.turn <= k);
        read.runs
            .get(next)
            .filter(|r| *r.points.start() <= k)
            .map(|_| next)
    };
    let both_removed = |(o, t): (&Fate, &Fate)| (o, t) == (&Fate::Removed, &Fate::Removed);
    let any = o_read.fates.iter().zip(&t_read.fates).any(both_removed);
    let (o_new, t_new) = if any {
        (sorted(&o_read, &o_log), sorted(&t_read, &t_log))
    } else {
        (Vec::new(), Vec::new())
    };
    let mut last = None;
    for (k, fates) in o_read.fates.iter().zip(&t_read.fates).enumerate() {
        if !both_removed(fates) {
            continue;
        }
        let pair = (covering(&o_read, k), covering(&t_read, k));
        if last == Some(pair) {
            continue;
        }
        last = Some(pair);
        let none = Vec::new();
        let o_side = pair.0.map_or(&none, |i| &o_new[i]);
        let t_side = pair.1.map_or(&none, |i| &t_new[i]);
        if o_side != t_side {
            return None;
        }
    }
    let (o_runs, t_runs) = (o_read.runs, t_read.runs);
    // A run over base entries its side removed, with a run of the other side's starting among
    // them: which of its entries were above that run (an edited heading's new version, say) can't
    // be told.
    let inside = |runs: &[Run], other: &[Run]| {
        runs.iter().filter(|r| *r.points.start() < r.turn).any(|r| {
            let next = other.partition_point(|x| x.points.start() <= r.points.start());
            other.get(next).is_some_and(|x| *x.points.start() < r.turn)
        })
    };
    if inside(&o_runs, &t_runs) || inside(&t_runs, &o_runs) {
        return None;
    }
    let (mut o_at, mut t_at) = (vec![0; o_runs.len()], vec![0; t_runs.len()]);
    let (mut i, mut j) = (0, 0);
    while i < o_runs.len() || j < t_runs.len() {
        match (o_runs.get(i), t_runs.get(j)) {
            (Some(o), Some(t)) => {
                let (os, ts) = (*o.points.start(), *t.points.start());
                let at = os.max(ts);
                // A run over entries its side removed (an edited heading among them) stays where
                // the first of them was, unless the other side's run starts there or above it:
                // the other side's entries under the old one stay under the new.
                let apart = (os < o.turn && ts > os) || (ts < t.turn && os > ts);
                if apart {
                    if os < ts {
                        o_at[i] = os;
                        i += 1;
                    } else {
                        t_at[j] = ts;
                        j += 1;
                    }
                } else if at <= *o.points.end().min(t.points.end()) {
                    (o_at[i], t_at[j]) = (at, at);
                    (i, j) = (i + 1, j + 1);
                } else if o.points.end() < t.points.start() {
                    o_at[i] = *o.points.start();
                    i += 1;
                } else {
                    t_at[j] = *t.points.start();
                    j += 1;
                }
            }
            (Some(o), None) => {
                o_at[i] = *o.points.start();
                i += 1;
            }
            (None, Some(t)) => {
                t_at[j] = *t.points.start();
                j += 1;
            }
            (None, None) => unreachable!("the loop runs while either side has a run left"),
        }
    }
    // Each run's point, its entries there, and, for one over base entries its side removed, the
    // date of the first of them (the old entry where the run is).
    let removing = |r: &Run| {
        let start = *r.points.start();
        (start < r.turn).then(|| date(b.entries[start])).flatten()
    };
    type Placed<'x> = (usize, Vec<&'x [u8]>, Option<&'x [u8]>);
    let o: Vec<Placed> = o_runs
        .iter()
        .zip(&o_at)
        .map(|(r, &at)| (at, r.at(&o_log, at).to_vec(), removing(r)))
        .collect();
    // Theirs' entries ours added too go only where ours has them, each copy once.
    let mut ours_added: std::collections::HashMap<&[u8], usize> = std::collections::HashMap::new();
    for e in o.iter().flat_map(|(_, run, _)| run) {
        *ours_added.entry(e).or_default() += 1;
    }
    let t: Vec<Placed> = t_runs
        .iter()
        .zip(&t_at)
        .map(|(r, &at)| {
            let kept = r
                .at(&t_log, at)
                .iter()
                .copied()
                .filter(|e| match ours_added.get_mut(e) {
                    Some(n) if *n > 0 => {
                        *n -= 1;
                        false
                    }
                    _ => true,
                })
                .collect();
            (at, kept, removing(r))
        })
        .collect();
    // Theirs' copies of ours' entries are left out above, and ours' are checked.
    if o.iter()
        .chain(&t)
        .flat_map(|(_, run, _)| run)
        .any(|e| touched.contains(heading(e)))
    {
        return None;
    }
    let eol = line_end(ours);
    let mut out = b.preamble.to_vec();
    let mut starts = Vec::new();
    let mut push = |part: &[u8]| {
        // An entry that ended the file without a line end doesn't run into the next one.
        if !out.is_empty() && !out.ends_with(b"\n") && !part.is_empty() {
            out.extend_from_slice(eol);
        }
        starts.push(out.len());
        out.extend_from_slice(part);
    };
    let (mut o, mut t) = (o.into_iter().peekable(), t.into_iter().peekable());
    for g in 0..=b.entries.len() {
        let (o_run, o_old) = o
            .next_if(|(at, ..)| *at == g)
            .map(|r| (r.1, r.2))
            .unwrap_or_default();
        let (t_run, t_old) = t
            .next_if(|(at, ..)| *at == g)
            .map(|r| (r.1, r.2))
            .unwrap_or_default();
        // On one date theirs go first. But a run over entries its side removed is where the
        // first of them was, so the other side's run there was above that old entry: on the old
        // entry's date or before (its edited heading, say) the other side's go first.
        let yields = |old: Option<&[u8]>, d: Option<&[u8]>| old.is_some() && d <= old;
        let (mut i, mut j) = (0, 0);
        while i < o_run.len() || j < t_run.len() {
            let theirs_next = match (o_run.get(i), t_run.get(j)) {
                (Some(a), Some(b)) if date(a) == date(b) => {
                    !yields(t_old, date(b)) || yields(o_old, date(a))
                }
                (Some(a), Some(b)) => date(b) > date(a),
                (None, Some(_)) => true,
                _ => false,
            };
            if theirs_next {
                push(t_run[j]);
                j += 1;
            } else {
                push(o_run[i]);
                i += 1;
            }
        }
        if let Some(Some(e)) = slots.get(g) {
            push(e);
        }
    }
    // Every entry still its own, where it was put: a fence an entry leaves open would take in
    // the headings after it (and one it closes again would let a quoted one out).
    let log = split(&out);
    let mut at = log.preamble.len();
    let got: Vec<usize> = log
        .entries
        .iter()
        .map(|e| {
            at += e.len();
            at - e.len()
        })
        .collect();
    if got != starts {
        return None;
    }
    before_check(&mut out);
    keeps_untouched(base, ours, theirs, &out).then_some(out)
}

/// The line end a merge writes where it adds one: CRLF when ours has one, else LF.
fn line_end(ours: &[u8]) -> &'static [u8] {
    if ours.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    }
}

/// Does `result` keep every entry neither side touched (0218)? An untouched entry is one
/// byte-identical in base, ours and theirs; the result must have it whole, as one of its own
/// entries, as many times as it's untouched: the fewest copies in base and in each side, where a
/// side's copies are less the base copies the other side alone changed. A changed copy is the
/// only evidence that lowers it: each base entry is matched to the side as `matched` matches it
/// (whatever the preamble or new entries are), and one matched to other bytes is changed. A side
/// can only have changed as many copies as it no longer holds whole, so a mismatch beside a copy
/// that's still there lowers nothing. A side's own changed copy is already missing from its
/// count, so it isn't taken off again. A removed copy never lowers it: an entry another one was
/// moved around reads as removed, though its bytes are still on that side. So a side removing
/// copy 2 while the other edits copy 1 needs none, a side editing copy 2 while the other keeps
/// both still needs copy 1, and both sides removing a different copy still need one. Whole
/// is byte for byte, but for the line end `entry_wise` adds to an entry that ended the file
/// without one. A preamble the same in all three must be the result's. It doesn't trust how the
/// result was made: a change to `matched` or the run placement that drops or cuts such an entry,
/// or a clean `git merge-file` that lined an entry up with its twin (every entry ends in the same
/// Verified line), is refused, never committed. An entry a side changed or removed isn't
/// untouched, so its change or removal merges as before. An entry the log holds more than once
/// is counted by content where no copy reads as changed, so some correct merges of one are
/// refused on purpose: a refusal leaves all three files whole, so nothing is lost.
fn keeps_untouched(base: &[u8], ours: &[u8], theirs: &[u8], result: &[u8]) -> bool {
    use std::collections::HashMap;
    let (b, o, t, r) = (split(base), split(ours), split(theirs), split(result));
    if b.preamble == o.preamble && b.preamble == t.preamble && r.preamble != b.preamble {
        return false;
    }
    fn counts<'a>(log: &Log<'a>) -> HashMap<&'a [u8], usize> {
        let mut c = HashMap::new();
        for e in &log.entries {
            *c.entry(*e).or_default() += 1;
        }
        c
    }
    let (bc, oc, tc, rc) = (counts(&b), counts(&o), counts(&t), counts(&r));
    let n = |c: &HashMap<&[u8], usize>, e: &[u8]| c.get(e).copied().unwrap_or(0);
    // Which base copies a side matched to other bytes.
    let changed = |s: &Log| -> Vec<bool> {
        matched(&b.entries, &s.entries)
            .iter()
            .zip(&b.entries)
            .map(|(p, e)| p.is_some_and(|k| s.entries[k] != *e))
            .collect()
    };
    let (o_changed, t_changed) = (changed(&o), changed(&t));
    // Per entry: the base copies only ours changed, and only theirs.
    let mut touched: HashMap<&[u8], (usize, usize)> = HashMap::new();
    for (i, e) in b.entries.iter().enumerate() {
        let c = touched.entry(*e).or_default();
        c.0 += usize::from(o_changed[i] && !t_changed[i]);
        c.1 += usize::from(t_changed[i] && !o_changed[i]);
    }
    bc.iter().all(|(&e, &bn)| {
        let (on, tn) = (n(&oc, e), n(&tc, e));
        let (only_o, only_t) = touched.get(e).copied().unwrap_or_default();
        // Each side's whole copies, less those the other side alone changed: a side's own
        // changed copies are already missing from its count.
        let o_kept = on.saturating_sub(only_t.min(bn.saturating_sub(tn)));
        let t_kept = tn.saturating_sub(only_o.min(bn.saturating_sub(on)));
        let need = bn.min(o_kept).min(t_kept);
        let mut got = n(&rc, e);
        if !e.ends_with(b"\n") {
            got += n(&rc, &[e, b"\n"].concat()) + n(&rc, &[e, b"\r\n"].concat());
        }
        got >= need
    })
}

/// The last fallback (0218), for a clean `git merge-file` result that loses an untouched entry
/// when `entry_wise` can't merge either: all of ours, base and theirs, each whole, between
/// merge-file's marker lines, so nothing is lost and git stops on it.
fn whole_file_markers(base: &[u8], ours: &[u8], theirs: &[u8]) -> Vec<u8> {
    let eol = line_end(ours);
    let mut out = Vec::with_capacity(base.len() + ours.len() + theirs.len() + 64);
    for (marker, part) in [
        (&b"<<<<<<< ours"[..], ours),
        (b"||||||| base", base),
        (b"=======", theirs),
        (b">>>>>>> theirs", b""),
    ] {
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.extend_from_slice(eol);
        }
        out.extend_from_slice(marker);
        out.extend_from_slice(eol);
        out.extend_from_slice(part);
    }
    out
}

#[cfg(test)]
type ResultHook = Box<dyn Fn(&mut Vec<u8>)>;

#[cfg(test)]
thread_local! {
    /// Changes a merged result just before `keeps_untouched` checks it: a test's lost line.
    static BEFORE_CHECK: std::cell::RefCell<Option<ResultHook>> =
        const { std::cell::RefCell::new(None) };
}

/// A test's change to a merged result before the check (`BEFORE_CHECK`); nothing in a build.
fn before_check(_text: &mut Vec<u8>) {
    #[cfg(test)]
    BEFORE_CHECK.with(|h| {
        if let Some(h) = h.borrow().as_ref() {
            h(_text)
        }
    });
}

/// The lines of `text` (counted from 1) that are conflict markers a merge left behind, outside
/// fenced code (`Fences`), where docs may quote them (0209): a line starting with `<<<<<<< `,
/// `||||||| ` or `>>>>>>> `, or one that is exactly `=======`. The gate's test runs it over the
/// repo's docs (`tests/conflict_markers.rs`).
pub fn stray_markers(text: &[u8]) -> Vec<usize> {
    let mut fences = Fences::default();
    let mut found = Vec::new();
    for (n, line) in text.split_inclusive(|&b| b == b'\n').enumerate() {
        if fences.fenced(line) {
            continue;
        }
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let marker = [&b"<<<<<<< "[..], b"||||||| ", b">>>>>>> "]
            .iter()
            .any(|m| line.starts_with(m));
        if marker || line == b"=======" {
            found.push(n + 1);
        }
    }
    found
}

/// How a merge of the log came out: the text to write, clean or with conflict markers. Refused is
/// a clean merge that would lose an untouched entry (0218): ours, base and theirs, whole, in
/// markers (`whole_file_markers`).
#[derive(Debug, PartialEq)]
pub enum Merged {
    Clean(Vec<u8>),
    Conflict(Vec<u8>),
    Refused(Vec<u8>),
}

fn read(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let size = fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if size > max {
        return Err(format!(
            "{} is {size} bytes, more than a work log's {max}",
            path.display()
        ));
    }
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Merge the three versions of the log at `base`, `ours` and `theirs`: a plain three-way merge
/// (`git merge-file`) when it's clean, else the entry-wise merge (`entry_wise`) when it can
/// tell what each side did to each entry, else the three-way merge's conflict markers. Either
/// clean result must keep every entry neither side touched (`keeps_untouched`, 0218), or it's
/// refused: merge-file's goes to the entry-wise merge, then to markers around all three.
pub fn merge(base: &Path, ours: &Path, theirs: &Path) -> Result<Merged, String> {
    merge_capped(base, ours, theirs, MAX_BYTES)
}

/// `merge`, refusing a file over `max` bytes.
fn merge_capped(base: &Path, ours: &Path, theirs: &Path, max: u64) -> Result<Merged, String> {
    let (b, o, t) = (read(base, max)?, read(ours, max)?, read(theirs, max)?);
    let out = Command::new("git")
        .args([
            "merge-file",
            "-p",
            "-L",
            "ours",
            "-L",
            "base",
            "-L",
            "theirs",
        ])
        .args([ours, base, theirs])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run git merge-file: {e}"))?;
    match out.status.code() {
        // A clean result that loses an untouched entry (0218) goes to the entry-wise merge, which
        // is checked the same way; when that can't merge either, all three, whole, in markers.
        Some(0) => {
            let mut text = out.stdout;
            before_check(&mut text);
            if keeps_untouched(&b, &o, &t, &text) {
                return Ok(Merged::Clean(text));
            }
            Ok(match entry_wise(&b, &o, &t) {
                Some(text) => Merged::Clean(text),
                None => Merged::Refused(whole_file_markers(&b, &o, &t)),
            })
        }
        // The number of conflicts; a negative one (an error) comes out as 128 and up.
        Some(1..=127) => Ok(match entry_wise(&b, &o, &t) {
            Some(text) => Merged::Clean(text),
            None => Merged::Conflict(out.stdout),
        }),
        _ => Err(format!(
            "git merge-file: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// `muster merge-log <base> <ours> <theirs>`, as git runs a merge driver: the result is written
/// into `ours`, whole or not at all (`replace`); 0 when it's clean, 1 with conflict markers. When
/// it can't merge (a file it can't read or over 64 MB, merge-file failing, `ours` it can't
/// replace), git's merge-file merges the untouched `ours` in place instead: its markers, or its
/// clean result. When even that fails, `ours` is left as it was and it's 2, which the configured
/// driver hands to merge-file again (`driver_command`).
pub fn run(args: &[String]) -> Result<i32, String> {
    run_capped(args, MAX_BYTES)
}

/// Replace `path` with `text` whole: written to a new file beside it (the same folder, so the
/// rename stays on one filesystem), with its permissions, then renamed over it. On any failure
/// `path` is as it was and the new file is gone, so a write that fails partway (a full disk)
/// never leaves a cut-off `ours` for the fallback to merge. Only a muster killed between making
/// the new file and renaming it leaves it behind (`.<name>.muster-merge-<pid>`, in the worktree
/// root, where git makes %A); %A is untouched then too.
fn replace(path: &Path, text: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".muster-merge-{}", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let fail = |what: &str, e: std::io::Error| format!("{what} {}: {e}", tmp.display());
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| fail("couldn't make", e))?;
    // No fsync: git reads %A straight back and writes the result itself, and a full sync
    // (F_FULLFSYNC on macOS) can fail on network and external volumes for no gain here.
    let written = file
        .write_all(text)
        .map_err(|e| fail("couldn't write", e))
        .and_then(|()| {
            let perms = fs::metadata(path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .permissions();
            fs::set_permissions(&tmp, perms).map_err(|e| fail("couldn't set the mode of", e))
        })
        .and_then(|()| {
            fs::rename(&tmp, path).map_err(|e| {
                format!(
                    "couldn't rename {} over {}: {e}",
                    tmp.display(),
                    path.display()
                )
            })
        });
    drop(file);
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// `run`, with `merge_capped`'s limit.
fn run_capped(args: &[String], max: u64) -> Result<i32, String> {
    let [base, ours, theirs] = args else {
        return Err(
            "merge-log takes the three files git gives a merge driver: muster merge-log %O %A %B (see `muster help`)"
                .into(),
        );
    };
    let (base, ours, theirs) = (Path::new(base), Path::new(ours), Path::new(theirs));
    let merged = merge_capped(base, ours, theirs, max).and_then(|m| {
        let (text, why) = match m {
            Merged::Clean(t) => (t, None),
            Merged::Conflict(t) => (
                t,
                Some("both sides changed more than new dated entries; left the conflict markers"),
            ),
            Merged::Refused(t) => (
                t,
                Some("refused the merge, which would lose an entry neither side changed; left ours, base and theirs whole between conflict markers"),
            ),
        };
        replace(ours, &text)?;
        Ok(why)
    });
    let e = match merged {
        Ok(None) => return Ok(0),
        Ok(Some(why)) => {
            say(&format!("muster merge-log: {why}"));
            return Ok(1);
        }
        Err(e) => e,
    };
    let out = Command::new("git")
        .args(["merge-file", "-L", "ours", "-L", "base", "-L", "theirs"])
        .args([ours, base, theirs])
        .stdin(Stdio::null())
        .output();
    match out.as_ref().map(|o| o.status.code()) {
        Ok(Some(0)) => {
            say(&format!("muster merge-log: {e}; git merge-file merged it"));
            Ok(0)
        }
        Ok(Some(1..=127)) => {
            say(&format!(
                "muster merge-log: {e}; git merge-file left the conflict markers"
            ));
            Ok(1)
        }
        _ => {
            let why = match &out {
                Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
                Err(e) => e.to_string(),
            };
            say(&format!(
                "muster merge-log: {e}; git merge-file failed too ({why}), so it's as it was"
            ));
            Ok(2)
        }
    }
}

/// A line on stderr for the person merging, its controls shown as the CLI shows them
/// (`i18n::escape_controls`, mu-221): it can quote a file's path or git's own words.
fn say(line: &str) {
    eprintln!("{}", crate::i18n::escape_controls(line));
}

/// Write the driver to the repo's own config (`.git/config`, which every worktree reads) for
/// each of its two keys git doesn't have yet, from any config: one that's there, the owner's or
/// a custom one, and every other key, are left as they are. Returns the keys it wrote.
pub fn configure(dir: &Path) -> Result<Vec<String>, String> {
    let mut wrote = Vec::new();
    for (key, value) in missing(dir)? {
        git(dir, &["config", "--local", &key, &value])?;
        wrote.push(key);
    }
    Ok(wrote)
}

/// The driver's keys git doesn't have yet in `dir`'s repo, from any config, with the values
/// `configure` writes: none once both are set, whatever they say.
pub fn missing(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut missing = Vec::new();
    for (key, value) in [
        ("name", NAME.to_string()),
        ("driver", driver_command("muster")),
    ] {
        let key = format!("merge.{DRIVER}.{key}");
        let out = git_output(dir, &["config", "--get", &key])?;
        match out.status.code() {
            Some(0) => {}
            Some(1) => missing.push((key, value)),
            _ => {
                return Err(format!(
                    "git config --get {key}: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        }
    }
    Ok(missing)
}

/// Does the repo at `dir` have git merge `docs/WORKLOG.md` (or anything) with the driver: a
/// `merge=muster-log` in its `.gitattributes`, outside a comment?
pub fn wanted(dir: &Path) -> bool {
    let want = format!("merge={DRIVER}");
    fs::read_to_string(dir.join(".gitattributes")).is_ok_and(|t| {
        t.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .any(|l| l.split_whitespace().skip(1).any(|w| w == want))
    })
}

/// What `start` and `init` say when `configure` failed with `e` (0094): git's reason, what it
/// means, and the two lines that set the driver. The reason is the words after the last `: ` of
/// git's last line (`… could not lock config file .git/config: Operation not permitted`), so
/// neither the command's values nor a path is in it.
pub fn not_set(e: &str) -> String {
    let line = e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(e);
    let reason = line.rsplit(": ").next().unwrap_or(line).trim();
    format!(
        "Couldn't set the work log's merge driver in .git/config: {reason}.\n{}",
        fix_lines()
    )
}

/// What a missing driver means and the two `git config --local` lines that set it, for a person
/// or a session with the sandbox off to run (`start`, `init`, `check`); muster never runs them.
pub fn fix_lines() -> String {
    let mut out = String::from(
        "Without it, a clash in docs/WORKLOG.md is a conflict instead of merging by itself.\nTo set it, run these two lines in the repo, from outside the sandbox:\n",
    );
    for (key, value) in [
        ("name", NAME.to_string()),
        ("driver", driver_command("muster")),
    ] {
        out.push_str(&format!(
            "  git config --local merge.{DRIVER}.{key} {}\n",
            sh_quote(&value)
        ));
    }
    out
}

/// `-c` options that define the driver for one git command, run by the muster binary at
/// `muster`, so a merge works with no config written at all. Git expands `%` in the driver
/// before its shell sees it, so the path's `%` is doubled, then quoted for the shell. The
/// fallback is the configured one's: muster's own merge aborts on any conflict anyway, but a
/// muster that dies still leaves both sides.
pub fn config_args(muster: &Path) -> Result<Vec<String>, String> {
    let path = muster
        .to_str()
        .ok_or_else(|| format!("{} isn't a UTF-8 path", muster.display()))?;
    Ok(vec![
        "-c".into(),
        format!("merge.{DRIVER}.name={NAME}"),
        "-c".into(),
        format!(
            "merge.{DRIVER}.driver={}",
            driver_command(&sh_quote(&path.replace('%', "%%")))
        ),
    ])
}

/// `s` as one word for `sh`: as it is when that's safe, else in single quotes.
fn sh_quote(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-+,:@".contains(&b));
    if plain {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// This muster binary, which Muster's own merge has git run as the driver. A unit test runs in
/// cargo's test harness (`target/debug/deps/…`), so there it's the muster binary cargo built
/// beside it.
pub fn this_muster() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("can't find this muster binary: {e}"))?;
    #[cfg(test)]
    let exe = built_muster(&exe)?;
    Ok(exe)
}

/// The muster binary for a unit test, which runs in cargo's test harness
/// (`<target>/<profile>/deps/…`): built once per test run (`cargo build --bin muster`, quick when
/// it's up to date), into the same target and profile, so `cargo test --lib` gets a current one
/// too.
#[cfg(test)]
fn built_muster(harness: &Path) -> Result<PathBuf, String> {
    static BUILT: std::sync::OnceLock<Result<PathBuf, String>> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let profile_dir = harness
                .parent()
                .and_then(Path::parent)
                .ok_or("the test harness isn't in <target>/<profile>/deps")?;
            let target = profile_dir
                .parent()
                .ok_or("the test harness has no target folder")?;
            let profile = match profile_dir.file_name().and_then(|n| n.to_str()) {
                Some("debug") => "dev",
                Some(p) => p,
                None => return Err("the test harness's profile folder has no name".into()),
            };
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let out = Command::new(cargo)
                .args(["build", "--quiet", "--bin", "muster", "--profile", profile])
                .arg("--manifest-path")
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
                .arg("--target-dir")
                .arg(target)
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("couldn't run cargo to build the muster binary: {e}"))?;
            let bin = profile_dir.join("muster");
            if out.status.success() && bin.is_file() {
                Ok(bin)
            } else {
                Err(format!(
                    "couldn't build the muster binary the merge tests run as git's driver ({}): {}",
                    bin.display(),
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parallel::tests::g;

    const PRE: &str = "# Log\n\nNewest first.\n\n";
    const OLD: &str = "## 2026-10-12 — Old work\n**Did:** the old thing\n**Verified:** `gate: green (pnpm check)`\n\n";
    const OLDER: &str = "## 2026-10-11 — Older work\n**Did:** first\n";

    /// An entry dated `day`, ending in the Verified line every entry shares.
    fn entry(day: &str, what: &str) -> String {
        format!("## {day} — {what}\n**Did:** {what}, in two lines\nof text.\n**Verified:** `gate: green (pnpm check)`\n\n")
    }

    /// Merge three versions through files, as git hands them over.
    fn merged(base: &str, ours: &str, theirs: &str) -> Merged {
        merged_bytes(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
    }

    fn merged_bytes(base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), base).unwrap();
        fs::write(p("a"), ours).unwrap();
        fs::write(p("b"), theirs).unwrap();
        merge(&p("o"), &p("a"), &p("b")).unwrap()
    }

    fn clean(m: Merged) -> String {
        match m {
            Merged::Clean(t) => String::from_utf8(t).unwrap(),
            Merged::Conflict(t) | Merged::Refused(t) => {
                panic!("a conflict:\n{}", String::from_utf8_lossy(&t))
            }
        }
    }

    fn conflict(m: Merged) -> String {
        match m {
            Merged::Conflict(t) => String::from_utf8(t).unwrap(),
            Merged::Clean(t) => panic!("clean:\n{}", String::from_utf8_lossy(&t)),
            Merged::Refused(t) => panic!("refused:\n{}", String::from_utf8_lossy(&t)),
        }
    }

    fn refused(m: Merged) -> String {
        match m {
            Merged::Refused(t) => String::from_utf8(t).unwrap(),
            Merged::Clean(t) => panic!("clean:\n{}", String::from_utf8_lossy(&t)),
            Merged::Conflict(t) => panic!("a conflict:\n{}", String::from_utf8_lossy(&t)),
        }
    }

    #[test]
    fn merge_log_keeps_both_new_entries_whole_where_union_interleaves_them() {
        // Both end in the same Verified line: union matched it and cut A's off (decision 0009).
        let base = format!("{PRE}{OLD}{OLDER}");
        let a = entry("2026-10-14", "Task A");
        let b = entry("2026-10-13", "Task B");
        let got = clean(merged(
            &base,
            &format!("{PRE}{a}{OLD}{OLDER}"),
            &format!("{PRE}{b}{OLD}{OLDER}"),
        ));
        assert_eq!(got, format!("{PRE}{a}{b}{OLD}{OLDER}"), "newest date first");
        // The newer one on the incoming side goes on top just the same.
        let got = clean(merged(
            &base,
            &format!("{PRE}{b}{OLD}{OLDER}"),
            &format!("{PRE}{a}{OLD}{OLDER}"),
        ));
        assert_eq!(got, format!("{PRE}{a}{b}{OLD}{OLDER}"));
    }

    #[test]
    fn merge_log_puts_the_incoming_side_on_top_on_the_same_date() {
        let base = format!("{PRE}{OLD}");
        let mine = entry("2026-10-14", "Landed first");
        let incoming = entry("2026-10-14", "Lands now");
        let got = clean(merged(
            &base,
            &format!("{PRE}{mine}{OLD}"),
            &format!("{PRE}{incoming}{OLD}"),
        ));
        assert_eq!(got, format!("{PRE}{incoming}{mine}{OLD}"));
    }

    #[test]
    fn merge_log_interleaves_several_entries_by_date_keeping_each_sides_order() {
        let base = format!("{PRE}{OLD}");
        let (o1, o2) = (
            entry("2026-10-14", "Ours late"),
            entry("2026-10-12", "Ours early"),
        );
        let (t1, t2) = (
            entry("2026-10-13", "Theirs"),
            entry("2026-10-12", "Theirs early"),
        );
        let got = clean(merged(
            &base,
            &format!("{PRE}{o1}{o2}{OLD}"),
            &format!("{PRE}{t1}{t2}{OLD}"),
        ));
        assert_eq!(got, format!("{PRE}{o1}{t1}{t2}{o2}{OLD}"));
        // An entry both sides added (the same commit on both) is kept once.
        let both = entry("2026-10-13", "On both");
        let got = clean(merged(
            &base,
            &format!("{PRE}{o1}{both}{OLD}"),
            &format!("{PRE}{both}{OLD}"),
        ));
        assert_eq!(got, format!("{PRE}{o1}{both}{OLD}"));
    }

    #[test]
    fn merge_log_both_insert_at_the_same_point() {
        // The repro's shape (0190): a later-dated entry on top of both, ours one entry under it,
        // theirs four, all on one date. Theirs above ours, each side's order kept.
        let top = entry("2026-10-15", "Coordinator's log");
        let base = format!("{PRE}{top}{OLD}{OLDER}");
        let o = entry("2026-10-14", "Ours");
        let t: String = (1..=4)
            .map(|n| entry("2026-10-14", &format!("Theirs {n}")))
            .collect();
        let got = clean(merged(
            &base,
            &format!("{PRE}{top}{o}{OLD}{OLDER}"),
            &format!("{PRE}{top}{t}{OLD}{OLDER}"),
        ));
        assert_eq!(got, format!("{PRE}{top}{t}{o}{OLD}{OLDER}"));
        // The other way round: the incoming side's run still goes first on the same date.
        let got = clean(merged(
            &base,
            &format!("{PRE}{top}{t}{OLD}{OLDER}"),
            &format!("{PRE}{top}{o}{OLD}{OLDER}"),
        ));
        assert_eq!(got, format!("{PRE}{top}{o}{t}{OLD}{OLDER}"));
    }

    #[test]
    fn merge_log_inserts_at_a_deeper_point_interleave_newest_first() {
        let (top, mid) = (entry("2026-10-16", "Top"), entry("2026-10-15", "Middle"));
        let base = format!("{PRE}{top}{mid}{OLD}{OLDER}");
        let (o1, o2) = (
            entry("2026-10-14", "Ours late"),
            entry("2026-10-12", "Ours early"),
        );
        let (t1, t2) = (
            entry("2026-10-13", "Theirs"),
            entry("2026-10-12", "Theirs early"),
        );
        let got = clean(merged(
            &base,
            &format!("{PRE}{top}{mid}{o1}{o2}{OLD}{OLDER}"),
            &format!("{PRE}{top}{mid}{t1}{t2}{OLD}{OLDER}"),
        ));
        assert_eq!(got, format!("{PRE}{top}{mid}{o1}{t1}{t2}{o2}{OLD}{OLDER}"));
        // At the very bottom, after the last entry, and an entry both added kept once.
        let both = entry("2026-10-10", "On both");
        let (o, t) = (entry("2026-10-10", "Ours"), entry("2026-10-10", "Theirs"));
        let got = clean(merged(
            &base,
            &format!("{base}{o}{both}"),
            &format!("{base}{t}{both}"),
        ));
        assert_eq!(got, format!("{base}{t}{o}{both}"));
        // A run that repeats the entry above it fits two points, as a different turn at each:
        // ours [S, N, S, Old] is [N, S] under S, or [S, N] on top. Theirs is under S, so ours is
        // taken there too, and the result is still ours with theirs' entry added.
        let (s, n, t) = (
            entry("2026-10-15", "Sync"),
            entry("2026-10-15", "N"),
            entry("2026-10-15", "T"),
        );
        let got = entry_wise(
            format!("{PRE}{s}{OLD}").as_bytes(),
            format!("{PRE}{s}{n}{s}{OLD}").as_bytes(),
            format!("{PRE}{s}{t}{OLD}").as_bytes(),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(got).unwrap(),
            format!("{PRE}{s}{t}{n}{s}{OLD}")
        );
    }

    #[test]
    fn merge_log_inserts_still_give_markers_for_anything_else() {
        let top = entry("2026-10-15", "Top");
        let base = format!("{PRE}{top}{OLD}{OLDER}");
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-14", "B"));
        let (c, d) = (entry("2026-10-12", "C"), entry("2026-10-12", "D"));
        // Two runs on one side, the other inserting at one of them: merged per point (0196).
        assert_eq!(
            clean(merged(
                &base,
                &format!("{PRE}{top}{a}{OLD}{c}{OLDER}"),
                &format!("{PRE}{top}{b}{OLD}{OLDER}"),
            )),
            format!("{PRE}{top}{b}{a}{OLD}{c}{OLDER}")
        );
        // An undated inserted entry.
        conflict(merged(
            &base,
            &format!("{PRE}{top}{a}{OLD}{OLDER}"),
            &format!("{PRE}{top}## Unreleased\n- b\n\n{OLD}{OLDER}"),
        ));
        // A changed older entry beside the insert: one side's change, kept (0209).
        let changed = OLDER.replace("first", "changed");
        assert_eq!(
            clean(merged(
                &base,
                &format!("{PRE}{top}{a}{OLD}{changed}"),
                &format!("{PRE}{top}{b}{OLD}{OLDER}"),
            )),
            format!("{PRE}{top}{b}{a}{OLD}{changed}")
        );
        // A changed entry above the insert point: kept too, the runs under it merged (0209).
        for changed in [
            top.replace("Top,", "Top, changed,"),
            top.replace("(pnpm check)", "(changed)"),
        ] {
            let o = format!("{PRE}{changed}{a}{OLD}{OLDER}");
            let t = format!("{PRE}{top}{b}{OLD}{OLDER}");
            assert_eq!(
                inserted_text(&base, &o, &t),
                format!("{PRE}{changed}{b}{a}{OLD}{OLDER}")
            );
        }
        // A changed preamble.
        conflict(merged(
            &base,
            &format!("# Renamed\n\n{top}{a}{OLD}{OLDER}"),
            &format!("{PRE}{top}{b}{OLD}{OLDER}"),
        ));
        // Inserts at two different points: merge-file merges them itself, even with only a
        // one-line entry between them, and entry_wise gives the same (0196).
        let o = format!("{PRE}{a}{top}{OLD}{OLDER}");
        let t = format!("{PRE}{top}{OLD}{c}{OLDER}");
        let want = format!("{PRE}{a}{top}{OLD}{c}{OLDER}");
        assert_eq!(inserted_text(&base, &o, &t), want);
        assert_eq!(clean(merged(&base, &o, &t)), want);
        let o = format!("{PRE}{top}{a}{OLD}{OLDER}");
        let t = format!("{PRE}{top}{OLD}{d}{OLDER}");
        assert_eq!(
            inserted_text(&base, &o, &t),
            format!("{PRE}{top}{a}{OLD}{d}{OLDER}")
        );
        let one_line = "## 2026-10-12 — One line\n";
        let base1 = format!("{PRE}{top}{one_line}{OLDER}");
        let o1 = format!("{PRE}{top}{a}{one_line}{OLDER}");
        let t1 = format!("{PRE}{top}{one_line}{d}{OLDER}");
        let want = format!("{PRE}{top}{a}{one_line}{d}{OLDER}");
        assert_eq!(inserted_text(&base1, &o1, &t1), want);
        assert_eq!(clean(merged(&base1, &o1, &t1)), want);
    }

    /// `entry_wise`'s result as text; panics when it gives up.
    fn inserted_text(base: &str, ours: &str, theirs: &str) -> String {
        let got = entry_wise(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
            .expect("entry_wise gave up");
        String::from_utf8(got).unwrap()
    }

    /// `muster merge-log` run as git runs it, through the binary, in a home of its own with no
    /// MUSTER_BASE, MUSTER_ROOT or TASKS_ROOT: its exit code, what it left in ours, its stderr.
    fn through_binary(base: &[u8], ours: &[u8], theirs: &[u8]) -> (i32, Vec<u8>, String) {
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::create_dir(p("home")).unwrap();
        fs::write(p("o"), base).unwrap();
        fs::write(p("a"), ours).unwrap();
        fs::write(p("b"), theirs).unwrap();
        let out = Command::new(this_muster().unwrap())
            .arg("merge-log")
            .args([p("o"), p("a"), p("b")])
            .env("HOME", p("home"))
            .env_remove("MUSTER_BASE")
            .env_remove("MUSTER_ROOT")
            .env_remove("TASKS_ROOT")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            fs::read(p("a")).unwrap(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    const GIVE_UP: &str =
        "muster merge-log: both sides changed more than new dated entries; left the conflict markers";

    #[test]
    fn merge_log_inserts_at_any_number_of_points() {
        // mu-173's rebase (0196): theirs put two round entries on top and its first entry deeper,
        // under the base's newer entries; ours put one entry on top, on the same date.
        let (n1, n2) = (
            entry("2026-10-15", "New one"),
            entry("2026-10-15", "New two"),
        );
        let base = format!("{PRE}{n1}{n2}{OLD}{OLDER}");
        let o = entry("2026-10-15", "Ours");
        let (t1, t2) = (
            entry("2026-10-15", "Round two"),
            entry("2026-10-15", "Round one"),
        );
        let deep = entry("2026-10-14", "First entry");
        let ours = format!("{PRE}{o}{n1}{n2}{OLD}{OLDER}");
        let theirs = format!("{PRE}{t1}{t2}{n1}{n2}{deep}{OLD}{OLDER}");
        // Merge-file alone can't: it leaves the clash on top.
        conflict(merged_file_only(&base, &ours, &theirs));
        let want = format!("{PRE}{t1}{t2}{o}{n1}{n2}{deep}{OLD}{OLDER}");
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        let (code, text, err) = through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
        assert_eq!((code, String::from_utf8(text).unwrap()), (0, want), "{err}");
        // The sides swapped: the incoming side's run is still the one above on the same date.
        let want = format!("{PRE}{o}{t1}{t2}{n1}{n2}{deep}{OLD}{OLDER}");
        assert_eq!(clean(merged(&base, &theirs, &ours)), want);
        let (code, text, err) = through_binary(base.as_bytes(), theirs.as_bytes(), ours.as_bytes());
        assert_eq!((code, String::from_utf8(text).unwrap()), (0, want), "{err}");
    }

    #[test]
    fn merge_log_keeps_merge_files_clean_result_with_an_entry_both_added_twice() {
        // The same entry on top in ours and deeper in theirs: the points don't touch, so
        // merge-file merges it cleanly and its result stands, with the entry twice (0009).
        let (top, x) = (entry("2026-10-15", "Top"), entry("2026-10-14", "X"));
        let base = format!("{PRE}{top}{OLD}{OLDER}");
        let ours = format!("{PRE}{x}{top}{OLD}{OLDER}");
        let theirs = format!("{PRE}{top}{OLD}{x}{OLDER}");
        let want = format!("{PRE}{x}{top}{OLD}{x}{OLDER}");
        assert_eq!(clean(merged_file_only(&base, &ours, &theirs)), want);
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
    }

    /// Plain `git merge-file -p`, to show a case is one merge-file can't merge.
    fn merged_file_only(base: &str, ours: &str, theirs: &str) -> Merged {
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), base).unwrap();
        fs::write(p("a"), ours).unwrap();
        fs::write(p("b"), theirs).unwrap();
        let out = Command::new("git")
            .args(["merge-file", "-p"])
            .args([p("a"), p("o"), p("b")])
            .output()
            .unwrap();
        match out.status.code() {
            Some(0) => Merged::Clean(out.stdout),
            _ => Merged::Conflict(out.stdout),
        }
    }

    #[test]
    fn merge_log_inserts_merge_per_point() {
        let top = entry("2026-10-15", "Top");
        let base = format!("{PRE}{top}{OLD}{OLDER}");
        // Runs at the same two points on both sides: each point merged on its own, newest
        // first, theirs above ours on a date, each side's order kept.
        let (o1, o2, o3) = (
            entry("2026-10-17", "Ours newest"),
            entry("2026-10-16", "Ours new"),
            entry("2026-10-12", "Ours deep"),
        );
        let (t1, t2, t3) = (
            entry("2026-10-16", "Theirs new"),
            entry("2026-10-12", "Theirs deep"),
            entry("2026-10-11", "Theirs deeper"),
        );
        let ours = format!("{PRE}{o1}{o2}{top}{OLD}{o3}{OLDER}");
        let theirs = format!("{PRE}{t1}{top}{OLD}{t2}{t3}{OLDER}");
        let want = format!("{PRE}{o1}{t1}{o2}{top}{OLD}{t2}{o3}{t3}{OLDER}");
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        assert_eq!(inserted_text(&base, &ours, &theirs), want);
        // Runs at disjoint points on each side all go in as they are.
        let (a, b, c, d) = (
            entry("2026-10-16", "A"),
            entry("2026-10-14", "B"),
            entry("2026-10-12", "C"),
            entry("2026-10-10", "D"),
        );
        let ours = format!("{PRE}{a}{top}{OLD}{c}{OLDER}");
        let theirs = format!("{PRE}{top}{b}{OLD}{OLDER}{d}");
        assert_eq!(
            inserted_text(&base, &ours, &theirs),
            format!("{PRE}{a}{top}{b}{OLD}{c}{OLDER}{d}")
        );
        // An entry both sides added at the same point is kept once, beside a second point.
        let x = entry("2026-10-14", "On both");
        let ours = format!("{PRE}{a}{top}{x}{OLD}{OLDER}");
        let theirs = format!("{PRE}{top}{b}{x}{OLD}{c}{OLDER}");
        let want = format!("{PRE}{a}{top}{b}{x}{OLD}{c}{OLDER}");
        assert_eq!(inserted_text(&base, &ours, &theirs), want);
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        // At different points: kept once, at ours' point (what the target branch shows). Theirs
        // also inserts on top with ours, so merge-file can't merge it and it's ours to do.
        let ours = format!("{PRE}{a}{top}{x}{OLD}{OLDER}");
        let theirs = format!("{PRE}{b}{x}{top}{OLD}{OLDER}");
        let want = format!("{PRE}{a}{b}{top}{x}{OLD}{OLDER}");
        assert_eq!(inserted_text(&base, &ours, &theirs), want);
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        let ours = format!("{PRE}{a}{top}{OLD}{OLDER}{x}");
        assert_eq!(
            inserted_text(&base, &ours, &theirs),
            format!("{PRE}{a}{b}{top}{OLD}{OLDER}{x}")
        );
        // A run that repeats the entry above it (S) fits two points; beside a second run it's
        // placed as mu-190 places it: at the point the other side inserts at, else the topmost.
        let (s, n, t) = (
            entry("2026-10-15", "Sync"),
            entry("2026-10-15", "N"),
            entry("2026-10-15", "T"),
        );
        let base = format!("{PRE}{s}{OLD}{OLDER}");
        let ours = format!("{PRE}{s}{n}{s}{OLD}{c}{OLDER}");
        let theirs = format!("{PRE}{s}{t}{OLD}{d}{OLDER}");
        let want = format!("{PRE}{s}{t}{n}{s}{OLD}{c}{d}{OLDER}");
        for _ in 0..3 {
            assert_eq!(
                inserted_text(&base, &ours, &theirs),
                want,
                "the same every run"
            );
        }
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        // A run holding a copy of the base entry above it, with new entries on both sides of
        // the copy, is one run, as on main (mu-190's bytes), not a copy on top and the rest
        // under it.
        let (ba, bb) = (entry("2026-10-01", "Base A"), entry("2026-09-01", "Base B"));
        let (x, y, z) = (
            entry("2026-10-05", "X"),
            entry("2026-10-03", "Y"),
            entry("2026-10-04", "Z"),
        );
        assert_eq!(
            inserted_text(
                &format!("{PRE}{ba}{bb}"),
                &format!("{PRE}{ba}{x}{ba}{y}{bb}"),
                &format!("{PRE}{ba}{z}{bb}"),
            ),
            format!("{PRE}{ba}{x}{z}{ba}{y}{bb}")
        );
        // The other side at no point of its range: the topmost, [S, N] on top.
        let theirs = format!("{PRE}{s}{OLD}{d}{OLDER}");
        assert_eq!(
            inserted_text(&base, &ours, &theirs),
            format!("{PRE}{s}{n}{s}{OLD}{c}{d}{OLDER}")
        );
    }

    #[test]
    fn merge_log_multi_point_inserts_still_refuse_anything_else() {
        let top = entry("2026-10-15", "Top");
        let base = format!("{PRE}{top}{OLD}{OLDER}");
        let (a, b, c, d) = (
            entry("2026-10-16", "A"),
            entry("2026-10-16", "B"),
            entry("2026-10-14", "C"),
            entry("2026-10-12", "D"),
        );
        let theirs = format!("{PRE}{b}{top}{d}{OLD}{OLDER}");
        // An edited or removed base entry merges now (0209): ours' change, theirs' runs.
        let edited = OLD.replace("the old thing", "the old thing, edited");
        assert_eq!(
            clean(merged(
                &base,
                &format!("{PRE}{a}{top}{c}{edited}{OLDER}"),
                &theirs
            )),
            format!("{PRE}{b}{a}{top}{c}{d}{edited}{OLDER}")
        );
        assert_eq!(
            clean(merged(&base, &format!("{PRE}{a}{top}{c}{OLDER}"), &theirs)),
            format!("{PRE}{b}{a}{top}{c}{d}{OLDER}")
        );
        for (what, ours) in [
            (
                "a changed preamble",
                format!("# Renamed\n\n{a}{top}{c}{OLD}{OLDER}"),
            ),
            (
                "an undated new entry",
                format!("{PRE}{a}{top}## Unreleased\n- c\n\n{OLD}{OLDER}"),
            ),
        ] {
            assert!(
                entry_wise(base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).is_none(),
                "{what}"
            );
            let t = conflict(merged(&base, &ours, &theirs));
            assert!(
                t.contains("<<<<<<< ours\n") && t.contains(">>>>>>> theirs\n"),
                "{what}: {t}"
            );
            // Through the binary, as git runs it: the markers, 1, and the message.
            let (code, text, err) =
                through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!(code, 1, "{what}");
            assert_eq!(String::from_utf8(text).unwrap(), t, "{what}");
            assert_eq!(err.trim(), GIVE_UP, "{what}");
        }
    }

    #[test]
    fn merge_log_multi_point_inserts_keep_crlf_and_a_missing_last_line_end() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let (pre, top, old) = (crlf(PRE), crlf(&entry("2026-10-15", "Top")), crlf(OLD));
        let (a, b, c) = (
            crlf(&entry("2026-10-16", "A")),
            crlf(&entry("2026-10-16", "B")),
            crlf(&entry("2026-10-14", "C")),
        );
        let got = clean(merged(
            &format!("{pre}{top}{old}"),
            &format!("{pre}{a}{top}{old}"),
            &format!("{pre}{b}{top}{c}{old}"),
        ));
        assert_eq!(got, format!("{pre}{b}{a}{top}{c}{old}"));
        // The file's last line has no line end on every side: it stays that way.
        let last = "## 2026-10-12 — Old\r\n**Did:** no line end";
        let got = clean(merged(
            &format!("{pre}{top}{last}"),
            &format!("{pre}{a}{top}{last}"),
            &format!("{pre}{b}{top}{c}{last}"),
        ));
        assert_eq!(got, format!("{pre}{b}{a}{top}{c}{last}"));
        // An entry added under it gives it a line end: that side changed the base's last entry,
        // so its version stands, byte for byte, both ways round (0209).
        let tail = "## 2026-10-11 — Tail\r\n**Did:** no line end either";
        let (o, t) = (
            format!("{pre}{a}{top}{last}"),
            format!("{pre}{b}{top}{c}{last}\r\n{tail}"),
        );
        let base = format!("{pre}{top}{last}");
        assert_eq!(
            clean(merged(&base, &o, &t)),
            format!("{pre}{b}{a}{top}{c}{last}\r\n{tail}")
        );
        assert_eq!(
            clean(merged(&base, &t, &o)),
            format!("{pre}{a}{b}{top}{c}{last}\r\n{tail}")
        );
        let got = clean(merged(
            &format!("{pre}{top}{last}"),
            &format!("{pre}{b}{top}{c}{last}"),
            &format!("{pre}{a}{top}{last}"),
        ));
        assert_eq!(got, format!("{pre}{a}{b}{top}{c}{last}"));
    }

    #[test]
    fn merge_log_merges_and_rebases_a_branch_with_runs_at_two_points() {
        // mu-173's shape in a real repo: the task's one commit put its round entries on top and
        // its first entry deeper; main put one entry on top. Merge and rebase, both ways round.
        let n1 = entry("2026-10-15", "Landed before");
        let (t1, t2) = (
            entry("2026-10-15", "Round two"),
            entry("2026-10-15", "Round one"),
        );
        let deep = entry("2026-10-14", "First entry");
        let m = entry("2026-10-15", "Main's");
        let task_log = format!("{PRE}{t1}{t2}{n1}{deep}{OLD}");
        let main_log = format!("{PRE}{m}{n1}{OLD}");
        for (how, onto, incoming) in [
            ("merge", "main", "task"),
            ("merge", "task", "main"),
            ("rebase", "main", "task"),
            ("rebase", "task", "main"),
        ] {
            let dir = crate::testkit::tempdir().unwrap();
            let home = dir.path().join("home");
            fs::create_dir(&home).unwrap();
            let g = |d: &Path, args: &[&str]| home_g(&home, d, args);
            let repo = log_repo_with(dir.path(), &g);
            let bin = this_muster().unwrap();
            g(&repo, &["config", &format!("merge.{DRIVER}.name"), NAME]);
            let driver = driver_command(&sh_quote(&bin.to_str().unwrap().replace('%', "%%")));
            g(
                &repo,
                &["config", &format!("merge.{DRIVER}.driver"), &driver],
            );
            fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{n1}{OLD}")).unwrap();
            g(&repo, &["commit", "-qam", "the base"]);
            g(&repo, &["checkout", "-q", "-b", "task"]);
            fs::write(repo.join("docs/WORKLOG.md"), &task_log).unwrap();
            g(&repo, &["commit", "-qam", "the task"]);
            g(&repo, &["checkout", "-q", "main"]);
            fs::write(repo.join("docs/WORKLOG.md"), &main_log).unwrap();
            g(&repo, &["commit", "-qam", "main's entry"]);
            g(
                &repo,
                &[
                    "checkout",
                    "-q",
                    if how == "merge" { onto } else { incoming },
                ],
            );
            if how == "merge" {
                g(&repo, &["merge", "-q", "--no-edit", incoming]);
            } else {
                g(&repo, &["rebase", "-q", onto]);
            }
            // The incoming side (the branch merged in, the commit replayed) above on the date.
            let want = if incoming == "task" {
                format!("{PRE}{t1}{t2}{m}{n1}{deep}{OLD}")
            } else {
                format!("{PRE}{m}{t1}{t2}{n1}{deep}{OLD}")
            };
            let got = fs::read_to_string(repo.join("docs/WORKLOG.md")).unwrap();
            assert_eq!(got, want, "{how} {incoming} onto {onto}");
            assert_eq!(g(&repo, &["status", "--porcelain"]), "", "{how}");
        }
    }

    /// A small fixed-seed generator for the randomised check (xorshift64*).
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
        }
    }

    /// Is `small` in `big` in the same order (not necessarily side by side)?
    fn in_order(small: &[&[u8]], big: &[&[u8]]) -> bool {
        let mut it = big.iter();
        small.iter().all(|s| it.any(|b| b == s))
    }

    #[test]
    fn merge_log_randomised_multi_point_inserts_keep_every_entry_and_each_sides_order() {
        let mut clean_by_us = 0;
        for seed in 1..=150u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
            let day = |r: &mut Rng| format!("2026-10-{:02}", 10 + r.below(8));
            let base_entries: Vec<String> = (0..1 + rng.below(6))
                .map(|i| entry(&day(&mut rng), &format!("Base {i}")))
                .collect();
            let shared: Vec<String> = (0..2)
                .map(|i| entry(&day(&mut rng), &format!("Shared {i}")))
                .collect();
            // A side: the base with runs at random points, a run sometimes repeating its
            // neighbour or holding an entry the other side adds too. Always one run on top, so
            // the two sides clash and it's the driver's merge, not merge-file's.
            let side = |name: &str, rng: &mut Rng| -> String {
                let mut out = PRE.to_string();
                for k in 0..=base_entries.len() {
                    if k == 0 || rng.below(3) == 0 {
                        for i in 0..1 + rng.below(3) {
                            out += &match rng.below(6) {
                                0 if k > 0 => base_entries[k - 1].clone(),
                                1 => shared[rng.below(2)].clone(),
                                _ => entry(&day(rng), &format!("{name} {k}.{i}")),
                            };
                        }
                    }
                    if let Some(e) = base_entries.get(k) {
                        out += e;
                    }
                }
                out
            };
            let base = format!("{PRE}{}", base_entries.concat());
            let (ours, theirs) = (side("Ours", &mut rng), side("Theirs", &mut rng));
            let (code, text, err) =
                through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!(code, 0, "seed {seed}: {err}\n{ours}\n---\n{theirs}");
            let by_us = entry_wise(base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).as_deref()
                == Some(&text[..]);
            clean_by_us += usize::from(by_us);
            let (b, o, t, r) = (
                split(base.as_bytes()),
                split(ours.as_bytes()),
                split(theirs.as_bytes()),
                split(&text),
            );
            let why = format!("seed {seed}:\n{}", String::from_utf8_lossy(&text));
            assert_eq!(r.preamble, PRE.as_bytes(), "{why}");
            // Every entry neither side touched, whole (0218).
            assert!(
                keeps_untouched(base.as_bytes(), ours.as_bytes(), theirs.as_bytes(), &text),
                "untouched: {why}"
            );
            assert!(in_order(&b.entries, &r.entries), "the base's order: {why}");
            assert!(in_order(&o.entries, &r.entries), "ours' order: {why}");
            // Theirs' order, but for the entries ours added too (kept at ours' point).
            let count = |log: &[&[u8]], e: &[u8]| log.iter().filter(|x| **x == e).count();
            let ours_added = |e: &[u8]| count(&o.entries, e) > count(&b.entries, e);
            let theirs_kept: Vec<&[u8]> = t
                .entries
                .iter()
                .copied()
                .filter(|e| !ours_added(e))
                .collect();
            assert!(in_order(&theirs_kept, &r.entries), "theirs' order: {why}");
            // Every entry as many times as the side with the most of it has it where the driver
            // made the text; where merge-file did, at least that and at most the base's plus both
            // sides' new ones. And no other entry.
            for e in o.entries.iter().chain(&t.entries) {
                let (bn, on, tn) = (
                    count(&b.entries, e),
                    count(&o.entries, e),
                    count(&t.entries, e),
                );
                let rn = count(&r.entries, e);
                if by_us {
                    assert_eq!(rn, on.max(tn), "{why}");
                } else {
                    assert!(on.max(tn) <= rn && rn <= on + tn - bn, "{why}");
                }
            }
            for e in &r.entries {
                assert!(o.entries.contains(e) || t.entries.contains(e), "{why}");
            }
        }
        assert!(
            clean_by_us >= 100,
            "only {clean_by_us} merged by the driver"
        );
    }

    #[test]
    fn merge_log_randomised_one_sided_edits_and_removals_keep_every_entry_and_change() {
        let (mut clean_by_us, mut clean_in_order, mut refused) = (0, 0, 0);
        for seed in 1..=300u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
            // The first half dated at random; the second newest first down each file, as a real
            // log is, where each side's order is checked with every base entry as the merge
            // has it (an edited heading where the old one was).
            let in_order_dates = seed > 150;
            let at = |d: usize| format!("2026-10-{d:02}");
            let mut newest = 17;
            let base_days: Vec<usize> = (0..2 + rng.below(6))
                .map(|_| {
                    if in_order_dates {
                        newest -= rng.below(2);
                        newest
                    } else {
                        10 + rng.below(8)
                    }
                })
                .collect();
            let base_entries: Vec<String> = base_days
                .iter()
                .enumerate()
                .map(|(i, &d)| entry(&at(d), &format!("Base {i}")))
                .collect();
            let n = base_entries.len();
            let shared_days: Vec<usize> = (0..2).map(|_| 10 + rng.below(8)).collect();
            let shared: Vec<String> = shared_days
                .iter()
                .enumerate()
                .map(|(i, &d)| entry(&at(d), &format!("Shared {i}")))
                .collect();
            // What each base entry gets: 0-2 nothing; 3/4 ours/theirs edits it; 5/6 ours/theirs
            // removes it; 7/8 ours/theirs edits its heading; 9 both edit it alike; 10 both
            // remove it.
            // An entry both removed with new entries where it was is refused (it may be an
            // edited heading), so 10 is only for one between untouched entries, with no run
            // beside it.
            let mut plan: Vec<usize> = (0..n).map(|_| rng.below(11)).collect();
            for k in 0..n {
                let calm = |j: Option<usize>| j.is_some_and(|j| j < n && plan[j] <= 2);
                if plan[k] == 10 && !(calm(k.checked_sub(1)) && calm(Some(k + 1))) {
                    plan[k] = 0;
                }
            }
            let untouched = |k: usize| plan[k] <= 2;
            let both_gone = |p: usize| (p < n && plan[p] == 10) || (p > 0 && plan[p - 1] == 10);
            // A copy near such an entry can make the matching read that side's new entries as a
            // run beside the removed one, which is refused too, so none there.
            let near_gone = |p: usize| (p.saturating_sub(2)..(p + 2).min(n)).any(|j| plan[j] == 10);
            let edit = |e: &str, who: &str| e.replace("of text.\n", &format!("of text, {who}.\n"));
            let rename = |e: &str, who: &str| e.replacen('\n', &format!(", renamed by {who}\n"), 1);
            let version = |k: usize, ours: bool| -> Option<String> {
                let (e, who) = (&base_entries[k], if ours { "ours" } else { "theirs" });
                match (plan[k], ours) {
                    (3, true) | (4, false) => Some(edit(e, who)),
                    (5, true) | (6, false) | (10, _) => None,
                    (7, true) | (8, false) => Some(rename(e, who)),
                    (9, _) => Some(edit(e, "both")),
                    _ => Some(e.clone()),
                }
            };
            // A side as in the insert-only check (runs at random points, one always on top, a
            // run sometimes repeating its untouched neighbour or holding a shared entry), with
            // its own versions of the base entries.
            // Dated in order, a run at point k is between the dates of the entries around it,
            // newest first. None between two entries the other side removed or renamed: where
            // it goes among their new versions can't be told, which is refused.
            let side = |name: &str, ours: bool, rng: &mut Rng| -> String {
                let other_gone =
                    |j: usize| matches!((plan[j], ours), (6 | 8, true) | (5 | 7, false));
                let mut out = PRE.to_string();
                for k in 0..=n {
                    let among = k > 0 && k < n && other_gone(k - 1) && other_gone(k);
                    if (k == 0 || rng.below(3) == 0) && !both_gone(k) && !among {
                        let above = if k == 0 { 18 } else { base_days[k - 1] };
                        let below = base_days.get(k).copied().unwrap_or(10);
                        let mut cur = above;
                        for i in 0..1 + rng.below(3) {
                            let pick = rng.below(6);
                            let shared_one = (pick == 1).then(|| rng.below(2)).filter(|&s| {
                                !in_order_dates || (below..=cur).contains(&shared_days[s])
                            });
                            out += &if pick == 0
                                && k > 0
                                && untouched(k - 1)
                                && !near_gone(k)
                                && (!in_order_dates || cur == above)
                            {
                                base_entries[k - 1].clone()
                            } else if let Some(s) = shared_one {
                                cur = cur.min(shared_days[s]);
                                shared[s].clone()
                            } else if in_order_dates {
                                cur -= rng.below(cur - below + 1);
                                entry(&at(cur), &format!("{name} {k}.{i}"))
                            } else {
                                entry(&at(10 + rng.below(8)), &format!("{name} {k}.{i}"))
                            };
                        }
                    }
                    if let Some(v) = (k < n).then(|| version(k, ours)).flatten() {
                        out += &v;
                    }
                }
                out
            };
            let base = format!("{PRE}{}", base_entries.concat());
            let (ours, theirs) = (
                side("Ours", true, &mut rng),
                side("Theirs", false, &mut rng),
            );
            let (code, text, err) =
                through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!(code, 0, "seed {seed}: {err}\n{ours}\n---\n{theirs}");
            let by_us = entry_wise(base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).as_deref()
                == Some(&text[..]);
            clean_by_us += usize::from(by_us);
            clean_in_order += usize::from(by_us && in_order_dates);
            let (b, o, t, r) = (
                split(base.as_bytes()),
                split(ours.as_bytes()),
                split(theirs.as_bytes()),
                split(&text),
            );
            let why = format!("seed {seed}:\n{}", String::from_utf8_lossy(&text));
            assert_eq!(r.preamble, PRE.as_bytes(), "{why}");
            // Every entry neither side touched, whole (0218).
            assert!(
                keeps_untouched(base.as_bytes(), ours.as_bytes(), theirs.as_bytes(), &text),
                "untouched: {why}"
            );
            let count = |log: &[&[u8]], e: &[u8]| log.iter().filter(|x| **x == e).count();
            // Every base entry as the side that touched it has it, in the base's order (one with
            // its heading edited where the old one was); the versions a side changed or removed
            // are gone.
            let finals: Vec<Option<String>> =
                (0..n).map(|k| version(k, plan[k] % 2 == 1)).collect();
            let present: Vec<&[u8]> = finals.iter().flatten().map(|e| e.as_bytes()).collect();
            assert!(in_order(&present, &r.entries), "the base's order: {why}");
            let gone = |e: &[u8]| (0..n).any(|k| !untouched(k) && base_entries[k].as_bytes() == e);
            for k in (0..n).filter(|&k| !untouched(k)) {
                assert_eq!(count(&r.entries, base_entries[k].as_bytes()), 0, "{why}");
            }
            let ours_kept: Vec<&[u8]> = o.entries.iter().copied().filter(|e| !gone(e)).collect();
            assert!(in_order(&ours_kept, &r.entries), "ours' order: {why}");
            let ours_added = |e: &[u8]| count(&o.entries, e) > count(&b.entries, e);
            let theirs_kept: Vec<&[u8]> = t
                .entries
                .iter()
                .copied()
                .filter(|e| !gone(e) && !ours_added(e))
                .collect();
            assert!(in_order(&theirs_kept, &r.entries), "theirs' order: {why}");
            // The order oracle: each side's entries, every base entry in them as the merge has
            // it (the other side's change or edited heading, none when removed), in that order.
            if in_order_dates {
                let as_merged = |log: &Log, skip: &dyn Fn(&[u8]) -> bool| -> Vec<Vec<u8>> {
                    log.entries
                        .iter()
                        .filter(|e| !skip(e))
                        .filter_map(|e| {
                            match base_entries.iter().position(|x| x.as_bytes() == *e) {
                                Some(k) => finals[k].as_ref().map(|f| f.as_bytes().to_vec()),
                                None => Some(e.to_vec()),
                            }
                        })
                        .collect()
                };
                for (what, list) in [
                    ("ours", as_merged(&o, &|_| false)),
                    ("theirs", as_merged(&t, &|e| ours_added(e))),
                ] {
                    let list: Vec<&[u8]> = list.iter().map(|e| &e[..]).collect();
                    assert!(
                        in_order(&list, &r.entries),
                        "{what}' order as merged: {why}\n{ours}\n---\n{theirs}"
                    );
                }
            }
            for e in o.entries.iter().chain(&t.entries).filter(|e| !gone(e)) {
                let (bn, on, tn) = (
                    count(&b.entries, e),
                    count(&o.entries, e),
                    count(&t.entries, e),
                );
                let rn = count(&r.entries, e);
                if by_us {
                    assert_eq!(rn, on.max(tn), "{why}");
                } else {
                    assert!(on.max(tn) <= rn && rn <= on + tn - bn, "{why}");
                }
            }
            for e in &r.entries {
                assert!(o.entries.contains(e) || t.entries.contains(e), "{why}");
            }
            // The same sides with one untouched entry also changed both ways: refused.
            if let Some(k) = (0..n).find(|&k| untouched(k)) {
                let e = &base_entries[k];
                let (o2, t2) = (
                    ours.replace(e.as_str(), &edit(e, "ours again")),
                    theirs.replace(e.as_str(), &edit(e, "theirs again")),
                );
                let (code, text, err) =
                    through_binary(base.as_bytes(), o2.as_bytes(), t2.as_bytes());
                let text = String::from_utf8(text).unwrap();
                assert_eq!((code, err.trim()), (1, GIVE_UP), "seed {seed}: {text}");
                assert!(text.contains("<<<<<<< ours\n"), "seed {seed}: {text}");
                refused += 1;
            }
        }
        assert!(
            clean_by_us >= 200,
            "only {clean_by_us} merged by the driver"
        );
        assert!(
            clean_in_order >= 100,
            "only {clean_in_order} dated in order merged by the driver"
        );
        assert!(refused >= 100, "only {refused} both-sides edits tried");
    }

    #[test]
    fn merge_log_inserts_keep_crlf_and_fenced_headings() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let top = crlf(&entry("2026-10-15", "Top"));
        let (pre, old) = (crlf(PRE), crlf(OLD));
        let (a, b) = (
            crlf(&entry("2026-10-14", "A")),
            crlf(&entry("2026-10-13", "B")),
        );
        let got = clean(merged(
            &format!("{pre}{top}{old}"),
            &format!("{pre}{top}{b}{old}"),
            &format!("{pre}{top}{a}{old}"),
        ));
        assert_eq!(got, format!("{pre}{top}{a}{b}{old}"));
        // A dated heading quoted in a fence stays inside its entry under the top one.
        let top = entry("2026-10-15", "Top");
        let x = "## 2026-10-14 — X\n```md\n## 2026-10-12 — example\n```\n\n";
        let y = entry("2026-10-14", "Y");
        let got = clean(merged(
            &format!("{PRE}{top}{OLD}"),
            &format!("{PRE}{top}{x}{OLD}"),
            &format!("{PRE}{top}{y}{OLD}"),
        ));
        assert_eq!(got, format!("{PRE}{top}{y}{x}{OLD}"));
    }

    #[test]
    fn merge_log_merges_edits_to_older_entries_three_way() {
        let base = format!("{PRE}{OLD}{OLDER}");
        let fixed = OLDER.replace("first", "first, fixed");
        let a = entry("2026-10-14", "Task A");
        // One side's new entry on top, the other's edit further down: git's own merge.
        let got = clean(merged(
            &base,
            &format!("{PRE}{a}{OLD}{OLDER}"),
            &format!("{PRE}{OLD}{fixed}"),
        ));
        assert_eq!(got, format!("{PRE}{a}{OLD}{fixed}"));
        // A side that adds on top and edits an older entry, against one that only adds on top:
        // merge-file conflicts on top, and the entry-wise merge keeps both (0209).
        let b = entry("2026-10-13", "Task B");
        let (o, t) = (
            format!("{PRE}{a}{OLD}{fixed}"),
            format!("{PRE}{b}{OLD}{OLDER}"),
        );
        conflict(merged_file_only(&base, &o, &t));
        assert_eq!(
            clean(merged(&base, &o, &t)),
            format!("{PRE}{a}{b}{OLD}{fixed}")
        );
    }

    #[test]
    fn merge_log_leaves_conflict_markers_for_anything_else() {
        let base = format!("{PRE}{OLD}{OLDER}");
        let a = entry("2026-10-14", "Task A");
        let b = entry("2026-10-14", "Task B");
        let markers = |m: Merged| {
            let t = conflict(m);
            assert!(
                t.contains("<<<<<<< ours\n")
                    && t.contains("=======\n")
                    && t.contains(">>>>>>> theirs\n"),
                "{t}"
            );
            t
        };
        // The preamble changed on both sides.
        markers(merged(
            &base,
            &format!("# Log\n\nOurs.\n\n{a}{OLD}{OLDER}"),
            &format!("# Log\n\nTheirs.\n\n{b}{OLD}{OLDER}"),
        ));
        // The preamble changed on one side, with entries on top on both.
        markers(merged(
            &base,
            &format!("# Log, renamed\n\nNewest first.\n\n{a}{OLD}{OLDER}"),
            &format!("{PRE}{b}{OLD}{OLDER}"),
        ));
        // The same older entry changed both ways.
        let t = markers(merged(
            &base,
            &format!("{PRE}{OLD}{}", OLDER.replace("first", "ours")),
            &format!("{PRE}{OLD}{}", OLDER.replace("first", "theirs")),
        ));
        assert!(
            t.contains("**Did:** ours\n") && t.contains("**Did:** theirs\n"),
            "{t}"
        );
        // A side that removed an entry while the other added one on top merges (0209); one that
        // changed the entry the other removed doesn't.
        assert_eq!(
            clean(merged(
                &base,
                &format!("{PRE}{a}{OLD}{OLDER}"),
                &format!("{PRE}{b}{OLDER}"),
            )),
            format!("{PRE}{b}{a}{OLDER}")
        );
        markers(merged(
            &base,
            &format!(
                "{PRE}{a}{}{OLDER}",
                OLD.replace("old thing", "old thing, fixed")
            ),
            &format!("{PRE}{b}{OLDER}"),
        ));
        // A new entry without a date to order it by.
        markers(merged(
            &base,
            &format!("{PRE}{a}{OLD}{OLDER}"),
            &format!("{PRE}## Unreleased\n- b\n\n{OLD}{OLDER}"),
        ));
    }

    #[test]
    fn merge_log_keeps_crlf_and_a_missing_last_line_end() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let base = crlf(&format!("{PRE}{OLD}"));
        let a = crlf(&entry("2026-10-14", "Task A"));
        let b = crlf(&entry("2026-10-13", "Task B"));
        let got = clean(merged(
            &base,
            &format!("{}{a}{}", crlf(PRE), crlf(OLD)),
            &format!("{}{b}{}", crlf(PRE), crlf(OLD)),
        ));
        assert_eq!(got, format!("{}{a}{b}{}", crlf(PRE), crlf(OLD)));
        // The file's last line has no line end, on every side: it stays that way.
        let last = "## 2026-10-12 — Old\n**Did:** no line end";
        let got = clean(merged(
            &format!("{PRE}{last}"),
            &format!("{PRE}{}{last}", entry("2026-10-14", "A")),
            &format!("{PRE}{}{last}", entry("2026-10-13", "B")),
        ));
        assert_eq!(
            got,
            format!(
                "{PRE}{}{}{last}",
                entry("2026-10-14", "A"),
                entry("2026-10-13", "B")
            )
        );
        // A base with no entries: each side's one ends the file without a line end, and gets one
        // where the other side's now follows it. No line is lost or reordered.
        let got = clean(merged(
            PRE,
            &format!("{PRE}## 2026-10-13 — A\n**Did:** a"),
            &format!("{PRE}## 2026-10-14 — B\n**Did:** b"),
        ));
        assert_eq!(
            got,
            format!("{PRE}## 2026-10-14 — B\n**Did:** b\n## 2026-10-13 — A\n**Did:** a")
        );
    }

    #[test]
    fn merge_log_with_an_empty_base() {
        // Both sides made the file (an add/add conflict): only entries on both sides merge.
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-14", "B"));
        assert_eq!(clean(merged("", &a, &b)), format!("{b}{a}"));
        // Each with its own preamble: the preamble changed on both sides.
        conflict(merged("", &format!("{PRE}{a}"), &format!("# Other\n\n{b}")));
        // Bytes that aren't UTF-8 are carried as they are.
        let bin = b"## 2026-10-14 \xff\xfe\n";
        let Merged::Clean(t) = merged_bytes(b"", bin, a.as_bytes()) else {
            panic!("not clean")
        };
        assert_eq!(t, [a.as_bytes(), bin].concat());
    }

    #[test]
    fn merge_log_splits_at_level_two_headings_only() {
        let text = format!("{PRE}## 2026-10-14 — A\n### Detail\n##no space\n{OLD}");
        let log = split(text.as_bytes());
        assert_eq!(log.preamble, PRE.as_bytes());
        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.entries.concat(), &text.as_bytes()[PRE.len()..]);
        assert_eq!(
            date("## 2026-10-14 — A".as_bytes()),
            Some(&b"2026-10-14"[..])
        );
        for undated in [
            &b"## Unreleased"[..],
            b"## 2026-10-1",
            b"## 2026-10-144",
            b"## 2026/10/14",
        ] {
            assert_eq!(date(undated), None, "{}", String::from_utf8_lossy(undated));
        }
    }

    #[test]
    fn merge_log_run_writes_the_result_into_ours_and_says_conflict_by_its_exit() {
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        let base = format!("{PRE}{OLD}");
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        fs::write(p("base"), &base).unwrap();
        fs::write(p("our s"), format!("{PRE}{a}{OLD}")).unwrap();
        fs::write(p("theirs"), format!("{PRE}{b}{OLD}")).unwrap();
        let args =
            |o: &str| [p("base"), p(o), p("theirs")].map(|x| x.to_str().unwrap().to_string());
        assert_eq!(run(&args("our s")), Ok(0));
        assert_eq!(
            fs::read_to_string(p("our s")).unwrap(),
            format!("{PRE}{a}{b}{OLD}")
        );
        fs::write(p("our s"), format!("# Changed\n\n{a}{OLD}")).unwrap();
        fs::write(p("theirs"), format!("# Also\n\n{b}{OLD}")).unwrap();
        assert_eq!(run(&args("our s")), Ok(1));
        assert!(fs::read_to_string(p("our s"))
            .unwrap()
            .contains("<<<<<<< ours\n"));
        // Wrong arguments: an error. A file that isn't there: 2, and ours as it was.
        assert!(run(&args("our s")[..2])
            .unwrap_err()
            .contains("see `muster help`"));
        let before = fs::read(p("our s")).unwrap();
        let mut gone = args("our s");
        gone[0] = p("missing").to_str().unwrap().into();
        assert_eq!(run(&gone), Ok(2));
        assert_eq!(fs::read(p("our s")).unwrap(), before);
    }

    /// `keeps_untouched` over text.
    fn holds(base: &str, ours: &str, theirs: &str, result: &str) -> bool {
        keeps_untouched(
            base.as_bytes(),
            ours.as_bytes(),
            theirs.as_bytes(),
            result.as_bytes(),
        )
    }

    #[test]
    fn untouched_check_fails_on_a_lost_line_byte_copy_or_preamble_line() {
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        let base = format!("{PRE}{a}{OLD}{b}");
        let ours = format!("{PRE}{}{a}{OLD}{b}", entry("2026-10-15", "Ours"));
        let theirs = base.clone();
        assert!(holds(&base, &ours, &theirs, &ours));
        // OLD's middle line, its last (blank) line, one byte of it; the last entry's last line.
        for lost in [
            ours.replacen("**Did:** the old thing\n", "", 1),
            ours.replacen(OLD, OLD.strip_suffix('\n').unwrap(), 1),
            ours.replacen("the old thing", "the old thinG", 1),
            ours.strip_suffix('\n').unwrap().to_string(),
            // The preamble, the same in all three.
            ours.replacen("Newest first.\n", "", 1),
            // An entry cut in two, or run into the one above it.
            ours.replacen(
                "**Did:** the old thing\n",
                "**Did:** the old thing\n## 2026-10-12 — cut\n",
                1,
            ),
            ours.replacen(&format!("\n{OLD}"), &format!("\n{}", &OLD[3..]), 1),
        ] {
            assert_ne!(lost, ours);
            assert!(!holds(&base, &ours, &theirs, &lost), "{lost}");
        }
        // An entry all three have twice must be there twice; once when a side removed a copy.
        let twice = format!("{PRE}{a}{OLD}{OLD}{b}");
        let once = format!("{PRE}{a}{OLD}{b}");
        assert!(!holds(&twice, &twice, &twice, &once));
        assert!(holds(&twice, &once, &twice, &once));
        assert!(holds(&twice, &twice, &twice, &twice));
    }

    #[test]
    fn untouched_check_passes_what_each_side_did() {
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        let (n, m) = (entry("2026-10-15", "New"), entry("2026-10-15", "Mine"));
        let base = format!("{PRE}{a}{OLD}{b}{OLDER}");
        let edit = |e: &str, who: &str| e.replace("of text.\n", &format!("of text, {who}.\n"));
        // A one-sided removal, with the untouched entries under it moved up.
        let gone = format!("{PRE}{OLD}{b}{OLDER}");
        assert!(holds(&base, &gone, &base, &gone));
        assert!(holds(&base, &base, &gone, &gone));
        // A one-sided change; the same change on both sides.
        let changed = format!("{PRE}{a}{OLD}{}{OLDER}", edit(&b, "theirs"));
        assert!(holds(&base, &base, &changed, &changed));
        assert!(holds(&base, &changed, &changed, &changed));
        // An entry both sides added, kept once; one new entry each, in the order merged.
        let ours = format!("{PRE}{n}{a}{OLD}{b}{OLDER}");
        let theirs = format!("{PRE}{m}{n}{a}{OLD}{b}{OLDER}");
        assert!(holds(&base, &ours, &theirs, &theirs));
        // A changed preamble on one side isn't checked; the result has one side's.
        let pre2 = format!("# Log\n\nChanged.\n\n{a}{OLD}{b}{OLDER}");
        assert!(holds(&base, &pre2, &base, &pre2));
        // Each, as the merge makes it: the check holds on what `merge` gives.
        for (o, t) in [
            (&gone, &base),
            (&changed, &base),
            (&ours, &theirs),
            (
                &format!("{PRE}{n}{a}{OLD}{OLDER}"),
                &format!("{PRE}{m}{a}{OLD}{b}{OLDER}"),
            ),
        ] {
            let got = clean(merged(&base, o, t));
            assert!(holds(&base, o, t, &got), "{got}");
        }
        // An entry that ended the file without a line end, given one where an entry follows.
        let last = OLDER.strip_suffix('\n').unwrap();
        let (b2, o2) = (format!("{PRE}{OLD}{last}"), format!("{PRE}{n}{OLD}{last}"));
        assert!(holds(&b2, &o2, &b2, &format!("{PRE}{n}{OLD}{last}\n")));
        assert!(holds(&b2, &o2, &b2, &format!("{PRE}{n}{OLD}{last}\r\n")));
        assert!(!holds(
            &b2,
            &o2,
            &b2,
            &format!("{PRE}{n}{OLD}{}", &last[..last.len() - 1])
        ));
    }

    /// Sets `BEFORE_CHECK` for one test and clears it when dropped, a failing assert included.
    struct Seam;
    impl Seam {
        fn set(hook: impl Fn(&mut Vec<u8>) + 'static) -> Seam {
            BEFORE_CHECK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
            Seam
        }
    }
    impl Drop for Seam {
        fn drop(&mut self) {
            BEFORE_CHECK.with(|h| *h.borrow_mut() = None);
        }
    }

    /// Drops the first `line` from a result: a test's lost line of an untouched entry.
    fn drop_line(line: &'static str) -> impl Fn(&mut Vec<u8>) {
        move |text: &mut Vec<u8>| {
            let s = String::from_utf8(text.clone()).unwrap();
            *text = s.replacen(line, "", 1).into_bytes();
        }
    }

    /// Every line of each input is in `text`, at least as many times as any input has it.
    fn has_every_line(text: &str, inputs: &[&str]) {
        let count = |s: &str, l: &str| s.split_inclusive('\n').filter(|x| *x == l).count();
        for input in inputs {
            for line in input.split_inclusive('\n') {
                let most = inputs.iter().map(|i| count(i, line)).max().unwrap();
                assert!(count(text, line) >= most, "{line:?} lost:\n{text}");
            }
        }
    }

    #[test]
    fn untouched_loss_in_the_entry_wise_result_gives_markers_and_exit_1() {
        let base = format!("{PRE}{OLD}{OLDER}");
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        let (ours, theirs) = (
            format!("{PRE}{a}{OLD}{OLDER}"),
            format!("{PRE}{b}{OLD}{OLDER}"),
        );
        // Both on top: merge-file clashes, the entry-wise merge merges it clean.
        conflict(merged_file_only(&base, &ours, &theirs));
        let want = format!("{PRE}{a}{b}{OLD}{OLDER}");
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        // The same, with a line of OLD (in all three) lost before the check: refused.
        let _seam = Seam::set(drop_line("**Did:** the old thing\n"));
        let text = conflict(merged(&base, &ours, &theirs));
        assert!(text.contains("<<<<<<< ours\n"), "{text}");
        has_every_line(&text, &[&base, &ours, &theirs]);
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), &base).unwrap();
        fs::write(p("a"), &ours).unwrap();
        fs::write(p("b"), &theirs).unwrap();
        let args = [p("o"), p("a"), p("b")].map(|x| x.to_str().unwrap().to_string());
        assert_eq!(run(&args), Ok(1));
        let left = fs::read_to_string(p("a")).unwrap();
        assert_eq!(left, text);
    }

    #[test]
    fn untouched_merge_file_clean_result_that_loses_one_goes_to_entry_wise_then_markers() {
        let base = format!("{PRE}{OLD}{OLDER}");
        let a = entry("2026-10-14", "A");
        let ours = format!("{PRE}{a}{OLD}{OLDER}");
        // One side's new entry: merge-file merges it clean.
        assert_eq!(clean(merged_file_only(&base, &ours, &base)), ours);
        // Merge-file's result losing a line goes to the entry-wise merge, which keeps it.
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = calls.clone();
        let lose = drop_line("**Did:** the old thing\n");
        let seam = Seam::set(move |text| {
            seen.set(seen.get() + 1);
            if seen.get() == 1 {
                lose(text)
            }
        });
        assert_eq!(clean(merged(&base, &ours, &base)), ours);
        assert_eq!(calls.get(), 2, "merge-file's result, then entry_wise's");
        drop(seam);
        // Both losing it: all three whole between merge-file's markers, and exit 1.
        let _seam = Seam::set(drop_line("**Did:** the old thing\n"));
        let want =
            format!("<<<<<<< ours\n{ours}||||||| base\n{base}=======\n{base}>>>>>>> theirs\n");
        assert_eq!(refused(merged(&base, &ours, &base)), want);
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), &base).unwrap();
        fs::write(p("a"), &ours).unwrap();
        fs::write(p("b"), &base).unwrap();
        let args = [p("o"), p("a"), p("b")].map(|x| x.to_str().unwrap().to_string());
        assert_eq!(run(&args), Ok(1));
        assert_eq!(fs::read_to_string(p("a")).unwrap(), want);
    }

    #[test]
    fn untouched_merge_file_loss_the_entry_wise_merge_cant_take_gives_markers() {
        // Theirs changed the preamble, which only merge-file merges: its result losing a line
        // of OLD is refused, and the entry-wise merge refuses the changed preamble.
        let base = format!("{PRE}{OLD}{OLDER}");
        let a = entry("2026-10-14", "A");
        let ours = format!("{PRE}{a}{OLD}{OLDER}");
        let theirs = format!("# Log\n\nChanged.\n\n{OLD}{OLDER}");
        let merged_clean = clean(merged(&base, &ours, &theirs));
        assert_eq!(
            merged_clean,
            format!("# Log\n\nChanged.\n\n{a}{OLD}{OLDER}")
        );
        let _seam = Seam::set(drop_line("**Did:** the old thing\n"));
        let want =
            format!("<<<<<<< ours\n{ours}||||||| base\n{base}=======\n{theirs}>>>>>>> theirs\n");
        assert_eq!(refused(merged(&base, &ours, &theirs)), want);
    }

    const REFUSED: &str = "muster merge-log: refused the merge, which would lose an entry neither side changed; left ours, base and theirs whole between conflict markers";

    #[test]
    fn untouched_twice_counts_each_copy_by_what_each_side_did_to_it() {
        let (e, x) = (entry("2026-10-13", "Twice"), entry("2026-10-12", "X"));
        let e2 = e.replace("of text.\n", "of text, edited.\n");
        let base = format!("{PRE}{e}{x}{e}");
        // One side removes copy 2, the other edits copy 1: no copy is untouched, so the edit
        // with the removal is clean, as merge-file merges it, either way round.
        let (removed, edited) = (format!("{PRE}{e}{x}"), format!("{PRE}{e2}{x}{e}"));
        let want = format!("{PRE}{e2}{x}");
        assert!(holds(&base, &removed, &edited, &want));
        assert_eq!(clean(merged_file_only(&base, &removed, &edited)), want);
        for (o, t) in [(&removed, &edited), (&edited, &removed)] {
            assert_eq!(clean(merged(&base, o, t)), want);
            let (code, text, err) = through_binary(base.as_bytes(), o.as_bytes(), t.as_bytes());
            assert_eq!((code, String::from_utf8(text).unwrap()), (0, want.clone()));
            assert_eq!(err, "");
        }
        // A side `read_side` can't read (here its preamble changed) is matched entry by entry
        // all the same: the edit of copy 1 still counts, so the same result keeps all it needs.
        let pre2 = format!("# Log\n\nChanged.\n\n{e}{x}");
        assert!(holds(
            &base,
            &pre2,
            &edited,
            &format!("# Log\n\nChanged.\n\n{e2}{x}")
        ));
    }

    #[test]
    fn untouched_a_copy_the_side_still_holds_whole_isnt_changed() {
        let (e, x) = (entry("2026-10-13", "Moved"), entry("2026-10-12", "X"));
        let e2 = e.replace("of text.\n", "of text, edited.\n");
        let base = format!("{PRE}{e}{x}");
        // Ours moved E under X and put an entry with E's heading where it was: matched may read
        // that as E changed, but ours still holds E whole, so E is untouched and must stay.
        let ours = format!("{PRE}{e2}{x}{e}");
        assert!(!holds(&base, &ours, &base, &format!("{PRE}{e2}{x}")));
        assert!(holds(&base, &ours, &base, &ours));
    }

    /// Seeds 2633, 4034 and 5139 of review round 2's randomised differential (R3): one side
    /// moves an entry, which `matched` reads as others removed, and the other side's edit lets
    /// a clean merge-file result cut an entry neither side changed (a `~~~` fence that opens
    /// inside it and swallows the next heading, or its last blank line dropped where theirs cut
    /// the file's last line end). Main merges each clean and loses it; they're refused, all three
    /// files whole.
    #[test]
    fn untouched_a_moved_entry_doesnt_unguard_the_ones_it_moved_past() {
        let e =
            |head: &str, body: &str| format!("## {head}\n\n{body}**Verified:** gate: green\n\n");
        let pre = "# Work log\n\nNewest first.\n\n";
        let fix2 = e("2026-10-09 — fix 2", "**Decided:** log\n");
        let queue1 = e(
            "2026-10-14 — queue 1",
            "**Did:** test\n**Decided:** merge\n**Decided:** add\n",
        );
        let rtl8 = e("2026-10-10 — rtl 8", "");
        let rtl8_fenced = format!("{rtl8}~~~\n");
        let board2 = e("2026-10-08 — the board 2", "**Fixed:** add\n**Did:** fix\n");
        let seed2633 = [
            format!("{pre}{fix2}{queue1}{rtl8}{board2}"),
            format!("{pre}{rtl8}{fix2}{board2}"),
            format!("{pre}{fix2}{queue1}{rtl8_fenced}{board2}"),
        ];
        let add0 = e("2026-10-01 — add 0", "");
        let log0 = e(
            "2026-10-18 — log 0",
            "**Decided:** fix\n**Decided:** merge\n**Fixed:** add\n",
        );
        let fix3 = e("2026-10-05 — fix 3", "**Did:** merge\n");
        let fix8 = e("2026-10-14 — fix 8", "**Did:** rtl\n");
        let log1 = e("2026-10-04 — log 1", "**Fixed:** gate\n");
        let log1_open = format!(
            "{}~~~\n",
            e("2026-10-04 — log 1", "**Fixed:** gate\n**Open:** test\n")
        );
        let queue2 = e("2026-10-12 — queue 2", "**Did:** add\n**Did:** گزارش\n");
        let seed4034 = [
            format!("{pre}{add0}{fix3}{fix8}{log1}{queue2}"),
            format!("{pre}{add0}{fix3}{log1}{queue2}{fix8}"),
            format!("{pre}{add0}{log0}{fix3}{fix8}{log1_open}{queue2}"),
        ];
        let test5 = e("2026-10-03 — test 5", "");
        let queue0 = e("2026-10-11 — queue 0", "");
        let queue0_cut = queue0.strip_suffix('\n').unwrap();
        let seed5139 = [
            format!("{pre}{test5}{queue0}"),
            format!("{pre}{queue0}{test5}"),
            format!("{pre}{test5}{queue0_cut}"),
        ];
        for [base, ours, theirs] in [seed2633, seed4034, seed5139] {
            let lossy = clean(merged_file_only(&base, &ours, &theirs));
            assert!(!holds(&base, &ours, &theirs, &lossy), "{lossy}");
            let want = whole_file_markers(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!(refused(merged(&base, &ours, &theirs)).as_bytes(), want);
        }
    }

    /// Review round 3's R4: ours edits copy 2 and theirs keeps both, so copy 1 is untouched.
    /// Ours' edit is already missing from ours' count and isn't taken off again.
    #[test]
    fn untouched_twice_one_side_editing_copy_2_still_needs_copy_1() {
        let (e, x) = (entry("2026-10-13", "Twice"), entry("2026-10-12", "X"));
        let e2 = e.replace("of text.\n", "of text, edited.\n");
        let x2 = x.replace("of text.\n", "of text, edited.\n");
        let base = format!("{PRE}{e}{x}{e}");
        let ours = format!("{PRE}{e}{x}{e2}");
        // Theirs as base, or editing X: copy 1 gone is a loss either way, round 3's scratch test.
        for theirs in [base.clone(), format!("{PRE}{e}{x2}{e}")] {
            assert!(!holds(&base, &ours, &theirs, &format!("{PRE}{x}{e2}")));
            assert!(!holds(&base, &ours, &theirs, &format!("{PRE}{x2}{e2}")));
            assert!(holds(&base, &ours, &theirs, &ours));
        }
        // Both sides changing copy 1 the same way leave copy 2 untouched: it's needed.
        let both = format!("{PRE}{e2}{x}{e}");
        assert!(!holds(&base, &both, &both, &format!("{PRE}{e2}{x}")));
        assert!(holds(&base, &both, &both, &both));
        // Each side editing a different copy leaves none untouched.
        let (o, t) = (format!("{PRE}{e2}{x}{e}"), format!("{PRE}{e}{x}{e2}"));
        assert!(holds(&base, &o, &t, &format!("{PRE}{e2}{x}{e2}")));
    }

    #[test]
    fn untouched_twice_both_sides_removing_a_different_copy_is_refused() {
        let (e, x) = (entry("2026-10-13", "Twice"), entry("2026-10-12", "X"));
        let base = format!("{PRE}{e}{x}{e}");
        let (ours, theirs) = (format!("{PRE}{e}{x}"), format!("{PRE}{x}{e}"));
        // Each kept one copy: a result with none loses it, which merge-file gives clean.
        assert_eq!(
            clean(merged_file_only(&base, &ours, &theirs)),
            format!("{PRE}{x}")
        );
        assert!(!holds(&base, &ours, &theirs, &format!("{PRE}{x}")));
        assert!(holds(&base, &ours, &theirs, &format!("{PRE}{e}{x}")));
        let want =
            format!("<<<<<<< ours\n{ours}||||||| base\n{base}=======\n{theirs}>>>>>>> theirs\n");
        assert_eq!(refused(merged(&base, &ours, &theirs)), want);
        // The binary says why, and that the markers wrap all three files whole.
        let (code, text, err) = through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
        assert_eq!((code, String::from_utf8(text).unwrap()), (1, want));
        assert_eq!(err.trim_end(), REFUSED);
    }

    #[test]
    fn untouched_merge_file_markers_keep_crlf_and_a_missing_last_line_end() {
        let (base, ours, theirs) = (b"o\r\nx".as_slice(), b"a\r\nx".as_slice(), b"".as_slice());
        assert_eq!(
            whole_file_markers(base, ours, theirs),
            b"<<<<<<< ours\r\na\r\nx\r\n||||||| base\r\no\r\nx\r\n=======\r\n>>>>>>> theirs\r\n"
        );
        assert_eq!(
            whole_file_markers(b"o", b"a", b"t"),
            b"<<<<<<< ours\na\n||||||| base\no\n=======\nt\n>>>>>>> theirs\n"
        );
    }

    #[test]
    fn merge_log_quotes_the_binary_for_the_shell_git_runs_it_in() {
        assert_eq!(sh_quote("/usr/local/bin/muster"), "/usr/local/bin/muster");
        assert_eq!(sh_quote("/a b/it's/muster"), "'/a b/it'\\''s/muster'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("/a/100%/muster"), "'/a/100%/muster'");
        let args = config_args(Path::new("/x y/100%A/muster")).unwrap();
        assert_eq!(
            args,
            [
                "-c",
                "merge.muster-log.name=muster work log: entries stay whole, newest on top",
                "-c",
                "merge.muster-log.driver='/x y/100%%A/muster' merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; git merge-file -L ours -L base -L theirs %A %O %B",
            ]
        );
    }

    /// A temp repo whose log has one entry, with the attribute committed and nothing in its
    /// config about the driver, in a folder with a space.
    fn log_repo(dir: &Path) -> PathBuf {
        log_repo_with(dir, &g)
    }

    /// `log_repo`, its git run by `git`.
    fn log_repo_with(dir: &Path, git: &dyn Fn(&Path, &[&str]) -> String) -> PathBuf {
        let repo = dir.join("a repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Test Person"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            git(&repo, &["config", k, v]);
        }
        fs::create_dir(repo.join("docs")).unwrap();
        fs::write(
            repo.join(".gitattributes"),
            "docs/WORKLOG.md merge=muster-log\n",
        )
        .unwrap();
        fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{OLD}")).unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "base"]);
        repo
    }

    /// `g` in a home of its own: no global or system git config, and no MUSTER_BASE,
    /// MUSTER_ROOT or TASKS_ROOT for git or the driver it starts.
    fn home_g(home: &Path, dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("HOME", home)
            .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("MUSTER_BASE")
            .env_remove("MUSTER_ROOT")
            .env_remove("TASKS_ROOT")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn merge_log_runs_as_gits_driver_from_a_path_with_spaces_in_a_worktree() {
        let dir = crate::testkit::tempdir().unwrap();
        let repo = log_repo(dir.path());
        let wt = dir.path().join("the worktree");
        g(
            &repo,
            &["worktree", "add", "-q", "-b", "task", wt.to_str().unwrap()],
        );
        let (a, b) = (
            entry("2026-10-14", "Main's"),
            entry("2026-10-14", "The task's"),
        );
        fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{a}{OLD}")).unwrap();
        g(&repo, &["commit", "-qam", "main's entry"]);
        fs::write(wt.join("docs/WORKLOG.md"), format!("{PRE}{b}{OLD}")).unwrap();
        g(&wt, &["commit", "-qam", "the task's entry"]);
        // The binary in a folder with a space, a quote and a %A git mustn't expand, as git's shell
        // must find it.
        let bin = dir.path().join("muster's 100%A bin");
        fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink(this_muster().unwrap(), bin.join("muster")).unwrap();
        let args = config_args(&bin.join("muster")).unwrap();
        let mut rebase: Vec<&str> = args.iter().map(String::as_str).collect();
        rebase.extend(["rebase", "-q", "main"]);
        g(&wt, &rebase);
        assert_eq!(
            fs::read_to_string(wt.join("docs/WORKLOG.md")).unwrap(),
            format!("{PRE}{b}{a}{OLD}"),
            "the replayed commit's entry on top"
        );
        assert_eq!(g(&wt, &["status", "--porcelain"]), "");
    }

    #[test]
    fn merge_log_merges_and_rebases_branches_that_insert_under_the_same_entry() {
        // Main's log got a later-dated entry after both tasks started from it (0190's repro): each
        // task puts its entries under it. Merge and rebase, each in both orders, with the driver
        // in the repo's config.
        let top = entry("2026-10-15", "Coordinator's log");
        let one = entry("2026-10-14", "Task one");
        let four: String = (1..=4)
            .map(|n| entry("2026-10-14", &format!("Task four, part {n}")))
            .collect();
        for (how, first, second) in [
            ("merge", "one", "four"),
            ("merge", "four", "one"),
            ("rebase", "one", "four"),
            ("rebase", "four", "one"),
        ] {
            let dir = crate::testkit::tempdir().unwrap();
            let repo = log_repo(dir.path());
            let bin = this_muster().unwrap();
            g(&repo, &["config", &format!("merge.{DRIVER}.name"), NAME]);
            let driver = driver_command(&sh_quote(&bin.to_str().unwrap().replace('%', "%%")));
            g(
                &repo,
                &["config", &format!("merge.{DRIVER}.driver"), &driver],
            );
            fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{top}{OLD}")).unwrap();
            g(&repo, &["commit", "-qam", "main's later entry"]);
            for (branch, run) in [("one", &one), ("four", &four)] {
                g(&repo, &["checkout", "-q", "-b", branch, "main"]);
                fs::write(
                    repo.join("docs/WORKLOG.md"),
                    format!("{PRE}{top}{run}{OLD}"),
                )
                .unwrap();
                g(&repo, &["commit", "-qam", branch]);
            }
            let run = |b: &str| if b == "one" { &one } else { &four };
            // The incoming side (the branch merged in, the commit replayed) goes above.
            let want = format!("{PRE}{top}{}{}{OLD}", run(second), run(first));
            g(&repo, &["checkout", "-q", "main"]);
            g(&repo, &["merge", "-q", "--ff-only", first]);
            if how == "merge" {
                g(&repo, &["merge", "-q", "--no-edit", second]);
            } else {
                g(&repo, &["checkout", "-q", second]);
                g(&repo, &["rebase", "-q", "main"]);
            }
            let got = fs::read_to_string(repo.join("docs/WORKLOG.md")).unwrap();
            assert_eq!(got, want, "{how} {second} onto {first}");
            assert_eq!(g(&repo, &["status", "--porcelain"]), "", "{how}");
        }
    }

    #[test]
    fn merge_log_configure_writes_only_the_missing_keys_to_the_repos_own_config() {
        let dir = crate::testkit::tempdir().unwrap();
        let repo = log_repo(dir.path());
        let config = || fs::read_to_string(repo.join(".git/config")).unwrap();
        let before = config();
        assert_eq!(
            configure(&repo),
            Ok(vec![
                "merge.muster-log.name".to_string(),
                "merge.muster-log.driver".to_string()
            ])
        );
        assert_eq!(
            g(&repo, &["config", "--local", "merge.muster-log.driver"]),
            driver_command("muster")
        );
        assert_eq!(
            g(&repo, &["config", "--local", "merge.muster-log.name"]),
            NAME
        );
        assert!(config().starts_with(&before), "the rest as it was");
        // Again: nothing to write.
        let after = config();
        assert_eq!(configure(&repo), Ok(Vec::new()));
        assert_eq!(config(), after);
        // An owner's own driver stays; only the missing name is added.
        g(&repo, &["config", "--unset", "merge.muster-log.name"]);
        g(
            &repo,
            &["config", "merge.muster-log.driver", "my-driver %O %A %B"],
        );
        assert_eq!(
            configure(&repo),
            Ok(vec!["merge.muster-log.name".to_string()])
        );
        assert_eq!(
            g(&repo, &["config", "merge.muster-log.driver"]),
            "my-driver %O %A %B"
        );
        // From a worktree: the repo's shared config.
        let wt = dir.path().join("wt");
        g(
            &repo,
            &["worktree", "add", "-q", "-b", "w", wt.to_str().unwrap()],
        );
        g(&repo, &["config", "--unset", "merge.muster-log.name"]);
        assert_eq!(
            configure(&wt),
            Ok(vec!["merge.muster-log.name".to_string()])
        );
        assert_eq!(
            g(&repo, &["config", "--local", "merge.muster-log.name"]),
            NAME
        );
        // Not a repo: an error, nothing written.
        assert!(configure(dir.path()).is_err());
    }

    #[test]
    fn merge_log_a_driver_not_set_says_why_and_the_two_lines_that_set_it() {
        let dir = crate::testkit::tempdir().unwrap();
        let repo = log_repo(dir.path());
        assert!(wanted(&repo));
        assert_eq!(missing(&repo).unwrap().len(), 2);
        // A config git can't lock, as a sandboxed session's: configure fails and writes nothing.
        let lock = repo.join(".git/config.lock");
        fs::write(&lock, "").unwrap();
        let e = configure(&repo).unwrap_err();
        let said = not_set(&e);
        let mut lines = said.lines();
        assert_eq!(
            lines.next(),
            Some("Couldn't set the work log's merge driver in .git/config: File exists."),
            "{e}"
        );
        assert!(
            !said.contains(dir.path().to_str().unwrap()),
            "no path: {said}"
        );
        assert!(said.contains("a clash in docs/WORKLOG.md is a conflict instead of merging"));
        let run: Vec<&str> = lines.filter_map(|l| l.strip_prefix("  ")).collect();
        assert_eq!(run.len(), 2, "{said}");
        assert_eq!(missing(&repo).unwrap().len(), 2);
        // Run as printed, through a shell, the two lines set what configure would have. (`sh -c`
        // on purpose: the lines are this test's own output, and their quoting is what's tested.)
        fs::remove_file(&lock).unwrap();
        for line in run {
            assert!(
                line.starts_with("git config --local merge.muster-log."),
                "{line}"
            );
            let out = Command::new("sh")
                .args(["-c", line])
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success(), "{line}: {out:?}");
        }
        assert_eq!(missing(&repo).unwrap(), Vec::new());
        assert_eq!(
            g(&repo, &["config", "--local", "merge.muster-log.driver"]),
            driver_command("muster")
        );
        assert_eq!(
            g(&repo, &["config", "--local", "merge.muster-log.name"]),
            NAME
        );
        // `wanted`: only a merge=muster-log attribute outside a comment.
        for (attrs, want) in [
            ("docs/WORKLOG.md merge=muster-log\n", true),
            (
                "*.md text\n  docs/WORKLOG.md   merge=muster-log  -diff\n",
                true,
            ),
            ("# docs/WORKLOG.md merge=muster-log\n", false),
            ("docs/WORKLOG.md merge=muster-logs\n", false),
            ("merge=muster-log\n", false),
            ("", false),
        ] {
            fs::write(repo.join(".gitattributes"), attrs).unwrap();
            assert_eq!(wanted(&repo), want, "{attrs:?}");
        }
        fs::remove_file(repo.join(".gitattributes")).unwrap();
        assert!(!wanted(&repo));
    }

    #[test]
    fn merge_log_ignores_headings_inside_code_fences() {
        let base = format!("{PRE}{OLD}");
        let y = entry("2026-10-13", "Y");
        // A dated heading in a ``` fence, then in a ~~~ fence that a shorter ~~~ doesn't close,
        // indented up to 3 spaces: each stays inside its entry.
        let x = "## 2026-10-14 — X\n**Did:** x, with an example:\n```md\n## 2026-10-12 — example\n```\n\n";
        let z = "## 2026-10-14 — Z\n  ~~~~\n## 2026-10-10 — a\n~~~\n## 2026-10-09 — b\n~~~~~ \n\n";
        for e in [x, z] {
            assert_eq!(
                split(format!("{PRE}{e}{OLD}").as_bytes()).entries.len(),
                2,
                "{e}"
            );
            let got = clean(merged(
                &base,
                &format!("{PRE}{e}{OLD}"),
                &format!("{PRE}{y}{OLD}"),
            ));
            assert_eq!(got, format!("{PRE}{e}{y}{OLD}"));
        }
        // A backtick fence's info can't hold a backtick, so "``` a`b" opens nothing.
        let text = "## 2026-10-14 — N\n``` a`b\n## 2026-10-13 — real\n";
        assert_eq!(split(text.as_bytes()).entries.len(), 2);
        // A fence closes only on its own character: ``` inside ~~~ doesn't close it.
        let text = "## 2026-10-14 — T\n~~~\n```\n## 2026-10-13 — quoted\n~~~\n";
        assert_eq!(split(text.as_bytes()).entries.len(), 1);
        let both = format!("{text}{OLD}");
        let log = split(both.as_bytes());
        assert_eq!(log.entries, [text.as_bytes(), OLD.as_bytes()]);
        // And only with nothing after it: "``` md" inside ``` opens nothing and closes nothing.
        let text = "## 2026-10-14 — M\n```\n``` md\n## 2026-10-13 — quoted\n```\n";
        assert_eq!(split(text.as_bytes()).entries.len(), 1);
        let both = format!("{text}{OLD}");
        let log = split(both.as_bytes());
        assert_eq!(log.entries, [text.as_bytes(), OLD.as_bytes()]);
        // Four spaces is code, not a fence.
        let text = "## 2026-10-14 — N\n    ```\n## 2026-10-13 — real\n";
        assert_eq!(split(text.as_bytes()).entries.len(), 2);
        // An unclosed fence runs to the end of the file, the base's entries inside it: read as
        // the base entry removed, but the fence would take in the other side's entry after it,
        // so it's a conflict (0209).
        let open = "## 2026-10-14 — U\n```\n## 2026-10-10 — inside\n";
        assert_eq!(
            split(format!("{PRE}{open}{OLD}").as_bytes()).entries.len(),
            1
        );
        conflict(merged(
            &base,
            &format!("{PRE}{open}{OLD}"),
            &format!("{PRE}{y}{OLD}"),
        ));
    }

    #[test]
    fn merge_log_puts_a_crlf_line_end_between_entries_that_had_none() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let got = clean(merged(
            &crlf(PRE),
            &format!("{}## 2026-10-13 — A\r\n**Did:** a", crlf(PRE)),
            &format!("{}## 2026-10-14 — B\r\n**Did:** b", crlf(PRE)),
        ));
        assert_eq!(
            got,
            format!(
                "{}## 2026-10-14 — B\r\n**Did:** b\r\n## 2026-10-13 — A\r\n**Did:** a",
                crlf(PRE)
            )
        );
    }

    #[test]
    fn merge_log_falls_back_to_gits_markers_when_it_cant_merge() {
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        fs::write(p("base"), format!("{PRE}{OLD}")).unwrap();
        fs::write(p("ours"), format!("{PRE}{a}{OLD}")).unwrap();
        fs::write(p("theirs"), format!("{PRE}{b}{OLD}")).unwrap();
        let args = [p("base"), p("ours"), p("theirs")].map(|x| x.to_str().unwrap().to_string());
        // Over the size limit: git's own markers, both entries in them, and a conflict.
        assert_eq!(run_capped(&args, 16), Ok(1));
        let text = fs::read_to_string(p("ours")).unwrap();
        assert!(
            text.contains("<<<<<<< ours\n") && text.contains(">>>>>>> theirs\n"),
            "{text}"
        );
        assert!(
            text.contains("**Did:** A,") && text.contains("**Did:** B,"),
            "{text}"
        );
        // Not even that works (the base is gone): ours as it was, and 2, which the configured
        // driver's own fallback takes over from.
        fs::write(p("ours"), format!("{PRE}{a}{OLD}")).unwrap();
        fs::remove_file(p("base")).unwrap();
        assert_eq!(run(&args), Ok(2));
        assert_eq!(
            fs::read_to_string(p("ours")).unwrap(),
            format!("{PRE}{a}{OLD}")
        );
    }

    /// The names in `dir`, sorted.
    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn merge_log_replaces_ours_whole_or_not_at_all() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::testkit::tempdir().unwrap();
        let (side, copy) = (dir.path().join("side"), dir.path().join("copy"));
        let (a, b) = (entry("2026-10-14", "A"), entry("2026-10-13", "B"));
        for d in [&side, &copy] {
            fs::create_dir(d).unwrap();
            fs::write(d.join("base"), format!("{PRE}{OLD}")).unwrap();
            fs::write(d.join("ours"), format!("{PRE}{a}{OLD}")).unwrap();
            fs::write(d.join("theirs"), format!("{PRE}{b}{OLD}")).unwrap();
        }
        // A replace that works: the new text, the old file's mode, nothing left beside it.
        fs::set_permissions(copy.join("ours"), fs::Permissions::from_mode(0o600)).unwrap();
        replace(&copy.join("ours"), b"new\n").unwrap();
        assert_eq!(fs::read(copy.join("ours")).unwrap(), b"new\n");
        let mode = fs::metadata(copy.join("ours"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(names(&copy), ["base", "ours", "theirs"]);
        fs::write(copy.join("ours"), format!("{PRE}{a}{OLD}")).unwrap();
        // What git merge-file makes of the untouched ours: what the fallback must leave.
        let out = Command::new("git")
            .args(["merge-file", "-L", "ours", "-L", "base", "-L", "theirs"])
            .args([copy.join("ours"), copy.join("base"), copy.join("theirs")])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "a conflict");
        let expected = fs::read(copy.join("ours")).unwrap();
        // %A's folder read-only: nothing new can be made beside it, so the merged text can't
        // replace it. Ours is byte for byte as it was, with nothing left beside it.
        let before = fs::read(side.join("ours")).unwrap();
        fs::set_permissions(&side, fs::Permissions::from_mode(0o555)).unwrap();
        // As root, or where the filesystem ignores modes, the folder stays writable: nothing to
        // test here then.
        let probe = side.join("probe");
        if fs::write(&probe, b"").is_ok() {
            let _ = fs::remove_file(&probe);
            fs::set_permissions(&side, fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped the read-only part: {} is still writable after chmod 555 (root, or a filesystem that ignores modes)", side.display());
            return;
        }
        let refused = replace(&side.join("ours"), b"cut off");
        let after = fs::read(side.join("ours")).unwrap();
        let left = names(&side);
        // The driver: its clean merge can't be written, so merge-file merges the original ours.
        let args = [side.join("base"), side.join("ours"), side.join("theirs")]
            .map(|x| x.to_str().unwrap().to_string());
        let code = run(&args);
        let merged = fs::read(side.join("ours")).unwrap();
        let left_after = names(&side);
        fs::set_permissions(&side, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refused.unwrap_err().contains("couldn't make"));
        assert_eq!(after, before);
        assert_eq!(left, ["base", "ours", "theirs"]);
        assert_eq!(code, Ok(1));
        assert_eq!(
            String::from_utf8_lossy(&merged),
            String::from_utf8_lossy(&expected)
        );
        assert_eq!(left_after, ["base", "ours", "theirs"]);
    }

    #[test]
    fn merge_log_replace_removes_its_new_file_when_it_fails_after_making_it() {
        let dir = crate::testkit::tempdir().unwrap();
        // %A is a folder: the new file is made and written, then the rename over it fails.
        let ours = dir.path().join("ours");
        fs::create_dir(&ours).unwrap();
        fs::write(ours.join("inside"), "kept\n").unwrap();
        let e = replace(&ours, b"merged\n").unwrap_err();
        assert!(e.starts_with("couldn't rename "), "{e}");
        assert_eq!(names(dir.path()), ["ours"], "no .ours.muster-merge-… left");
        assert_eq!(names(&ours), ["inside"]);
        assert_eq!(fs::read_to_string(ours.join("inside")).unwrap(), "kept\n");
        // %A isn't there: the new file is made and written, then reading %A's mode fails.
        let gone = dir.path().join("gone");
        let e = replace(&gone, b"merged\n").unwrap_err();
        assert!(e.contains("gone"), "{e}");
        assert_eq!(
            names(dir.path()),
            ["ours"],
            "no .gone.muster-merge-… left, no gone"
        );
    }

    /// The folder this test's `git` is in.
    fn git_dir() -> PathBuf {
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .find(|d| d.join("git").is_file())
            .expect("git on the PATH")
    }

    #[test]
    fn merge_log_a_driver_git_cant_start_still_leaves_markers_with_both_entries() {
        let dir = crate::testkit::tempdir().unwrap();
        let repo = log_repo(dir.path());
        configure(&repo).unwrap();
        let wt = dir.path().join("the worktree");
        g(
            &repo,
            &["worktree", "add", "-q", "-b", "task", wt.to_str().unwrap()],
        );
        let (a, b) = (
            entry("2026-10-14", "Main's"),
            entry("2026-10-14", "The task's"),
        );
        fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{a}{OLD}")).unwrap();
        g(&repo, &["commit", "-qam", "main's entry"]);
        fs::write(wt.join("docs/WORKLOG.md"), format!("{PRE}{b}{OLD}")).unwrap();
        g(&wt, &["commit", "-qam", "the task's entry"]);
        // A PATH with the suite's git first and no muster, as a GUI git client may have.
        let gits = git_dir();
        for d in [gits.as_path(), Path::new("/usr/bin"), Path::new("/bin")] {
            assert!(!d.join("muster").exists(), "{}", d.display());
        }
        let path = format!("{}:/usr/bin:/bin", gits.display());
        let out = Command::new("git")
            .arg("-C")
            .arg(&wt)
            .args(["rebase", "main"])
            .env("PATH", &path)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!out.status.success(), "the rebase stops on it");
        let text = fs::read_to_string(wt.join("docs/WORKLOG.md")).unwrap();
        assert!(
            text.contains("<<<<<<< ours\n") && text.contains(">>>>>>> theirs\n"),
            "{text}"
        );
        assert!(
            text.contains("**Did:** Main's,") && text.contains("**Did:** The task's,"),
            "{text}"
        );
        g(&wt, &["rebase", "--abort"]);
    }

    /// An older entry with a stray line a merge left in it, as mu-131's log had (0209).
    fn stray(day: &str, what: &str) -> String {
        format!("## {day} — {what}\n**Did:** {what}\n||||||| base\n**Verified:** `gate: green`\n\n")
    }

    #[test]
    fn merge_log_entry_wise_merges_the_mu_131_shape() {
        // The base: entries, two older ones each holding a stray marker line. Ours (main, which
        // removed both lines) put a run on top; theirs (the task) one on top and one deeper.
        let (e1, e2, e3) = (
            entry("2026-10-14", "E1"),
            entry("2026-10-13", "E2"),
            entry("2026-10-11", "E3"),
        );
        let (s1, s2) = (stray("2026-10-12", "S1"), stray("2026-10-10", "S2"));
        let (c1, c2) = (
            s1.replace("||||||| base\n", ""),
            s2.replace("||||||| base\n", ""),
        );
        let (o1, o2) = (entry("2026-10-16", "Ours 1"), entry("2026-10-15", "Ours 2"));
        let (t1, td) = (entry("2026-10-15", "Theirs"), entry("2026-10-13", "Deep"));
        let base = format!("{PRE}{e1}{e2}{s1}{e3}{s2}");
        let ours = format!("{PRE}{o1}{o2}{e1}{e2}{c1}{e3}{c2}");
        let theirs = format!("{PRE}{t1}{e1}{e2}{s1}{e3}{td}{s2}");
        conflict(merged_file_only(&base, &ours, &theirs));
        let want = format!("{PRE}{o1}{t1}{o2}{e1}{e2}{c1}{e3}{td}{c2}");
        // Through `run`, as git's driver: exit 0, the result in ours.
        let dir = crate::testkit::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), &base).unwrap();
        fs::write(p("a"), &ours).unwrap();
        fs::write(p("b"), &theirs).unwrap();
        let args = [p("o"), p("a"), p("b")].map(|x| x.to_str().unwrap().to_string());
        assert_eq!(run(&args), Ok(0));
        let got = fs::read_to_string(p("a")).unwrap();
        assert_eq!(got, want);
        assert!(!got.contains("||||||| base"));
        // Each entry byte-identical to its source.
        let r = split(got.as_bytes());
        for e in [&o1, &t1, &o2, &e1, &e2, &c1, &e3, &td, &c2] {
            assert_eq!(r.entries.iter().filter(|x| **x == e.as_bytes()).count(), 1);
        }
        // The other way round (a merge of main into the task): main's cleaned two still, and
        // main's run above the task's on their date, as the incoming side.
        let want = format!("{PRE}{o1}{o2}{t1}{e1}{e2}{c1}{e3}{td}{c2}");
        assert_eq!(clean(merged(&base, &theirs, &ours)), want);
    }

    /// `want` from merging `ours` and `theirs` both ways round (the dates order every point), by
    /// `merge` and through the binary.
    fn both_ways(what: &str, base: &str, ours: &str, theirs: &str, want: &str) {
        for (o, t) in [(ours, theirs), (theirs, ours)] {
            assert_eq!(clean(merged(base, o, t)), want, "{what}");
            let (code, text, err) = through_binary(base.as_bytes(), o.as_bytes(), t.as_bytes());
            assert_eq!(
                (code, String::from_utf8(text).unwrap().as_str()),
                (0, want),
                "{what}: {err}"
            );
        }
    }

    #[test]
    fn merge_log_entry_wise_keeps_one_sided_changes_and_removals() {
        let (e1, e2, e3, e4) = (
            entry("2026-10-14", "E1"),
            entry("2026-10-13", "E2"),
            entry("2026-10-12", "E3"),
            entry("2026-10-11", "E4"),
        );
        let base = format!("{PRE}{e1}{e2}{e3}{e4}");
        // Runs on both sides: on top and under E3.
        let (o, t, d) = (
            entry("2026-10-16", "Ours"),
            entry("2026-10-15", "Theirs"),
            entry("2026-10-12", "Deep"),
        );
        let fixed = e2.replace("**Did:** E2,", "**Did:** E2, fixed,");
        // A fenced heading inside the change stays in it.
        let fenced = e2.replace("of text.\n", "of text.\n```\n## 2026-10-01 — quoted\n```\n");
        let renamed = e2.replace("— E2\n", "— E2, renamed\n");
        let theirs = format!("{PRE}{t}{e1}{e2}{e3}{d}{e4}");
        for (what, ours, want) in [
            (
                "changed on one side",
                format!("{PRE}{o}{e1}{fixed}{e3}{e4}"),
                format!("{PRE}{o}{t}{e1}{fixed}{e3}{d}{e4}"),
            ),
            (
                "changed with a fenced heading",
                format!("{PRE}{o}{e1}{fenced}{e3}{e4}"),
                format!("{PRE}{o}{t}{e1}{fenced}{e3}{d}{e4}"),
            ),
            (
                "removed on one side",
                format!("{PRE}{o}{e1}{e3}{e4}"),
                format!("{PRE}{o}{t}{e1}{e3}{d}{e4}"),
            ),
            (
                "the last one removed",
                format!("{PRE}{o}{e1}{e2}{e3}"),
                format!("{PRE}{o}{t}{e1}{e2}{e3}{d}"),
            ),
            (
                "an edited heading: removed, and a new entry in its place",
                format!("{PRE}{o}{e1}{renamed}{e3}{e4}"),
                format!("{PRE}{o}{t}{e1}{renamed}{e3}{d}{e4}"),
            ),
        ] {
            both_ways(what, &base, &ours, &theirs, &want);
        }
        // The same change on both sides, or the same removal: there once, or gone once.
        both_ways(
            "changed the same way on both",
            &base,
            &format!("{PRE}{o}{e1}{fixed}{e3}{e4}"),
            &format!("{PRE}{t}{e1}{fixed}{e3}{d}{e4}"),
            &format!("{PRE}{o}{t}{e1}{fixed}{e3}{d}{e4}"),
        );
        both_ways(
            "a heading edited the same way on both",
            &base,
            &format!("{PRE}{o}{e1}{renamed}{e3}{e4}"),
            &format!("{PRE}{t}{e1}{renamed}{e3}{d}{e4}"),
            &format!("{PRE}{o}{t}{e1}{renamed}{e3}{d}{e4}"),
        );
        both_ways(
            "removed on both",
            &base,
            &format!("{PRE}{o}{e1}{e3}{e4}"),
            &format!("{PRE}{t}{e1}{e3}{d}{e4}"),
            &format!("{PRE}{o}{t}{e1}{e3}{d}{e4}"),
        );
        // Each side changing a different entry, one removing a third: all of it kept.
        both_ways(
            "a change on each side",
            &base,
            &format!("{PRE}{o}{e1}{fixed}{e3}"),
            &format!(
                "{PRE}{t}{e1}{e2}{}{d}{e4}",
                e3.replace("of text.", "of text, fixed.")
            ),
            &format!(
                "{PRE}{o}{t}{e1}{fixed}{}{d}",
                e3.replace("of text.", "of text, fixed.")
            ),
        );
        // A run beside a removed entry is at that entry's point, where the old entry was, and
        // merges there with the other side's run above it: on one date, that run's first. The
        // other side's run under it stays under it. Either way round.
        let (n1, n2) = (entry("2026-10-13", "N1"), entry("2026-10-13", "N2"));
        let ours = format!("{PRE}{o}{e1}{n1}{e3}{e4}");
        both_ways(
            "a run above the removed entry",
            &base,
            &ours,
            &format!("{PRE}{t}{e1}{n2}{e2}{e3}{e4}"),
            &format!("{PRE}{o}{t}{e1}{n2}{n1}{e3}{e4}"),
        );
        both_ways(
            "a run under the removed entry",
            &base,
            &ours,
            &format!("{PRE}{t}{e1}{e2}{n2}{e3}{e4}"),
            &format!("{PRE}{o}{t}{e1}{n1}{n2}{e3}{e4}"),
        );
    }

    #[test]
    fn merge_log_entry_wise_keeps_an_edited_heading_in_the_old_ones_place() {
        let (a, x, y, z) = (
            entry("2026-10-14", "A"),
            entry("2026-10-13", "X"),
            entry("2026-10-13", "Y"),
            entry("2026-10-12", "Z"),
        );
        let (o, t) = (entry("2026-10-16", "Ours"), entry("2026-10-15", "Theirs"));
        let rename = |e: &str| e.replacen('\n', ", renamed\n", 1);
        let (x2, y2) = (rename(&x), rename(&y));
        let base = format!("{PRE}{a}{x}{y}{z}");
        let ours = format!("{PRE}{o}{a}{x2}{y}{z}");
        // The other side's new entry under the old heading, on its date: under the new one.
        let n = entry("2026-10-13", "N");
        both_ways(
            "a new entry under the edited one",
            &base,
            &ours,
            &format!("{PRE}{t}{a}{x}{n}{y}{z}"),
            &format!("{PRE}{o}{t}{a}{x2}{n}{y}{z}"),
        );
        // Above it, on its date: above the new one.
        both_ways(
            "a new entry above the edited one",
            &base,
            &ours,
            &format!("{PRE}{t}{a}{n}{x}{y}{z}"),
            &format!("{PRE}{o}{t}{a}{n}{x2}{y}{z}"),
        );
        // Each side editing one of two headings next to each other, on one date: each in its
        // own place.
        both_ways(
            "a heading edited on each side",
            &base,
            &ours,
            &format!("{PRE}{t}{a}{x}{y2}{z}"),
            &format!("{PRE}{o}{t}{a}{x2}{y2}{z}"),
        );
        // And the same with a new entry between them on the other side.
        both_ways(
            "a heading edited on each side, a new entry between",
            &base,
            &ours,
            &format!("{PRE}{t}{a}{x}{n}{y2}{z}"),
            &format!("{PRE}{o}{t}{a}{x2}{n}{y2}{z}"),
        );
        // Both headings edited on one side, a new entry between the old ones on the other:
        // whether it goes above or under each new version can't be told, so markers.
        let both = format!("{PRE}{o}{a}{x2}{y2}{z}");
        let between = format!("{PRE}{t}{a}{x}{n}{y}{z}");
        for (ours, theirs) in [(&both, &between), (&between, &both)] {
            conflict(merged(&base, ours, theirs));
            let (code, _, err) =
                through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!((code, err.trim()), (1, GIVE_UP));
        }
    }

    #[test]
    fn merge_log_entry_wise_still_gives_markers() {
        let (e1, e2, e3) = (
            entry("2026-10-14", "E1"),
            entry("2026-10-13", "E2"),
            entry("2026-10-12", "E3"),
        );
        let base = format!("{PRE}{e1}{e2}{e3}");
        let (o, t) = (entry("2026-10-16", "Ours"), entry("2026-10-15", "Theirs"));
        let ours_fix = e2.replace("**Did:** E2,", "**Did:** E2, ours,");
        let theirs_fix = e2.replace("**Did:** E2,", "**Did:** E2, theirs,");
        let renamed = e2.replace("— E2\n", "— E2, renamed\n");
        let copy = entry("2026-10-13", "E2").replace("of text.", "of text, a copy.");
        for (what, ours, theirs) in [
            (
                "one entry changed differently on each side",
                format!("{PRE}{o}{e1}{ours_fix}{e3}"),
                format!("{PRE}{t}{e1}{theirs_fix}{e3}"),
            ),
            (
                "one side changed what the other removed",
                format!("{PRE}{o}{e1}{ours_fix}{e3}"),
                format!("{PRE}{t}{e1}{e3}"),
            ),
            (
                "a heading edited where the other side changed the entry",
                format!("{PRE}{o}{e1}{renamed}{e3}"),
                format!("{PRE}{t}{e1}{theirs_fix}{e3}"),
            ),
            (
                "a heading edited where the other side removed the entry",
                format!("{PRE}{o}{e1}{renamed}{e3}"),
                format!("{PRE}{t}{e1}{e3}"),
            ),
            (
                "a heading edited differently on each side",
                format!("{PRE}{o}{e1}{renamed}{e3}"),
                format!("{PRE}{t}{e1}{}{e3}", e2.replace("— E2\n", "— E2, theirs\n")),
            ),
            (
                "an entry removed on both, new entries where it was on one",
                format!("{PRE}{o}{e1}{}{e3}", entry("2026-10-13", "New")),
                format!("{PRE}{t}{e1}{e3}"),
            ),
            (
                "a fence left open that a new entry closes, letting a quoted heading out",
                format!(
                    "{PRE}{o}{e1}{e2}{e3}{}",
                    "## 2026-10-11 — N\n```\n## 2026-10-01 — quoted\n```\n"
                ),
                format!(
                    "{PRE}{t}{e1}{e2}{}",
                    e3.replace("of text.\n", "of text.\n```\n")
                ),
            ),
            (
                "ours' preamble changed",
                format!("# Log, renamed\n\nNewest first.\n\n{o}{e1}{e2}{e3}"),
                format!("{PRE}{t}{e1}{e2}{e3}"),
            ),
            (
                "theirs' preamble changed",
                format!("{PRE}{o}{e1}{ours_fix}{e3}"),
                format!("# Log, renamed\n\nNewest first.\n\n{t}{e1}{e2}{e3}"),
            ),
            (
                "an undated new entry beside a change",
                format!("{PRE}{o}{e1}{ours_fix}{e3}"),
                format!("{PRE}## Unreleased\n- t\n\n{e1}{e2}{e3}"),
            ),
            (
                "an entry moved",
                format!("{PRE}{o}{e2}{e1}{e3}"),
                format!("{PRE}{t}{e1}{e2}{e3}"),
            ),
            (
                "a new entry with the heading of one the other side changed",
                format!("{PRE}{o}{copy}{e1}{e2}{e3}"),
                format!("{PRE}{t}{e1}{theirs_fix}{e3}"),
            ),
        ] {
            assert!(
                entry_wise(base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).is_none(),
                "{what}"
            );
            let m = conflict(merged(&base, &ours, &theirs));
            assert!(
                m.contains("<<<<<<< ours\n") && m.contains(">>>>>>> theirs\n"),
                "{what}: {m}"
            );
            let (code, text, err) =
                through_binary(base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
            assert_eq!(code, 1, "{what}");
            assert_eq!(String::from_utf8(text).unwrap(), m, "{what}");
            assert_eq!(err.trim(), GIVE_UP, "{what}");
        }
    }

    #[test]
    fn merge_log_entry_wise_matches_an_entry_the_base_has_twice() {
        // The real log has one entry twice (a merge-file merge kept both copies, 0196). Ours
        // removes one copy, theirs adds on top and deeper: the others stay matched in place.
        let (e1, d, e2, e3) = (
            entry("2026-10-14", "E1"),
            entry("2026-10-13", "Twice"),
            entry("2026-10-12", "E2"),
            entry("2026-10-11", "E3"),
        );
        let base = format!("{PRE}{e1}{d}{e2}{d}{e3}");
        let (o, t, deep) = (
            entry("2026-10-16", "Ours"),
            entry("2026-10-15", "Theirs"),
            entry("2026-10-12", "Deep"),
        );
        let theirs = format!("{PRE}{t}{e1}{d}{e2}{deep}{d}{e3}");
        for ours in [
            format!("{PRE}{o}{e1}{d}{e2}{e3}"),
            format!("{PRE}{o}{e1}{e2}{d}{e3}"),
        ] {
            let got = clean(merged(&base, &ours, &theirs));
            let r = split(got.as_bytes());
            let n = |e: &str| r.entries.iter().filter(|x| **x == e.as_bytes()).count();
            assert_eq!((n(&d), n(&deep), n(&o), n(&t)), (1, 1, 1, 1), "{got}");
            assert!(got.starts_with(&format!("{PRE}{o}{t}{e1}")), "{got}");
            assert!(got.ends_with(&e3), "{got}");
            for _ in 0..3 {
                assert_eq!(
                    clean(merged(&base, &ours, &theirs)),
                    got,
                    "the same every run"
                );
            }
        }
    }

    #[test]
    fn merge_log_entry_wise_keeps_crlf_and_a_missing_last_line_end() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let (pre, e1, e2) = (
            crlf(PRE),
            crlf(&entry("2026-10-14", "E1")),
            crlf(&entry("2026-10-13", "E2")),
        );
        let (o, t) = (
            crlf(&entry("2026-10-16", "Ours")),
            crlf(&entry("2026-10-15", "Theirs")),
        );
        let fixed = e1.replace("of text.", "of text, fixed.");
        // The last line has no line end, and one side changes the entry it ends.
        let last = "## 2026-10-12 — Last\r\n**Did:** no line end";
        let last_fixed = "## 2026-10-12 — Last\r\n**Did:** no line end, fixed";
        let base = format!("{pre}{e1}{e2}{last}");
        let ours = format!("{pre}{o}{fixed}{e2}{last}");
        let theirs = format!("{pre}{t}{e1}{e2}{last_fixed}");
        let want = format!("{pre}{o}{t}{fixed}{e2}{last_fixed}");
        assert_eq!(clean(merged(&base, &ours, &theirs)), want);
        assert_eq!(clean(merged(&base, &theirs, &ours)), want);
        // One side removes the last entry: the one above it ends the file, as that side has it.
        let ours = format!("{pre}{o}{fixed}{e2}");
        let want = format!("{pre}{o}{t}{fixed}{e2}");
        assert_eq!(
            clean(merged(&base, &ours, &format!("{pre}{t}{e1}{e2}{last}"))),
            want
        );
    }

    #[test]
    fn merge_log_merges_and_rebases_a_branch_with_runs_at_two_points_onto_a_fixed_entry() {
        // mu-131's rebase in a real repo: main removed a stray marker line from an older entry
        // and put an entry on top; the task put round entries on top and its first one deeper.
        let n1 = entry("2026-10-15", "Landed before");
        let (t1, t2) = (
            entry("2026-10-15", "Round two"),
            entry("2026-10-15", "Round one"),
        );
        let deep = entry("2026-10-14", "First entry");
        let m = entry("2026-10-15", "Main's");
        let (s, fixed) = (
            stray("2026-10-12", "Stray"),
            stray("2026-10-12", "Stray").replace("||||||| base\n", ""),
        );
        let base_log = format!("{PRE}{n1}{s}{OLDER}");
        let task_log = format!("{PRE}{t1}{t2}{n1}{deep}{s}{OLDER}");
        let main_log = format!("{PRE}{m}{n1}{fixed}{OLDER}");
        for (how, onto, incoming) in [
            ("merge", "main", "task"),
            ("merge", "task", "main"),
            ("rebase", "main", "task"),
            ("rebase", "task", "main"),
        ] {
            let dir = crate::testkit::tempdir().unwrap();
            let home = dir.path().join("home");
            fs::create_dir(&home).unwrap();
            let g = |d: &Path, args: &[&str]| home_g(&home, d, args);
            let repo = log_repo_with(dir.path(), &g);
            let bin = this_muster().unwrap();
            g(&repo, &["config", &format!("merge.{DRIVER}.name"), NAME]);
            let driver = driver_command(&sh_quote(&bin.to_str().unwrap().replace('%', "%%")));
            g(
                &repo,
                &["config", &format!("merge.{DRIVER}.driver"), &driver],
            );
            fs::write(repo.join("docs/WORKLOG.md"), &base_log).unwrap();
            g(&repo, &["commit", "-qam", "the base"]);
            g(&repo, &["checkout", "-q", "-b", "task"]);
            fs::write(repo.join("docs/WORKLOG.md"), &task_log).unwrap();
            g(&repo, &["commit", "-qam", "the task"]);
            g(&repo, &["checkout", "-q", "main"]);
            fs::write(repo.join("docs/WORKLOG.md"), &main_log).unwrap();
            g(&repo, &["commit", "-qam", "main's entry and fix"]);
            g(
                &repo,
                &[
                    "checkout",
                    "-q",
                    if how == "merge" { onto } else { incoming },
                ],
            );
            if how == "merge" {
                g(&repo, &["merge", "-q", "--no-edit", incoming]);
            } else {
                g(&repo, &["rebase", "-q", onto]);
            }
            let want = if incoming == "task" {
                format!("{PRE}{t1}{t2}{m}{n1}{deep}{fixed}{OLDER}")
            } else {
                format!("{PRE}{m}{t1}{t2}{n1}{deep}{fixed}{OLDER}")
            };
            let got = fs::read_to_string(repo.join("docs/WORKLOG.md")).unwrap();
            assert_eq!(got, want, "{how} {incoming} onto {onto}");
            assert_eq!(g(&repo, &["status", "--porcelain"]), "", "{how}");
        }
    }

    #[test]
    fn stray_markers_finds_each_marker_line_outside_code() {
        let text = "# Doc\n<<<<<<< ours\na\n||||||| base\nb\n=======\nc\n>>>>>>> theirs\n";
        assert_eq!(stray_markers(text.as_bytes()), [2, 4, 6, 8]);
        // CRLF line ends, and a marker on the last line with none.
        assert_eq!(stray_markers(b"x\r\n=======\r\n>>>>>>> theirs"), [2, 3]);
        // Not markers: other runs of '=', a marker without its space, one indented, or mid-line.
        for not in [
            "=====\n",
            "========\n",
            "======= \n",
            "<<<<<<<\n",
            " <<<<<<< ours\n",
            "a <<<<<<< ours\n",
            "<<<<<<<< ours\n",
        ] {
            assert_eq!(
                stray_markers(not.as_bytes()),
                Vec::<usize>::new(),
                "{not:?}"
            );
        }
        // Quoted in a fence (``` or ~~~): skipped; after the fence closes: caught again.
        let text = "a\n```\n<<<<<<< ours\n=======\n>>>>>>> theirs\n```\n~~~md\n||||||| base\n~~~\n=======\n";
        assert_eq!(stray_markers(text.as_bytes()), [10]);
        assert_eq!(stray_markers(b""), Vec::<usize>::new());
    }
}
