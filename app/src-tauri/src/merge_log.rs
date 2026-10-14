//! The work log's merge driver (task 0067, decision 0009): `muster merge-log %O %A %B`, which git
//! runs for `docs/WORKLOG.md` (`.gitattributes`: `docs/WORKLOG.md merge=muster-log`). Every task
//! puts its entry at the top of the log, so two branches always clash there; this keeps both
//! entries whole, newest on top, so a merge or a rebase doesn't stop for them. Anything else is a
//! plain three-way merge, conflict markers and all. Git finds the driver in config: `muster start`
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

/// Split at every `## ` line outside a fenced code block, so a heading quoted in an entry's
/// code stays in that entry. A fence closes on the same character, at least as many, and
/// nothing after them but spaces; one never closed runs to the end of the file.
fn split(text: &[u8]) -> Log<'_> {
    let mut starts = Vec::new();
    let mut at = 0;
    let mut open: Option<(u8, usize)> = None;
    for line in text.split_inclusive(|&b| b == b'\n') {
        match (open, fence(line)) {
            (Some((c, n)), Some((c2, n2, rest)))
                if c2 == c && n2 >= n && rest.iter().all(|b| b" \t\r\n".contains(b)) =>
            {
                open = None
            }
            (Some(_), _) => {}
            // A backtick fence's info string can't hold a backtick (then it's inline code).
            (None, Some((c, n, rest))) if c == b'~' || !rest.contains(&b'`') => open = Some((c, n)),
            (None, _) if line.starts_with(b"## ") => starts.push(at),
            (None, _) => {}
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

/// The entries a side put on top of the base's, when that's all it changed (the same preamble,
/// the base's entries untouched below them), each with a date.
fn added_on_top<'a>(base: &Log, side: &Log<'a>) -> Option<Vec<&'a [u8]>> {
    if side.preamble != base.preamble || !side.entries.ends_with(&base.entries) {
        return None;
    }
    let new = &side.entries[..side.entries.len() - base.entries.len()];
    new.iter().all(|e| date(e).is_some()).then(|| new.to_vec())
}

/// Both sides only added whole entries on top of the base: the base with all of them on top,
/// newest date first and, on the same date, theirs (the incoming side: the commit a rebase
/// replays, the branch a merge brings in) above ours. Each side's entries keep their own order;
/// one both sides added is kept once. None when the sides changed anything else.
fn both_on_top(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let b = split(base);
    let o = added_on_top(&b, &split(ours))?;
    let t: Vec<&[u8]> = added_on_top(&b, &split(theirs))?
        .into_iter()
        .filter(|e| !o.contains(e))
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
    let (mut i, mut j) = (0, 0);
    while i < o.len() || j < t.len() {
        let theirs_next = match (o.get(i), t.get(j)) {
            (Some(a), Some(b)) => date(b) >= date(a),
            (None, Some(_)) => true,
            _ => false,
        };
        if theirs_next {
            push(t[j]);
            j += 1;
        } else {
            push(o[i]);
            i += 1;
        }
    }
    for e in &b.entries {
        push(e);
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
/// (`git merge-file`) when it's clean, else both sides' new top entries (`both_on_top`) when
/// that's all they added, else the three-way merge's conflict markers.
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
        Some(1..=127) => Ok(match both_on_top(&b, &o, &t) {
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
/// into `ours`; 0 when it's clean, 1 with conflict markers. When it can't merge (a file it can't
/// read or over 64 MB, merge-file failing, `ours` it can't write), git's merge-file merges in
/// place instead: its markers, or its clean result. When even that fails, `ours` is left as it
/// was and it's 2, which the configured driver hands to merge-file again (`driver_command`).
pub fn run(args: &[String]) -> Result<i32, String> {
    run_capped(args, MAX_BYTES)
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
        fs::write(ours, text).map_err(|e| format!("{}: {e}", ours.display()))?;
        Ok(code)
    });
    let e = match merged {
        Ok(0) => return Ok(0),
        Ok(code) => {
            eprintln!("muster merge-log: both sides changed more than new entries on top; left the conflict markers");
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
    for (key, value) in [
        ("name", NAME.to_string()),
        ("driver", driver_command("muster")),
    ] {
        let key = format!("merge.{DRIVER}.{key}");
        let out = git_output(dir, &["config", "--get", &key])?;
        match out.status.code() {
            Some(0) => {}
            Some(1) => {
                git(dir, &["config", "--local", &key, &value])?;
                wrote.push(key);
            }
            _ => {
                return Err(format!(
                    "git config --get {key}: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ))
            }
        }
    }
    Ok(wrote)
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
        let repo = dir.join("a repo");
        fs::create_dir(&repo).unwrap();
        g(&repo, &["init", "-q", "-b", "main"]);
        for (k, v) in [
            ("user.name", "Test Person"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            g(&repo, &["config", k, v]);
        }
        fs::create_dir(repo.join("docs")).unwrap();
        fs::write(
            repo.join(".gitattributes"),
            "docs/WORKLOG.md merge=muster-log\n",
        )
        .unwrap();
        fs::write(repo.join("docs/WORKLOG.md"), format!("{PRE}{OLD}")).unwrap();
        g(&repo, &["add", "-A"]);
        g(&repo, &["commit", "-qm", "base"]);
        repo
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
        // A PATH with git and no muster, as a GUI git client may have.
        let gits = git_dir();
        assert!(!gits.join("muster").exists(), "{}", gits.display());
        let path = format!("/usr/bin:/bin:{}", gits.display());
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
