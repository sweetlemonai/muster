//! Task keys on the command line (task 0033), with the real `muster` on projects of its own: a
//! home and a playground in a temp folder, so the projects Muster knows are only these.
//! `muster show mu-12`, `12`, `0012` and `MU-12` find the same task; `br-7` finds braindump's
//! from Muster's folder; a prefix is set, and refused when it's bad or taken.

use std::path::{Path, PathBuf};
use std::process::Command;

struct World {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    base: PathBuf,
}

impl World {
    /// `muster` and `braindump` under the playground, each with a queue beside its repo folder.
    fn new() -> World {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let base = tmp.path().join("playground");
        std::fs::create_dir_all(&home).unwrap();
        for p in ["muster", "braindump"] {
            let q = base.join(p).join("tasks");
            for f in muster_lib::queue::STATUS_FOLDERS {
                std::fs::create_dir_all(q.join(f)).unwrap();
            }
            std::fs::write(q.join("README.md"), format!("# {p} — task queue\n")).unwrap();
            std::fs::create_dir_all(base.join(p).join("main")).unwrap();
        }
        World {
            _tmp: tmp,
            home,
            base,
        }
    }

    fn queue(&self, project: &str) -> PathBuf {
        self.base.join(project).join("tasks")
    }

    /// `muster <args>` from `cwd`; (exit code, stdout and stderr).
    fn run_in(&self, cwd: &Path, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_muster"))
            .args(args)
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("MUSTER_BASE", &self.base)
            .env("MUSTER_AS", "wt-a")
            .env("MUSTER_TOOLS", "/opt/muster/bin")
            .env_remove("MUSTER_ROOT")
            .env_remove("TASKS_ROOT")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .output()
            .expect("muster runs");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (out.status.code().unwrap_or(-1), text)
    }

    /// `muster <args>` from Muster's repo folder.
    fn run(&self, args: &[&str]) -> (i32, String) {
        self.run_in(&self.base.join("muster/main"), args)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (code, out) = self.run(args);
        assert_eq!(code, 0, "muster {args:?}: {out}");
        out
    }

    fn file(&self, project: &str, id: &str) -> String {
        let q = self.queue(project);
        let hit = muster_lib::queue::find_task(&q, id).unwrap();
        std::fs::read_to_string(hit).unwrap()
    }
}

#[test]
fn every_form_of_a_key_finds_the_task() {
    let w = World::new();
    let out = w.ok(&["new", "Board flickers"]);
    assert!(out.starts_with("mu-1 "), "new names the key: {out}");
    w.ok(&["new", "Second"]);
    for typed in ["mu-1", "MU-1", "Mu-0001", "1", "0001"] {
        let out = w.ok(&["show", typed]);
        assert!(out.contains("# Board flickers"), "show {typed}: {out}");
        assert!(
            out.contains("key: mu-1"),
            "show {typed} says the key: {out}"
        );
    }
    let (code, out) = w.run(&["show", "mu-x"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("mu-12, 12 or 0012"), "{out}");
    let json = w.ok(&["show", "mu-2", "--json"]);
    assert!(json.contains("\"id\": \"0002\""), "{json}");
    assert!(json.contains("\"key\": \"mu-2\""), "{json}");
    // The file keeps its four digits.
    assert!(w.file("muster", "0001").starts_with("---\nid: 0001\n"));
    // And every command takes one.
    let out = w.ok(&["note", "MU-1", "Checked"]);
    assert!(out.starts_with("mu-1: noted"), "{out}");
    assert!(w.file("muster", "0001").contains("wt-a: Checked"));
}

#[test]
fn list_shows_keys_and_finds_them() {
    let w = World::new();
    w.ok(&["new", "Board flickers"]);
    w.ok(&["new", "Second"]);
    let out = w.ok(&["list"]);
    assert!(out.contains("  mu-1 "), "{out}");
    assert!(out.contains("  mu-2 "), "{out}");
    assert!(!out.contains("0001"), "{out}");
    let out = w.ok(&["list", "--search", "MU-2"]);
    assert!(out.contains("Second") && !out.contains("Board"), "{out}");
    let out = w.ok(&["list", "--search", "br-2"]);
    assert!(
        !out.contains("Second"),
        "another project's key finds nothing here: {out}"
    );
    let json = w.ok(&["list", "--json"]);
    assert!(json.contains("\"key\": \"mu-1\""), "{json}");
}

#[test]
fn another_projects_key_finds_its_queue() {
    let w = World::new();
    w.ok(&["new", "Muster's own"]);
    let (code, out) = w.run_in(&w.queue("braindump"), &["new", "A braindump task"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("br-1 "), "{out}");
    // From Muster's folder.
    let out = w.ok(&["show", "br-1"]);
    assert!(out.contains("# A braindump task"), "{out}");
    let out = w.ok(&["note", "br-1", "From muster's folder"]);
    assert!(out.starts_with("br-1: noted"), "{out}");
    assert!(w.file("braindump", "0001").contains("From muster's folder"));
    assert!(!w.file("muster", "0001").contains("From muster's folder"));
    // Even a claim, which the environment can't steer (0002).
    let out = w.ok(&["claim", "br-1"]);
    assert!(out.contains("claimed br-1 in braindump"), "{out}");
    // A plain number is still the folder's own queue.
    let out = w.ok(&["show", "1"]);
    assert!(out.contains("# Muster's own"), "{out}");
    // --queue that agrees is fine; one that doesn't is refused, saying why.
    let out = w.ok(&["show", "br-1", "--queue", "braindump"]);
    assert!(out.contains("# A braindump task"), "{out}");
    let (code, out) = w.run(&["show", "br-1", "--queue", "muster"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("br-1 is braindump's task, but --queue muster is mu"),
        "{out}"
    );
    let (code, out) = w.run(&["show", "zz-1"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("no project has the prefix zz"), "{out}");
}

#[test]
fn depends_and_parents_take_keys_of_the_same_project() {
    let w = World::new();
    w.ok(&["new", "First"]);
    w.ok(&["new", "Second"]);
    w.ok(&["new", "Ship it", "--level", "feature"]);
    w.ok(&["set", "mu-2", "depends", "mu-1"]);
    let f = w.file("muster", "0002");
    assert!(f.contains("\ndepends: [0001]\n"), "{f}");
    w.ok(&["block", "mu-3", "--on", "mu-1,MU-2"]);
    let f = w.file("muster", "0003");
    assert!(f.contains("\ndepends: [0001, 0002]\n"), "{f}");
    w.ok(&["set", "mu-3", "depends", ""]);
    let out = w.ok(&["new", "Under it", "--parent", "mu-3"]);
    assert!(out.starts_with("mu-4 "), "{out}");
    let f = w.file("muster", "0004");
    assert!(f.contains("\nparent: 0003\n"), "{f}");
    w.ok(&["set", "4", "parent", "MU-3"]);
    let (code, out) = w.run(&["set", "mu-2", "depends", "br-1"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("br-1 isn't a task of muster (mu)"), "{out}");
}

#[test]
fn a_prefix_is_set_and_refused_when_bad_or_taken() {
    let w = World::new();
    let out = w.ok(&["projects"]);
    assert!(out.contains("braindump") && out.contains("  br  "), "{out}");
    assert!(out.contains("  mu  "), "{out}");
    let (code, out) = w.run(&["projects", "set", "braindump", "prefix", "mu"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("muster has mu"), "{out}");
    let (code, out) = w.run(&["projects", "set", "braindump", "prefix", "B1"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("2 to 4 lowercase letters"), "{out}");
    let (code, out) = w.run(&["projects", "set", "braindump", "prefix", "brnde"]);
    assert_eq!(code, 1, "{out}");
    let out = w.ok(&["projects", "set", "braindump", "prefix", "bd"]);
    assert!(out.contains("braindump: prefix set to bd"), "{out}");
    let readme = std::fs::read_to_string(w.queue("braindump").join("README.md")).unwrap();
    assert_eq!(readme, "---\nprefix: bd\n---\n\n# braindump — task queue\n");
    let (code, out) = w.run_in(&w.queue("braindump"), &["new", "Renamed"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("bd-1 "), "{out}");
    assert!(w.ok(&["show", "bd-1"]).contains("# Renamed"));
    let (code, out) = w.run(&["show", "br-1"]);
    assert_eq!(code, 1, "the old prefix names nothing now: {out}");
    // Its own prefix again is fine.
    w.ok(&["projects", "set", "braindump", "prefix", "bd"]);
    let (code, out) = w.run(&["projects", "set", "nobody", "prefix", "nb"]);
    assert_eq!(code, 1, "{out}");
}

#[test]
fn init_stores_a_prefix_and_refuses_a_taken_one() {
    let w = World::new();
    for p in ["mumble", "mustang", "germ"] {
        std::fs::create_dir_all(w.base.join(p)).unwrap();
    }
    // Muster has mu (by default): a new project starting the same takes 3 letters, stored.
    let out = w.ok(&["init", w.base.join("mumble").to_str().unwrap()]);
    assert!(out.contains("prefix mum"), "{out}");
    let readme = std::fs::read_to_string(w.queue("mumble").join("README.md")).unwrap();
    assert!(
        readme.starts_with("---\nprefix: mum\n---\n\n# mumble — task queue"),
        "{readme}"
    );
    // Muster keeps mu: mumble's is stored.
    let out = w.ok(&["projects"]);
    assert!(out.contains("  mu  ") && out.contains("  mum  "), "{out}");
    let (code, out) = w.run(&[
        "init",
        w.base.join("mustang").to_str().unwrap(),
        "--prefix",
        "mu",
    ]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("muster has mu"), "{out}");
    assert!(!w.queue("mustang").exists(), "nothing made when refused");
    let (code, out) = w.run(&[
        "init",
        w.base.join("mustang").to_str().unwrap(),
        "--prefix",
        "M!",
    ]);
    assert_eq!(code, 1, "{out}");
    let out = w.ok(&[
        "init",
        w.base.join("germ").to_str().unwrap(),
        "--prefix",
        "gm",
    ]);
    assert!(out.contains("prefix gm"), "{out}");
    assert_eq!(
        muster_lib::key::stored_prefix(&w.queue("germ")).as_deref(),
        Some("gm")
    );
}
