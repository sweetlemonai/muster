//! The AI helper: the capture role, run as the owner's own `claude` CLI with their own login,
//! in print mode, one turn per message, resuming the project's conversation. It may only use
//! the `muster` command (to read and write tasks) and read-only tools on the project folder.
//! Its stream is turned into short events for the window: replies, the commands it ran, the
//! result. Router and token variables are stripped, so it never goes through a third party.
//!
//! Its account is one of the allowlist's (`accounts::allowed`, task 0099), and every `claude` it
//! runs has `CLAUDE_CONFIG_DIR` set to that account's folder: a bare `claude` is the company
//! account (`~/.claude`), which Muster never reads or uses.

use crate::accounts::Account;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// "reply", "tool", "toolResult", "done", "error", "session".
    pub kind: String,
    pub text: String,
}

/// Where the owner's `claude` is. A GUI app has a minimal PATH, so look in the usual places.
/// `~/.claude/local` is where Claude Code installs itself, not an account: nothing is read there.
pub fn find_claude(home: &Path) -> Option<PathBuf> {
    let candidates = [
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// The accounts the helper may use, by folder name (`.claude-mg`), in the allowlist's order:
/// never `.claude`, never a `.claude-*` folder that isn't on the list.
pub fn accounts(allowed: &[Account]) -> Vec<String> {
    allowed.iter().map(Account::folder_name).collect()
}

/// The allowed account whose folder name is `name` (`.claude-mg`), exactly. `.claude`, and any
/// folder not on the allowlist, is `None`.
pub fn account<'a>(name: &str, allowed: &'a [Account]) -> Option<&'a Account> {
    allowed.iter().find(|a| a.folder_name() == name)
}

/// `claude` run as `account`: router and token variables removed, and `CLAUDE_CONFIG_DIR` set to
/// the account's folder, always (without it, `claude` is the company account).
pub fn command(claude: &Path, account: &Account) -> Command {
    let mut cmd = Command::new(claude);
    cmd.stdin(Stdio::null());
    for k in STRIP_ENV {
        cmd.env_remove(k);
    }
    cmd.env("CLAUDE_CONFIG_DIR", account.dir());
    cmd
}

/// `claude auth status` as `account` (`command`).
pub fn status_command(claude: &Path, account: &Account) -> Command {
    let mut cmd = command(claude, account);
    cmd.args(["auth", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    cmd
}

/// One helper turn as `account` (`command`), with `args`, in `project_dir`: `PATH` is `path`, and
/// `muster` is pointed at the queue `root`.
pub fn run_command(
    claude: &Path,
    account: &Account,
    args: &[String],
    project_dir: &Path,
    path: &str,
    root: &Path,
) -> Command {
    let mut cmd = command(claude, account);
    cmd.args(args)
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PATH", path)
        .env("MUSTER_ROOT", root)
        .env("MUSTER_AS", "helper");
    cmd
}

/// The Terminal script that logs `account` in with `claude auth login`: routers and tokens unset,
/// `CLAUDE_CONFIG_DIR` exported to the account's folder, always.
pub fn login_script(claude: &Path, account: &Account) -> String {
    format!(
        "#!/bin/sh\n# Muster: log in the Claude account ~/{name} for the helper.\nunset ANTHROPIC_BASE_URL ANTHROPIC_CUSTOM_HEADERS ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN\nexport CLAUDE_CONFIG_DIR='{dir}'\n'{claude}' auth login\necho\necho 'Done. You can close this window and go back to Muster.'\n",
        name = account.folder_name(),
        dir = account.dir().display(),
        claude = claude.display()
    )
}

/// Is this account logged in? From `claude auth status` (JSON, no model call), run as the account
/// (`status_command`). `None` when it can't tell.
pub fn login_status(claude: &Path, account: &Account) -> Option<(bool, String)> {
    let mut child = status_command(claude, account).spawn().ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    parse_status(&out)
}

pub fn parse_status(json: &str) -> Option<(bool, String)> {
    let v: Value = serde_json::from_str(json.trim()).ok()?;
    let logged = v["loggedIn"].as_bool()?;
    let plan = v["subscriptionType"]
        .as_str()
        .or_else(|| v["authMethod"].as_str())
        .unwrap_or("")
        .to_string();
    Some((logged, if logged { plan } else { String::new() }))
}

/// Text for the "Not logged in" reply the CLI gives, so the window can offer a fix.
pub fn is_login_error(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("not logged in") || t.contains("please run /login") || t.contains("invalid api key")
}

pub fn brief(project: &str, queue: &Path) -> String {
    // A queue in the repo (0005): its files sit beside the code the helper reads.
    let place = if crate::layout::is_muster(queue) {
        " It lives in the repo's .muster/ folder (tasks in tasks/, notes in backlog/), which only muster writes."
    } else {
        ""
    };
    format!(
        "You are the capture helper for the Muster task queue of the project \"{project}\" ({queue}).{place} \
The owner talks to you here about ideas, bugs and tasks. Turn what they say into tasks that an agent can start on without asking.

Rules:
- Read and write tasks only with the `muster` command (it's on PATH and already pointed at this queue). Never edit files.
  `muster list`, `muster show <id>`, `muster new \"<title>\" --type code|bug|chore|tech-debt|spike|design|docs|research --priority high|normal|low --tags spike,tech-debt --time 2h --test-time 30m --tokens 500k --body \"<markdown>\"`,
  `muster note <id> \"<text>\"`, `muster set <id> priority|type|title <value>`, `muster move <id> blocked --on owner`, `muster attach <id> <file>`.
- A raw one-liner the owner wants kept for later, without enough to write a task yet: `muster add '<one line>'` puts it in the
  backlog as it is. Don't invent details to make it a task; a shaping session turns it into one.
- Shell quoting: always wrap the --body text (and titles) in single quotes, never double quotes, so backticks and $ stay plain
  text. Inside them, never type a straight apostrophe; write ’ instead. One `muster` command per Bash call, nothing chained.
  Every command must be on ONE line: write each line break inside --body as the two characters \\n (muster turns them back
  into line breaks). A command with a real line break in it is refused.
- Estimate every task for an agent doing it: --time (agent time, QA included), --test-time (the owner's check only) and --tokens. Rough sizes: S = 30m and 150k (a small fix, one file);
  M = 2h and 500k (a feature in a few files, with tests); L = 1d and 2M (a new screen or subsystem); XL = split it into tasks.
  Research and spikes cost tokens more than time. Say \"rough\" in your reply; estimates are for planning, not promises.
- Tags: add the ones that fit, from spike, chore, tech-debt, code, docs, design, research, test, perf, security (others are fine).
- Before filing, run `muster list` and check for a duplicate. If one matches, add to it with `muster note` and say so.
- One task per point. Each body has: **What the owner said:** (their words, close to verbatim); **Details:** what exists today,
  where in the code (read the project folder with Read, Grep and Glob), what's wanted; **Done looks like:**; then
  `## Acceptance criteria` with lines `- [ ] AC1: <what must be true>. — verify: command (<cmd>) | screenshot (<what>) | owner (<what>)`.
- Screenshots the owner pasted are saved as files. Their paths are in the message. Attach them with `muster attach`.
- If something is unclear, don't guess. Ask in your reply, in plain words: what it is, why it matters, the options, your pick.
  For technical choices, decide yourself from the project's own rules and say you did.
- Reply briefly, in plain words, no jargon: what you filed or changed (id and title), and any question. No file paths unless asked.",
        queue = queue.display()
    )
}

/// The models the helper can use, picked in Settings (0036). Filing tasks doesn't need the
/// biggest model (decision 0007 in Drover: cheaper models for asks that don't make code), so
/// Sonnet, the first, is the default.
pub const MODELS: [&str; 3] = ["sonnet", "haiku", "opus"];
pub const DEFAULT_MODEL: &str = MODELS[0];

/// `model` when it's one of `MODELS` exactly, else the default.
pub fn model_or_default(model: &str) -> &'static str {
    MODELS
        .into_iter()
        .find(|m| *m == model)
        .unwrap_or(DEFAULT_MODEL)
}

/// The command line for one turn with `model` (one of `MODELS`; anything else is the default).
/// `resume` continues the project's conversation.
pub fn args(message: &str, brief: &str, resume: Option<&str>, model: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-p".into(),
        message.into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--model".into(),
        model_or_default(model).into(),
        // Restricted: the account's own settings files are ignored, file tools stay inside the
        // project folder, and only the tools named here exist. Of those, the allow-list decides:
        // `muster` and reading run, anything else is refused without asking. Checked live: a
        // touch, an rm and a read of ~/.zshrc were refused, `muster note` ran.
        "--restricted".into(),
        "--strict-mcp-config".into(),
        "--tools".into(),
        "Bash".into(),
        "Read".into(),
        "Grep".into(),
        "Glob".into(),
        "--permission-mode".into(),
        "dontAsk".into(),
        "--append-system-prompt".into(),
        brief.into(),
        "--allowedTools".into(),
        "Bash(muster:*)".into(),
        "Read".into(),
        "Grep".into(),
        "Glob".into(),
    ];
    if let Some(s) = resume {
        a.push("--resume".into());
        a.push(s.into());
    }
    a
}

/// Environment variables removed before running the helper: routers and tokens that would
/// override the chosen account, or send the conversation through someone else.
pub const STRIP_ENV: [&str; 6] = [
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_CUSTOM_HEADERS",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CONFIG_DIR",
];

/// One stream-json line → events for the window. Unknown lines give nothing.
pub fn parse_line(line: &str) -> Vec<Event> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let ev = |kind: &str, text: String| Event {
        kind: kind.into(),
        text,
    };
    let short = |s: &str, n: usize| -> String {
        let s = s.trim();
        if s.chars().count() > n {
            format!("{}…", s.chars().take(n).collect::<String>())
        } else {
            s.to_string()
        }
    };
    match v["type"].as_str() {
        Some("system") if v["subtype"] == "init" => v["session_id"]
            .as_str()
            .map(|s| vec![ev("session", s.into())])
            .unwrap_or_default(),
        Some("assistant") => v["message"]["content"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| match p["type"].as_str() {
                        Some("text") => p["text"]
                            .as_str()
                            .filter(|t| !t.trim().is_empty())
                            .map(|t| ev("reply", t.into())),
                        Some("tool_use") => {
                            let name = p["name"].as_str().unwrap_or("tool");
                            let detail = p["input"]["command"]
                                .as_str()
                                .or_else(|| p["input"]["pattern"].as_str())
                                .or_else(|| p["input"]["file_path"].as_str())
                                .unwrap_or("");
                            Some(ev("tool", short(&format!("{name}: {detail}"), 300)))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("user") => v["message"]["content"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter(|p| p["type"] == "tool_result")
                    .filter_map(|p| {
                        let text = match &p["content"] {
                            Value::String(s) => s.clone(),
                            Value::Array(a) => a
                                .iter()
                                .filter_map(|x| x["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => String::new(),
                        };
                        let first = text.lines().next().unwrap_or("").to_string();
                        (!first.is_empty()).then(|| ev("toolResult", short(&first, 200)))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some("result") => {
            if v["is_error"].as_bool().unwrap_or(false) {
                vec![ev(
                    "error",
                    short(v["result"].as_str().unwrap_or("the helper failed"), 600),
                )]
            } else {
                let cost = v["total_cost_usd"]
                    .as_f64()
                    .map(|c| format!("${c:.3}"))
                    .unwrap_or_default();
                vec![ev("done", cost)]
            }
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brief_puts_raw_one_liners_in_the_backlog() {
        let b = brief("muster", Path::new("/q"));
        assert!(b.contains("`muster add '<one line>'`"), "{b}");
        assert!(b.contains("backlog"), "{b}");
    }

    #[test]
    fn the_model_is_the_one_picked_in_settings_sonnet_by_default() {
        let model = |m: &str| {
            let a = args("hi", "b", None, m);
            let at = a.iter().position(|x| x == "--model").unwrap();
            a[at + 1].clone()
        };
        assert_eq!(model("haiku"), "haiku");
        assert_eq!(model("opus"), "opus");
        assert_eq!(model("sonnet"), "sonnet");
        // Anything else (an old or hand-edited helper.json) is the default, never passed through.
        assert_eq!(model(""), "sonnet");
        assert_eq!(model("--dangerously-skip-permissions"), "sonnet");
        assert_eq!(model("Opus"), "sonnet");
        assert_eq!(MODELS[0], DEFAULT_MODEL);
    }

    #[test]
    fn the_brief_says_where_a_queue_in_the_repo_lives() {
        let b = brief("muster", Path::new("/r/main/.muster"));
        assert!(b.contains("(/r/main/.muster). It lives in the repo's .muster/ folder (tasks in tasks/, notes in backlog/), which only muster writes."), "{b}");
        assert!(!brief("muster", Path::new("/q/tasks")).contains(".muster"));
    }

    #[test]
    fn args_allow_only_muster_and_reading() {
        let a = args("hi", "brief", Some("abc"), "sonnet");
        assert_eq!(&a[..2], &["-p", "hi"]);
        let allowed = a.iter().position(|x| x == "--allowedTools").unwrap();
        assert_eq!(
            &a[allowed + 1..allowed + 5],
            &["Bash(muster:*)", "Read", "Grep", "Glob"]
        );
        let tools = a.iter().position(|x| x == "--tools").unwrap();
        assert_eq!(&a[tools + 1..tools + 5], &["Bash", "Read", "Grep", "Glob"]);
        assert!(
            a.contains(&"--restricted".to_string())
                && a.contains(&"--strict-mcp-config".to_string())
        );
        assert!(!a
            .iter()
            .any(|x| x == "Write" || x == "Edit" || x == "WebFetch"));
        assert_eq!(&a[a.len() - 2..], &["--resume", "abc"]);
        assert!(!args("hi", "b", None, "sonnet").contains(&"--resume".to_string()));
        let mode = a.iter().position(|x| x == "--permission-mode").unwrap();
        assert_eq!(a[mode + 1], "dontAsk");
    }

    #[test]
    fn parses_the_stream() {
        assert_eq!(
            parse_line(r#"{"type":"system","subtype":"init","session_id":"s1"}"#),
            vec![Event {
                kind: "session".into(),
                text: "s1".into()
            }]
        );
        let a = parse_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Filed 0010."},{"type":"tool_use","name":"Bash","input":{"command":"muster new \"X\""}}]}}"#,
        );
        assert_eq!(
            a[0],
            Event {
                kind: "reply".into(),
                text: "Filed 0010.".into()
            }
        );
        assert_eq!(
            a[1],
            Event {
                kind: "tool".into(),
                text: "Bash: muster new \"X\"".into()
            }
        );
        let r = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"0010 todo/0010-x.md\nmore"}]}}"#,
        );
        assert_eq!(r[0].text, "0010 todo/0010-x.md");
        let r = parse_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":[{"type":"text","text":"ok"}]}]}}"#,
        );
        assert_eq!(r[0].text, "ok");
        assert_eq!(
            parse_line(r#"{"type":"result","is_error":false,"total_cost_usd":0.0123}"#)[0],
            Event {
                kind: "done".into(),
                text: "$0.012".into()
            }
        );
        assert_eq!(
            parse_line(r#"{"type":"result","is_error":true,"result":"Not logged in"}"#)[0].kind,
            "error"
        );
        assert!(parse_line("not json").is_empty());
        assert!(parse_line(r#"{"type":"stream_event"}"#).is_empty());
    }

    #[test]
    fn reads_login_status() {
        assert_eq!(
            parse_status(
                r#"{"loggedIn": true, "authMethod": "claude.ai", "subscriptionType": "team"}"#
            ),
            Some((true, "team".into()))
        );
        assert_eq!(
            parse_status(r#"{"loggedIn": false, "authMethod": "none", "subscriptionType": null}"#),
            Some((false, String::new()))
        );
        assert_eq!(parse_status("Claude configuration file not found"), None);
        assert!(is_login_error("Not logged in · Please run /login"));
        assert!(!is_login_error("Filed 0012."));
    }

    use crate::accounts::{resolve, tests::fake_home};
    use std::ffi::OsStr;

    fn allowed(home: &Path, labels: &[&str]) -> Vec<Account> {
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        resolve(&labels, home)
    }

    /// The value `cmd` sets `key` to: `Some(None)` when it removes it, `None` when it leaves it.
    fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<Option<&'a OsStr>> {
        cmd.get_envs()
            .find(|(k, _)| *k == OsStr::new(key))
            .map(|(_, v)| v)
    }

    #[test]
    fn offers_only_the_allowlists_accounts_never_the_company_one() {
        // The home has .claude (company), .claude-ih, .claude-mg and .claude-hg.
        let d = fake_home();
        let a = allowed(d.path(), &["mg", "hg"]);
        assert_eq!(accounts(&a), [".claude-mg", ".claude-hg"]);
        assert_eq!(
            accounts(&allowed(d.path(), &["hg"])),
            [".claude-hg"],
            "a .claude-* folder that isn't listed isn't offered"
        );
        assert!(accounts(&[]).is_empty(), "an empty list offers nothing");
        assert_eq!(account(".claude-mg", &a), Some(&a[0]));
        for no in [
            ".claude",
            ".claude-ih",
            "",
            "mg",
            ".claude-m",
            "../.claude-mg",
        ] {
            assert_eq!(account(no, &a), None, "{no:?}");
        }
    }

    #[test]
    fn every_claude_it_runs_has_the_accounts_config_dir() {
        let d = fake_home();
        let a = allowed(d.path(), &["hg"]);
        let dir = d.path().join(".claude-hg");
        let claude = Path::new("/bin/claude");

        let status = status_command(claude, &a[0]);
        assert_eq!(
            env_of(&status, "CLAUDE_CONFIG_DIR"),
            Some(Some(dir.as_os_str()))
        );
        assert_eq!(
            status.get_args().collect::<Vec<_>>(),
            [OsStr::new("auth"), OsStr::new("status")]
        );

        let run = run_command(
            claude,
            &a[0],
            &args("hi", "b", None, "sonnet"),
            Path::new("/p"),
            "/bin",
            Path::new("/p/tasks"),
        );
        assert_eq!(
            env_of(&run, "CLAUDE_CONFIG_DIR"),
            Some(Some(dir.as_os_str()))
        );
        assert_eq!(env_of(&run, "MUSTER_AS"), Some(Some(OsStr::new("helper"))));
        assert_eq!(
            env_of(&run, "MUSTER_ROOT"),
            Some(Some(OsStr::new("/p/tasks")))
        );
        assert_eq!(env_of(&run, "PATH"), Some(Some(OsStr::new("/bin"))));
        assert_eq!(run.get_current_dir(), Some(Path::new("/p")));
        assert_eq!(run.get_args().next(), Some(OsStr::new("-p")));

        // Routers and tokens are removed from both.
        for cmd in [&status, &run] {
            for k in STRIP_ENV.iter().filter(|k| **k != "CLAUDE_CONFIG_DIR") {
                assert_eq!(env_of(cmd, k), Some(None), "{k}");
            }
        }
    }

    #[test]
    fn the_login_status_is_read_as_the_account() {
        // A stand-in `claude` that answers `auth status` with the folder it was given.
        let d = fake_home();
        let a = allowed(d.path(), &["mg"]);
        let claude = d.path().join("claude");
        std::fs::write(
            &claude,
            "#!/bin/sh\n[ \"$1 $2\" = 'auth status' ] || exit 1\nprintf '{\"loggedIn\": true, \"subscriptionType\": \"%s\"}' \"${CLAUDE_CONFIG_DIR:-unset}\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            login_status(&claude, &a[0]),
            Some((true, d.path().join(".claude-mg").display().to_string()))
        );
    }

    #[test]
    fn the_login_script_always_exports_the_accounts_folder() {
        let d = fake_home();
        let a = allowed(d.path(), &["mg"]);
        let s = login_script(Path::new("/x/claude"), &a[0]);
        let dir = d.path().join(".claude-mg");
        assert!(
            s.contains(&format!("\nexport CLAUDE_CONFIG_DIR='{}'\n", dir.display())),
            "{s}"
        );
        assert!(s.contains("~/.claude-mg for the helper"), "{s}");
        assert!(s.contains("\n'/x/claude' auth login\n"), "{s}");
        // The folder is never unset (that would log in the company account).
        assert!(
            s.lines()
                .filter(|l| l.trim_start().starts_with("unset"))
                .all(|l| !l.contains("CLAUDE_CONFIG_DIR")),
            "{s}"
        );
        // The export comes before the login.
        assert!(s.find("export CLAUDE_CONFIG_DIR") < s.find("auth login"));
    }
}
