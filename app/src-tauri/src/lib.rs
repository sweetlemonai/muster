//! Muster's desktop app: thin async commands over `queue` and `projects`. Disk and git work runs
//! in `spawn_blocking`. The window names a queue by its project name, and a project by the key
//! the core gave it, never by a path it made up. Folders are picked by the core's own dialog.

pub mod backlog;
pub mod cli;
pub mod fields;
pub mod help;
pub mod helper;
mod helper_cmds;
pub mod index;
pub mod ops;
pub mod projects;
pub mod queue;
pub mod session;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub(crate) struct AppState {
    cache: Mutex<queue::Cache>,
    /// Running helper turns, by queue name.
    pub(crate) helpers: Mutex<HashMap<String, std::process::Child>>,
    /// Serializes changes to projects.json.
    config: Mutex<()>,
    pub(crate) config_file: PathBuf,
}

fn base() -> PathBuf {
    std::env::var_os("MUSTER_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground"))
}

pub(crate) fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Muster's tools, which a new queue's stand-ins run.
pub(crate) fn tools() -> PathBuf {
    std::env::var_os("MUSTER_TOOLS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground/muster/main/bin"))
}

fn queues(config_file: &std::path::Path) -> Vec<(String, PathBuf)> {
    projects::active(&base(), &projects::load(config_file))
}

pub(crate) fn queue_root(config_file: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    queues(config_file)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, p)| p)
        .ok_or_else(|| format!("no queue named {name:?}"))
}

pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn state(app: tauri::State<'_, AppState>) -> Result<queue::State, String> {
    let file = app.config_file.clone();
    // The cache is small and the read is quick on a warm cache; take it inside the blocking task.
    let cache = std::mem::take(&mut *app.cache.lock().map_err(|e| e.to_string())?);
    let (state, cache) = blocking(move || {
        let mut cache = cache;
        let s = queue::read_state(&queues(&file), &mut cache);
        Ok((s, cache))
    })
    .await?;
    *app.cache.lock().map_err(|e| e.to_string())? = cache;
    Ok(state)
}

#[tauri::command]
async fn task_body(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || queue::read_body(&queue_root(&file, &queue)?, &id)).await
}

#[tauri::command]
async fn image(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    name: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || queue::read_image(&queue_root(&file, &queue)?, &id, &name)).await
}

/// The owner answers question `n`. `stamp` is the Mac's local time, `YYYY-MM-DD HH:MM`.
#[tauri::command]
async fn answer(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    n: u32,
    text: String,
    stamp: String,
) -> Result<Option<String>, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || queue::answer(&queue_root(&file, &queue)?, &id, n, &text, "owner", &stamp))
        .await
}

pub(crate) fn check_stamp(stamp: &str) -> Result<(), String> {
    let ok = stamp.len() == 16
        && stamp.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == ' ',
            13 => c == ':',
            _ => c.is_ascii_digit(),
        });
    if ok {
        Ok(())
    } else {
        Err("bad time stamp".into())
    }
}

#[tauri::command]
async fn create_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    task: ops::NewTask,
    files: Vec<(String, String)>,
    stamp: String,
) -> Result<ops::Created, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        let root = queue_root(&file, &queue)?;
        let created = ops::create_task(&root, &task, "owner", &stamp)?;
        for (name, data_url) in files {
            ops::attach(
                &root,
                &created.id,
                &name,
                &ops::data_url_bytes(&data_url)?,
                "owner",
                &stamp,
            )?;
        }
        Ok(created)
    })
    .await
}

#[tauri::command]
async fn move_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    status: String,
    waiting_on: String,
    stamp: String,
) -> Result<String, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        ops::move_task(
            &queue_root(&file, &queue)?,
            &id,
            &status,
            &waiting_on,
            "owner",
            &stamp,
        )
    })
    .await
}

pub(crate) fn check_id(id: &str) -> Result<(), String> {
    if id.len() == 4 && id.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err("bad task id".into())
    }
}

/// The owner hands a task to the next stage (the gate line and commit only where they apply).
/// `from` is the stage the window showed: refused if the task has moved on since.
#[tauri::command]
async fn pass_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    from: String,
    gate: String,
    commit: String,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        let opts = ops::PassOpts {
            gate,
            commit,
            from: Some(from),
        };
        ops::pass_task(&queue_root(&file, &queue)?, &id, "owner", &stamp, &opts)
    })
    .await
}

/// The owner sends a task back one stage, or to the earlier stage `to`, with the reason.
#[tauri::command]
async fn return_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    why: String,
    to: Option<String>,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        ops::return_task(
            &queue_root(&file, &queue)?,
            &id,
            &why,
            to.as_deref(),
            "owner",
            &stamp,
        )
    })
    .await
}

#[tauri::command]
async fn block_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    on: String,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || ops::block_task(&queue_root(&file, &queue)?, &id, &on, "owner", &stamp)).await
}

#[tauri::command]
async fn unblock_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || ops::unblock_task(&queue_root(&file, &queue)?, &id, "owner", &stamp)).await
}

/// The owner releases a claim whose lease ran out (the card says "quiet 12 h+"): back to Ready,
/// logged as reclaimed. Refused if the holder was heard from again since the window looked.
#[tauri::command]
async fn reclaim_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || release(&queue_root(&file, &queue)?, &id, &stamp, queue::now_secs())).await
}

/// What `reclaim_task` does once it has the queue: its arguments checked, then the core's rule,
/// with the default lease (the one the card's "quiet 12 h+" shows).
pub(crate) fn release(
    root: &std::path::Path,
    id: &str,
    stamp: &str,
    now_secs: u64,
) -> Result<String, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    ops::reclaim_task(root, id, ops::LEASE_MINUTES, "owner", stamp, now_secs)
}

/// The owner's override: put a task in any stage. `from`, when given, is the stage the window
/// showed: refused if the task has moved on since.
#[tauri::command]
async fn set_stage(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stage: String,
    from: Option<String>,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        ops::set_stage(
            &queue_root(&file, &queue)?,
            &id,
            &stage,
            from.as_deref(),
            "owner",
            &stamp,
        )
    })
    .await
}

#[tauri::command]
async fn set_field(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    field: String,
    value: String,
    stamp: String,
) -> Result<String, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        ops::set_field(
            &queue_root(&file, &queue)?,
            &id,
            &field,
            &value,
            "owner",
            &stamp,
        )
    })
    .await
}

#[tauri::command]
async fn add_note(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    note: String,
    stamp: String,
) -> Result<String, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || ops::add_note(&queue_root(&file, &queue)?, &id, &note, "owner", &stamp)).await
}

/// Tick or untick one of the owner's criteria (0028), as the owner.
#[tauri::command]
async fn tick_criterion(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    which: String,
    undo: bool,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        ops::tick_criterion(
            &queue_root(&file, &queue)?,
            &id,
            &which,
            undo,
            "owner",
            &stamp,
        )
    })
    .await
}

/// The owner's quick add (0015): `text`, one line, into the queue's backlog as a note. Returns
/// the note's file name.
#[tauri::command]
async fn add_backlog_note(
    app: tauri::State<'_, AppState>,
    queue: String,
    text: String,
    stamp: String,
) -> Result<String, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || backlog::add(&queue_root(&file, &queue)?, &text, "", "owner", &stamp)).await
}

/// The owner files backlog note `note` (its file name) as a task in Shaping.
#[tauri::command]
async fn file_note(
    app: tauri::State<'_, AppState>,
    queue: String,
    note: String,
    task: ops::NewTask,
    stamp: String,
) -> Result<backlog::Filed, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || backlog::file_note(&queue_root(&file, &queue)?, &note, &task, "owner", &stamp))
        .await
}

/// The owner drops backlog note `note`, with the reason. Returns where it went.
#[tauri::command]
async fn drop_note(
    app: tauri::State<'_, AppState>,
    queue: String,
    note: String,
    why: String,
    stamp: String,
) -> Result<String, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || backlog::drop_note(&queue_root(&file, &queue)?, &note, &why, "owner", &stamp))
        .await
}

/// The shaping brief for backlog note `note`, as text for the clipboard.
#[tauri::command]
async fn shape_note(
    app: tauri::State<'_, AppState>,
    queue: String,
    note: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || {
        backlog::shape(&queue_root(&file, &queue)?, &note).map(|b| backlog::brief_text(&b))
    })
    .await
}

/// The write-up the owner's Edit opens: refused while someone else holds the task in Building.
#[tauri::command]
async fn editable_body(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
) -> Result<String, String> {
    check_id(&id)?;
    let file = app.config_file.clone();
    blocking(move || open_to_edit(&queue_root(&file, &queue)?, &id)).await
}

/// The owner saves an edited write-up. `expected` is the text Edit opened: refused if the file's
/// is different now. None when the text is unchanged (nothing written).
#[tauri::command]
async fn edit_body(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    body: String,
    expected: String,
    stamp: String,
) -> Result<Option<String>, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || save_body(&queue_root(&file, &queue)?, &id, &body, &expected, &stamp)).await
}

/// What `editable_body` does once it has the queue: its id checked, then the core's rule.
pub(crate) fn open_to_edit(root: &std::path::Path, id: &str) -> Result<String, String> {
    check_id(id)?;
    ops::open_body(root, id, "owner")
}

/// What `edit_body` does once it has the queue: its arguments checked, then the core's rules.
pub(crate) fn save_body(
    root: &std::path::Path,
    id: &str,
    body: &str,
    expected: &str,
    stamp: &str,
) -> Result<Option<String>, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    ops::edit_body(root, id, body, expected, "owner", stamp)
}

/// The owner comments on a task (0017): any stage, whoever holds it.
#[tauri::command]
async fn add_comment(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    text: String,
    stamp: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || comment(&queue_root(&file, &queue)?, &id, &text, &stamp)).await
}

/// What `add_comment` does once it has the queue: its arguments checked, then the core's rules.
pub(crate) fn comment(
    root: &std::path::Path,
    id: &str,
    text: &str,
    stamp: &str,
) -> Result<String, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    fields::add_comment(root, id, text, "owner", stamp)
}

/// Open one of a task's links in the system browser: only an address the task has, and only a
/// plain https one, run as one argument to `open` (no shell, no new capability).
#[tauri::command]
async fn open_link(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    url: String,
) -> Result<(), String> {
    let file = app.config_file.clone();
    blocking(move || {
        let url = link_to_open(&queue_root(&file, &queue)?, &id, &url)?;
        let ok = std::process::Command::new("/usr/bin/open")
            .arg(&url)
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if ok {
            Ok(())
        } else {
            Err("couldn't open the link in your browser".into())
        }
    })
    .await
}

/// What `open_link` opens once it has the queue: its id checked, then the core's rule.
pub(crate) fn link_to_open(root: &std::path::Path, id: &str, url: &str) -> Result<String, String> {
    check_id(id)?;
    fields::link_to_open(root, id, url)
}

/// Attach files the window holds (pasted or dropped), as data URLs.
#[tauri::command]
async fn attach_data(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    files: Vec<(String, String)>,
    stamp: String,
) -> Result<Vec<String>, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        let root = queue_root(&file, &queue)?;
        files
            .iter()
            .map(|(name, url)| {
                ops::attach(
                    &root,
                    &id,
                    name,
                    &ops::data_url_bytes(url)?,
                    "owner",
                    &stamp,
                )
            })
            .collect()
    })
    .await
}

/// Attach files picked with the core's own dialog.
#[tauri::command]
async fn attach_pick(
    handle: tauri::AppHandle,
    queue: String,
    id: String,
    stamp: String,
) -> Result<Vec<String>, String> {
    check_stamp(&stamp)?;
    let h = handle.clone();
    let picked = blocking(move || {
        Ok(h.dialog()
            .file()
            .set_title("Attach files")
            .blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .collect::<Vec<_>>())
    })
    .await?;
    blocking(move || {
        let app = handle.state::<AppState>();
        let root = queue_root(&app.config_file, &queue)?;
        let mut saved = Vec::new();
        for p in picked {
            let meta = std::fs::metadata(&p).map_err(|e| e.to_string())?;
            if !meta.is_file() {
                continue;
            }
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            saved.push(ops::attach(&root, &id, &name, &bytes, "owner", &stamp)?);
        }
        Ok(saved)
    })
    .await
}

#[tauri::command]
fn help() -> help::Help {
    help::help()
}

#[tauri::command]
async fn projects(app: tauri::State<'_, AppState>) -> Result<Vec<projects::Project>, String> {
    let file = app.config_file.clone();
    blocking(move || Ok(projects::list(&base(), &projects::load(&file)))).await
}

/// Change projects.json under the lock: load, edit, save.
fn edit_config<T>(
    app: &AppState,
    f: impl FnOnce(&mut projects::Config) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = app.config.lock().map_err(|e| e.to_string())?;
    let mut c = projects::load(&app.config_file);
    let out = f(&mut c)?;
    projects::save(&app.config_file, &c)?;
    Ok(out)
}

fn known_key(c: &projects::Config, key: &str) -> Result<(), String> {
    if projects::list(&base(), c).iter().any(|p| p.key == key) {
        Ok(())
    } else {
        Err("no such project".into())
    }
}

/// Pick a folder with the core's dialog. None when the owner cancels.
async fn pick_folder(
    handle: tauri::AppHandle,
    title: &'static str,
) -> Result<Option<PathBuf>, String> {
    blocking(move || {
        Ok(handle
            .dialog()
            .file()
            .set_title(title)
            .set_directory(base())
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok()))
    })
    .await
}

#[tauri::command]
async fn add_project(handle: tauri::AppHandle) -> Result<Option<String>, String> {
    let Some(dir) = pick_folder(handle.clone(), "Choose a Muster queue folder").await? else {
        return Ok(None);
    };
    let h = handle.clone();
    blocking(move || {
        let app = h.state::<AppState>();
        edit_config(&app, |c| projects::add(c, &base(), &dir)).map(Some)
    })
    .await
}

#[tauri::command]
async fn create_project(handle: tauri::AppHandle) -> Result<Option<String>, String> {
    let Some(dir) = pick_folder(
        handle.clone(),
        "Choose the project folder (its tasks/ goes inside)",
    )
    .await?
    else {
        return Ok(None);
    };
    let h = handle.clone();
    blocking(move || {
        let q = projects::create(&dir, &tools())?;
        let app = h.state::<AppState>();
        edit_config(&app, |c| projects::add(c, &base(), &q)).map(Some)
    })
    .await
}

#[tauri::command]
async fn rename_project(handle: tauri::AppHandle, key: String, name: String) -> Result<(), String> {
    let name: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            known_key(c, &key)?;
            if name.is_empty() {
                c.names.remove(&key);
            } else {
                c.names.insert(key, name);
            }
            Ok(())
        })
    })
    .await
}

#[tauri::command]
async fn hide_project(handle: tauri::AppHandle, key: String, hidden: bool) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            known_key(c, &key)?;
            c.hidden.retain(|h| h != &key);
            if hidden {
                c.hidden.push(key);
            }
            Ok(())
        })
    })
    .await
}

/// Take an added project off the list. Its files are never touched.
#[tauri::command]
async fn remove_project(handle: tauri::AppHandle, key: String) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            if !c.added.contains(&key) {
                return Err("only added projects can be removed; hide a found one instead".into());
            }
            c.added.retain(|a| a != &key);
            c.names.remove(&key);
            c.hidden.retain(|h| h != &key);
            Ok(())
        })
    })
    .await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(AppState {
                cache: Mutex::new(queue::Cache::default()),
                helpers: Mutex::new(HashMap::new()),
                config: Mutex::new(()),
                config_file: dir.join("projects.json"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            help,
            task_body,
            image,
            answer,
            projects,
            add_project,
            create_project,
            rename_project,
            hide_project,
            remove_project,
            create_task,
            move_task,
            pass_task,
            return_task,
            block_task,
            unblock_task,
            reclaim_task,
            set_stage,
            set_field,
            add_note,
            tick_criterion,
            add_backlog_note,
            file_note,
            drop_note,
            shape_note,
            editable_body,
            edit_body,
            add_comment,
            open_link,
            attach_data,
            attach_pick,
            helper_cmds::helper_info,
            helper_cmds::helper_set_account,
            helper_cmds::helper_reset,
            helper_cmds::helper_stop,
            helper_cmds::helper_send,
            helper_cmds::helper_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Muster");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_arguments_are_checked() {
        assert!(check_id("0007").is_ok());
        for bad in ["7", "00007", "../x", "abcd", "٠١٢٣", ""] {
            assert!(check_id(bad).is_err(), "{bad:?}");
        }
        assert!(check_stamp("2026-10-12 14:05").is_ok());
        for bad in [
            "2026-10-12",
            "2026-10-12T14:05",
            "2026-10-12 14:5x",
            "２026-10-12 14:05",
        ] {
            assert!(check_stamp(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_owners_edit_checks_its_arguments_then_the_rules() {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        let file = d.path().join("todo/0007-t.md");
        std::fs::write(
            &file,
            "---\nid: 0007\nstage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:\n---\n\n# T\n\nOld.\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        let t = "2026-10-12 14:05";
        let opened = open_to_edit(d.path(), "0007").unwrap();
        assert_eq!(opened, "# T\n\nOld.");
        assert_eq!(open_to_edit(d.path(), "../7").unwrap_err(), "bad task id");
        assert_eq!(
            save_body(d.path(), "7", "# T\n\nNew.", &opened, t).unwrap_err(),
            "bad task id"
        );
        assert_eq!(
            save_body(d.path(), "0007", "# T\n\nNew.", &opened, "2026-10-12").unwrap_err(),
            "bad time stamp"
        );
        assert_eq!(
            save_body(d.path(), "0007", "# T\n\nNew.", &opened, t).unwrap(),
            Some("todo/0007-t.md".to_string())
        );
        assert!(std::fs::read_to_string(&file)
            .unwrap()
            .ends_with("- 2026-10-12 14:05 owner: edited the write-up\n"));
        assert!(save_body(d.path(), "0007", "# T\n\nAgain.", &opened, t)
            .unwrap_err()
            .contains("changed since you opened it"));
    }

    #[test]
    fn the_owners_comment_and_link_check_their_arguments_then_the_rules() {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        // Held by someone else: the owner may still comment.
        std::fs::write(
            d.path().join("doing/0007-t.md"),
            "---\nid: 0007\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat:\nwaiting_on:\nlinks: Spec <https://example.com/spec>; Old <http://example.com>\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        let t = "2026-10-12 14:05";
        assert_eq!(comment(d.path(), "7", "Hi.", t).unwrap_err(), "bad task id");
        assert_eq!(
            comment(d.path(), "0007", "Hi.", "2026-10-12").unwrap_err(),
            "bad time stamp"
        );
        assert!(comment(d.path(), "0007", " ", t)
            .unwrap_err()
            .contains("empty"));
        assert_eq!(
            comment(d.path(), "0007", "Hi.", t).unwrap(),
            "doing/0007-t.md"
        );
        let task = queue::read_task(d.path(), "0007").unwrap();
        assert_eq!(
            (
                task.comments[0].who.as_str(),
                task.comments[0].text.as_str()
            ),
            ("owner", "Hi.")
        );
        assert_eq!(
            link_to_open(d.path(), "0007", "https://example.com/spec").unwrap(),
            "https://example.com/spec"
        );
        assert_eq!(
            link_to_open(d.path(), "../7", "https://example.com/spec").unwrap_err(),
            "bad task id"
        );
        assert!(link_to_open(d.path(), "0007", "http://example.com")
            .unwrap_err()
            .contains("only https"));
        assert!(link_to_open(d.path(), "0007", "https://evil.example")
            .unwrap_err()
            .contains("no link"));
    }

    #[test]
    fn release_checks_its_arguments_then_the_lease() {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        std::fs::write(
            d.path().join("doing/0007-t.md"),
            "---\nid: 0007\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 10:00\nwaiting_on:\n---\n\n# T\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
        )
        .unwrap();
        let now = queue::now_secs();
        assert_eq!(
            release(d.path(), "7", "2026-10-13 10:00", now).unwrap_err(),
            "bad task id"
        );
        assert_eq!(
            release(d.path(), "0007", "2026-10-13T10:00", now).unwrap_err(),
            "bad time stamp"
        );
        assert!(release(d.path(), "0007", "2026-10-12 21:59", now)
            .unwrap_err()
            .contains("lease hasn't run out"));
        assert_eq!(
            release(d.path(), "0007", "2026-10-12 22:01", now).unwrap(),
            "todo/0007-t.md"
        );
        let t = queue::read_task(d.path(), "0007").unwrap();
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("ready", ""));
        assert!(t.log_tail.last().unwrap().ends_with(
            "owner: stage building → ready (reclaimed from wt-a (no heartbeat for 12 h))"
        ));
    }
}
