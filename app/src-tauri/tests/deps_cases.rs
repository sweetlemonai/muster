//! What the tasks a task depends on block (task 0042): the shared cases in
//! `app/src/deps-cases.json` are written as task files and read with the real `muster list`,
//! `check`, `claim` and `next`; `deps-cases.test.ts` gives the app the same tasks, with what the
//! core derives, and checks the board, the Graph and the Human board against the same answers.

use muster_lib::queue;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const CASES: &str = include_str!("../../src/deps-cases.json");
const STAMP: &str = "2026-10-13 09:00";

fn cases() -> Value {
    serde_json::from_str(CASES).expect("deps-cases.json is JSON")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str()
        .unwrap_or_else(|| panic!("{k} is a string in {v}"))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// The fixture's tasks as a v2 queue: each file in its status folder, its log as written.
fn write_queue(root: &Path) {
    for f in queue::STATUS_FOLDERS {
        std::fs::create_dir(root.join(f)).unwrap();
    }
    std::fs::write(root.join("README.md"), "q").unwrap();
    for t in cases()["tasks"].as_array().expect("tasks") {
        let blocked = !s(t, "waitingOn").is_empty();
        let status = queue::mirror_status(s(t, "stage"), blocked);
        let depends = strings(&t["depends"]).join(", ");
        let log: String = strings(&t["log"])
            .iter()
            .map(|l| format!("- {l}\n"))
            .collect();
        let text = format!(
            "---\nid: {}\ntype: code\npriority: normal\nstatus: {status}\nstage: {}\nfor: {}\ncaptured: 2026-10-12\nclaimed-by: {}\nheld-by: {}\nheartbeat: {STAMP}\nwaiting_on: {}\ndepends: [{depends}]\n---\n\n# {}\n\n## Log\n- 2026-10-12 09:00 capture: created\n{log}",
            s(t, "id"),
            s(t, "stage"),
            s(t, "for"),
            s(t, "claimedBy"),
            s(t, "heldBy"),
            s(t, "waitingOn"),
            s(t, "title"),
        );
        std::fs::write(root.join(status).join(format!("{}-t.md", s(t, "id"))), text).unwrap();
    }
}

fn muster(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_muster"))
        .args(args)
        .arg("--queue")
        .arg(root)
        .current_dir(std::env::temp_dir())
        .env_remove("MUSTER_ROOT")
        .env_remove("TASKS_ROOT")
        .env_remove("MUSTER_AS")
        .output()
        .expect("muster runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn muster_list_says_what_each_task_is_blocked_by() {
    let c = cases();
    let d = tempfile::tempdir().unwrap();
    write_queue(d.path());
    let out = muster(d.path(), &["list", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).expect("list --json is JSON");
    let tasks = json["tasks"].as_array().expect("tasks");
    let text = stdout(&muster(d.path(), &["list"]));
    for (id, want) in c["want"].as_object().expect("want") {
        let t = tasks
            .iter()
            .find(|t| t["id"] == id.as_str())
            .unwrap_or_else(|| panic!("{id} is listed"));
        assert_eq!(t["blockedBy"], want["blockedBy"], "{id} blocked by");
        let by = want["blockedBy"].as_array().unwrap();
        let kind = if t["blocked"] == true || !by.is_empty() {
            "tasks"
        } else {
            ""
        };
        assert_eq!(t["blockedKind"], kind, "{id} kind");
        // The Graph's ready: in Ready, nobody on it, not blocked by anything.
        let ready = t["stage"] == "ready"
            && t["claimedBy"] == ""
            && t["blocked"] == false
            && t["blockedKind"] == "";
        assert_eq!(Value::Bool(ready), want["ready"], "{id} ready");
        // The text list says it in the card's short words, ids only.
        let line = text
            .lines()
            .find(|l| l.contains(&format!("{id} ")))
            .unwrap_or_else(|| panic!("{id} in {text}"));
        let ids: Vec<&str> = by.iter().map(|b| b["id"].as_str().unwrap()).collect();
        let short = format!("[Blocked by tasks: {}]", ids.join(", "));
        assert_eq!(line.contains(&short), !ids.is_empty(), "{line}");
    }
}

#[test]
fn the_core_says_each_block_in_the_same_words_as_the_app() {
    let c = cases();
    let d = tempfile::tempdir().unwrap();
    write_queue(d.path());
    let state = queue::read_state_at(
        &[("q".into(), d.path().to_path_buf())],
        &mut queue::Cache::default(),
        STAMP,
    );
    for (id, want) in c["sentences"].as_object().expect("sentences") {
        let t = state.tasks.iter().find(|t| t.id == *id).unwrap();
        assert_eq!(
            muster_lib::block::sentence_of(t, false),
            want.as_str().unwrap(),
            "{id}"
        );
    }
}

#[test]
fn claim_and_next_take_exactly_what_is_free() {
    let c = cases();
    let claims = strings(&c["agentClaims"]);
    for t in c["tasks"].as_array().unwrap() {
        let id = s(t, "id");
        if s(t, "stage") != "ready" {
            continue;
        }
        let d = tempfile::tempdir().unwrap();
        write_queue(d.path());
        let out = muster(d.path(), &["claim", id, "--as", "wt-z"]);
        assert_eq!(
            out.status.success(),
            claims.contains(&id.to_string()),
            "claim {id}: {}{}",
            stdout(&out),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let d = tempfile::tempdir().unwrap();
    write_queue(d.path());
    let out = muster(d.path(), &["next", "--as", "wt-z"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(
        stdout(&out).starts_with(&format!("claimed {}", s(&c, "next")))
            || stdout(&out).contains(&format!("\n{}: Building", s(&c, "next"))),
        "{}",
        stdout(&out)
    );
}

#[test]
fn muster_check_reads_old_blocks_by_the_same_rule() {
    let c = cases();
    let d = tempfile::tempdir().unwrap();
    write_queue(d.path());
    let text = stdout(&muster(d.path(), &["check"]));
    for want in strings(&c["check"]) {
        assert!(
            text.lines().any(|l| l.ends_with(&want)),
            "{want}\nin:\n{text}"
        );
    }
}
