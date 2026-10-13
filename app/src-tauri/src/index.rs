//! INDEX.md (task 0018): one generated master list of every task in a queue, with a link to each
//! file. `queue::with_lock` brings it up to date after every change muster makes, and writes it
//! only when its text changed. `muster index` rebuilds it, and `muster check` and
//! `manifest.py --check` report one that doesn't match the files.
//!
//! The list is made from the task files and backlog notes alone: no clock and no file times, so
//! an unchanged queue gives the same file byte for byte. Titles and other front-matter text are
//! untrusted: they're escaped, so none of them can add a link, a heading or a list item.

use crate::backlog::{self, Note};
use crate::queue::{self, State, Task, MAX_TASK_BYTES, STAGES, STATUS_FOLDERS};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The master list's file, in the queue folder.
pub const FILE: &str = "INDEX.md";

/// How the last line of INDEX.md starts: what it was made from (`sources`), so `manifest.py
/// --check` can tell a stale list without making one.
const SOURCES_LINE: &str = "<!-- muster sources: ";

/// The most of an existing INDEX.md that's read to compare: more than 9,999 tasks' lines.
const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;

/// Nesting deeper than this is shown at this depth (goal → feature → task is 3).
const MAX_DEPTH: usize = 8;

/// The queue's master list, for the tasks and notes of `queue` in `state` (pure; `rollup` must
/// have run, as `read_state` does). A header saying it's generated; every goal and feature that
/// isn't done, with what sits under it as nested lists; then the tasks under no goal or feature
/// by stage, in `STAGES` order; then the backlog notes (file, title); then everything done, in a
/// collapsed "Done (n)" list. Each task appears once, in id order within its list.
pub fn index_md(state: &State, queue: &str) -> String {
    let mut tasks: Vec<&Task> = state.tasks.iter().filter(|t| t.queue == queue).collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
    let mut notes: Vec<&Note> = state.backlog.iter().filter(|n| n.queue == queue).collect();
    notes.sort_by(|a, b| a.file.cmp(&b.file));

    // Parent links by position (an id used twice links to its first file, as `rollup` does).
    let mut first: HashMap<&str, usize> = HashMap::new();
    for (i, t) in tasks.iter().enumerate() {
        first.entry(t.id.as_str()).or_insert(i);
    }
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];
    let mut is_root = vec![false; tasks.len()];
    for (i, t) in tasks.iter().enumerate() {
        match first.get(t.parent.as_str()) {
            Some(&p) if p != i && !t.parent.is_empty() => kids[p].push(i),
            _ => is_root[i] = true,
        }
    }
    // Features before tasks under a goal; id order within each.
    for k in &mut kids {
        k.sort_by_key(|&i| (level_rank(&tasks[i].level), i));
    }
    // Parents that go round in a loop reach none of the roots: those are listed on their own.
    let mut reached = vec![false; tasks.len()];
    let mut stack: Vec<usize> = (0..tasks.len()).filter(|&i| is_root[i]).collect();
    while let Some(i) = stack.pop() {
        if !std::mem::replace(&mut reached[i], true) {
            stack.extend(&kids[i]);
        }
    }
    let done = |i: usize| tasks[i].shown_stage == "done";
    let mut placed = vec![false; tasks.len()];
    // The ids still open, for what a task waits on (as `claim` reads `depends:`: any file with
    // that id not done holds it).
    let open: HashSet<&str> = tasks
        .iter()
        .filter(|t| t.stage != "done")
        .map(|t| t.id.as_str())
        .collect();
    let mut list =
        |i: usize, out: &mut String| list_from(&tasks, &open, &kids, &mut placed, i, out);

    let mut heads: Vec<usize> = (0..tasks.len())
        .filter(|&i| is_root[i] && tasks[i].level != "task" && !done(i))
        .collect();
    heads.sort_by_key(|&i| (level_rank(&tasks[i].level), i));
    let mut tree = String::new();
    for i in heads {
        list(i, &mut tree);
    }
    let loose: Vec<usize> = (0..tasks.len())
        .filter(|&i| (!reached[i] || (is_root[i] && tasks[i].level == "task")) && !done(i))
        .collect();
    let mut by_stage: Vec<(usize, String)> = vec![(0, String::new()); STAGES.len()];
    for i in loose {
        let at = STAGES
            .iter()
            .position(|s| *s == tasks[i].stage)
            .unwrap_or(0);
        let n = list(i, &mut by_stage[at].1);
        by_stage[at].0 += n;
    }
    let mut finished = String::new();
    let mut finished_n = 0;
    for i in (0..tasks.len()).filter(|&i| done(i) && (is_root[i] || !reached[i])) {
        finished_n += list(i, &mut finished);
    }

    let mut out = header(as_of(&tasks));
    if tasks.is_empty() && notes.is_empty() {
        out.push_str("\nNo tasks and no backlog notes yet.\n");
    }
    if !tree.is_empty() {
        out.push_str(&format!("\n## Goals and features\n\n{tree}"));
    }
    if by_stage.iter().any(|(n, _)| *n > 0) {
        out.push_str("\n## Tasks without a goal or feature\n");
        for (stage, (n, text)) in STAGES.iter().zip(&by_stage) {
            if *n > 0 {
                out.push_str(&format!("\n### {} ({n})\n\n{text}", stage_name(stage)));
            }
        }
    }
    if !notes.is_empty() {
        out.push_str(&format!("\n## Backlog notes ({})\n\n", notes.len()));
        for n in &notes {
            let title = match md_text(&n.title, 200) {
                t if t.is_empty() => "(empty note)".to_string(),
                t => t,
            };
            out.push_str(&format!("- {} · {title}\n", md_text(&n.file, 200)));
        }
    }
    if finished_n > 0 {
        out.push_str(&format!(
            "\n<details>\n<summary>Done ({finished_n})</summary>\n\n{finished}\n</details>\n"
        ));
    }
    out
}

/// Write `i` and everything under it into `out`, nested, each task once (`placed`); returns how
/// many lines it wrote. A stack, not recursion, so a hand-made chain of parents can't overflow.
fn list_from(
    tasks: &[&Task],
    open: &HashSet<&str>,
    kids: &[Vec<usize>],
    placed: &mut [bool],
    i: usize,
    out: &mut String,
) -> usize {
    let mut n = 0;
    let mut stack = vec![(i, 0usize)];
    while let Some((j, depth)) = stack.pop() {
        if std::mem::replace(&mut placed[j], true) {
            continue;
        }
        out.push_str(&"  ".repeat(depth.min(MAX_DEPTH)));
        out.push_str(&line(tasks[j], open));
        out.push('\n');
        n += 1;
        stack.extend(kids[j].iter().rev().map(|&k| (k, depth + 1)));
    }
    n
}

fn level_rank(level: &str) -> usize {
    queue::LEVELS.iter().position(|l| *l == level).unwrap_or(2)
}

/// A stage as people read it, a goal's or feature's "in-progress" included.
fn stage_name(stage: &str) -> String {
    match (stage, queue::stage_label(stage)) {
        ("in-progress", _) => "In progress".into(),
        (other, "") => md_text(other, 40),
        (_, label) => label.into(),
    }
}

/// One task's line: id, linked title, level (goals and features), "you" for the owner's own task,
/// stage (and what a blocked one waits on), the tasks it depends on that are still `open`, its
/// open vision conflicts, progress and due date, priority, type (tasks), estimate (`muster
/// list`'s text), and how many comments it has.
fn line(t: &Task, open: &HashSet<&str>) -> String {
    let title = match md_text(&t.title, 200) {
        s if s.is_empty() => "(untitled)".to_string(),
        s => s,
    };
    let mut parts = vec![format!(
        "{} [{title}]({})",
        md_text(&t.id, 40),
        link(&t.path)
    )];
    if t.level != "task" {
        parts.push(level_name(&t.level));
    }
    // The owner's own task (for: human, 0022).
    if t.is_human() {
        parts.push("you".into());
    }
    let mut stage = stage_name(&t.shown_stage);
    if t.blocked {
        stage.push_str(", blocked");
        let on = md_text(&t.waiting_on, 80);
        if !on.is_empty() {
            stage.push_str(&format!(": {on}"));
        }
    }
    parts.push(stage);
    let waits: Vec<&str> = t
        .depends
        .iter()
        .map(String::as_str)
        .filter(|d| *d != t.id && open.contains(d))
        .collect();
    if !waits.is_empty() {
        parts.push(format!("waits on {}", md_text(&waits.join(", "), 400)));
    }
    // Open vision conflicts (0029): they hold it in Shaping, as the card's badge says.
    if let Some(n) = t.vision_open.filter(|n| *n > 0) {
        parts.push(format!("⚠ {n} vision"));
    }
    if let Some(p) = t.progress.as_ref().filter(|_| t.level != "task") {
        parts.push(format!("{} of {} done", p.done, p.total));
    }
    if !t.due.is_empty() {
        parts.push(format!("due {}", md_text(&t.due, 40)));
    }
    if !t.priority.is_empty() {
        parts.push(md_text(&t.priority, 40));
    }
    if t.level == "task" && !t.kind.is_empty() {
        parts.push(md_text(&t.kind, 40));
    }
    let est = crate::ops::estimate_text(t);
    if !est.is_empty() {
        parts.push(format!("est {}", md_text(&est, 80)));
    }
    match t.comments.len() {
        0 => {}
        1 => parts.push("1 comment".into()),
        n => parts.push(format!("{n} comments")),
    }
    format!("- {}", parts.join(" · "))
}

fn level_name(level: &str) -> String {
    match level {
        "goal" => "Goal".into(),
        "feature" => "Feature".into(),
        other => md_text(other, 40),
    }
}

/// The newest stamp in the tasks' logs (`YYYY-MM-DD HH:MM`, or a day), their stage history and
/// capture days: the last change the list shows, never the clock.
fn as_of(tasks: &[&Task]) -> Option<String> {
    let stamp = |s: &str| -> Option<String> {
        match (s.get(..16), s.get(..10)) {
            (Some(m), _) if crate::ops::stamp_minutes(m).is_some() => Some(m.to_string()),
            (_, Some(d)) if queue::is_date(d) => Some(d.to_string()),
            _ => None,
        }
    };
    tasks
        .iter()
        .flat_map(|t| {
            t.log_tail
                .iter()
                .map(String::as_str)
                .chain(t.stage_history.iter().map(|s| s.at.as_str()))
                .chain([t.captured.as_str()])
                .filter_map(stamp)
        })
        .max()
}

fn header(as_of: Option<String>) -> String {
    let mut out = String::from(
        "# Master list\n\nEvery task in this queue, with a link to its file. Generated by muster from the task files: never edit it by hand, since the next change to the queue overwrites it. `muster index` rebuilds it.\n",
    );
    if let Some(at) = as_of {
        out.push_str(&format!(
            "\nAs of {at} (the newest change in a task's log).\n"
        ));
    }
    out
}

/// Text from a task file as Markdown that shows it as written: control characters and line
/// breaks as spaces, bidirectional overrides out, runs of white space as one space, at most
/// `max` characters; Markdown's characters (`\` `` ` `` `*` `_` `[` `]` `<` `>` `|` `&` `~` `#`
/// `!`) and a leading `-` or `+` escaped with a backslash.
fn md_text(s: &str, max: usize) -> String {
    let plain: String = s
        .chars()
        .filter(|&c| !is_bidi_control(c))
        .map(|c| {
            if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let words = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = if words.chars().count() > max {
        format!(
            "{}…",
            words
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        words
    };
    let mut out = String::with_capacity(cut.len() + 8);
    for (i, c) in cut.chars().enumerate() {
        if "\\`*_[]<>|&~#!".contains(c) || (i == 0 && (c == '-' || c == '+')) {
            out.push('\\');
        }
        out.push(c);
    }
    unlinked(&out)
}

/// `http://`, `https://` and `www.` (any case) with a zero-width space inside, so a viewer's
/// autolinking (GitHub's) shows a web address in a title as text, never as a live link.
fn unlinked(text: &str) -> String {
    // ASCII lowercasing keeps every byte where it was, so offsets in `lower` are offsets in `text`.
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len() + 6);
    let mut from = 0;
    let mut at = 0;
    while at < text.len() {
        let rest = &lower[at..];
        let split = if rest.starts_with("https://") {
            Some(at + "https:".len())
        } else if rest.starts_with("http://") {
            Some(at + "http:".len())
        } else if rest.starts_with("www.") {
            Some(at + "www".len())
        } else {
            None
        };
        match split {
            Some(s) => {
                out.push_str(&text[from..s]);
                out.push('\u{200b}');
                from = s;
                at = s + 1;
            }
            None => at += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    out.push_str(&text[from..]);
    out
}

/// Characters that reorder the text around them (an RTL override can make a title read as
/// another one).
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// A path relative to the queue as a Markdown link target: every byte other than letters,
/// digits, `-._~/` percent-encoded, so a space or a bracket in a file name can't end the link.
fn link(path: &str) -> String {
    path.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// The text of INDEX.md: the list, then the line naming its sources.
pub fn file_text(state: &State, queue: &str, sources: &str) -> String {
    format!("{}\n{SOURCES_LINE}{sources} -->\n", index_md(state, queue))
}

/// What INDEX.md is made from, as a 64-bit FNV-1a hash in hex: every task file (`NNNN-slug.md` in
/// the queue folder and its status folders, symlinks followed) and every backlog note (plain
/// files only), in order of their path relative to the queue, each as its path, a NUL, its size,
/// a NUL and its bytes (none for a file over 1 MB, which isn't read). `manifest.py` works out the
/// same. A name that isn't UTF-8 is left out on both sides.
pub fn sources(root: &Path) -> String {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let names = |dir: &Path| -> Vec<(String, fs::DirEntry)> {
        fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| Some((e.file_name().to_str()?.to_string(), e)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut task_dirs = vec![(String::new(), root.to_path_buf())];
    task_dirs.extend(
        STATUS_FOLDERS
            .iter()
            .map(|f| (format!("{f}/"), root.join(f))),
    );
    for (prefix, dir) in task_dirs {
        for (name, e) in names(&dir) {
            if queue::is_task_name(&name) && e.path().is_file() {
                files.push((format!("{prefix}{name}"), e.path()));
            }
        }
    }
    let notes = backlog::backlog_dir(root);
    let prefix = notes
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!("{n}/"))
        .unwrap_or_default();
    for (name, e) in names(&notes) {
        if backlog::is_note_file(&name) && e.file_type().is_ok_and(|t| t.is_file()) {
            files.push((format!("{prefix}{name}"), e.path()));
        }
    }
    files.sort();
    let mut hash = FNV_OFFSET;
    for (rel, path) in files {
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let bytes = if size > MAX_TASK_BYTES {
            Vec::new()
        } else {
            fs::read(&path).unwrap_or_default()
        };
        let size = size.to_string();
        for part in [rel.as_bytes(), b"\0", size.as_bytes(), b"\0", &bytes] {
            fnv1a(&mut hash, part);
        }
    }
    format!("{hash:016x}")
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

/// A queue's tasks (rolled up) and backlog notes as they are on disk now, as `index_md` takes
/// them (queue name ""). Unlike `read_state`, no git log and no problems: only what the list shows.
fn read(root: &Path) -> State {
    let mut tasks: Vec<Task> = queue::task_files(root)
        .into_iter()
        .filter_map(|(folder, path)| {
            let meta = fs::metadata(&path).ok()?;
            if meta.len() > MAX_TASK_BYTES {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            queue::parse_task("", root, &folder, &path, &text, 0)
        })
        .collect();
    queue::rollup(&mut tasks);
    let notes = backlog::read_notes("", root, &mut HashMap::new(), &mut HashSet::new());
    State {
        tasks,
        backlog: notes.notes,
        ..Default::default()
    }
}

/// INDEX.md as the files say it should be now.
pub fn build(root: &Path) -> String {
    let sources = sources(root);
    file_text(&read(root), "", &sources)
}

/// The INDEX.md there is now: None when there's none, or when it isn't a plain file of a sane
/// size (it's replaced, never followed).
fn current(root: &Path) -> Result<Option<String>, ()> {
    let path = root.join(FILE);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(m) if m.is_file() && m.len() <= MAX_INDEX_BYTES => {
            fs::read_to_string(&path).map(Some).map_err(|_| ())
        }
        _ => Err(()),
    }
}

/// Bring INDEX.md up to date, writing it (atomically) only when its text changed. Returns whether
/// it wrote. The caller holds the queue's lock.
pub(crate) fn refresh_locked(root: &Path) -> Result<bool, String> {
    let text = build(root);
    if current(root).is_ok_and(|old| old.as_deref() == Some(text.as_str())) {
        return Ok(false);
    }
    queue::write_atomic(&root.join(FILE), &text)?;
    Ok(true)
}

/// `muster index`: rebuild INDEX.md under the queue's lock. Returns whether it changed.
pub fn rebuild(root: &Path) -> Result<bool, String> {
    queue::hold_lock(root, || refresh_locked(root))
}

/// Whether INDEX.md matches the files.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Current,
    /// There, but not what the files make (or not a plain file).
    Stale,
    /// The list shows what the files say, but they changed since it was written, outside muster
    /// (Drover's answers): only its "As of" or sources line is behind.
    Refresh,
    Missing,
}

/// INDEX.md against the files, as `muster index --check` and `muster check` see it.
pub fn status(root: &Path) -> Status {
    match current(root) {
        Ok(None) => Status::Missing,
        Ok(Some(text)) => {
            let now = build(root);
            if text == now {
                Status::Current
            } else if shown(&text) == shown(&now) {
                Status::Refresh
            } else {
                Status::Stale
            }
        }
        Err(()) => Status::Stale,
    }
}

/// The list as it reads, without the lines that follow every change to the files rather than what
/// the list shows: "As of" and the sources line. No task's text can start a line, so these can't
/// be forged by a title.
fn shown(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("As of ") && !l.starts_with(SOURCES_LINE))
        .collect()
}

/// The problem `muster check` reports for a stale INDEX.md.
pub const STALE: &str = "INDEX.md doesn't match the task files (rebuild with `muster index`)";

/// The warning `muster check` gives for a list only due a refresh (`Status::Refresh`).
pub const REFRESH: &str = "INDEX.md: refresh due (the files changed outside muster; the next muster write or `muster index` refreshes it)";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops;
    use crate::queue::{read_state, Cache};

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }

    /// A task file: `rel`, its id, extra front matter, title and log lines.
    fn task(d: &Path, rel: &str, id: &str, fm: &str, title: &str, log: &str) {
        fs::write(
            d.join(rel),
            format!("---\nid: {id}\ntype: code\npriority: normal\n{fm}\n---\n\n# {title}\n\n## Log\n{log}"),
        )
        .unwrap();
    }

    fn state(d: &Path) -> State {
        read_state(&[("p".into(), d.to_path_buf())], &mut Cache::default())
    }

    /// Goals, features and tasks at every stage, one note and one task done.
    fn sample() -> tempfile::TempDir {
        let d = queue();
        let p = d.path();
        let created = "- 2026-10-12 09:00 capture: created\n";
        task(
            p,
            "todo/0001-ship.md",
            "0001",
            "status: todo\nstage: ready\nlevel: goal\npriority: high\ndue: 2026-12-01",
            "Ship 1.0",
            created,
        );
        task(
            p,
            "todo/0002-track.md",
            "0002",
            "status: todo\nstage: ready\nlevel: feature\nparent: 0001",
            "Stage track",
            created,
        );
        task(
            p,
            "doing/0003-pass.md",
            "0003",
            "status: doing\nstage: building\nparent: 0002\nclaimed-by: wt-a\ntype: bug\nest-time: 2h\nest-test-time: 30m\nest-tokens: 400k\ndepends: [0004, 0007]",
            "Pass button",
            "- 2026-10-12 09:00 capture: created\n- 2026-10-12 10:30 wt-a: stage ready → building (claimed)\n",
        );
        task(
            p,
            "blocked/0004-wait.md",
            "0004",
            "status: blocked\nstage: testing\nparent: 0001\nwaiting_on: 0012 the API\ndepends: 0007",
            "Waiting",
            created,
        );
        task(
            p,
            "todo/0005-loose.md",
            "0005",
            "status: todo\nstage: ready",
            "Loose",
            created,
        );
        let loose = p.join("todo/0005-loose.md");
        let text = fs::read_to_string(&loose).unwrap().replace(
            "## Log\n",
            "## Comments\n- **owner, 2026-10-12 09:30:** Smaller, please.\n- **wt-a, 2026-10-12 09:40:** Will do.\n  Next week.\n\n## Log\n",
        );
        fs::write(&loose, text).unwrap();
        task(
            p,
            "inbox/0006-raw.md",
            "0006",
            "status: inbox",
            "Raw",
            "- 2026-10-11 captured\n",
        );
        task(
            p,
            "done/0007-old.md",
            "0007",
            "status: done\nstage: done",
            "Old",
            "- 2026-10-13 08:00 owner: stage your-check → done\n",
        );
        fs::write(p.join("inbox/2026-10-12-0900-an-idea.md"), "# An idea\n").unwrap();
        d
    }

    const SAMPLE: &str = "# Master list

Every task in this queue, with a link to its file. Generated by muster from the task files: never edit it by hand, since the next change to the queue overwrites it. `muster index` rebuilds it.

As of 2026-10-13 08:00 (the newest change in a task's log).

## Goals and features

- 0001 [Ship 1.0](todo/0001-ship.md) · Goal · In progress · 0 of 2 done · due 2026-12-01 · high
  - 0002 [Stage track](todo/0002-track.md) · Feature · In progress · 0 of 1 done · normal
    - 0003 [Pass button](doing/0003-pass.md) · Building · waits on 0004 · normal · bug · est agent 2h · test 30m · 400k
  - 0004 [Waiting](blocked/0004-wait.md) · Testing, blocked: 0012 the API · normal · code

## Tasks without a goal or feature

### Backlog (1)

- 0006 [Raw](inbox/0006-raw.md) · Backlog · normal · code

### Ready (1)

- 0005 [Loose](todo/0005-loose.md) · Ready · normal · code · 2 comments

## Backlog notes (1)

- 2026-10-12-0900-an-idea.md · An idea

<details>
<summary>Done (1)</summary>

- 0007 [Old](done/0007-old.md) · Done · normal · code

</details>
";

    #[test]
    fn lists_goals_features_tasks_by_stage_notes_and_done() {
        let d = sample();
        assert_eq!(index_md(&state(d.path()), "p"), SAMPLE);
        // Only the queue asked for.
        assert_eq!(
            index_md(&state(d.path()), "other"),
            header(None) + "\nNo tasks and no backlog notes yet.\n"
        );
    }

    #[test]
    fn the_same_files_give_the_same_text_whatever_the_clock_or_file_times() {
        let d = sample();
        let before = build(d.path());
        let p = d.path().join("todo/0005-loose.md");
        fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(build(d.path()), before);
        // A change to a file is a change to its sources, so the list says it's stale.
        fs::write(
            &p,
            fs::read_to_string(&p).unwrap().replace("Loose", "Looser"),
        )
        .unwrap();
        let after = build(d.path());
        assert_ne!(after, before);
        assert!(after.contains("[Looser]"));
    }

    #[test]
    fn every_task_once_with_missing_parents_loops_and_tasks_under_tasks() {
        let d = queue();
        let p = d.path();
        let log = "- 2026-10-12 09:00 capture: created\n";
        task(
            p,
            "todo/0001-orphan.md",
            "0001",
            "status: todo\nstage: ready\nparent: 0099",
            "Orphan",
            log,
        );
        task(
            p,
            "todo/0002-a.md",
            "0002",
            "status: todo\nstage: ready\nlevel: feature\nparent: 0003",
            "Loop A",
            log,
        );
        task(
            p,
            "todo/0003-b.md",
            "0003",
            "status: todo\nstage: ready\nlevel: goal\nparent: 0002",
            "Loop B",
            log,
        );
        task(
            p,
            "todo/0004-top.md",
            "0004",
            "status: todo\nstage: ready",
            "Top task",
            log,
        );
        task(
            p,
            "todo/0005-under.md",
            "0005",
            "status: todo\nstage: ready\nparent: 0004",
            "Under a task",
            log,
        );
        task(
            p,
            "todo/0006-self.md",
            "0006",
            "status: todo\nstage: ready\nparent: 0006",
            "Own parent",
            log,
        );
        task(
            p,
            "done/0007-feat.md",
            "0007",
            "status: done\nstage: done\nlevel: feature",
            "Done feature",
            log,
        );
        task(
            p,
            "done/0008-kid.md",
            "0008",
            "status: done\nstage: done\nparent: 0007",
            "Done kid",
            log,
        );
        let text = index_md(&state(p), "p");
        for id in [
            "0001", "0002", "0003", "0004", "0005", "0006", "0007", "0008",
        ] {
            let lines = text
                .lines()
                .filter(|l| l.trim_start().starts_with(&format!("- {id} [")))
                .count();
            assert_eq!(lines, 1, "{id} once:\n{text}");
        }
        assert!(text.contains("- 0004 [Top task](todo/0004-top.md) · Ready · normal · code\n  - 0005 [Under a task]"), "{text}");
        assert!(text.contains("<summary>Done (2)</summary>\n\n- 0007 [Done feature](done/0007-feat.md) · Feature · Done · 1 of 1 done · normal\n  - 0008 [Done kid]"), "{text}");
        assert!(text.contains("### Ready (6)"), "{text}");
    }

    #[test]
    fn hostile_and_unicode_titles_are_escaped_text() {
        let d = queue();
        let p = d.path();
        let log = "- 2026-10-12 09:00 capture: created\n";
        task(
            p,
            "todo/0001-x.md",
            "0001",
            "status: todo\nstage: ready",
            "x",
            log,
        );
        let mut s = state(p);
        let cases = [
            (
                "[click](javascript:alert(1)) | `rm -rf` <script>",
                "\\[click\\](javascript:alert(1)) \\| \\`rm -rf\\` \\<script\\>",
            ),
            ("# Heading", "\\# Heading"),
            ("- item", "\\- item"),
            ("+ item", "\\+ item"),
            ("two\nlines\r\n## Log", "two lines \\#\\# Log"),
            (
                "*bold* _it_ ~s~ &amp; !x \\ end",
                "\\*bold\\* \\_it\\_ \\~s\\~ \\&amp; \\!x \\\\ end",
            ),
            ("Ünïcödé ✓ 日本語 🚀 👨‍👩‍👧", "Ünïcödé ✓ 日本語 🚀 👨‍👩‍👧"),
            ("safe\u{202e}txt.exe\u{2066}", "safetxt.exe"),
            ("tab\there\u{2028}sep\u{1b}[31m", "tab here sep \\[31m"),
            ("   ", "(untitled)"),
        ];
        for (title, want) in cases {
            s.tasks[0].title = title.into();
            let text = index_md(&s, "p");
            let want = format!("- 0001 [{want}](todo/0001-x.md) · Ready");
            assert!(
                text.contains(&want),
                "{title:?}: wanted {want:?} in\n{text}"
            );
            // Nothing in a title starts a line of its own.
            assert_eq!(
                text.lines().filter(|l| l.starts_with("- ")).count(),
                1,
                "{text}"
            );
        }
        assert_eq!(md_text(&"é".repeat(300), 200).chars().count(), 200);
        assert!(md_text(&"é".repeat(300), 200).ends_with('…'));
        // Front-matter text other than the title is escaped too.
        s.tasks[0].title = "x".into();
        s.tasks[0].priority = "[p](x)".into();
        s.tasks[0].kind = "|t|".into();
        s.tasks[0].blocked = true;
        s.tasks[0].waiting_on = "owner: <b>now</b>".into();
        let text = index_md(&s, "p");
        assert!(
            text.contains("· Ready, blocked: owner: \\<b\\>now\\</b\\> · \\[p\\](x) · \\|t\\|\n"),
            "{text}"
        );
    }

    #[test]
    fn a_line_shows_the_estimate_as_the_list_does_what_it_still_waits_on_and_its_comments() {
        let d = queue();
        let p = d.path();
        let log = "- 2026-10-12 09:00 capture: created\n";
        for (rel, id, stage) in [
            ("todo/0001-a.md", "0001", "ready"),
            ("todo/0002-b.md", "0002", "ready"),
            ("done/0003-c.md", "0003", "done"),
        ] {
            task(p, rel, id, &format!("stage: {stage}"), id, log);
        }
        let mut s = state(p);
        s.tasks.sort_by(|a, b| a.id.cmp(&b.id));
        let line_of = |s: &State| {
            index_md(s, "p")
                .lines()
                .find(|l| l.starts_with("- 0001 "))
                .unwrap()
                .to_string()
        };
        // Only test time: the same text `muster list` shows.
        s.tasks[0].est_test_time = "45m".into();
        assert_eq!(crate::ops::estimate_text(&s.tasks[0]), "test 45m");
        assert!(
            line_of(&s).ends_with(" · code · est test 45m"),
            "{}",
            line_of(&s)
        );
        // Done, missing and its own id never hold it; what's open is listed in its order.
        s.tasks[0].depends = ["0003", "0099", "0001"].map(String::from).to_vec();
        assert!(!line_of(&s).contains("waits on"), "{}", line_of(&s));
        s.tasks[0].depends = ["0002", "0003", "0001"].map(String::from).to_vec();
        assert!(
            line_of(&s).contains(" · Ready · waits on 0002 · "),
            "{}",
            line_of(&s)
        );
        s.tasks[2].stage = "testing".into();
        assert!(
            line_of(&s).contains(" · Ready · waits on 0002, 0003 · "),
            "{}",
            line_of(&s)
        );
        // One comment, then two.
        let c = |text: &str| crate::fields::Comment {
            who: "owner".into(),
            at: "2026-10-12 09:30".into(),
            text: text.into(),
        };
        s.tasks[0].comments = vec![c("one")];
        assert!(line_of(&s).ends_with(" · 1 comment"), "{}", line_of(&s));
        s.tasks[0].comments.push(c("two"));
        assert!(line_of(&s).ends_with(" · 2 comments"), "{}", line_of(&s));
    }

    #[test]
    fn a_line_shows_open_vision_conflicts_and_is_quiet_without_them() {
        let d = queue();
        let p = d.path();
        let log = "- 2026-10-12 09:00 capture: created\n";
        task(p, "todo/0001-a.md", "0001", "stage: shaping", "A", log);
        task(p, "todo/0002-b.md", "0002", "stage: ready", "B", log);
        let mut s = state(p);
        s.tasks.sort_by(|a, b| a.id.cmp(&b.id));
        s.tasks[0].depends = vec!["0002".into()];
        let line_of = |s: &State| {
            index_md(s, "p")
                .lines()
                .find(|l| l.starts_with("- 0001 "))
                .unwrap()
                .to_string()
        };
        // Never checked, or checked with none open: nothing.
        for open in [None, Some(0)] {
            s.tasks[0].vision_open = open;
            assert!(!line_of(&s).contains("vision"), "{}", line_of(&s));
        }
        s.tasks[0].vision_open = Some(2);
        assert!(
            line_of(&s).contains(" · Shaping · waits on 0002 · ⚠ 2 vision · normal · "),
            "{}",
            line_of(&s)
        );
    }

    #[test]
    fn a_vision_record_and_resolve_keep_the_list_current() {
        let d = queue();
        let p = d.path();
        let t = "2026-10-12 14:05";
        let n = ops::NewTask {
            title: "Checked".into(),
            ..Default::default()
        };
        ops::create_task(p, &n, "owner", t).unwrap();
        let first = || {
            fs::read_to_string(p.join(FILE))
                .unwrap()
                .lines()
                .find(|l| l.starts_with("- 0001 "))
                .unwrap()
                .to_string()
        };
        let two = r#"{"conflicts": [{"title": "A", "this": {"quote": "a", "at": "0001"}, "other": {"quote": "b", "at": "docs/VISION.md:3"}, "fix": "f"}, {"title": "B", "this": {"quote": "c", "at": "0001"}, "other": {"quote": "d", "at": "docs/DESIGN.md:9"}, "fix": "g"}]}"#;
        crate::vision::record(p, "0001", two, "wt-shape", t).unwrap();
        assert!(first().contains(" · ⚠ 2 vision · "), "{}", first());
        assert_eq!(status(p), Status::Current);
        crate::vision::resolve(p, "0001", "V1", "kept the spine", "owner", t).unwrap();
        assert!(first().contains(" · ⚠ 1 vision · "), "{}", first());
        crate::vision::resolve(p, "0001", "V2", "dropped it", "owner", t).unwrap();
        assert!(!first().contains("vision"), "{}", first());
        assert_eq!(status(p), Status::Current);
    }

    #[test]
    fn a_human_task_is_marked_you_and_an_agents_task_waiting_on_it_says_so() {
        // 0022: the owner's own tasks say "you"; agents' tasks say nothing extra.
        let d = queue();
        let p = d.path();
        let log = "- 2026-10-12 09:00 capture: created\n";
        task(
            p,
            "todo/0001-a.md",
            "0001",
            "stage: ready\nfor: human",
            "Buy the domain",
            log,
        );
        task(
            p,
            "todo/0002-b.md",
            "0002",
            "stage: ready\ndepends: [0001]",
            "Set up DNS",
            log,
        );
        task(
            p,
            "todo/0003-c.md",
            "0003",
            "stage: ready\nfor: ai",
            "Agent work",
            log,
        );
        let mut s = state(p);
        let text = index_md(&s, "p");
        assert!(
            text.contains(
                "- 0001 [Buy the domain](todo/0001-a.md) · you · Ready · normal · code\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "- 0002 [Set up DNS](todo/0002-b.md) · Ready · waits on 0001 · normal · code\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("- 0003 [Agent work](todo/0003-c.md) · Ready · normal · code\n"),
            "{text}"
        );
        // Both marks on one line: the owner's own task with open vision conflicts (0029).
        for t in s.tasks.iter_mut().filter(|t| t.id == "0001") {
            t.vision_open = Some(1);
        }
        let text = index_md(&s, "p");
        assert!(
            text.contains(
                "- 0001 [Buy the domain](todo/0001-a.md) · you · Ready · ⚠ 1 vision · normal · code\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn links_survive_odd_file_names() {
        assert_eq!(link("todo/0001-plain.md"), "todo/0001-plain.md");
        assert_eq!(
            link("todo/0001-a (b) [c]é.md"),
            "todo/0001-a%20%28b%29%20%5Bc%5D%C3%A9.md"
        );
    }

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        let h = |s: &[u8]| {
            let mut h = FNV_OFFSET;
            fnv1a(&mut h, s);
            format!("{h:016x}")
        };
        assert_eq!(h(b""), "cbf29ce484222325");
        assert_eq!(h(b"a"), "af63dc4c8601ec8c");
        assert_eq!(h(b"foobar"), "85944171f73967e8");
    }

    #[test]
    fn sources_cover_task_files_and_notes_only() {
        let d = sample();
        let p = d.path();
        let before = sources(p);
        assert_eq!(before.len(), 16);
        assert_eq!(sources(p), before);
        // Files that aren't sources don't count.
        fs::write(p.join("README.md"), "changed").unwrap();
        fs::write(p.join("tasks.json"), "{}").unwrap();
        fs::write(p.join("inbox/.hidden.md"), "x").unwrap();
        fs::create_dir(p.join("inbox/dropped")).unwrap();
        fs::write(p.join("inbox/dropped/old.md"), "x").unwrap();
        assert_eq!(sources(p), before);
        // A note, a task file, its name: each counts.
        fs::write(p.join("inbox/new.md"), "x").unwrap();
        let with_note = sources(p);
        assert_ne!(with_note, before);
        fs::rename(p.join("todo/0005-loose.md"), p.join("doing/0005-loose.md")).unwrap();
        assert_ne!(sources(p), with_note);
    }

    #[test]
    fn every_write_under_the_lock_keeps_the_list_and_only_rewrites_a_change() {
        let d = queue();
        let p = d.path();
        let t = "2026-10-12 14:05";
        assert_eq!(status(p), Status::Missing);
        let new = |title: &str| ops::NewTask {
            title: title.into(),
            ..Default::default()
        };
        ops::create_task(p, &new("First"), "owner", t).unwrap();
        assert_eq!(status(p), Status::Current);
        let text = fs::read_to_string(p.join(FILE)).unwrap();
        assert!(
            text.contains("- 0001 [First](todo/0001-first.md) · Ready"),
            "{text}"
        );
        assert!(text.ends_with(&format!("\n<!-- muster sources: {} -->\n", sources(p))));
        // A refusal changes nothing, so the file isn't written again.
        let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(7);
        fs::File::options()
            .write(true)
            .open(p.join(FILE))
            .unwrap()
            .set_modified(stamp)
            .unwrap();
        assert!(ops::claim_task(p, "0009", "wt-a", t).is_err());
        let modified = || fs::metadata(p.join(FILE)).unwrap().modified().unwrap();
        assert_eq!(modified(), stamp);
        assert!(!rebuild(p).unwrap());
        // Each kind of write moves it: a claim, a backlog note, an answer.
        ops::claim_task(p, "0001", "wt-a", t).unwrap();
        assert!(fs::read_to_string(p.join(FILE))
            .unwrap()
            .contains("· Building"));
        crate::backlog::add(p, "An idea", "", "owner", t).unwrap();
        assert!(fs::read_to_string(p.join(FILE))
            .unwrap()
            .contains("## Backlog notes (1)"));
        assert_eq!(status(p), Status::Current);
        // Questions change the files but not the list: the sources line still follows them.
        let sources_line = || {
            let text = fs::read_to_string(p.join(FILE)).unwrap();
            text.lines().last().unwrap().to_string()
        };
        let mut seen = vec![sources_line()];
        queue::ask(p, "0001", "Which port?", "wt-a", "aaaa1111", t).unwrap();
        seen.push(sources_line());
        queue::escalate(p, "0001", 1, "capture", t).unwrap();
        seen.push(sources_line());
        queue::answer(p, "0001", 1, "5173", "owner", t).unwrap();
        seen.push(sources_line());
        for w in seen.windows(2) {
            assert_ne!(w[0], w[1], "every write moves the sources line");
        }
        assert_eq!(status(p), Status::Current);
        // A hand edit leaves it stale until the next write, or `muster index`.
        let f = p.join("doing/0001-first.md");
        fs::write(
            &f,
            fs::read_to_string(&f)
                .unwrap()
                .replace("# First", "# Renamed"),
        )
        .unwrap();
        assert_eq!(status(p), Status::Stale);
        assert!(rebuild(p).unwrap());
        assert_eq!(status(p), Status::Current);
        assert!(fs::read_to_string(p.join(FILE))
            .unwrap()
            .contains("[Renamed]"));
    }

    #[test]
    fn depends_estimates_comments_links_and_checkins_keep_the_list_current() {
        let d = queue();
        let p = d.path();
        let t = "2026-10-12 14:05";
        for title in ["First", "Second"] {
            let n = ops::NewTask {
                title: title.into(),
                ..Default::default()
            };
            ops::create_task(p, &n, "owner", t).unwrap();
        }
        let text = || fs::read_to_string(p.join(FILE)).unwrap();
        let second = || {
            text()
                .lines()
                .find(|l| l.starts_with("- 0002 "))
                .unwrap()
                .to_string()
        };
        ops::set_field(p, "0002", "depends", "0001", "owner", t).unwrap();
        assert!(
            second().contains(" · Ready · waits on 0001 · "),
            "{}",
            second()
        );
        ops::set_field(p, "0002", "test-time", "45m", "owner", t).unwrap();
        assert!(second().ends_with(" · est test 45m"), "{}", second());
        crate::fields::add_comment(p, "0002", "Looks right", "owner", t).unwrap();
        assert!(second().ends_with(" · 1 comment"), "{}", second());
        assert_eq!(status(p), Status::Current);
        // Writes the list doesn't show still move its sources line, under the same lock.
        let sources_line = || text().lines().last().unwrap().to_string();
        let mut seen = vec![sources_line()];
        crate::fields::add_link(p, "0002", "https://example.com/spec", "Spec", "owner", t).unwrap();
        seen.push(sources_line());
        crate::fields::remove_link(p, "0002", "1", "owner", t).unwrap();
        seen.push(sources_line());
        ops::claim_task(p, "0001", "wt-a", t).unwrap();
        seen.push(sources_line());
        ops::checkin_task(p, "0001", "Half way", "50", "wt-a", t).unwrap();
        seen.push(sources_line());
        for w in seen.windows(2) {
            assert_ne!(w[0], w[1], "every write moves the sources line");
        }
        assert_eq!(status(p), Status::Current);
        // Once what it waits on is done, the marker goes.
        ops::set_stage(p, "0001", "done", None, "owner", t).unwrap();
        assert!(!second().contains("waits on"), "{}", second());
        assert_eq!(status(p), Status::Current);
    }

    #[test]
    fn a_write_outside_muster_the_list_does_not_show_is_a_refresh_not_stale() {
        let d = sample();
        let p = d.path();
        assert!(rebuild(p).unwrap());
        // What Drover's answer does, under its own lock and never through muster: an answer in
        // the Questions section and a newer log line (which moves "As of").
        let f = p.join("doing/0003-pass.md");
        let text = fs::read_to_string(&f).unwrap().replace(
            "## Log\n",
            "## Questions\n- [x] **Q1 (wt-a, session aaaa1111, 2026-10-12 10:00):** Which port?\n  **A (owner, 2026-10-14 09:00):** 5173\n\n## Log\n",
        ) + "- 2026-10-14 09:00 owner: answered Q1\n";
        fs::write(&f, text).unwrap();
        assert_eq!(status(p), Status::Refresh);
        // A change the list shows is stale.
        fs::write(
            &f,
            fs::read_to_string(&f)
                .unwrap()
                .replace("# Pass button", "# Renamed"),
        )
        .unwrap();
        assert_eq!(status(p), Status::Stale);
        assert!(rebuild(p).unwrap());
        assert_eq!(status(p), Status::Current);
    }

    #[test]
    fn a_list_that_cannot_be_written_never_fails_the_write() {
        let d = queue();
        let p = d.path();
        fs::create_dir(p.join(FILE)).unwrap();
        let n = ops::NewTask {
            title: "Still made".into(),
            ..Default::default()
        };
        let c = ops::create_task(p, &n, "owner", "2026-10-12 14:05").unwrap();
        assert!(p.join(&c.path).is_file());
        assert_eq!(status(p), Status::Stale);
        assert!(rebuild(p).is_err());
        // No temp file left behind in the queue folder.
        let left: Vec<String> = fs::read_dir(p)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn web_addresses_in_titles_stay_text() {
        for (title, want) in [
            (
                "see https://evil.example/x",
                "see https:\u{200b}//evil.example/x",
            ),
            (
                "HTTP://A.B and http://c.d",
                "HTTP:\u{200b}//A.B and http:\u{200b}//c.d",
            ),
            ("go to www.example.com", "go to www\u{200b}.example.com"),
            ("WWW.X.Y", "WWW\u{200b}.X.Y"),
            ("plain words, no link", "plain words, no link"),
            ("été https://é.fr", "été https:\u{200b}//é.fr"),
        ] {
            assert_eq!(md_text(title, 200), want, "{title:?}");
        }
    }

    #[test]
    fn a_planted_index_is_replaced_never_followed() {
        let d = sample();
        let p = d.path();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("victim.md");
        fs::write(&target, "keep me").unwrap();
        std::os::unix::fs::symlink(&target, p.join(FILE)).unwrap();
        assert_eq!(status(p), Status::Stale);
        assert!(rebuild(p).unwrap());
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep me");
        assert!(!fs::symlink_metadata(p.join(FILE)).unwrap().is_symlink());
        assert_eq!(status(p), Status::Current);
    }

    #[test]
    fn a_flat_queue_gets_one_too() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let n = ops::NewTask {
            title: "Flat".into(),
            ..Default::default()
        };
        ops::create_task(p, &n, "owner", "2026-10-12 14:05").unwrap();
        crate::backlog::add(p, "A flat note", "", "owner", "2026-10-12 14:05").unwrap();
        let text = fs::read_to_string(p.join(FILE)).unwrap();
        assert!(
            text.contains("- 0001 [Flat](0001-flat.md) · Ready"),
            "{text}"
        );
        assert!(text.contains("A flat note"), "{text}");
        assert_eq!(status(p), Status::Current);
    }

    #[test]
    fn an_empty_queue_says_so() {
        let d = queue();
        let text = build(d.path());
        assert!(text.starts_with("# Master list\n"));
        assert!(text.contains("No tasks and no backlog notes yet."));
        assert!(!text.contains("As of"));
    }
}
