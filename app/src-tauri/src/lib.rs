//! Muster's desktop app: thin async commands over `queue`. Disk and git work runs in
//! `spawn_blocking`. The window names a queue by its project name, never by a path, so it can
//! only reach the queues Muster discovered.

pub mod queue;

use std::path::PathBuf;
use std::sync::Mutex;

struct AppState {
    cache: Mutex<queue::Cache>,
}

fn base() -> PathBuf {
    std::env::var_os("MUSTER_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("playground")
        })
}

fn queue_root(name: &str) -> Result<PathBuf, String> {
    queue::discover(&base())
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
    let queues = blocking(|| Ok(queue::discover(&base()))).await?;
    // The cache is small and the read is quick on a warm cache; take it inside the blocking task.
    let cache = std::mem::take(&mut *app.cache.lock().map_err(|e| e.to_string())?);
    let (state, cache) = blocking(move || {
        let mut cache = cache;
        let s = queue::read_state(&queues, &mut cache);
        Ok((s, cache))
    })
    .await?;
    *app.cache.lock().map_err(|e| e.to_string())? = cache;
    Ok(state)
}

#[tauri::command]
async fn task_body(queue: String, id: String) -> Result<String, String> {
    blocking(move || queue::read_body(&queue_root(&queue)?, &id)).await
}

#[tauri::command]
async fn image(queue: String, id: String, name: String) -> Result<String, String> {
    blocking(move || queue::read_image(&queue_root(&queue)?, &id, &name)).await
}

/// The owner answers question `n`. `stamp` is the Mac's local time, `YYYY-MM-DD HH:MM`.
#[tauri::command]
async fn answer(
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
    blocking(move || queue::answer(&queue_root(&queue)?, &id, n, &text, "owner", &stamp)).await
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            cache: Mutex::new(queue::Cache::default()),
        })
        .invoke_handler(tauri::generate_handler![state, task_body, image, answer])
        .run(tauri::generate_context!())
        .expect("error while running Muster");
}
