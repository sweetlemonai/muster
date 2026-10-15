//! The work log's merge driver (task 0067, decision 0009): `muster merge-log %O %A %B`, which git
//! runs for `docs/WORKLOG.md` (`.gitattributes`: `docs/WORKLOG.md merge=muster-log`). Every task
//! puts its entry at the top of the log, or under a later-dated one that landed on main meanwhile,
//! and its review rounds' entries on top of that, so two branches always clash; this keeps every
//! new entry whole, newest first, so a merge or a rebase doesn't stop for them (0190: under an
//! entry, not only on top; 0196: at any number of points). Anything else is a plain three-way
//! merge, conflict markers and all. Git finds the driver in config: `muster start`
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

/// One run of new entries a side put in among the base's (0190, 0196): the points in the base's
/// entries it can be at (0 is on top; more than one when the run repeats the base entries below
/// it), where it starts in the side's entries at the first of them, and how many entries it has.
/// At point `points.start() + j` the run is the side's entries `at + j..at + j + len`: a different
/// turn of the same entries, so it's sliced at the point chosen, never moved there from another.
struct Run {
    points: std::ops::RangeInclusive<usize>,
    at: usize,
    len: usize,
}

impl Run {
    /// The run's entries in `side` when it's at `point`, one of its points.
    fn at<'a, 'l>(&self, side: &'l Log<'a>, point: usize) -> &'l [&'a [u8]] {
        let k = self.at + point - self.points.start();
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
            at,
            len,
        });
        g = p + 1;
    }
    Some(runs)
}

/// Both sides only added runs of whole dated entries among the base's, at any number of points
/// (on top, or under any base entry): the base with each side's runs at their points. Where both
/// sides have a run at one point, the two are merged there newest date first and, on the same
/// date, theirs (the incoming side: the commit a rebase replays, the branch a merge brings in)
/// above ours; each side's entries keep their own order. An entry both sides added is kept once,
/// at ours' point (what the target branch already shows). The base's entries never move or
/// change. None when a side changed anything else: an edited or removed base entry, a changed
/// preamble, a new entry without a date.
///
/// A run that fits several points (it repeats its neighbours) goes where mu-190 put it: at a
/// point the other side's run can be at too, the topmost such, else its own topmost. The runs on
/// one side are at disjoint ranges of points, in order, so one pass over both pairs them.
fn both_inserted(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let b = split(base);
    let (o_log, t_log) = (split(ours), split(theirs));
    let (o_runs, t_runs) = (inserted(&b, &o_log)?, inserted(&b, &t_log)?);
    let (mut o_at, mut t_at) = (vec![0; o_runs.len()], vec![0; t_runs.len()]);
    let (mut i, mut j) = (0, 0);
    while i < o_runs.len() || j < t_runs.len() {
        match (o_runs.get(i), t_runs.get(j)) {
            (Some(o), Some(t)) => {
                let at = *o.points.start().max(t.points.start());
                if at <= *o.points.end().min(t.points.end()) {
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
    let o: Vec<(usize, Vec<&[u8]>)> = o_runs
        .iter()
        .zip(&o_at)
        .map(|(r, &at)| (at, r.at(&o_log, at).to_vec()))
        .collect();
    // Theirs' entries ours added too go only where ours has them, each copy once.
    let mut ours_added: std::collections::HashMap<&[u8], usize> = std::collections::HashMap::new();
    for e in o.iter().flat_map(|(_, run)| run) {
        *ours_added.entry(e).or_default() += 1;
    }
    let t: Vec<(usize, Vec<&[u8]>)> = t_runs
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
            (at, kept)
        })
        .collect();
    let eol: &[u8] = if ours.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut out = b.preamble.to_vec();
    let mut push = |part: &[u8]| {
        // An entry that ended the file without a line end doesn't run into the next one.
        if !out.is_empty() && !out.ends_with(b"\n") && !part.is_empty() {
            out.extend_from_slice(eol);
        }
        out.extend_from_slice(part);
    };
    let (mut o, mut t) = (o.into_iter().peekable(), t.into_iter().peekable());
    for g in 0..=b.entries.len() {
        let o_run = o
            .next_if(|(at, _)| *at == g)
            .map(|r| r.1)
            .unwrap_or_default();
        let t_run = t
            .next_if(|(at, _)| *at == g)
            .map(|r| r.1)
            .unwrap_or_default();
        let (mut i, mut j) = (0, 0);
        while i < o_run.len() || j < t_run.len() {
            let theirs_next = match (o_run.get(i), t_run.get(j)) {
                (Some(a), Some(b)) => date(b) >= date(a),
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
        if let Some(e) = b.entries.get(g) {
            push(e);
        }
    }
    Some(out)
}

/// How a merge of the log came out: the text to write, clean or with conflict markers.
#[derive(Debug, PartialEq)]
pub enum Merged {
    Clean(Vec<u8>),
    Conflict(Vec<u8>),
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
/// (`git merge-file`) when it's clean, else both sides' new entries (`both_inserted`) when that's
/// all they added, at any number of points, else the three-way merge's conflict markers.
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
        Some(0) => Ok(Merged::Clean(out.stdout)),
        // The number of conflicts; a negative one (an error) comes out as 128 and up.
        Some(1..=127) => Ok(match both_inserted(&b, &o, &t) {
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
        let (text, code) = match m {
            Merged::Clean(t) => (t, 0),
            Merged::Conflict(t) => (t, 1),
        };
        replace(ours, &text)?;
        Ok(code)
    });
    let e = match merged {
        Ok(0) => return Ok(0),
        Ok(code) => {
            eprintln!("muster merge-log: both sides changed more than new dated entries; left the conflict markers");
            return Ok(code);
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
            eprintln!("muster merge-log: {e}; git merge-file merged it");
            Ok(0)
        }
        Ok(Some(1..=127)) => {
            eprintln!("muster merge-log: {e}; git merge-file left the conflict markers");
            Ok(1)
        }
        _ => {
            let why = match &out {
                Ok(o) => String::from_utf8_lossy(&o.stderr).trim().to_string(),
                Err(e) => e.to_string(),
            };
            eprintln!(
                "muster merge-log: {e}; git merge-file failed too ({why}), so it's as it was"
            );
            Ok(2)
        }
    }
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
        let dir = tempfile::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n);
        fs::write(p("o"), base).unwrap();
        fs::write(p("a"), ours).unwrap();
        fs::write(p("b"), theirs).unwrap();
        merge(&p("o"), &p("a"), &p("b")).unwrap()
    }

    fn clean(m: Merged) -> String {
        match m {
            Merged::Clean(t) => String::from_utf8(t).unwrap(),
            Merged::Conflict(t) => panic!("a conflict:\n{}", String::from_utf8_lossy(&t)),
        }
    }

    fn conflict(m: Merged) -> String {
        match m {
            Merged::Conflict(t) => String::from_utf8(t).unwrap(),
            Merged::Clean(t) => panic!("clean:\n{}", String::from_utf8_lossy(&t)),
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
        let got = both_inserted(
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
        // A changed older entry beside the insert.
        conflict(merged(
            &base,
            &format!("{PRE}{top}{a}{OLD}{}", OLDER.replace("first", "changed")),
            &format!("{PRE}{top}{b}{OLD}{OLDER}"),
        ));
        // A changed entry above the insert point isn't an insert (merge-file, which runs first,
        // merges these itself when the lines it matches let it).
        for changed in [
            top.replace("Top,", "Top, changed,"),
            top.replace("(pnpm check)", "(changed)"),
        ] {
            let o = format!("{PRE}{changed}{a}{OLD}{OLDER}");
            let t = format!("{PRE}{top}{b}{OLD}{OLDER}");
            assert!(both_inserted(base.as_bytes(), o.as_bytes(), t.as_bytes()).is_none());
        }
        // A changed preamble.
        conflict(merged(
            &base,
            &format!("# Renamed\n\n{top}{a}{OLD}{OLDER}"),
            &format!("{PRE}{top}{b}{OLD}{OLDER}"),
        ));
        // Inserts at two different points: merge-file merges them itself, even with only a
        // one-line entry between them, and both_inserted gives the same (0196).
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

    /// `both_inserted`'s result as text; panics when it gives up.
    fn inserted_text(base: &str, ours: &str, theirs: &str) -> String {
        let got = both_inserted(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
            .expect("both_inserted gave up");
        String::from_utf8(got).unwrap()
    }

    /// `muster merge-log` run as git runs it, through the binary, in a home of its own with no
    /// MUSTER_BASE, MUSTER_ROOT or TASKS_ROOT: its exit code, what it left in ours, its stderr.
    fn through_binary(base: &[u8], ours: &[u8], theirs: &[u8]) -> (i32, Vec<u8>, String) {
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let edited = OLD.replace("the old thing", "the old thing, edited");
        for (what, ours) in [
            (
                "an edited base entry",
                format!("{PRE}{a}{top}{c}{edited}{OLDER}"),
            ),
            ("a removed base entry", format!("{PRE}{a}{top}{c}{OLDER}")),
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
                both_inserted(base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).is_none(),
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
        // An entry added under it gives it a line end: that's an edit of the base's last entry,
        // so it's merge-file's markers.
        let tail = "## 2026-10-11 — Tail\r\n**Did:** no line end either";
        conflict(merged(
            &format!("{pre}{top}{last}"),
            &format!("{pre}{a}{top}{last}"),
            &format!("{pre}{b}{top}{c}{last}\r\n{tail}"),
        ));
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
            let dir = tempfile::tempdir().unwrap();
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
            let by_us = both_inserted(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
                .as_deref()
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
        // not just entries on top, so it's the three-way merge's conflict.
        let b = entry("2026-10-13", "Task B");
        let got = conflict(merged(
            &base,
            &format!("{PRE}{a}{OLD}{fixed}"),
            &format!("{PRE}{b}{OLD}{OLDER}"),
        ));
        assert!(
            got.contains("<<<<<<< ours\n") && got.contains(">>>>>>> theirs\n"),
            "{got}"
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
        // A side that removed an entry while the other added one on top.
        markers(merged(
            &base,
            &format!("{PRE}{a}{OLD}{OLDER}"),
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
            let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        // An unclosed fence runs to the end of the file, the base's entries inside it: not
        // just entries on top, so it's a conflict.
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
}
