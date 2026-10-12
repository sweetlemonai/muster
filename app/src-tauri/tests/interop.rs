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
