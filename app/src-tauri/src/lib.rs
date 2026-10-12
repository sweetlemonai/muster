//! Muster's desktop app: thin async commands over `queue` and `projects`. Disk and git work runs
//! in `spawn_blocking`. The window names a queue by its project name, and a project by the key
//! the core gave it, never by a path it made up. Folders are picked by the core's own dialog.

pub mod projects;
pub mod queue;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

struct AppState {
    cache: Mutex<queue::Cache>,
    /// Serializes changes to projects.json.
    config: Mutex<()>,
    config_file: PathBuf,
}

fn base() -> PathBuf {
    std::env::var_os("MUSTER_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground"))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Muster's tools, which a new queue's stand-ins run.
fn tools() -> PathBuf {
    std::env::var_os("MUSTER_TOOLS")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("playground/muster/main/bin"))
}

fn queues(config_file: &std::path::Path) -> Vec<(String, PathBuf)> {
    projects::active(&base(), &projects::load(config_file))
}

fn queue_root(config_file: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    queues(config_file)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, p)| p)
        .ok_or_else(|| format!("no queue named {name:?}"))
}

async fn blocking<T: Send + 'static>(
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
    let ok = stamp.len() == 16
        && stamp.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == ' ',
            13 => c == ':',
            _ => c.is_ascii_digit(),
        });
    if !ok {
        return Err("bad time stamp".into());
    }
    let file = app.config_file.clone();
    blocking(move || queue::answer(&queue_root(&file, &queue)?, &id, n, &text, "owner", &stamp))
        .await
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
                config: Mutex::new(()),
                config_file: dir.join("projects.json"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            task_body,
            image,
            answer,
            projects,
            add_project,
            create_project,
            rename_project,
            hide_project,
            remove_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running Muster");
}
