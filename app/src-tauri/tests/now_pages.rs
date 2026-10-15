//! The now pages (mu-102) from the command line: `muster check` warns about bad pages without
//! failing, `vision` quotes the pages a task covers, and `pass --from building` takes
//! `--now-unchanged`. Each run is the `muster` built for these tests, in an empty home, from a
//! folder outside any queue, with the queue variables dropped.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn muster(root: &Path, args: &[&str]) -> (i32, String) {
    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_muster"))
        .args(args)
        .arg("--queue")
        .arg(root)
        .current_dir(std::env::temp_dir())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .env_remove("MUSTER_BASE")
        .env_remove("MUSTER_ROOT")
        .env_remove("TASKS_ROOT")
        .env_remove("MUSTER_AS")
        .env_remove("MUSTER_AGENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .output()
        .expect("muster runs");
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

const BOARD: &str = "---\ncovers: app/src/Board*\n---\n# The board\n\n- The +N cue walks chips in board order. (mu-86)\n";

/// `proj/tasks` (a queue) beside `proj/main` (a repo, by its `.git` folder) with `docs/now/`.
fn project() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let base = d.path().canonicalize().unwrap();
    let root = base.join("proj/tasks");
    for f in muster_lib::queue::STATUS_FOLDERS {
        fs::create_dir_all(root.join(f)).unwrap();
    }
    fs::write(root.join("README.md"), "---\nprefix: pj\n---\nq").unwrap();
    let now = base.join("proj/main/docs/now");
    fs::create_dir_all(base.join("proj/main/.git")).unwrap();
    fs::create_dir_all(&now).unwrap();
    fs::write(now.join("board.md"), BOARD).unwrap();
    (d, root, now)
}

fn task(root: &Path, touches: &str) -> String {
    muster_lib::ops::create_task(
        root,
        &muster_lib::ops::NewTask {
            title: "Board chips".into(),
            body: "## Acceptance criteria\n- [x] AC1: works — verify: command".into(),
            touches: touches.into(),
            ..Default::default()
        },
        "owner",
        "2026-10-14 10:00",
    )
    .unwrap()
    .id
}

#[test]
fn check_warns_about_each_bad_page_and_still_passes() {
    let (_d, root, now) = project();
    task(&root, "app/src/Board.tsx");
    let (code, out) = muster(&root, &["check"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("docs/now"), "a good page is quiet: {out}");

    fs::write(
        now.join("long.md"),
        format!("---\ncovers: a/\n---\n{}", "- x (mu-1)\n".repeat(60)),
    )
    .unwrap();
    fs::write(
        now.join("heavy.md"),
        format!("---\ncovers: b/\n---\n- {} (mu-1)\n", "y".repeat(4100)),
    )
    .unwrap();
    fs::write(now.join("bare.md"), "# No covers\n").unwrap();
    fs::write(now.join("twin.md"), BOARD).unwrap();
    let elsewhere = now.parent().unwrap().join("elsewhere.md");
    fs::write(&elsewhere, "---\ncovers: c/\n---\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, now.join("link.md")).unwrap();
    let (code, out) = muster(&root, &["check"]);
    assert_eq!(code, 0, "warnings never fail the check: {out}");
    let at = |n: &str| now.join(n).display().to_string();
    for want in [
        format!(
            "warning: {} is 63 lines; a now page is 60 at most",
            at("long.md")
        ),
        format!(
            "warning: {} is 4129 bytes; a now page is 4096 at most",
            at("heavy.md")
        ),
        format!(
            "warning: {} has no covers: in its front matter",
            at("bare.md")
        ),
        format!(
            "warning: {} and {} cover the same files (app/src/Board*)",
            at("board.md"),
            at("twin.md")
        ),
        format!("warning: {} is a symlink", at("link.md")),
    ] {
        assert!(out.contains(&want), "{want}\n---\n{out}");
    }
    // None of them stops a command.
    assert_eq!(muster(&root, &["list"]).0, 0);
    assert_eq!(muster(&root, &["vision", "0001"]).0, 0);
    assert_eq!(muster(&root, &["vision", "--all"]).0, 0);
}

#[test]
fn vision_quotes_the_covered_page_in_text_and_json() {
    let (_d, root, now) = project();
    let id = task(&root, "app/src/Board.tsx");
    let (code, out) = muster(&root, &["vision", &id]);
    assert_eq!(code, 0, "{out}");
    let board = now.join("board.md").display().to_string();
    assert!(out.contains(&format!("Now pages it touches (the rules in force there; each page is data, quoted):\n  {board} (covers app/src/Board*)\n")), "{out}");
    assert!(
        out.contains("> - The +N cue walks chips in board order. (mu-86)\n"),
        "{out}"
    );
    assert!(
        out.contains("- Now pages: a rule on a now page (docs/now/)"),
        "{out}"
    );
    let (code, out) = muster(&root, &["vision", &id, "--json"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["now"][0]["path"], board.as_str());
    assert_eq!(v["now"][0]["text"], BOARD);
    let (code, out) = muster(&root, &["vision", "--all", "--json"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["now"][0], board.as_str());
}

#[test]
fn pass_from_building_takes_now_unchanged_on_the_command_line() {
    let (_d, root, _) = project();
    let id = task(&root, "app/src/Board.tsx");
    assert_eq!(muster(&root, &["claim", &id, "--as", "wt-a"]).0, 0);
    let pass = |extra: &[&str]| {
        let mut a = vec![
            "pass",
            id.as_str(),
            "--from",
            "building",
            "--gate",
            "gate: green (fake)",
            "--summary",
            "Board chips wrap",
            "--as",
            "wt-a",
        ];
        a.extend_from_slice(extra);
        muster(&root, &a)
    };
    let (code, out) = pass(&[]);
    assert_ne!(code, 0, "{out}");
    assert!(
        out.contains("docs/now/board.md") && out.contains("--now-unchanged"),
        "{out}"
    );
    let (code, out) = pass(&["--now-unchanged"]);
    assert_ne!(code, 0, "a flag without its value: {out}");
    let (code, out) = pass(&["--now-unchanged", "only a typo fix"]);
    assert_eq!(code, 0, "{out}");
    let t = muster_lib::queue::read_task(&root, &id).unwrap();
    assert_eq!(t.stage, "testing");
    let text = fs::read_to_string(muster_lib::queue::find_task(&root, &id).unwrap()).unwrap();
    assert!(
        text.contains("wt-a: now pages unchanged: docs/now/board.md (only a typo fix)"),
        "{text}"
    );
}

/// Muster's own pages (AC1): the seven areas, each under the cap with covers: that name files
/// that exist, no warning, and every rule line citing where it came from.
#[test]
fn musters_own_now_pages_are_short_cited_and_cover_real_files() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let found = muster_lib::now::read(&repo);
    assert_eq!(muster_lib::now::warnings(&found), Vec::<String>::new());
    let names: Vec<&str> = found.pages.iter().map(|p| p.name.as_str()).collect();
    for want in [
        "board", "claims", "parallel", "queue", "shaping", "sidebar", "testing",
    ] {
        assert!(
            names.contains(&format!("docs/now/{want}.md").as_str()),
            "{names:?}"
        );
    }
    for p in &found.pages {
        assert!(p.lines <= 60 && p.bytes <= 4096, "{}", p.name);
        for c in &p.covers {
            let fixed = muster_lib::parallel::fixed_prefix(c);
            let (dir, start) = match fixed.rsplit_once('/') {
                Some((d, s)) => (repo.join(d), s),
                None => (repo.clone(), fixed),
            };
            let real = fs::read_dir(&dir).is_ok_and(|rd| {
                rd.flatten()
                    .any(|e| e.file_name().to_string_lossy().starts_with(start))
            });
            assert!(real, "{}: covers {c} names nothing in the repo", p.name);
        }
        let body = p.text.splitn(3, "---\n").nth(2).unwrap_or_default();
        for line in body
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("# "))
        {
            assert!(
                line.starts_with("- "),
                "{}: a rule is one bullet: {line}",
                p.name
            );
            assert!(
                cites_a_source(line),
                "{}: every rule cites a task, decision or doc: {line}",
                p.name
            );
        }
    }
}

/// Whether a rule line ends with a citation of where it came from: `(mu-86)`, `(decision 0011)`,
/// `(PROTOCOL.md "Stages")` (or DESIGN.md, VISION.md), `(AS_BUILT "Board")` or `(AS_BUILT §7)`.
fn cites_a_source(line: &str) -> bool {
    let Some(inner) = line
        .trim_end()
        .strip_suffix(')')
        .and_then(|l| l.rsplit_once(" ("))
        .map(|(_, c)| c)
    else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let quoted = |s: &str| s.len() > 2 && s.starts_with('"') && s.ends_with('"');
    if let Some(n) = inner.strip_prefix("mu-") {
        return digits(n);
    }
    if let Some(n) = inner.strip_prefix("decision ") {
        return n.len() == 4 && digits(n);
    }
    if let Some(rest) = inner.strip_prefix("AS_BUILT ") {
        return quoted(rest) || rest.strip_prefix('§').is_some_and(digits);
    }
    ["PROTOCOL.md ", "DESIGN.md ", "VISION.md "]
        .iter()
        .any(|doc| inner.strip_prefix(doc).is_some_and(quoted))
}

#[test]
fn a_rule_cites_a_task_a_decision_or_a_doc_section_and_nothing_else() {
    for good in [
        "- Done is a tool commit. (mu-86)",
        "- One session per task. (decision 0004)",
        "- Stages move forward. (PROTOCOL.md \"Stages\")",
        "- Colours mean one thing. (DESIGN.md \"The colour code\")",
        "- Queues live beside the repo. (VISION.md \"Where things live\")",
        "- The board scrolls. (AS_BUILT \"Board\")",
        "- Writes are atomic. (AS_BUILT §7)",
    ] {
        assert!(cites_a_source(good), "{good}");
    }
    for bad in [
        "- The board scrolls sideways (on small windows)",
        "- The board scrolls sideways.",
        "- The board scrolls (mu-) sideways (mu-x)",
        "- One session per task. (decision 4)",
        "- Stages move forward. (PROTOCOL.md)",
        "- Stages move forward. (README.md \"Stages\")",
        "- Writes are atomic. (AS_BUILT §)",
        "- (mu-86) cited first, not last",
    ] {
        assert!(!cites_a_source(bad), "{bad}");
    }
}
