//! What's true now (mu-102): a short page per area in the repo's `docs/now/`, each saying which
//! rules hold right now for one part of the product, one line each, citing the task or decision
//! that made it true. A page's front matter says what it covers (`covers: <globs>`, the syntax of
//! a task's `touches:`), and a task's pages are those whose `covers:` overlap its `touches:`
//! (`parallel::touches_overlap`).
//!
//! The builder keeps them true in the task's branch: `muster pass <id> --from building` is
//! refused while a page the task covers is unchanged on the branch, unless `--now-unchanged
//! "<why>"` says why it's still true (`pass_check`). The vision check (`vision::brief`) quotes the
//! task's pages, the shaping brief (`backlog::shape`) names the pages its matched tasks cover, and
//! `muster check` warns about a page over the cap, without `covers:`, with another's `covers:`,
//! or a symlink (`warnings`). A project without `docs/now/` is never asked.
//!
//! Page files are untrusted: only plain `*.md` files directly in `docs/now/` are read (a symlink
//! is passed over and warned about, and so is a `docs/now/` that leads out of its repo), each read
//! is bounded, and the briefs quote the text as data. Every git call is an argument array.

use crate::parallel::{self, git, git_output};
use crate::{backlog, queue};
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Where the pages live, relative to a repo.
pub const DIR: &str = "docs/now";
/// A page's caps: lines, and bytes (so a page in Persian meets the line cap first).
pub const MAX_LINES: usize = 60;
pub const MAX_BYTES: u64 = 4096;
/// What's read of a page at most: the cap and a margin, so an oversized page is still matched.
const READ_CAP: u64 = MAX_BYTES * 4;
/// Pages read from one folder, at most.
const MAX_PAGES: usize = 100;
/// `--now-unchanged`'s reason, in characters at most.
pub const MAX_WHY: usize = 500;

/// One page as read.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The file, absolute.
    pub path: PathBuf,
    /// Its name in the repo: `docs/now/board.md`.
    pub name: String,
    /// `covers:` from its front matter; empty when it has none.
    pub covers: Vec<String>,
    /// Its text, control characters out, at most `READ_CAP` bytes.
    pub text: String,
    pub lines: usize,
    pub bytes: u64,
}

/// What `read` found: the pages in name order, and what's wrong in the folder (warnings).
#[derive(Debug, Default)]
pub struct Found {
    pub pages: Vec<Page>,
    pub problems: Vec<String>,
}

/// `covers:` in a page's front matter (`---` lines around `key: value` lines), as `touches:` is
/// read: a comma list, each entry trimmed. Empty without front matter or the field.
pub fn covers(text: &str) -> Vec<String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Vec::new();
    }
    for line in lines {
        let line = line.trim_end();
        if line == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("covers:") {
            return queue::parse_touches(v);
        }
    }
    Vec::new()
}

/// The first `cap` bytes of `path` as text (control characters out but line breaks and tabs), and
/// the file's size.
fn read_capped(path: &Path, cap: u64) -> Result<(String, u64), String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let mut buf = Vec::new();
    f.take(cap)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok((
        backlog::clean_text(&String::from_utf8_lossy(&buf)),
        len.max(buf.len() as u64),
    ))
}

/// The pages in `repo`'s `docs/now/`. Nothing (and no problem) when there's no such folder.
pub fn read(repo: &Path) -> Found {
    let mut out = Found::default();
    let dir = repo.join(DIR);
    if !dir.is_dir() {
        return out;
    }
    let shown = dir.display().to_string();
    // The folder itself must stay in the repo: a symlinked docs/now could lead anywhere.
    let inside = match (dir.canonicalize(), repo.canonicalize()) {
        (Ok(d), Ok(r)) => d.starts_with(&r) && !is_link(&dir),
        _ => false,
    };
    if !inside {
        out.problems.push(format!(
            "warning: {shown} leads out of its repo (a symlink); now pages must be plain files in the repo, so none were read"
        ));
        return out;
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return out;
    };
    let mut entries: Vec<fs::DirEntry> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Some(file) = e.file_name().to_str().map(String::from) else {
            continue;
        };
        if file.starts_with('.') || !file.ends_with(".md") {
            continue;
        }
        let path = e.path();
        let shown = backlog::one_line_name(&path.display().to_string());
        let Ok(kind) = e.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            out.problems.push(format!(
                "warning: {shown} is a symlink; a now page must be a plain file, so it wasn't read"
            ));
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        if out.pages.len() >= MAX_PAGES {
            out.problems.push(format!(
                "warning: {} holds more than {MAX_PAGES} pages; the rest weren't read",
                backlog::one_line_name(&dir.display().to_string())
            ));
            break;
        }
        match read_capped(&path, READ_CAP) {
            Ok((text, bytes)) => out.pages.push(Page {
                name: format!("{DIR}/{}", backlog::one_line_name(&file)),
                covers: covers(&text),
                lines: text.lines().count(),
                bytes,
                text,
                path,
            }),
            Err(err) => out
                .problems
                .push(format!("warning: {shown} couldn't be read: {err}")),
        }
    }
    out
}

fn is_link(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// The places of a queue's project that have a `docs/now/` folder (`backlog::project_places`:
/// the project folder, then each repo beside the queue).
pub fn places(root: &Path) -> Vec<PathBuf> {
    backlog::project_places(root)
        .into_iter()
        .filter(|p| p.join(DIR).is_dir())
        .collect()
}

/// Every page of a queue's project, from each of its places.
pub fn for_queue(root: &Path) -> Found {
    let mut out = Found::default();
    for place in places(root) {
        let f = read(&place);
        out.pages.extend(f.pages);
        out.problems.extend(f.problems);
    }
    out
}

/// The pages whose `covers:` overlap `touches` (none when `touches` is empty).
pub fn covered<'a>(pages: &'a [Page], touches: &[String]) -> Vec<&'a Page> {
    if touches.is_empty() {
        return Vec::new();
    }
    pages
        .iter()
        .filter(|p| !p.covers.is_empty() && parallel::touches_overlap(&p.covers, touches))
        .collect()
}

/// What `muster check` warns about in `found`: what `read` found wrong, a page over the cap, one
/// without `covers:`, and two with identical `covers:` (the same entries, in any order). None of
/// them stops a command.
pub fn warnings(found: &Found) -> Vec<String> {
    let mut out = found.problems.clone();
    for p in &found.pages {
        let shown = backlog::one_line_name(&p.path.display().to_string());
        if p.lines > MAX_LINES {
            out.push(format!(
                "warning: {shown} is {} lines; a now page is {MAX_LINES} at most: shorten it or split the area",
                p.lines
            ));
        }
        if p.bytes > MAX_BYTES {
            out.push(format!(
                "warning: {shown} is {} bytes; a now page is {MAX_BYTES} at most: shorten it or split the area",
                p.bytes
            ));
        }
        if p.covers.is_empty() {
            out.push(format!(
                "warning: {shown} has no covers: in its front matter, so no task reads it: start it with a front matter that says covers: <globs>"
            ));
        }
    }
    let key = |p: &Page| {
        let mut c = p.covers.clone();
        c.sort();
        c.dedup();
        c
    };
    for (i, a) in found.pages.iter().enumerate() {
        if a.covers.is_empty() {
            continue;
        }
        for b in &found.pages[i + 1..] {
            if key(a) == key(b) {
                out.push(format!(
                    "warning: {} and {} cover the same files ({}); merge them or narrow one",
                    backlog::one_line_name(&a.path.display().to_string()),
                    backlog::one_line_name(&b.path.display().to_string()),
                    a.covers.join(", ")
                ));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The builder's pass

/// What the building pass found about a task's pages.
#[derive(Debug, PartialEq)]
pub enum Pass {
    /// Nothing to ask: no `touches:`, no `docs/now/`, or no page covers it.
    Free,
    /// The pages it covers: those its branch changes, those it doesn't, and why the rest couldn't
    /// be read from a branch (it has none, or git failed), if so.
    Covered {
        updated: Vec<String>,
        unchanged: Vec<String>,
        unread: Option<String>,
    },
}

/// The pages task `t` covers and which of them its branch changes (`git diff --name-only
/// <merge-base>..<tip> -- docs/now/`). Read from the task's worktree while it exists (the branch's
/// own pages); else from the project's places, against the branch's tip when the branch is there.
pub fn pass_check(root: &Path, t: &queue::Task) -> Pass {
    if t.touches.is_empty() {
        return Pass::Free;
    }
    let wt = Path::new(&t.worktree);
    let (repo, tip, pages) = if !t.worktree.is_empty() && wt.is_dir() {
        if !wt.join(DIR).is_dir() {
            return Pass::Free;
        }
        (
            Some(wt.to_path_buf()),
            Some("HEAD".to_string()),
            read(wt).pages,
        )
    } else {
        let places = places(root);
        if places.is_empty() {
            return Pass::Free;
        }
        let branch = format!("refs/heads/{}", t.branch);
        let at = (!t.branch.is_empty())
            .then(|| {
                places
                    .iter()
                    .find(|p| git(p, &["rev-parse", "--verify", "--quiet", &branch]).is_ok())
            })
            .flatten()
            .cloned();
        let pages = places.iter().flat_map(|p| read(p).pages).collect();
        let tip = at.as_ref().map(|_| branch);
        (at, tip, pages)
    };
    let mine: Vec<&Page> = covered(&pages, &t.touches);
    if mine.is_empty() {
        return Pass::Free;
    }
    let names: Vec<String> = mine.iter().map(|p| p.name.clone()).collect();
    let changed = match (&repo, &tip) {
        (Some(repo), Some(tip)) => changed_pages(repo, tip),
        _ => Err(if t.branch.is_empty() {
            "it wasn't started on its own branch (muster start)".to_string()
        } else {
            format!("its branch {} isn't there", t.branch)
        }),
    };
    match changed {
        Ok(changed) => {
            let (updated, unchanged) = names.into_iter().partition(|n| changed.contains(n));
            Pass::Covered {
                updated,
                unchanged,
                unread: None,
            }
        }
        Err(why) => Pass::Covered {
            updated: Vec::new(),
            unchanged: names,
            unread: Some(why),
        },
    }
}

/// The files under `docs/now/` that `tip` changes since it left the repo's main branch, relative
/// to `repo` (`docs/now/board.md`), named as `read` names them. `-z` keeps git from quoting a
/// non-ASCII name (`"docs/now/\330\252..."`), so a page named in Persian matches.
fn changed_pages(repo: &Path, tip: &str) -> Result<Vec<String>, String> {
    let r = parallel::repo_of(repo)?;
    let base = git(
        repo,
        &["merge-base", &format!("refs/heads/{}", r.trunk), tip],
    )?;
    let args = [
        "diff",
        "--name-only",
        "-z",
        "--relative",
        "--no-renames",
        &base,
        tip,
        "--",
        DIR,
    ];
    let out = git_output(repo, &args)?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            backlog::one_line_name(String::from_utf8_lossy(&out.stderr).trim())
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|n| !n.is_empty())
        .map(backlog::one_line_name)
        .collect())
}

/// Refused, naming the pages and the flag, when `p` has a page the branch doesn't change and no
/// `why` was given. Otherwise the log line's text (after `<who>: `), or None when there's nothing
/// to say.
pub fn pass_line(id: &str, p: &Pass, why: &str) -> Result<Option<String>, String> {
    let Pass::Covered {
        updated,
        unchanged,
        unread,
    } = p
    else {
        return Ok(None);
    };
    if !unchanged.is_empty() && why.is_empty() {
        let pages = unchanged.join(", ");
        let which = if unchanged.len() == 1 {
            "page"
        } else {
            "pages"
        };
        return Err(match unread {
            None => format!(
                "{id} touches what the now {which} {pages} cover{}, and its branch doesn't change {}: update the {which} in the branch if the task changed a rule there, or pass with --now-unchanged \"<why it's still true>\"",
                if unchanged.len() == 1 { "s" } else { "" },
                if unchanged.len() == 1 { "it" } else { "them" },
            ),
            Some(u) => format!(
                "{id} touches what the now {which} {pages} cover{}, and muster can't see whether {} changed: {u}. Pass with --now-unchanged \"<why it's still true, or where you updated it>\"",
                if unchanged.len() == 1 { "s" } else { "" },
                if unchanged.len() == 1 { "it" } else { "they" },
            ),
        });
    }
    let mut parts = Vec::new();
    if !updated.is_empty() {
        parts.push(format!("now pages updated: {}", updated.join(", ")));
    }
    if !unchanged.is_empty() {
        parts.push(format!(
            "now pages unchanged: {} ({why})",
            unchanged.join(", ")
        ));
    }
    Ok(Some(parts.join("; ")))
}

// ---------------------------------------------------------------------------------------------
// The briefs

/// A page as a brief lists it.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    /// The file, absolute.
    pub path: String,
    pub covers: Vec<String>,
    /// Its text, for a page the task covers; None when only its name is listed.
    pub text: Option<String>,
    /// The tasks that cover it (the shaping brief's matched tasks); empty elsewhere.
    pub tasks: Vec<String>,
}

impl Listed {
    pub fn of(p: &Page, with_text: bool) -> Listed {
        Listed {
            path: p.path.display().to_string(),
            covers: p.covers.clone(),
            text: with_text.then(|| p.text.clone()),
            tasks: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn page(name: &str, covers: &[&str]) -> Page {
        Page {
            path: PathBuf::from(format!("/r/{DIR}/{name}")),
            name: format!("{DIR}/{name}"),
            covers: covers.iter().map(|s| s.to_string()).collect(),
            text: String::new(),
            lines: 3,
            bytes: 40,
        }
    }

    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn covers_reads_the_front_matter_like_touches() {
        assert_eq!(
            covers("---\ncovers: app/src/Board*, app/src-tauri/src/ops.rs ,\n---\n# B\n"),
            sv(&["app/src/Board*", "app/src-tauri/src/ops.rs"])
        );
        assert_eq!(covers("---\r\ncovers: a/\r\n---\r\n"), sv(&["a/"]));
        // Only in the front matter, and only with one.
        assert!(covers("# B\ncovers: a/\n").is_empty());
        assert!(covers("---\ntitle: x\n---\ncovers: a/\n").is_empty());
        assert!(covers("---\ncovers:\n---\n").is_empty());
        assert!(covers("").is_empty());
    }

    #[test]
    fn covered_matches_touches_by_the_parallel_rule() {
        let pages = vec![
            page("board.md", &["app/src/Board*"]),
            page("claims.md", &["app/src-tauri/src/session.rs"]),
            page("bare.md", &[]),
        ];
        let names = |t: &[&str]| -> Vec<String> {
            covered(&pages, &sv(t))
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };
        assert_eq!(names(&["app/src/Board.tsx"]), sv(&["docs/now/board.md"]));
        assert_eq!(names(&["app/src/"]), sv(&["docs/now/board.md"]));
        assert_eq!(
            names(&["app/src-tauri/src/session.rs", "app/src/BoardView.tsx"]),
            sv(&["docs/now/board.md", "docs/now/claims.md"])
        );
        assert!(names(&["docs/"]).is_empty());
        // No touches, no pages; a page without covers is never a task's.
        assert!(names(&[]).is_empty());
    }

    #[test]
    fn read_takes_plain_md_files_and_warns_about_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let now = d.path().join(DIR);
        fs::create_dir_all(now.join("sub")).unwrap();
        fs::write(
            now.join("b.md"),
            "---\ncovers: x/\n---\n# B\n- one (mu-1)\n",
        )
        .unwrap();
        fs::write(now.join("a.md"), "# A\x07\n").unwrap();
        fs::write(now.join("notes.txt"), "x").unwrap();
        fs::write(now.join(".hidden.md"), "x").unwrap();
        fs::write(now.join("sub/c.md"), "x").unwrap();
        fs::write(d.path().join("outside.md"), "secret").unwrap();
        std::os::unix::fs::symlink(d.path().join("outside.md"), now.join("link.md")).unwrap();
        let f = read(d.path());
        let names: Vec<&str> = f.pages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["docs/now/a.md", "docs/now/b.md"]);
        assert_eq!(f.pages[0].text, "# A\n", "control characters out");
        assert_eq!(f.pages[1].covers, sv(&["x/"]));
        assert_eq!(f.pages[1].lines, 5);
        assert_eq!(f.problems.len(), 1);
        assert!(
            f.problems[0].contains("link.md is a symlink"),
            "{:?}",
            f.problems
        );
        assert!(f.pages.iter().all(|p| !p.text.contains("secret")));
        // No folder: nothing, quietly.
        let e = tempfile::tempdir().unwrap();
        let f = read(e.path());
        assert!(f.pages.is_empty() && f.problems.is_empty());
    }

    #[test]
    fn read_refuses_a_now_folder_that_leads_out_of_the_repo() {
        let d = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("x.md"), "---\ncovers: a/\n---\n").unwrap();
        fs::create_dir(d.path().join("docs")).unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), d.path().join(DIR)).unwrap();
        let f = read(d.path());
        assert!(f.pages.is_empty());
        assert!(
            f.problems[0].contains("leads out of its repo"),
            "{:?}",
            f.problems
        );
    }

    #[test]
    fn read_refuses_a_now_folder_linked_to_another_folder_in_the_repo() {
        let d = tempfile::tempdir().unwrap();
        let other = d.path().join("docs/other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("x.md"), "---\ncovers: a/\n---\n").unwrap();
        std::os::unix::fs::symlink(&other, d.path().join(DIR)).unwrap();
        let f = read(d.path());
        assert!(f.pages.is_empty(), "{:?}", f.pages);
        assert!(
            f.problems[0].contains("leads out of its repo"),
            "{:?}",
            f.problems
        );
    }

    #[test]
    fn read_bounds_a_big_page_and_still_counts_its_size() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(DIR)).unwrap();
        let big = format!("---\ncovers: a/\n---\n{}", "x\n".repeat(50_000));
        fs::write(d.path().join(DIR).join("big.md"), &big).unwrap();
        let f = read(d.path());
        let p = &f.pages[0];
        assert_eq!(p.bytes, big.len() as u64);
        assert!(p.text.len() as u64 <= READ_CAP);
        assert_eq!(p.covers, sv(&["a/"]));
        let w = warnings(&f);
        assert!(
            w.iter()
                .any(|l| l.contains("bytes; a now page is 4096 at most")),
            "{w:?}"
        );
        assert!(
            w.iter()
                .any(|l| l.contains("lines; a now page is 60 at most")),
            "{w:?}"
        );
    }

    #[test]
    fn warnings_name_each_bad_page_and_never_a_good_one() {
        let mut long = page("long.md", &["a/"]);
        long.lines = 61;
        let mut heavy = page("heavy.md", &["b/"]);
        heavy.bytes = 4097;
        let mut fine = page("fine.md", &["c/"]);
        fine.lines = 60;
        fine.bytes = 4096;
        let f = Found {
            pages: vec![
                long,
                heavy,
                fine,
                page("bare.md", &[]),
                page("one.md", &["d/", "e/*"]),
                page("two.md", &["e/*", "d/"]),
                page("other-bare.md", &[]),
            ],
            problems: vec!["warning: x is a symlink".into()],
        };
        let w = warnings(&f);
        assert_eq!(w.len(), 6, "{w:?}");
        assert!(w.iter().all(|l| l.starts_with("warning: ")));
        assert!(w.iter().all(|l| !l.contains("fine.md")));
        assert!(w[1].contains("long.md is 61 lines"));
        assert!(w[2].contains("heavy.md is 4097 bytes"));
        assert!(w[3].contains("bare.md has no covers:"));
        assert!(w[4].contains("other-bare.md has no covers:"));
        assert!(
            w[5].contains("one.md and /r/docs/now/two.md cover the same files"),
            "{w:?}"
        );
    }

    #[test]
    fn pass_line_refuses_an_unchanged_page_without_a_why() {
        let p = Pass::Covered {
            updated: vec![],
            unchanged: sv(&["docs/now/board.md"]),
            unread: None,
        };
        let e = pass_line("0007", &p, "").unwrap_err();
        assert!(e.contains("docs/now/board.md"), "{e}");
        assert!(e.contains("--now-unchanged"), "{e}");
        assert_eq!(
            pass_line("0007", &p, "a typo fix").unwrap(),
            Some("now pages unchanged: docs/now/board.md (a typo fix)".into())
        );
        let both = Pass::Covered {
            updated: sv(&["docs/now/a.md"]),
            unchanged: sv(&["docs/now/b.md"]),
            unread: None,
        };
        assert!(pass_line("0007", &both, "").is_err());
        assert_eq!(
            pass_line("0007", &both, "b still holds").unwrap(),
            Some("now pages updated: docs/now/a.md; now pages unchanged: docs/now/b.md (b still holds)".into())
        );
        let done = Pass::Covered {
            updated: sv(&["docs/now/a.md"]),
            unchanged: vec![],
            unread: None,
        };
        assert_eq!(
            pass_line("0007", &done, "").unwrap(),
            Some("now pages updated: docs/now/a.md".into())
        );
        assert_eq!(pass_line("0007", &Pass::Free, "why").unwrap(), None);
        let unread = Pass::Covered {
            updated: vec![],
            unchanged: sv(&["docs/now/a.md", "docs/now/b.md"]),
            unread: Some("it wasn't started on its own branch (muster start)".into()),
        };
        let e = pass_line("0007", &unread, "").unwrap_err();
        assert!(
            e.contains("can't see whether they changed: it wasn't started"),
            "{e}"
        );
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn changed_pages_reads_the_branch_since_it_left_main() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        git_in(r, &["init", "-q", "-b", "main"]);
        git_in(r, &["config", "user.email", "t@example.com"]);
        git_in(r, &["config", "user.name", "t"]);
        fs::create_dir_all(r.join(DIR)).unwrap();
        fs::write(r.join(DIR).join("a.md"), "---\ncovers: x/\n---\n").unwrap();
        fs::write(r.join(DIR).join("b.md"), "---\ncovers: y/\n---\n").unwrap();
        git_in(r, &["add", "-A"]);
        git_in(r, &["commit", "-qm", "base"]);
        git_in(r, &["checkout", "-qb", "task"]);
        assert!(changed_pages(r, "HEAD").unwrap().is_empty());
        fs::write(
            r.join(DIR).join("b.md"),
            "---\ncovers: y/\n---\n- new (mu-2)\n",
        )
        .unwrap();
        fs::write(r.join("other.txt"), "x").unwrap();
        git_in(r, &["add", "-A"]);
        git_in(r, &["commit", "-qm", "b"]);
        // Main moving on doesn't count against the branch.
        git_in(r, &["checkout", "-q", "main"]);
        fs::write(
            r.join(DIR).join("a.md"),
            "---\ncovers: x/\n---\n- main (mu-3)\n",
        )
        .unwrap();
        git_in(r, &["commit", "-qam", "main"]);
        assert_eq!(
            changed_pages(r, "refs/heads/task").unwrap(),
            sv(&["docs/now/b.md"])
        );
    }

    #[test]
    fn changed_pages_names_a_persian_page_as_read_does() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        git_in(r, &["init", "-q", "-b", "main"]);
        git_in(r, &["config", "user.email", "t@example.com"]);
        git_in(r, &["config", "user.name", "t"]);
        fs::create_dir_all(r.join(DIR)).unwrap();
        fs::write(r.join(DIR).join("تابلو.md"), "---\ncovers: x/\n---\n").unwrap();
        git_in(r, &["add", "-A"]);
        git_in(r, &["commit", "-qm", "base"]);
        git_in(r, &["checkout", "-qb", "task"]);
        fs::write(
            r.join(DIR).join("تابلو.md"),
            "---\ncovers: x/\n---\n- تازه (mu-2)\n",
        )
        .unwrap();
        git_in(r, &["commit", "-qam", "page"]);
        let changed = changed_pages(r, "HEAD").unwrap();
        assert_eq!(changed, sv(&["docs/now/تابلو.md"]));
        assert_eq!(read(r).pages[0].name, changed[0], "the names match");
    }

    // ---- The building pass, in a temp project (mu-102 AC2, AC3) ----

    use crate::ops::{self, PassOpts};
    use crate::parallel::tests::{commit, fx, task, Fx};
    use crate::parallel::{start, Place};

    const PAGE: &str = "---\ncovers: b/\n---\n# B\n\n- b holds. (mu-1)\n";

    fn pass(f: &Fx, id: &str, why: &str) -> Result<String, String> {
        ops::pass_task(
            &f.root,
            id,
            "wt-a",
            "2026-10-13 11:00",
            &PassOpts {
                gate: "gate: green (built)".into(),
                summary: "A synthetic summary".into(),
                from: Some("building".into()),
                now_unchanged: why.into(),
                ..Default::default()
            },
        )
    }

    fn file(f: &Fx, id: &str) -> String {
        fs::read_to_string(queue::find_task(&f.root, id).unwrap()).unwrap()
    }

    fn stage(f: &Fx, id: &str) -> String {
        queue::read_task(&f.root, id).unwrap().stage
    }

    /// A started task touching `touches`, with `b/x.txt` built on its branch, in a project whose
    /// main has the page `docs/now/b.md` covering `b/`.
    fn started(touches: &str) -> (Fx, String, PathBuf) {
        let f = fx();
        commit(&f.repo, "docs/now/b.md", PAGE, "now page");
        let id = task(&f.root, "Build b", touches);
        let s = start(
            &f.root,
            &id,
            "wt-a",
            "2026-10-13 10:00",
            &f.repo,
            &Place::Default,
        )
        .unwrap();
        commit(&s.worktree, "b/x.txt", "built\n", "build it");
        (f, id, s.worktree)
    }

    #[test]
    fn a_pass_is_refused_while_a_covered_page_is_unchanged_on_the_branch() {
        let (f, id, _) = started("b/");
        let e = pass(&f, &id, "").unwrap_err();
        assert!(e.contains("docs/now/b.md"), "{e}");
        assert!(e.contains("--now-unchanged"), "{e}");
        assert_eq!(stage(&f, &id), "building");
        // An uncommitted change doesn't count: the merge takes commits.
        let wt = PathBuf::from(queue::read_task(&f.root, &id).unwrap().worktree);
        fs::write(wt.join("docs/now/b.md"), format!("{PAGE}- more. (mu-2)\n")).unwrap();
        assert!(pass(&f, &id, "").is_err());
    }

    #[test]
    fn the_owners_refused_pass_points_at_the_track_too() {
        // The app's Pass sends no --now-unchanged; the owner's way out is the track.
        let f = fx();
        commit(&f.repo, "docs/now/b.md", PAGE, "now page");
        let id = task(&f.root, "Build b", "b/");
        ops::claim_task(&f.root, &id, "owner", "2026-10-13 10:00").unwrap();
        let opts = PassOpts {
            gate: "gate: green (built)".into(),
            summary: "A synthetic summary".into(),
            from: Some("building".into()),
            ..Default::default()
        };
        let e = ops::pass_task(&f.root, &id, "owner", "2026-10-13 11:00", &opts).unwrap_err();
        assert!(e.contains("docs/now/b.md"), "{e}");
        assert!(e.contains("or move it to Testing on the track"), "{e}");
        assert_eq!(stage(&f, &id), "building");
        // A builder has no track: its refusal names the flag only.
        let (f, id, _) = started("b/");
        let e = pass(&f, &id, "").unwrap_err();
        assert!(!e.contains("on the track"), "{e}");
    }

    #[test]
    fn a_pass_goes_through_when_the_branch_changes_the_page_and_logs_it() {
        let (f, id, wt) = started("b/");
        commit(
            &wt,
            "docs/now/b.md",
            &format!("{PAGE}- b grew. (mu-2)\n"),
            "page",
        );
        pass(&f, &id, "").unwrap();
        assert_eq!(stage(&f, &id), "testing");
        assert!(file(&f, &id).contains("wt-a: now pages updated: docs/now/b.md"));
    }

    #[test]
    fn a_pass_goes_through_with_now_unchanged_and_logs_why() {
        let (f, id, _) = started("b/x.txt, c/");
        assert!(pass(&f, &id, "x\ny").is_ok(), "folded to one line");
        assert_eq!(stage(&f, &id), "testing");
        assert!(file(&f, &id).contains("wt-a: now pages unchanged: docs/now/b.md (x y)"));
    }

    #[test]
    fn now_unchanged_is_capped_and_only_for_the_building_pass() {
        let (f, id, _) = started("b/");
        let long = "w".repeat(MAX_WHY + 1);
        assert!(pass(&f, &id, &long).unwrap_err().contains("longer than"));
        assert_eq!(stage(&f, &id), "building");
        // Out of any other stage it's refused, not ignored.
        let f = fx();
        let id = task(&f.root, "Anything", "");
        ops::claim_task(&f.root, &id, "wt-a", "2026-10-13 10:00").unwrap();
        pass(&f, &id, "").unwrap();
        let e = ops::pass_task(
            &f.root,
            &id,
            "qa",
            "2026-10-13 12:00",
            &PassOpts {
                from: Some("testing".into()),
                now_unchanged: "why".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("--now-unchanged is only"), "{e}");
        assert_eq!(stage(&f, &id), "testing");
    }

    #[test]
    fn the_pass_is_unchanged_without_touches_without_a_covering_page_or_without_docs_now() {
        // No page covers c/.
        let (f, id, _) = started("c/");
        pass(&f, &id, "").unwrap();
        assert!(!file(&f, &id).contains("now pages"));
        // No touches: claimed and built without a branch.
        let f = fx();
        commit(&f.repo, "docs/now/b.md", PAGE, "now page");
        let id = task(&f.root, "Anything", "");
        ops::claim_task(&f.root, &id, "wt-a", "2026-10-13 10:00").unwrap();
        pass(&f, &id, "").unwrap();
        assert!(!file(&f, &id).contains("now pages"));
        // No docs/now/ in the project at all.
        let f = fx();
        let id = task(&f.root, "Build b", "b/");
        let s = start(
            &f.root,
            &id,
            "wt-a",
            "2026-10-13 10:00",
            &f.repo,
            &Place::Default,
        )
        .unwrap();
        commit(&s.worktree, "b/x.txt", "built\n", "build it");
        pass(&f, &id, "").unwrap();
        assert_eq!(stage(&f, &id), "testing");
    }

    #[test]
    fn a_covered_task_without_a_branch_says_why_or_is_refused() {
        let f = fx();
        commit(&f.repo, "docs/now/b.md", PAGE, "now page");
        let id = task(&f.root, "Build b", "b/");
        ops::claim_task(&f.root, &id, "wt-a", "2026-10-13 10:00").unwrap();
        let e = pass(&f, &id, "").unwrap_err();
        assert!(e.contains("docs/now/b.md"), "{e}");
        assert!(e.contains("wasn't started on its own branch"), "{e}");
        pass(&f, &id, "updated on main in abc1234").unwrap();
        assert!(file(&f, &id)
            .contains("now pages unchanged: docs/now/b.md (updated on main in abc1234)"));
    }

    #[test]
    fn a_page_the_branch_adds_counts_as_updated() {
        let f = fx();
        let id = task(&f.root, "Build b", "b/");
        let s = start(
            &f.root,
            &id,
            "wt-a",
            "2026-10-13 10:00",
            &f.repo,
            &Place::Default,
        )
        .unwrap();
        commit(&s.worktree, "docs/now/b.md", PAGE, "first page");
        pass(&f, &id, "").unwrap();
        assert!(file(&f, &id).contains("now pages updated: docs/now/b.md"));
    }

    #[test]
    fn a_branch_that_updates_a_persian_named_page_passes_and_logs_it() {
        let f = fx();
        commit(&f.repo, "docs/now/تابلو.md", PAGE, "now page");
        let id = task(&f.root, "Build b", "b/");
        let s = start(
            &f.root,
            &id,
            "wt-a",
            "2026-10-13 10:00",
            &f.repo,
            &Place::Default,
        )
        .unwrap();
        commit(
            &s.worktree,
            "docs/now/تابلو.md",
            &format!("{PAGE}- b grew. (mu-2)\n"),
            "page",
        );
        pass(&f, &id, "").unwrap();
        assert_eq!(stage(&f, &id), "testing");
        assert!(
            file(&f, &id).contains("wt-a: now pages updated: docs/now/تابلو.md"),
            "{}",
            file(&f, &id)
        );
    }

    #[test]
    fn with_its_worktree_gone_the_pass_reads_the_branch_or_names_it_missing() {
        let (f, id, wt) = started("b/");
        commit(
            &wt,
            "docs/now/b.md",
            &format!("{PAGE}- b grew. (mu-2)\n"),
            "page",
        );
        let branch = queue::read_task(&f.root, &id).unwrap().branch;
        git_in(
            &f.repo,
            &["worktree", "remove", "--force", wt.to_str().unwrap()],
        );
        assert!(!wt.exists());
        assert_eq!(
            pass_check(&f.root, &queue::read_task(&f.root, &id).unwrap()),
            Pass::Covered {
                updated: sv(&["docs/now/b.md"]),
                unchanged: Vec::new(),
                unread: None,
            }
        );
        // A branch that isn't there: refused, naming it.
        let path = queue::find_task(&f.root, &id).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let line = format!("branch: {branch}\n");
        assert!(text.contains(&line), "{text}");
        fs::write(&path, text.replace(&line, "branch: task/gone\n")).unwrap();
        let e = pass(&f, &id, "").unwrap_err();
        assert!(e.contains("docs/now/b.md"), "{e}");
        assert!(e.contains("its branch task/gone isn't there"), "{e}");
        assert_eq!(stage(&f, &id), "building");
        // Put back, it passes and logs the update.
        fs::write(&path, text).unwrap();
        pass(&f, &id, "").unwrap();
        assert_eq!(stage(&f, &id), "testing");
        assert!(file(&f, &id).contains("wt-a: now pages updated: docs/now/b.md"));
    }
}
