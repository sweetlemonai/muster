//! `muster list` gives the app's answers (task 0032): the shared cases in
//! `app/src/search-cases.json` are written as task files and listed with the real command;
//! `search-cases.test.ts` runs the app's filter on the same cases.

mod common;

use muster_lib::queue;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const CASES: &str = include_str!("../../src/search-cases.json");

fn cases() -> Value {
    serde_json::from_str(CASES).expect("search-cases.json is JSON")
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

/// The fixture's tasks as a v2 queue: each file in its status folder, with its change time.
fn write_queue(root: &Path, prefix: &str, tasks: &[Value]) {
    for f in queue::STATUS_FOLDERS {
        std::fs::create_dir(root.join(f)).unwrap();
    }
    std::fs::write(
        root.join("README.md"),
        format!("---\nprefix: {prefix}\n---\n"),
    )
    .unwrap();
    for t in tasks {
        let blocked = !s(t, "waitingOn").is_empty();
        let status = queue::mirror_status(s(t, "stage"), blocked);
        let text = format!(
            "---\nid: {}\ntype: {}\npriority: {}\nstatus: {status}\nstage: {}\ncaptured: {}\nclaimed-by: {}\nheld-by: {}\nheartbeat:\nwaiting_on: {}\ndepends: {}\nest-time: {}\ntags: {}\n---\n\n# {}\n\n{}",
            s(t, "id"),
            s(t, "type"),
            s(t, "priority"),
            s(t, "stage"),
            s(t, "captured"),
            s(t, "claimedBy"),
            s(t, "heldBy"),
            s(t, "waitingOn"),
            strings(&t["depends"]).join(", "),
            s(t, "estTime"),
            strings(&t["tags"]).join(", "),
            s(t, "title"),
            s(t, "body"),
        );
        let path = root.join(status).join(format!("{}-t.md", s(t, "id")));
        std::fs::write(&path, text).unwrap();
        let secs = t["modified"].as_u64().expect("modified");
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(secs))
            .unwrap();
    }
}

/// A case's query as `muster list` options.
fn args(q: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(v) = q["search"].as_str() {
        out.extend(["--search".to_string(), v.to_string()]);
    }
    for (key, flag) in [
        ("stage", "--stage"),
        ("type", "--type"),
        ("tag", "--tag"),
        ("holder", "--holder"),
    ] {
        let vals = strings(&q[key]);
        if !vals.is_empty() {
            out.extend([flag.to_string(), vals.join(",")]);
        }
    }
    if q["blocked"].as_bool() == Some(true) {
        out.push("--blocked".into());
    }
    if let Some(v) = q["sort"].as_str() {
        out.extend(["--sort".to_string(), v.to_string()]);
    }
    out
}

/// A home folder of its own for `muster` (task 0048), so `list --json` never runs this Mac's
/// Claude Code or reads its projects: a `claude` where muster looks first, which lists no
/// sessions and records each call in `claude.calls`, and the `cc-hg` account's folder.
fn fake_home() -> common::TestDir {
    let h = common::tempdir().unwrap();
    let bin = h.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir(h.path().join(".claude-hg")).unwrap();
    let claude = bin.join("claude");
    std::fs::write(
        &claude,
        "#!/bin/sh\necho \"$@\" >> \"$0.calls\"\necho '[]'\n",
    )
    .unwrap();
    let mut perms = std::fs::metadata(&claude).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&claude, perms).unwrap();
    h
}

fn list(root: &Path, home: &Path, extra: &[String]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_muster"))
        .arg("list")
        .arg("--queue")
        .arg(root)
        .arg("--json")
        .args(extra)
        .current_dir(std::env::temp_dir())
        .env("HOME", home)
        .env_remove("MUSTER_ROOT")
        .env_remove("TASKS_ROOT")
        .env_remove("MUSTER_LANG")
        .env_remove("MUSTER_BASE")
        .output()
        .expect("muster runs");
    assert!(
        out.status.success(),
        "{extra:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("list --json is JSON")
}

#[test]
fn the_core_reads_each_fixture_task_as_the_app_is_told() {
    let c = cases();
    let tasks = c["tasks"].as_array().expect("tasks");
    let d = common::tempdir().unwrap();
    write_queue(d.path(), s(&c, "prefix"), tasks);
    for t in tasks {
        let got = queue::read_task(d.path(), s(t, "id")).expect("reads");
        let want = &t["parsed"];
        assert_eq!(got.search, s(want, "search"), "{} search", got.id);
        assert_eq!(got.kind, s(t, "type"), "{} type", got.id);
        // The app is given the stamped holder as written, and cuts it the same way (holder()).
        assert_eq!(got.held_by, s(t, "heldBy"), "{} held-by", got.id);
        assert_eq!(got.claimed_by, s(t, "claimedBy"), "{} claimed-by", got.id);
        assert_eq!(
            Value::Bool(got.blocked),
            want["blocked"],
            "{} blocked",
            got.id
        );
        assert_eq!(
            Value::Bool(got.waiting_for_owner),
            want["waitingForOwner"],
            "{} needs you",
            got.id
        );
        assert_eq!(
            Some(got.est_minutes),
            want["estMinutes"].as_u64(),
            "{}",
            got.id
        );
    }
    // Why each is blocked, as the window is told: read with the whole queue, since a block by
    // tasks is derived from the others (`block::derive`).
    let state = queue::read_state(
        &[("q".to_string(), d.path().to_path_buf())],
        &mut queue::Cache::default(),
    );
    for t in tasks {
        let got = state
            .tasks
            .iter()
            .find(|g| g.id == s(t, "id"))
            .expect("in the state");
        assert_eq!(
            got.blocked_kind,
            s(&t["parsed"], "blockedKind"),
            "{} blocked kind",
            got.id
        );
    }
}

#[test]
fn muster_list_gives_each_cases_tasks_in_order() {
    let c = cases();
    let d = common::tempdir().unwrap();
    write_queue(
        d.path(),
        s(&c, "prefix"),
        c["tasks"].as_array().expect("tasks"),
    );
    let home = fake_home();
    for case in c["cases"].as_array().expect("cases") {
        let opts = args(&case["query"]);
        let out = list(d.path(), home.path(), &opts);
        let ids: Vec<String> = out["tasks"]
            .as_array()
            .expect("tasks")
            .iter()
            .map(|t| s(t, "id").to_string())
            .collect();
        assert_eq!(ids, strings(&case["ids"]), "muster list {opts:?}");
        assert!(
            out["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t.get("search").is_none()),
            "--json leaves the search text out"
        );
    }
    // 0202 is held in Testing under a stamped session, so list --json asked Claude Code: the
    // fake one in the test's own home, never this Mac's.
    let calls =
        std::fs::read_to_string(home.path().join(".local/bin/claude.calls")).unwrap_or_default();
    assert!(calls.lines().count() > 0, "list --json ran the fake claude");
    assert!(calls.lines().all(|l| l == "agents --json --all"), "{calls}");
}

#[test]
fn fold_gives_the_shared_answers() {
    for pair in cases()["fold"].as_array().expect("fold") {
        let (from, to) = (
            pair[0].as_str().expect("from"),
            pair[1].as_str().expect("to"),
        );
        assert_eq!(muster_lib::search::fold(from), to, "{from}");
    }
}
