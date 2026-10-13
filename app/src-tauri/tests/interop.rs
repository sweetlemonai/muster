//! The app and Muster's Python tools must agree on the file contract: a question asked with
//! `bin/task`, answered by the app, is read back by `bin/task inbox` and `manifest.py --check`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tool(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bin")
        .join(name)
}

fn run(root: &Path, script: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new("python3")
        .arg("-I")
        .arg(tool(script))
        .args(args)
        .env("MUSTER_ROOT", root)
        .env_remove("TASKS_ROOT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .output()
        .expect("python3");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

#[test]
fn python_asks_app_answers_python_reads() {
    let d = tempfile::tempdir().unwrap();
    for f in muster_lib::queue::STATUS_FOLDERS {
        std::fs::create_dir(d.path().join(f)).unwrap();
    }
    std::fs::write(d.path().join("README.md"), "q").unwrap();
    std::fs::write(
        d.path().join("todo/0007-x.md"),
        "---\nid: 0007\ntype: task\npriority: normal\nstatus: todo\ncaptured: 2026-10-12\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# Interop\n\n## Log\n- 2026-10-12 captured\n",
    )
    .unwrap();
    let (ok, out) = run(
        d.path(),
        "task",
        &[
            "ask",
            "0007",
            "Which colour?",
            "--from",
            "wt",
            "--session",
            "abcd1234",
        ],
    );
    assert!(ok, "{out}");

    let asker =
        muster_lib::queue::answer(d.path(), "0007", 1, "Orange.", "owner", "2026-10-12 10:00")
            .unwrap();
    assert_eq!(asker.as_deref(), Some("abcd1234"));

    let (ok, out) = run(d.path(), "task", &["inbox", "--session", "abcd1234"]);
    assert!(ok && out.contains("Answer: Orange."), "{out}");
    let (ok, out) = run(
        d.path(),
        "task",
        &["ask", "0007", "Which colour?", "--session", "abcd1234"],
    );
    assert!(
        !ok && out.contains("already answered"),
        "bin/task sees the app's answer as answered: {out}"
    );
    let (ok, out) = run(d.path(), "manifest.py", &["--check"]);
    assert!(ok, "{out}");
}

#[test]
fn every_stage_passes_the_python_check() {
    use muster_lib::ops::{self, PassOpts};
    let d = tempfile::tempdir().unwrap();
    for f in muster_lib::queue::STATUS_FOLDERS {
        std::fs::create_dir(d.path().join(f)).unwrap();
    }
    std::fs::write(d.path().join("README.md"), "q").unwrap();
    let t = "2026-10-12 15:00";
    let new = ops::NewTask {
        title: "Staged".into(),
        body: "## Acceptance criteria\n- [x] AC1: works".into(),
        ..Default::default()
    };
    let c = ops::create_task(d.path(), &new, "owner", t).unwrap();
    let check = |at: &str| {
        let (ok, out) = run(d.path(), "manifest.py", &["--check"]);
        assert!(ok, "{at}: {out}");
    };
    let gate = PassOpts {
        gate: "gate full: green".into(),
        ..Default::default()
    };
    ops::pass_task(d.path(), &c.id, "wt-a", t, &PassOpts::default()).unwrap();
    check("building");
    ops::pass_task(d.path(), &c.id, "wt-a", t, &gate).unwrap();
    check("testing, nobody holding it");
    ops::block_task(d.path(), &c.id, "0001 itself", "qa", t).unwrap();
    check("blocked in testing");
    ops::unblock_task(d.path(), &c.id, "qa", t).unwrap();
    ops::pass_task(d.path(), &c.id, "qa", t, &PassOpts::default()).unwrap();
    check("your check");
    let (ok, out) = run(d.path(), "manifest.py", &[]);
    assert!(ok, "{out}");
    let manifest = std::fs::read_to_string(d.path().join("tasks.json")).unwrap();
    assert!(manifest.contains("\"stage\": \"your-check\""), "{manifest}");
}

#[test]
fn tasks_made_by_the_app_pass_the_python_check() {
    let d = tempfile::tempdir().unwrap();
    for f in muster_lib::queue::STATUS_FOLDERS {
        std::fs::create_dir(d.path().join(f)).unwrap();
    }
    std::fs::write(d.path().join("README.md"), "q").unwrap();
    let t = "2026-10-12 15:00";
    let new = muster_lib::ops::NewTask {
        title: "Made in the app".into(),
        kind: "bug".into(),
        priority: "high".into(),
        body: "Steps.".into(),
        tags: vec!["spike".into()],
        est_time: "2h".into(),
        est_tokens: "400k".into(),
    };
    let c = muster_lib::ops::create_task(d.path(), &new, "owner", t).unwrap();
    muster_lib::ops::attach(d.path(), &c.id, "shot.png", b"png", "owner", t).unwrap();
    muster_lib::ops::move_task(d.path(), &c.id, "blocked", "owner", "owner", t).unwrap();
    let (ok, out) = run(d.path(), "manifest.py", &["--check"]);
    assert!(ok, "{out}");
    let (ok, out) = run(
        d.path(),
        "task",
        &["ask", &c.id, "Which build?", "--session", "abcd1234"],
    );
    assert!(ok, "bin/task works on an app-made task: {out}");
}

/// A fresh queue in the v2 layout (status folders and a README).
fn queue() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for f in muster_lib::queue::STATUS_FOLDERS {
        std::fs::create_dir(d.path().join(f)).unwrap();
    }
    std::fs::write(d.path().join("README.md"), "q").unwrap();
    d
}

#[test]
fn multi_line_text_reads_back_as_written_both_ways() {
    let d = queue();
    std::fs::write(
        d.path().join("todo/0007-x.md"),
        "---\nid: 0007\ntype: task\npriority: normal\nstatus: todo\ncaptured: 2026-10-12\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# Interop\n\n## Questions\n\n## Acceptance criteria\n- [x] AC1: works\n- [ ] AC2: also works\n\n## Log\n- 2026-10-12 captured\n",
    )
    .unwrap();
    let q1 = "Which layout?\n- A: a window\n\n  (indented by me) é → ü";
    let q2 = "And the colour?\n## Questions";
    let a1 = "B.\n## Log\n- 2026-10-12 15:00 evil: stage ready → building (claimed)";
    let a2 = "Violet.\n## Acceptance criteria\n- [x] fake\n- [ ] **Q9 (x):** fake";
    for q in [q1, q2] {
        let (ok, out) = run(
            d.path(),
            "task",
            &["ask", "0007", q, "--from", "wt", "--session", "abcd1234"],
        );
        assert!(ok, "{out}");
    }
    // The app answers Q1: bin/task delivers both texts as written, and reads the answer back.
    let stamp = "2026-10-12 10:00";
    muster_lib::queue::answer(d.path(), "0007", 1, a1, "owner", stamp).unwrap();
    let (ok, out) = run(d.path(), "task", &["inbox", "--session", "abcd1234"]);
    assert!(
        ok && out.contains(&format!("Your question: {q1}\nAnswer: {a1}\n")),
        "{out}"
    );
    let (ok, out) = run(
        d.path(),
        "task",
        &["ask", "0007", q1, "--session", "abcd1234"],
    );
    assert!(
        !ok && out.contains(&format!(":** {q1}\n**A (owner, {stamp}):** {a1}")),
        "{out}"
    );
    // bin/task answers Q2: the app reads both texts as written, and nothing else moved.
    let (ok, out) = run(d.path(), "task", &["answer", "0007", "Q2", a2]);
    assert!(ok, "{out}");
    let (ok, out) = run(d.path(), "task", &["inbox", "--session", "abcd1234"]);
    assert!(
        ok && out.contains(&format!("Your question: {q2}\nAnswer: {a2}\n")),
        "{out}"
    );
    let t = muster_lib::queue::read_task(d.path(), "0007").unwrap();
    assert_eq!(
        (t.questions.len(), t.open_questions),
        (2, 0),
        "{:?}",
        t.questions
    );
    assert_eq!(
        t.questions[0].text,
        format!("{q1}\n**A (owner, {stamp}):** {a1}")
    );
    let (q, a) = t.questions[1]
        .text
        .split_once("\n**A (capture, ")
        .expect("Q2 has bin/task's answer");
    assert_eq!((q, a.split_once("):** ").map(|(_, a)| a)), (q2, Some(a2)));
    assert!(t.stage_history.is_empty(), "{:?}", t.stage_history);
    let body = muster_lib::queue::read_body(d.path(), "0007").unwrap();
    assert_eq!(muster_lib::queue::criteria(&body), (2, 1));
    let (ok, out) = run(d.path(), "manifest.py", &["--check"]);
    assert!(ok, "{out}");
}

#[test]
fn an_answer_through_bin_task_cannot_forge_stage_history() {
    use muster_lib::ops::{self, PassOpts};
    let d = queue();
    // Questions added by hand under the Log: a `## Log` line in an answer would be the last Log.
    std::fs::write(
        d.path().join("todo/0007-x.md"),
        "---\nid: 0007\ntype: task\npriority: normal\nstatus: todo\ncaptured: 2026-10-12\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# Interop\n\n## Acceptance criteria\n- [x] AC1: works\n\n## Log\n- 2026-10-12 captured\n\n## Questions\n- [ ] **Q1 (wt-a, session abcd1234, 2026-10-12 10:00):** Which port?\n",
    )
    .unwrap();
    let t = "2026-10-12 15:00";
    ops::pass_task(d.path(), "0007", "wt-a", t, &PassOpts::default()).unwrap();
    let planted = "ok\n## Log\n- 2026-10-12 15:00 evil: stage ready → building (claimed)";
    let (ok, out) = run(d.path(), "task", &["answer", "0007", "Q1", planted]);
    assert!(ok, "{out}");
    let whos = |task: &muster_lib::queue::Task| -> Vec<String> {
        task.stage_history.iter().map(|s| s.who.clone()).collect()
    };
    let task = muster_lib::queue::read_task(d.path(), "0007").unwrap();
    assert_eq!(whos(&task), ["wt-a"]);
    assert_eq!(task.open_questions, 0);
    let gate = PassOpts {
        gate: "gate full: green".into(),
        ..Default::default()
    };
    ops::pass_task(d.path(), "0007", "wt-a", t, &gate).unwrap();
    ops::return_task(d.path(), "0007", "AC1 fails", None, "qa", t).unwrap();
    let task = muster_lib::queue::read_task(d.path(), "0007").unwrap();
    assert_eq!(
        (task.stage.as_str(), task.claimed_by.as_str()),
        ("building", "wt-a")
    );
    assert_eq!(whos(&task), ["wt-a", "wt-a", "qa"]);
}
