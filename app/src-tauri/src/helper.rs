//! The AI helper: the capture role, run as the owner's own `claude` CLI with their own login,
//! in print mode, one turn per message, resuming the project's conversation. It may only use
//! the `muster` command (to read and write tasks) and read-only tools on the project folder.
//! Its stream is turned into short events for the window: replies, the commands it ran, the
//! result. Router and token variables are stripped, so it never goes through a third party.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// "reply", "tool", "toolResult", "done", "error", "session".
    pub kind: String,
    pub text: String,
}

/// Where the owner's `claude` is. A GUI app has a minimal PATH, so look in the usual places.
pub fn find_claude(home: &Path) -> Option<PathBuf> {
    let candidates = [
        home.join(".local/bin/claude"),
        home.join(".claude/local/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Claude Code config folders: `~/.claude` and `~/.claude-*` (each is one account).
pub fn accounts(home: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(home)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n == ".claude" || n.starts_with(".claude-"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Is this account logged in? From `claude auth status` (JSON, no model call). `None` when it
/// can't tell. `.claude` is the default folder, so CLAUDE_CONFIG_DIR stays unset for it.
pub fn login_status(claude: &Path, home: &Path, account: &str) -> Option<(bool, String)> {
    let mut cmd = std::process::Command::new(claude);
    cmd.args(["auth", "status"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    for k in STRIP_ENV {
        cmd.env_remove(k);
    }
    if account != ".claude" {
        cmd.envs([("CLAUDE_CONFIG_DIR", home.join(account))]);
    }
    let mut child = cmd.spawn().ok()?;
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
    format!(
        "You are the capture helper for the Muster task queue of the project \"{project}\" ({queue}). \
The owner talks to you here about ideas, bugs and tasks. Turn what they say into tasks that an agent can start on without asking.

Rules:
- Read and write tasks only with the `muster` command (it's on PATH and already pointed at this queue). Never edit files.
  `muster list`, `muster show <id>`, `muster new \"<title>\" --type bug|task|idea --priority high|normal|low --body \"<markdown>\"`,
  `muster note <id> \"<text>\"`, `muster set <id> priority|type|title <value>`, `muster move <id> blocked --on owner`, `muster attach <id> <file>`.
- Shell quoting: always wrap the --body text (and titles) in single quotes, never double quotes, so backticks and $ stay plain
  text. Inside them, never type a straight apostrophe; write ’ instead. One `muster` command per Bash call, nothing chained.
  Every command must be on ONE line: write each line break inside --body as the two characters \\n (muster turns them back
  into line breaks). A command with a real line break in it is refused.
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

/// The command line for one turn. `resume` continues the project's conversation.
pub fn args(message: &str, brief: &str, resume: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-p".into(),
        message.into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        // Filing tasks doesn't need the biggest model (decision 0007 in Drover: cheaper models
        // for asks that don't make code).
        "--model".into(),
        "sonnet".into(),
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
    fn args_allow_only_muster_and_reading() {
        let a = args("hi", "brief", Some("abc"));
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
        assert!(!args("hi", "b", None).contains(&"--resume".to_string()));
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

    #[test]
    fn finds_accounts() {
        let h = tempfile::tempdir().unwrap();
        for d in [".claude", ".claude-mg", ".config", ".clauder"] {
            std::fs::create_dir(h.path().join(d)).unwrap();
        }
        std::fs::write(h.path().join(".claude-file"), "").unwrap();
        assert_eq!(accounts(h.path()), vec![".claude", ".claude-mg"]);
    }
}
