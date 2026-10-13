//! The app's commands for the AI helper (see `helper`): its account, transcript, and turns.

use crate::{blocking, check_stamp, helper, home, ops, queue, queue_root, tools, AppState};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct HelperConfig {
    /// Claude Code config folder name under $HOME (".claude", ".claude-mg"…), chosen once.
    account: String,
    /// Conversation to resume, by queue name.
    sessions: HashMap<String, String>,
    /// The model it runs (0036), one of `helper::MODELS`; empty or unknown is the default.
    model: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HelperLine {
    role: String,
    kind: String,
    text: String,
    at: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    name: String,
    /// None when it couldn't be checked.
    logged_in: Option<bool>,
    plan: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperInfo {
    claude: Option<String>,
    accounts: Vec<AccountInfo>,
    account: String,
    /// The model it runs, and the ones it can (0036).
    model: &'static str,
    models: [&'static str; 3],
    running: bool,
    lines: Vec<HelperLine>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HelperPush {
    queue: String,
    line: HelperLine,
}

fn helper_dir(app: &AppState) -> PathBuf {
    app.config_file
        .parent()
        .map(|p| p.join("helper"))
        .unwrap_or_default()
}

fn load_helper(app: &AppState) -> HelperConfig {
    std::fs::read_to_string(helper_dir(app).join("helper.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_helper(app: &AppState, c: &HelperConfig) -> Result<(), String> {
    let dir = helper_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    queue::write_atomic(
        &dir.join("helper.json"),
        &serde_json::to_string_pretty(c).map_err(|e| e.to_string())?,
    )
}

fn transcript_file(app: &AppState, queue: &str) -> PathBuf {
    let safe: String = queue
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    helper_dir(app).join(format!("{safe}.jsonl"))
}

fn read_transcript(app: &AppState, queue: &str) -> Vec<HelperLine> {
    let text = std::fs::read_to_string(transcript_file(app, queue)).unwrap_or_default();
    let lines: Vec<HelperLine> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    lines[lines.len().saturating_sub(400)..].to_vec()
}

fn append_transcript(app: &AppState, queue: &str, line: &HelperLine) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(helper_dir(app));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_file(app, queue))
    {
        let _ = writeln!(f, "{}", serde_json::to_string(line).unwrap_or_default());
    }
}

fn push(handle: &tauri::AppHandle, queue: &str, role: &str, kind: &str, text: &str) {
    let line = HelperLine {
        role: role.into(),
        kind: kind.into(),
        text: text.into(),
        at: queue::now_secs(),
    };
    let app = handle.state::<AppState>();
    append_transcript(&app, queue, &line);
    let _ = handle.emit(
        "helper",
        HelperPush {
            queue: queue.into(),
            line,
        },
    );
}

#[tauri::command]
pub async fn helper_info(handle: tauri::AppHandle, queue: String) -> Result<HelperInfo, String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let running = app
            .helpers
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(&queue);
        let config = load_helper(&app);
        let claude = helper::find_claude(&home());
        let accounts = helper::accounts(&home())
            .into_iter()
            .map(|name| {
                let status = claude
                    .as_ref()
                    .and_then(|c| helper::login_status(c, &home(), &name));
                AccountInfo {
                    logged_in: status.as_ref().map(|s| s.0),
                    plan: status.map(|s| s.1).unwrap_or_default(),
                    name,
                }
            })
            .collect();
        Ok(HelperInfo {
            claude: claude.map(|p| p.to_string_lossy().to_string()),
            accounts,
            account: config.account,
            model: helper::model_or_default(&config.model),
            models: helper::MODELS,
            running,
            lines: read_transcript(&app, &queue),
        })
    })
    .await
}

/// Open Terminal on the account's own login (`claude auth login`). The owner signs in there;
/// Muster never sees the login.
#[tauri::command]
pub async fn helper_login(handle: tauri::AppHandle, account: String) -> Result<(), String> {
    blocking(move || {
        if !helper::accounts(&home()).contains(&account) {
            return Err("no such account folder".into());
        }
        let claude = helper::find_claude(&home()).ok_or("couldn't find the claude command")?;
        let app = handle.state::<AppState>();
        let dir = helper_dir(&app);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let script = dir.join("login.command");
        let folder = if account == ".claude" {
            "unset CLAUDE_CONFIG_DIR".to_string()
        } else {
            format!("export CLAUDE_CONFIG_DIR='{}'", home().join(&account).display())
        };
        let body = format!(
            "#!/bin/sh\n# Muster: log in the Claude account ~/{account} for the helper.\nunset ANTHROPIC_BASE_URL ANTHROPIC_CUSTOM_HEADERS ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN\n{folder}\n'{}' auth login\necho\necho 'Done. You can close this window and go back to Muster.'\n",
            claude.display()
        );
        std::fs::write(&script, body).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        let ok = std::process::Command::new("open")
            .args(["-a", "Terminal"])
            .arg(&script)
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if ok {
            Ok(())
        } else {
            Err("couldn't open Terminal".into())
        }
    })
    .await
}

#[tauri::command]
pub async fn helper_set_account(handle: tauri::AppHandle, account: String) -> Result<(), String> {
    blocking(move || {
        if !helper::accounts(&home()).contains(&account) {
            return Err("no such account folder".into());
        }
        let app = handle.state::<AppState>();
        let mut c = load_helper(&app);
        c.account = account;
        c.sessions.clear(); // a conversation belongs to one account
        save_helper(&app, &c)
    })
    .await
}

/// The model the helper runs from its next turn (0036): one of `helper::MODELS`, exactly.
#[tauri::command]
pub async fn helper_set_model(handle: tauri::AppHandle, model: String) -> Result<(), String> {
    blocking(move || {
        if !helper::MODELS.contains(&model.as_str()) {
            return Err(format!("no model named {model:?}"));
        }
        let app = handle.state::<AppState>();
        let mut c = load_helper(&app);
        c.model = model;
        save_helper(&app, &c)
    })
    .await
}

/// Start a new conversation for this project (the transcript stays, with a marker).
#[tauri::command]
pub async fn helper_reset(handle: tauri::AppHandle, queue: String) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let mut c = load_helper(&app);
        c.sessions.remove(&queue);
        save_helper(&app, &c)?;
        push(&handle, &queue, "system", "note", "New conversation.");
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn helper_stop(handle: tauri::AppHandle, queue: String) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        if let Some(child) = app
            .helpers
            .lock()
            .map_err(|e| e.to_string())?
            .get_mut(&queue)
        {
            let _ = child.kill();
        }
        Ok(())
    })
    .await
}

/// One message to the helper. Pasted files are saved in the queue's `inbox/helper/` and named
/// in the message. The reply streams to the window as `helper` events, then `helper-idle`.
#[tauri::command]
pub async fn helper_send(
    handle: tauri::AppHandle,
    queue: String,
    message: String,
    files: Vec<(String, String)>,
    stamp: String,
) -> Result<(), String> {
    check_stamp(&stamp)?;
    let message = match queue::clean_text(&message) {
        Ok(m) => m,
        Err(_) if !files.is_empty() => String::new(),
        Err(e) => return Err(e),
    };
    blocking(move || {
        let app = handle.state::<AppState>();
        if app.helpers.lock().map_err(|e| e.to_string())?.contains_key(&queue) {
            return Err("the helper is still working on your last message".into());
        }
        let root = queue_root(&app.config_file, &queue)?;
        let claude = helper::find_claude(&home())
            .ok_or("couldn't find the claude command (looked in ~/.local/bin, ~/.claude/local, /opt/homebrew/bin, /usr/local/bin)")?;
        let cfg = load_helper(&app);
        if cfg.account.is_empty() {
            return Err("choose which Claude account the helper uses first".into());
        }
        let mut text = message.clone();
        if !files.is_empty() {
            let dir = root.join("inbox").join("helper");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            for (name, url) in &files {
                let bytes = ops::data_url_bytes(url)?;
                let name = ops::attachment_name(name)?;
                let p = dir.join(format!("{}-{name}", stamp.replace([' ', ':'], "-")));
                std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
                text.push_str(&format!("\n[pasted file: {}]", p.display()));
            }
        }
        push(&handle, &queue, "owner", "message", &text);
        let project_dir = root.parent().unwrap_or(&root).to_path_buf();
        let brief = helper::brief(&queue, &root);
        let resume = cfg.sessions.get(&queue).cloned();
        let path = format!(
            "{}:{}:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin",
            tools().display(),
            home().join(".local/bin").display()
        );
        let mut cmd = std::process::Command::new(&claude);
        cmd.args(helper::args(&text, &brief, resume.as_deref(), &cfg.model))
            .current_dir(&project_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for k in helper::STRIP_ENV {
            cmd.env_remove(k);
        }
        let mut vars: Vec<(&str, std::ffi::OsString)> =
            vec![("PATH", path.into()), ("MUSTER_ROOT", root.clone().into()), ("MUSTER_AS", "helper".into())];
        if cfg.account != ".claude" {
            vars.push(("CLAUDE_CONFIG_DIR", home().join(&cfg.account).into()));
        }
        cmd.envs(vars);
        let mut child = cmd.spawn().map_err(|e| format!("couldn't start claude: {e}"))?;
        let stdout = child.stdout.take().ok_or("no output from claude")?;
        let stderr = child.stderr.take();
        app.helpers.lock().map_err(|e| e.to_string())?.insert(queue.clone(), child);
        let h = handle.clone();
        std::thread::spawn(move || {
            let mut got_result = false;
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                for ev in helper::parse_line(&line) {
                    if ev.kind == "session" {
                        let app = h.state::<AppState>();
                        let mut c = load_helper(&app);
                        c.sessions.insert(queue.clone(), ev.text.clone());
                        let _ = save_helper(&app, &c);
                        continue;
                    }
                    if ev.kind == "done" || ev.kind == "error" {
                        got_result = true;
                    }
                    // "Not logged in" comes as a reply and an error: show it once, as a login
                    // problem the window can offer a fix for.
                    if helper::is_login_error(&ev.text) {
                        if ev.kind == "error" {
                            push(&h, &queue, "helper", "login", &ev.text);
                        }
                        continue;
                    }
                    push(&h, &queue, "helper", &ev.kind, &ev.text);
                }
            }
            let app = h.state::<AppState>();
            let child = app.helpers.lock().ok().and_then(|mut m| m.remove(&queue));
            let status = child.and_then(|mut c| c.wait().ok());
            if !got_result {
                let mut err = String::new();
                if let Some(mut e) = stderr {
                    let _ = std::io::Read::read_to_string(&mut std::io::Read::take(&mut e, 4000), &mut err);
                }
                let why = if status.map(|s| s.success()).unwrap_or(false) {
                    "The helper stopped without a reply.".to_string()
                } else if err.trim().is_empty() {
                    "Stopped.".to_string()
                } else {
                    err.trim().to_string()
                };
                push(&h, &queue, "helper", "error", &why);
            }
            let _ = h.emit("helper-idle", queue.clone());
        });
        Ok(())
    })
    .await
}
