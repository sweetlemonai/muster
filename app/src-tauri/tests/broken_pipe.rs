//! A reader that leaves early (mu-170): `muster show pj-1 | head -1` ends quietly. The command
//! finishes, exits with its own status, and says nothing on stderr. Each run is the `muster` built
//! for these tests, in an empty home, from a folder outside any queue, with the queue variables
//! dropped; its stdout is a pipe this test closes.

mod common;

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How the test treats the pipe: read one line then close it, close it before muster writes, or
/// read it all (a reader that stays).
enum Reader {
    OneLine,
    Gone,
    All,
}

/// Runs muster on `root` with stdout a pipe treated per `reader`: (status, what was read, stderr).
fn muster(root: &Path, args: &[&str], reader: Reader) -> (i32, String, String) {
    let home = common::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_muster"))
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("muster runs");
    let stdout = child.stdout.take().unwrap();
    let mut first = String::new();
    match reader {
        Reader::OneLine => {
            BufReader::new(stdout).read_line(&mut first).unwrap();
        }
        Reader::Gone => drop(stdout),
        Reader::All => {
            BufReader::new(stdout).read_to_string(&mut first).unwrap();
        }
    }
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    let code = child.wait().unwrap().code().unwrap_or(-1);
    (code, first, err)
}

/// `proj/tasks`, a queue with the prefix `pj`.
fn queue() -> (common::TestDir, PathBuf) {
    let d = common::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap().join("proj/tasks");
    for f in muster_lib::queue::STATUS_FOLDERS {
        fs::create_dir_all(root.join(f)).unwrap();
    }
    fs::write(root.join("README.md"), "---\nprefix: pj\n---\nq").unwrap();
    (d, root)
}

fn task(root: &Path, title: &str, body: &str) -> muster_lib::ops::Created {
    muster_lib::ops::create_task(
        root,
        &muster_lib::ops::NewTask {
            title: title.into(),
            body: body.into(),
            ..Default::default()
        },
        "owner",
        "2026-10-14 10:00",
    )
    .unwrap()
}

/// Details far past any pipe's buffer (64 KB on macOS and Linux) and past what `create_task`
/// takes, so they're added to the task's file after it's made.
fn grow(root: &Path, c: &muster_lib::ops::Created) {
    let path = root.join(&c.path);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(
        &"A line of the task's details, long enough to fill the pipe quickly.\n".repeat(4_000),
    );
    fs::write(&path, text).unwrap();
}

#[test]
fn show_ends_quietly_when_its_reader_takes_one_line() {
    let (_d, root) = queue();
    let c = task(&root, "صف کارها: a big one", "## Details\n\nکارت تازه");
    grow(&root, &c);
    let (code, first, err) = muster(&root, &["show", &c.id], Reader::OneLine);
    assert_eq!(err, "", "nothing on stderr");
    assert_eq!(first, "---\n", "the reader got the file's first line");
    assert_eq!(code, 0, "the command's own status");
}

#[test]
fn list_ends_quietly_when_its_reader_takes_one_line() {
    let (_d, root) = queue();
    let long = "a task title long enough that a few hundred of them overflow the pipe ".repeat(2);
    let made = task(&root, long.trim(), "");
    let text = fs::read_to_string(root.join(&made.path)).unwrap();
    let folder = Path::new(&made.path).parent().unwrap();
    // Copies of the first task's file, far quicker than making 1000 tasks one by one.
    for n in 2..=1000 {
        let id = format!("{n:04}");
        fs::write(
            root.join(folder).join(format!("{id}-copy.md")),
            text.replace(&format!("id: {}", made.id), &format!("id: {id}")),
        )
        .unwrap();
    }
    let (code, whole, _) = muster(&root, &["list"], Reader::All);
    assert_eq!(code, 0);
    assert!(
        whole.len() > 128 * 1024,
        "the list overflows the pipe: {} bytes",
        whole.len()
    );
    let (code, first, err) = muster(&root, &["list"], Reader::OneLine);
    assert!(!first.is_empty(), "the reader got a line");
    assert_eq!(err, "", "nothing on stderr");
    assert_eq!(code, 0, "the command's own status");
}

#[test]
fn help_ends_quietly_on_a_closed_pipe() {
    let (_d, root) = queue();
    let (code, _, err) = muster(&root, &["help"], Reader::Gone);
    assert_eq!((code, err.as_str()), (0, ""));
}

#[test]
fn a_write_still_completes_when_nobody_reads_what_it_says() {
    let (_d, root) = queue();
    let id = task(&root, "Noted", "## Details\n\nx").id;
    let (code, _, err) = muster(&root, &["note", &id, "written anyway"], Reader::Gone);
    assert_eq!((code, err.as_str()), (0, ""));
    let body = muster_lib::queue::read_body(&root, &id).unwrap();
    assert!(
        body.contains("written anyway"),
        "the note is in the task: {body}"
    );
}

#[test]
fn a_refusal_keeps_its_status_and_its_stderr_line() {
    let (_d, root) = queue();
    let (code, _, err) = muster(&root, &["show", "pj-99"], Reader::Gone);
    assert_eq!(code, 1);
    assert!(err.starts_with("muster: "), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn cli_rs_has_no_bare_print() {
    let src = include_str!("../src/cli.rs");
    let bare: Vec<_> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            ["print!(", "println!("]
                .iter()
                .any(|m| l.match_indices(m).any(|(at, _)| !l[..at].ends_with('e')))
        })
        .map(|(n, l)| format!("{}: {l}", n + 1))
        .collect();
    assert!(
        bare.is_empty(),
        "every stdout write goes through out!/outln!:\n{}",
        bare.join("\n")
    );
}
