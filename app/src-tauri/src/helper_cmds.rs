//! The app's commands for the AI helper (see `helper`): its account, transcript, and turns.

use crate::accounts::{self, Account};
use crate::{
    blocking, check_stamp, helper, home, ops, queue, queue_root, settings, tools, AppState,
};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

/// Refused: an account that isn't on the allowlist (`accounts::allowed`, 0099).
const NOT_ALLOWED: &str =
    "that Claude account isn't on Muster's list of accounts it may use (Settings, Agents)";

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct HelperConfig {
    /// Claude Code config folder name under $HOME (".claude-mg"…), chosen once, one of the
    /// allowlist's (0099).
    account: String,
    /// Conversation to resume, by queue name.
    sessions: HashMap<String, String>,
    /// The model it runs (0036), one of `helper::MODELS`; empty or unknown is the default.
    model: String,
}

impl HelperConfig {
    /// Forget a saved account that isn't on the allowlist (`.claude`, the company account, or
    /// one taken off the list), and its conversations, so the picker asks again. True when it
    /// changed.
    fn forget_unallowed(&mut self, allowed: &[Account]) -> bool {
        if self.account.is_empty() || helper::account(&self.account, allowed).is_some() {
            return false;
        }
        self.account.clear();
        self.sessions.clear(); // a conversation belongs to one account
        true
    }
}

/// The accounts the helper may use, read from the settings now, so a change applies at once.
fn allowed(app: &AppState) -> Vec<Account> {
    accounts::allowed(&settings::load(&app.settings_file), &home())
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
        let allowed = allowed(&app);
        let mut config = load_helper(&app);
        if config.forget_unallowed(&allowed) {
            save_helper(&app, &config)?;
        }
        let claude = helper::find_claude(&home());
        let accounts = allowed
            .iter()
            .map(|account| {
                let status = claude
                    .as_ref()
                    .and_then(|c| helper::login_status(c, account));
                AccountInfo {
                    logged_in: status.as_ref().map(|s| s.0),
                    plan: status.map(|s| s.1).unwrap_or_default(),
                    name: account.folder_name(),
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
        let app = handle.state::<AppState>();
        let allowed = allowed(&app);
        let account = helper::account(&account, &allowed).ok_or(NOT_ALLOWED)?;
        let claude = helper::find_claude(&home()).ok_or("couldn't find the claude command")?;
        let dir = helper_dir(&app);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let script = dir.join("login.command");
        std::fs::write(&script, helper::login_script(&claude, account))
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
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
        let app = handle.state::<AppState>();
        if helper::account(&account, &allowed(&app)).is_none() {
            return Err(NOT_ALLOWED.into());
        }
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
        let allowed = allowed(&app);
        let account = helper::account(&cfg.account, &allowed).ok_or(NOT_ALLOWED)?;
        let mut text = message.clone();
        if !files.is_empty() {
            let dir = crate::backlog::backlog_dir(&root).join("helper");
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
        let mut cmd = helper::run_command(
            &claude,
            account,
            &helper::args(&text, &brief, resume.as_deref(), &cfg.model),
            &project_dir,
            &path,
            &root,
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{resolve, tests::fake_home};

    fn saved(account: &str) -> HelperConfig {
        HelperConfig {
            account: account.into(),
            sessions: HashMap::from([("muster".to_string(), "s1".to_string())]),
            model: "haiku".into(),
        }
    }

    #[test]
    fn a_saved_account_off_the_allowlist_is_forgotten_so_the_picker_asks_again() {
        // The home has .claude (company), .claude-ih, .claude-mg and .claude-hg.
        let d = fake_home();
        let allowed = resolve(&["mg".into(), "hg".into()], d.path());
        for gone in [".claude", ".claude-ih", ".claude-gone", "../.claude-mg"] {
            let mut c = saved(gone);
            assert!(c.forget_unallowed(&allowed), "{gone:?}");
            assert_eq!(c.account, "", "{gone:?}");
            assert!(c.sessions.is_empty(), "its conversations go with it");
            assert_eq!(c.model, "haiku", "the model stays");
            // What's saved is the cleared setting.
            let back: HelperConfig =
                serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
            assert_eq!(back.account, "");
        }
        // An account taken off the list.
        let mut c = saved(".claude-hg");
        assert!(c.forget_unallowed(&resolve(&["mg".into()], d.path())));
        assert_eq!(c.account, "");
        // An allowed account, or none chosen yet, stays as it is.
        for keep in [".claude-mg", ".claude-hg", ""] {
            let mut c = saved(keep);
            assert!(!c.forget_unallowed(&allowed), "{keep:?}");
            assert_eq!(c.account, keep);
            assert_eq!(c.sessions.len(), 1);
        }
    }
}
