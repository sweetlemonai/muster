//! Queue text never reaches the terminal as control bytes (mu-221). A task's title, body, note and
//! log line hold ESC, CSI and OSC sequences, a lone CR, BEL, BS, DEL and a C1 CSI; every command's
//! output for a person shows each as a visible escape (`\e`, `\r`, `\x07`, `\x9b`…), and `--json`
//! stays as it was, byte for byte. Each run is the `muster` built for these tests, in an empty home,
//! from a folder outside any queue, with the queue variables dropped.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What the task's text holds: a screen clear, an OSC 8 link, an OSC 52 clipboard write, a lone
/// CR, BEL, BS, DEL, a C1 CSI and a NUL.
const PAYLOAD: &str = "\x1b[2J<\x1b]8;;https://evil.example\x07link\x1b]8;;\x07>\x1b]52;c;ZXZpbA==\x07lone\rCR\x07bs\x08del\x7f\u{9b}31mcsi\0nul";

/// The payload as a person sees it.
const SHOWN: &str = r"\e[2J<\e]8;;https://evil.example\x07link\e]8;;\x07>\e]52;c;ZXZpbA==\x07lone\rCR\x07bs\x08del\x7f\x9b31mcsi\x00nul";

/// Runs muster on `root` with `args`: (status, stdout, stderr), each as raw bytes.
fn muster(root: &Path, args: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let home = common::tempdir().unwrap();
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
        .env_remove("MUSTER_LANG")
        .env_remove("MUSTER_AS")
        .env_remove("MUSTER_AGENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("TERM_PROGRAM")
        .stdin(Stdio::null())
        .output()
        .expect("muster runs");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

/// `proj/tasks`, a queue with the prefix `pj`, holding one Ready task (pj-1) whose title, body,
/// note and log line carry `PAYLOAD`, written into its file as a teammate or an agent could; and
/// the task's file.
fn queue() -> (common::TestDir, PathBuf, PathBuf) {
    let d = common::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap().join("proj/tasks");
    for f in muster_lib::queue::STATUS_FOLDERS {
        fs::create_dir_all(root.join(f)).unwrap();
    }
    fs::write(root.join("README.md"), "---\nprefix: pj\n---\nq").unwrap();
    let c = muster_lib::ops::create_task(
        &root,
        &muster_lib::ops::NewTask {
            title: "Evil TITLE".into(),
            body: "## Details\n\nEvil BODY\n\n## Acceptance criteria\n- [ ] AC1: works — verify: command".into(),
            ..Default::default()
        },
        "owner",
        "2026-10-14 10:00",
    )
    .unwrap();
    muster_lib::ops::add_note(&root, &c.id, "Evil NOTE", "owner", "2026-10-14 10:01").unwrap();
    let path = root.join(&c.path);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("Evil NOTE"), "{text}");
    let text = text
        .replace("Evil TITLE", &format!("Evil {PAYLOAD}"))
        .replace("Evil BODY", &format!("Evil {PAYLOAD}"))
        .replace("Evil NOTE", &format!("Evil {PAYLOAD}"))
        .replace("stage: backlog", "stage: ready")
        .replace("stage: shaping", "stage: ready");
    let text = format!(
        "{}\n- 2026-10-14 10:02 owner: log {PAYLOAD}\n",
        text.trim_end()
    );
    fs::write(&path, text).unwrap();
    (d, root, path)
}

/// Every character a terminal acts on rather than shows: C0 but newline and tab, DEL and C1.
fn controls(s: &str) -> Vec<String> {
    s.chars()
        .filter(|&c| {
            (c < ' ' && c != '\n' && c != '\t') || c == '\x7f' || ('\u{80}'..='\u{9f}').contains(&c)
        })
        .map(|c| format!("U+{:04X}", u32::from(c)))
        .collect()
}

/// The output as text: muster writes UTF-8, so a stray 0x80-0x9F byte can't hide in it.
fn text(bytes: Vec<u8>, what: &str) -> String {
    String::from_utf8(bytes).unwrap_or_else(|e| panic!("{what} isn't UTF-8: {e}"))
}

/// AC1: every command that prints for a person shows the payload's controls as escapes.
#[test]
fn no_human_output_prints_a_control_from_queue_text() {
    let (_d, root, file) = queue();
    let runs: &[&[&str]] = &[
        &["list"],
        &["list", "--all"],
        &["show", "pj-1"],
        &["show", "pj-1", "--lang", "fa"],
        &["ready"],
        &["ready", "--parallel", "2"],
        &["history"],
        &["index", "--check"],
        &["index"],
        &["check"],
        &["inbox", "--keep"],
        &["gates", "pj-1"],
        &["note", "pj-1", "plain note", "--as", "tester"],
        &["claim", "pj-1", "--as", "tester"],
        &["show", "pj-1"],
        &["pass", "pj-1", "--as", "tester", "--from", "building"],
        &["list", "--lang", "fa"],
    ];
    let mut shown = 0;
    for args in runs {
        let (_, out, err) = muster(&root, args);
        let (out, err) = (text(out, "stdout"), text(err, "stderr"));
        assert_eq!(
            controls(&out),
            Vec::<String>::new(),
            "{args:?} stdout:\n{out}"
        );
        assert_eq!(
            controls(&err),
            Vec::<String>::new(),
            "{args:?} stderr:\n{err}"
        );
        if out.contains("Evil") || err.contains("Evil") {
            assert!(
                out.contains(SHOWN) || err.contains(SHOWN) || out.contains(r"\e[2J"),
                "{args:?} shows the payload escaped:\n{out}\n{err}"
            );
            shown += 1;
        }
    }
    assert!(shown >= 6, "only {shown} runs printed the task");
    // `show` prints the whole file: title, body, note and log line, each escaped.
    let (code, out, _) = muster(&root, &["show", "pj-1"]);
    let out = text(out, "stdout");
    assert_eq!(code, 0);
    assert_eq!(out.matches(SHOWN).count(), 4, "{out}");
    // The file itself is as it was written (claim moved it to another folder): only the output
    // escapes.
    let name = file.file_name().unwrap();
    let file = muster_lib::queue::STATUS_FOLDERS
        .iter()
        .map(|f| root.join(f).join(name))
        .find(|p| p.exists())
        .expect("the task's file");
    assert_eq!(
        fs::read_to_string(file).unwrap().matches(PAYLOAD).count(),
        4
    );
}

/// AC1: CRLF line ends print as plain line ends, a lone CR as `\r`.
#[test]
fn a_crlf_line_end_is_a_line_end() {
    let (_d, root, file) = queue();
    let crlf = fs::read_to_string(&file).unwrap().replace('\n', "\r\n");
    fs::write(&file, crlf).unwrap();
    let (_, out, _) = muster(&root, &["show", "pj-1"]);
    let out = text(out, "stdout");
    assert_eq!(controls(&out), Vec::<String>::new(), "{out}");
    assert!(!out.contains(r"\r\n"), "{out}");
    assert!(out.contains(r"lone\rCR"), "{out}");
}

/// AC2: `--json` prints the payload as JSON escapes, the same bytes as before mu-221
/// (tests/fixtures/terminal-controls.json, made by the muster before it, the queue's folder
/// written as `<root>` and the file's time as `<mtime>`).
#[test]
fn json_output_is_byte_identical() {
    let (_d, root, _) = queue();
    let mut got = String::new();
    for args in [
        &["show", "pj-1", "--json"][..],
        &["list", "--json"],
        &["ready", "--json"],
    ] {
        let (code, out, err) = muster(&root, args);
        assert_eq!(code, 0, "{args:?}: {}", String::from_utf8_lossy(&err));
        got.push_str(&format!("$ muster {}\n", args.join(" ")));
        for line in text(out, "stdout").lines() {
            match line.split_once("\"modified\": ") {
                Some((indent, _)) => got.push_str(&format!("{indent}\"modified\": <mtime>,\n")),
                None => got.push_str(&format!("{line}\n")),
            }
        }
    }
    let got = got.replace(&root.to_string_lossy().to_string(), "<root>");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terminal-controls.json");
    if std::env::var_os("MUSTER_BLESS").is_some() {
        fs::write(&fixture, &got).unwrap();
    }
    let want = fs::read_to_string(&fixture).unwrap();
    assert_eq!(got, want);
    assert!(got.contains(r"\u001b[2J"), "serde escapes the controls");
}

/// The functions that may write to stdout or stderr without `out_str`/`err_str`, by file, each
/// for a reason: the writers themselves, JSON a script reads byte for byte, the status line's
/// stdin passed through as it is, the failure line naming only an I/O error, merge-log's own
/// escaped line, and two warnings holding only a count.
const ALLOWED: &[(&str, &[&str])] = &[
    (
        "cli.rs",
        &[
            "out_str",
            "out_raw",
            "err_str",
            "print_json",
            "main",
            "record_statusline",
        ],
    ),
    ("merge_log.rs", &["say"]),
    ("profiles.rs", &["load"]),
    ("settings.rs", &["account_labels"]),
];

/// What prints past the escaping: the std print macros (`print!`, `println!`, `eprint!`,
/// `eprintln!`), stdout or stderr taken directly (`writeln!(std::io::stderr(), …)`), and the raw
/// writer.
const PRINTS: &[&str] = &["print!(", "println!(", "stdout()", "stderr()", "out_raw("];

/// The lines of `src` (the file `file`) that print past `out_str`/`err_str` outside the functions
/// `ALLOWED` names for it, as `file:line: text`. Comments don't count, nor does the file's test
/// module (the first top-level `mod tests`, which runs only under `cargo test`).
fn bare_prints(file: &str, src: &str) -> Vec<String> {
    let allowed = ALLOWED
        .iter()
        .find(|(f, _)| *f == file)
        .map_or(&[][..], |(_, fns)| *fns);
    let mut in_fn = "";
    let mut found = Vec::new();
    for (n, line) in src.lines().enumerate() {
        if ["mod tests {", "pub mod tests {", "pub(crate) mod tests {"].contains(&line) {
            break;
        }
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        if let Some(name) = fn_name(code) {
            in_fn = name;
        }
        if PRINTS.iter().any(|p| code.contains(p)) && !allowed.contains(&in_fn) {
            found.push(format!("{file}:{}: {code}", n + 1));
        }
    }
    found
}

/// The name a line declares a function by (`pub(crate) const fn name(`), if it does.
fn fn_name(code: &str) -> Option<&str> {
    let mut rest = code;
    for q in [
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "const ",
        "async ",
        "unsafe ",
    ] {
        rest = rest.strip_prefix(q).unwrap_or(rest);
    }
    let rest = rest.strip_prefix("fn ")?;
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    Some(&rest[..end])
}

/// Every source file of the crate: (its path under src/, its text).
fn sources() -> Vec<(String, String)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut all = Vec::new();
    for dir in [src.clone(), src.join("bin")] {
        for e in fs::read_dir(&dir).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_some_and(|x| x == "rs") {
                let name = path
                    .strip_prefix(&src)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                all.push((name, fs::read_to_string(&path).unwrap()));
            }
        }
    }
    all.sort();
    all
}

/// AC3: no human print bypasses `out!`/`outln!`/`errln!`, in the CLI or any module it calls.
#[test]
fn every_print_goes_through_the_escaping() {
    let all = sources();
    assert!(all.iter().any(|(f, _)| f == "cli.rs"), "no cli.rs in src/");
    let bare: Vec<_> = all.iter().flat_map(|(f, s)| bare_prints(f, s)).collect();
    assert!(
        bare.is_empty(),
        "every print for a person goes through out!/outln!/errln! (out_str, err_str):\n{}",
        bare.join("\n")
    );
    // Each allowed function still exists, so the list can't keep a name nobody uses.
    for (file, fns) in ALLOWED {
        let src = &all.iter().find(|(f, _)| f == file).unwrap().1;
        for name in *fns {
            assert!(
                src.lines().any(|l| fn_name(l.trim_start()) == Some(name)),
                "{file} has no fn {name}"
            );
        }
    }
}

/// AC3: a new command printing a title raw, by any of the ways past the escaping, is refused.
#[test]
fn a_raw_print_in_a_command_is_refused() {
    let cli = include_str!("../src/cli.rs");
    let at = cli.find("fn ready_cmd(").expect("ready_cmd is in cli.rs");
    let body = cli[at..].find('\n').unwrap() + at + 1;
    for mutant in [
        "    println!(\"{}\", t.title);",
        "    print!(\"{}\", t.title);",
        "    eprintln!(\"{}\", t.title);",
        "    let _ = writeln!(std::io::stdout(), \"{}\", t.title);",
        "    let _ = std::io::stderr().write_all(t.title.as_bytes());",
        "    out_raw(&t.title);",
    ] {
        let src = format!("{}{mutant}\n{}", &cli[..body], &cli[body..]);
        let found = bare_prints("cli.rs", &src);
        assert_eq!(found.len(), 1, "{mutant}: {found:?}");
        assert!(found[0].ends_with(mutant.trim_start()), "{found:?}");
    }
    // The same print in another module, and in a function cli.rs allows only by name.
    assert_eq!(
        bare_prints("ops.rs", "fn f() {\n    println!(\"x\");\n}\n").len(),
        1
    );
    assert_eq!(
        bare_prints("ops.rs", "fn main() {\n    println!(\"x\");\n}\n").len(),
        1
    );
    // A comment and the test module don't count.
    let ok = "fn f() {\n    // println!(\"x\")\n}\n#[cfg(test)]\nmod tests {\n    fn g() { println!(\"x\"); }\n}\n";
    assert_eq!(bare_prints("ops.rs", ok), Vec::<String>::new());
    assert_eq!(fn_name("pub(crate) const fn new() -> Self {"), Some("new"));
    assert_eq!(fn_name("let f = |x| x;"), None);
}
