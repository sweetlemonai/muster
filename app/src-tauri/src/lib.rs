//! Muster's desktop app: thin async commands over `queue` and `projects`. Disk and git work runs
//! in `spawn_blocking`. The window names a queue by its project name, and a project by the key
//! the core gave it, never by a path it made up. Folders are picked by the core's own dialog.

pub mod accounts;
pub mod agents;
pub mod arrange;
pub mod backlog;
pub mod badge;
pub mod block;
pub mod check_limit;
pub mod choices;
pub mod clarify;
pub mod cli;
pub mod fields;
pub mod help;
pub mod helper;
mod helper_cmds;
pub mod history;
pub mod index;
pub mod key;
pub mod labels;
pub mod layout;
pub mod lenses;
pub mod lessons;
pub mod merge_log;
pub mod migrate;
pub mod notify;
pub mod now;
pub mod old_pause;
pub mod ops;
pub mod parallel;
pub mod pause;
mod pause_cmds;
pub mod ping;
mod ping_cmds;
pub mod projects;
pub mod queue;
pub mod review;
mod review_cmds;
pub mod roles;
pub mod run;
pub mod search;
pub mod session;
pub mod settings;
pub mod transcripts;
pub mod tray;
pub mod vision;
pub mod work;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub(crate) struct AppState {
    cache: Mutex<queue::Cache>,
    /// Running helper turns, by queue name.
    pub(crate) helpers: Mutex<HashMap<String, std::process::Child>>,
    /// Serializes changes to projects.json and settings.json.
    config: Mutex<()>,
    pub(crate) config_file: PathBuf,
    /// This machine's settings (0036), beside projects.json; `muster` reads the same file.
    settings_file: PathBuf,
    /// The role profiles (0112), beside projects.json; `muster team` reads the same file.
    roles_file: PathBuf,
    /// Session transcripts under the home folder (0025), read and cached off the main thread.
    transcripts: Arc<transcripts::Reader>,
    /// Claude Code's session states (0048), read off the main thread at most every 15 s.
    agents: Arc<agents::Monitor>,
    /// The Dock badge's count and the window's last poll (0006).
    badge: badge::Badge,
    /// What the Mac notifications have seen, and the project the window shows (0052).
    notify: notify::Notifier,
    /// Each project's latest run from the app (0079), in memory only.
    runs: run::Runs,
    /// The menu bar icon's latest count and what it shows (0085), fed by the same reads.
    menu_bar: tray::MenuBar,
}

/// The folder queues are found under, from `projects::places`.
fn base() -> PathBuf {
    projects::places().base
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

/// Run `f` on the queue named `name`; what the core refuses with names tasks by their keys
/// (0033), as the command line says them.
pub(crate) fn in_queue<T>(
    config_file: &std::path::Path,
    name: &str,
    f: impl FnOnce(&std::path::Path) -> Result<T, String>,
) -> Result<T, String> {
    let root = queue_root(config_file, name)?;
    f(&root).map_err(|e| key::name_tasks(&root, &e, &[]))
}

pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn state(
    handle: tauri::AppHandle,
    app: tauri::State<'_, AppState>,
) -> Result<queue::State, String> {
    // The window is visible while it polls, and its polls keep the Dock badge current.
    app.badge.polled(std::time::Instant::now());
    let file = app.config_file.clone();
    let settings_file = app.settings_file.clone();
    let roles_file = app.roles_file.clone();
    // The cache is small and the read is quick on a warm cache; take it inside the blocking task.
    let cache = std::mem::take(&mut *app.cache.lock().map_err(|e| e.to_string())?);
    let reader = app.transcripts.clone();
    let monitor = app.agents.clone();
    let h = handle.clone();
    let (state, cache, menu_bar) = blocking(move || {
        let mut cache = cache;
        // Read every poll, so a change in Settings (or by hand) applies at the next one.
        let settings = settings::load(&settings_file);
        let config = projects::load(&file);
        let list = projects::list(&base(), &config);
        // The queues with their keys, limits, labels, headings and places (0073: one builder,
        // which the DashAfzar fixture's test calls too).
        let mut s = projects::read_state(&config, &list, &settings, &mut cache);
        // The accounts Muster reads (0099), from the settings like the LIVE window.
        let allowed = accounts::allowed(&settings, &home());
        // The roles' names (0112), read again only when roles.toml changes, so a rename in
        // Settings or by hand shows at the next poll.
        s.role_names = roles::load(&roles_file)
            .map(|r| roles::names(&r))
            .unwrap_or_default();
        // The cards' live dot: file times only, about a millisecond for 50 sessions.
        reader.set_live_secs(settings.live_secs());
        work::mark_live(&mut s.tasks, &reader, &allowed, s.now);
        // Claude Code's word on each holder's session (0048): the latest read, never waited on.
        let sessions = monitor.poll(&s.tasks, &allowed, s.now);
        agents::apply(&mut s.tasks, &sessions, &allowed);
        notify_new(&h, &s, &sessions);
        // The sidebar's "running" chip (0079).
        let running = h.state::<AppState>().runs.running();
        for q in &mut s.queues {
            q.running = running.contains(&q.path);
        }
        Ok((s, cache, settings.menu_bar))
    })
    .await?;
    *app.cache.lock().map_err(|e| e.to_string())? = cache;
    let n = badge::dock_count(&state.tasks, &state.queues);
    show_badge(&handle, &app.badge, n);
    app.menu_bar.sync(menu_bar, Some(n), &tray::Tray(&handle));
    Ok(state)
}

/// Puts `n` on the Dock icon, or takes the badge off at 0, unless it already shows that.
fn show_badge(handle: &tauri::AppHandle, badge: &badge::Badge, n: usize) {
    badge.show(n, |label| {
        handle
            .get_webview_window("main")
            .is_some_and(|w| w.set_badge_count(label).is_ok())
    });
}

/// A Mac notification of what's new for the owner in `s` (0052), whose sessions `sessions` told,
/// unless Settings turned them off (`notify::after_read`).
fn notify_new(handle: &tauri::AppHandle, s: &queue::State, sessions: &agents::Snapshot) {
    notify::after_read(
        &handle.state::<AppState>().notify,
        s,
        sessions,
        &notify::Mac(handle),
    );
}

/// The shown projects' state, read as the window's poll reads it (with its cache, the tasks' keys,
/// the projects' labels and Claude Code's session states), for the Dock badge and the
/// notifications while the window doesn't poll; with the session states it applied.
fn waiting_now(app: &AppState) -> Result<(queue::State, agents::Snapshot), String> {
    waiting_in(
        &base(),
        &home(),
        &app.config_file,
        &app.settings_file,
        &app.cache,
        &app.agents,
    )
}

/// `waiting_now` for the projects under `base`, reading the Claude accounts under `home`.
fn waiting_in(
    base: &std::path::Path,
    home: &std::path::Path,
    config_file: &std::path::Path,
    settings_file: &std::path::Path,
    cache: &Mutex<queue::Cache>,
    monitor: &Arc<agents::Monitor>,
) -> Result<(queue::State, agents::Snapshot), String> {
    let settings = settings::load(settings_file);
    let list = projects::list(base, &projects::load(config_file));
    let prefix_of = |name: &str| {
        list.iter()
            .find(|p| p.name == name)
            .map(|p| p.prefix.clone())
            .unwrap_or_default()
    };
    let mut taken = std::mem::take(&mut *cache.lock().map_err(|e| e.to_string())?);
    let mut s =
        queue::read_state_settled(&projects::shown(&list), &mut taken, &settings, &prefix_of);
    *cache.lock().map_err(|e| e.to_string())? = taken;
    labels::apply(&list, &mut s.queues);
    // As the poll does (0048): a session waiting for input waits on the owner. The monitor reads
    // `claude agents` at most every 15 s, on its own thread, and never holds this up.
    let allowed = accounts::allowed(&settings, home);
    let sessions = monitor.poll(&s.tasks, &allowed, s.now);
    agents::apply(&mut s.tasks, &sessions, &allowed);
    Ok((s, sessions))
}

/// Keeps the Dock badge and the menu bar icon (0085) current while the window is hidden and
/// doesn't poll (0006): every `badge::HIDDEN_EVERY` with no poll in between, it reads the queues
/// itself, and notifies what's new (0052). A read that fails leaves the badge as it was until the next one. Runs as long as
/// the app does.
fn watch_badge(handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(badge::HIDDEN_EVERY);
        let app = handle.state::<AppState>();
        if app.badge.due(std::time::Instant::now()) {
            if let Ok((s, sessions)) = waiting_now(&app) {
                let n = badge::dock_count(&s.tasks, &s.queues);
                show_badge(&handle, &app.badge, n);
                let on = settings::load(&app.settings_file).menu_bar;
                app.menu_bar.sync(on, Some(n), &tray::Tray(&handle));
                notify_new(&handle, &s, &sessions);
            }
        }
    });
}

/// The queues a search index covers: the one named (checked like every command's), or every
/// queue for none.
fn index_scope(
    config_file: &std::path::Path,
    queue: Option<&str>,
) -> Result<Vec<(String, PathBuf)>, String> {
    match queue {
        Some(name) => Ok(vec![(name.to_string(), queue_root(config_file, name)?)]),
        None => Ok(queues(config_file)),
    }
}

/// Each task's search text by `queue/id`, for one queue or every queue (`null`): the window asks
/// only while its search has words, and again only when a task changed (task 0032), so the 2 s
/// `state` poll doesn't carry it.
#[tauri::command]
async fn search_index(
    app: tauri::State<'_, AppState>,
    queue: Option<String>,
) -> Result<HashMap<String, String>, String> {
    let file = app.config_file.clone();
    // Read with the cache taken out, as `state` does; put back with `merge`, so a `state` that
    // ran meanwhile (and found it taken) keeps its fuller cache.
    let cache = std::mem::take(&mut *app.cache.lock().map_err(|e| e.to_string())?);
    let (index, cache) = blocking(move || {
        let mut cache = cache;
        let index = index_scope(&file, queue.as_deref())
            .map(|scope| queue::search_index(&scope, &mut cache));
        Ok((index, cache))
    })
    .await?;
    app.cache.lock().map_err(|e| e.to_string())?.merge(cache);
    index
}

#[tauri::command]
async fn task_body(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| queue::read_body(root, &id))).await
}

#[tauri::command]
async fn image(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    name: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| queue::read_image(root, &id, &name))).await
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
    blocking(move || {
        in_queue(&file, &queue, |root| {
            queue::answer(root, &id, n, &text, "owner", &stamp)
        })
    })
    .await
}

/// The owner answers question `n` by picking one of its options (0049): `pick` is its letter,
/// `extra` any words to add after it. Returns the asking session, if one was found.
#[tauri::command]
async fn answer_pick(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    n: u32,
    pick: String,
    extra: String,
    stamp: String,
) -> Result<Option<String>, String> {
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let root = root.to_path_buf();
            queue::answer_pick(&root, &id, n, &pick, &extra, "owner", &stamp)
                .map(|(asker, _)| asker)
        })
    })
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
        in_queue(&file, &queue, |root| {
            let root = root.to_path_buf();
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
        in_queue(&file, &queue, |root| {
            ops::move_task(root, &id, &status, &waiting_on, "owner", &stamp)
        })
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

/// What the window's Pass sends besides the task: `from`, the stage it showed (refused if the task
/// has moved on since), and the gate line and summary (Building → Testing, mu-78) and the commit
/// (Your check → Done), empty where they don't apply.
#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PassFields {
    from: String,
    #[serde(default)]
    gate: String,
    #[serde(default)]
    commit: String,
    #[serde(default)]
    summary: String,
}

/// The owner hands a task to the next stage.
#[tauri::command]
async fn pass_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    pass: PassFields,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let opts = ops::PassOpts {
                gate: pass.gate,
                commit: pass.commit,
                summary: pass.summary,
                from: Some(pass.from),
                now_unchanged: String::new(),
            };
            ops::pass_task(root, &id, "owner", &stamp, &opts)
        })
    })
    .await
}

/// What Accept all did in one queue (0043): the ids it accepted, in order, and each it refused
/// with the core's reason.
#[derive(serde::Serialize, Debug, Default, PartialEq)]
pub(crate) struct AcceptAll {
    pub accepted: Vec<String>,
    pub refused: Vec<(String, String)>,
}

/// Accept all (0043): the owner accepts each task the window listed under "Just accept", as
/// Accept does (Done, with the commit the merge recorded), one after the other.
#[tauri::command]
async fn accept_all(
    app: tauri::State<'_, AppState>,
    queue: String,
    ids: Vec<String>,
    stamp: String,
) -> Result<AcceptAll, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| accept_all_in(root, &ids, &stamp))).await
}

/// What `accept_all` does once it has the queue: every argument checked before anything is
/// touched, then each task passed from Your check as the owner, as Accept does. The core's rules
/// decide: one of the owner's criteria still open, a block, or a task that has left Your check
/// is refused with the reason and the rest go on. Each accepted task's log gets its stage line.
pub(crate) fn accept_all_in(
    root: &std::path::Path,
    ids: &[String],
    stamp: &str,
) -> Result<AcceptAll, String> {
    for id in ids {
        check_id(id)?;
    }
    check_stamp(stamp)?;
    let opts = ops::PassOpts {
        from: Some("your-check".into()),
        ..ops::PassOpts::default()
    };
    let mut out = AcceptAll::default();
    for id in ids {
        if let Some(why) = queue::read_task(root, id)
            .ok()
            .and_then(|t| not_just_accept(&t))
        {
            out.refused.push((id.clone(), why));
            continue;
        }
        match ops::pass_task(root, id, "owner", stamp, &opts) {
            Ok(_) => out.accepted.push(id.clone()),
            Err(why) => out.refused.push((id.clone(), why)),
        }
    }
    Ok(out)
}

/// Why Accept all leaves a task alone that a single Accept would take: it's a goal or a feature
/// (Accept all is for tasks), an agent's criterion isn't ticked (nothing proves it, mu-78: the
/// window files it under One look, not Just accept), or a question escalated to the owner is still
/// open. Read just before
/// the pass; the pass itself rechecks the stage, the owner's criteria and blocks under the lock. A
/// task that can't be read goes on to the pass, which says why.
fn not_just_accept(t: &queue::Task) -> Option<String> {
    if t.level != "task" {
        return Some(format!(
            "{} is a {}; Accept all takes tasks only",
            t.id, t.level
        ));
    }
    let unproven: Vec<String> = t
        .criteria
        .iter()
        .filter(|c| !c.owner && !c.ticked)
        .map(|c| {
            if c.label.is_empty() {
                format!("criterion {}", c.n)
            } else {
                c.label.clone()
            }
        })
        .collect();
    if !unproven.is_empty() {
        let (what, them) = if unproven.len() == 1 {
            ("a criterion", "it")
        } else {
            ("criteria", "them")
        };
        return Some(format!(
            "{} has {what} nobody has proven ({}); send it back, or check {them} yourself and accept it on its own",
            t.id,
            unproven.join(", ")
        ));
    }
    let open: Vec<String> = t
        .questions
        .iter()
        .filter(|q| !q.ticked && q.escalated)
        .map(|q| q.n.map_or_else(|| "Q?".to_string(), |n| format!("Q{n}")))
        .collect();
    (!open.is_empty()).then(|| {
        format!(
            "{} has {} for the owner ({}); answer {} first",
            t.id,
            if open.len() == 1 {
                "a question"
            } else {
                "questions"
            },
            open.join(", "),
            if open.len() == 1 { "it" } else { "them" }
        )
    })
}

/// The owner sends a task back one stage, or to the earlier stage `to`, with the reason. Returns
/// what the core did (mu-101): where it went, the owner finding it recorded, whether the builder
/// was told, so the pane words it from that.
#[tauri::command]
async fn return_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    why: String,
    to: Option<String>,
    stamp: String,
) -> Result<ops::Returned, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            ops::return_back(root, &id, &why, to.as_deref(), "owner", &stamp)
        })
    })
    .await
}

/// The owner blocks a task (0024): `kind` is tasks, you, permission, question or outside, and
/// `detail` what it waits on (task ids, a question's number, or words). `ops::block_as` checks both.
#[tauri::command]
async fn block_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    kind: String,
    detail: String,
    stamp: String,
) -> Result<String, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            ops::block_as(root, &id, &kind, &detail, "owner", &stamp)
        })
    })
    .await
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
    blocking(move || {
        in_queue(&file, &queue, |root| {
            ops::unblock_task(root, &id, "owner", &stamp)
        })
    })
    .await
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
    let settings_file = app.settings_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let lease = settings::load(&settings_file).lease_minutes;
            release(root, &id, &stamp, queue::now_secs(), lease)
        })
    })
    .await
}

/// What `reclaim_task` does once it has the queue: its arguments checked, then the core's rule,
/// with this machine's lease of `lease` minutes (the one the card's "quiet 12 h+" shows).
pub(crate) fn release(
    root: &std::path::Path,
    id: &str,
    stamp: &str,
    now_secs: u64,
    lease: u64,
) -> Result<String, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    ops::reclaim_task(root, id, lease, "owner", stamp, now_secs)
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
        in_queue(&file, &queue, |root| {
            ops::set_stage(root, &id, &stage, from.as_deref(), "owner", &stamp)
        })
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
        in_queue(&file, &queue, |root| {
            ops::set_field(root, &id, &field, &value, "owner", &stamp)
        })
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
    blocking(move || {
        in_queue(&file, &queue, |root| {
            ops::add_note(root, &id, &note, "owner", &stamp)
        })
    })
    .await
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
        in_queue(&file, &queue, |root| {
            ops::tick_criterion(root, &id, &which, undo, "owner", &stamp)
        })
    })
    .await
}

/// Who worked on a task and what it cost (0025): its sessions with tokens, times and live, the
/// totals against the estimates, and a progress guess. Transcripts are read on this Mac only.
#[tauri::command]
async fn task_sessions(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
) -> Result<work::Work, String> {
    check_id(&id)?;
    let file = app.config_file.clone();
    let settings_file = app.settings_file.clone();
    let reader = app.transcripts.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let settings = settings::load(&settings_file);
            reader.set_live_secs(settings.live_secs());
            let allowed = accounts::allowed(&settings, &home());
            work::for_task(&reader, &allowed, root, &id, queue::now_secs())
        })
    })
    .await
}

/// One of a task's sessions' transcripts, folded to Markdown for the task pane (0025). Only a
/// session the task names, stamped with an account Muster reads (0099, `work::transcript`).
#[tauri::command]
async fn session_transcript(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    session: String,
) -> Result<String, String> {
    check_id(&id)?;
    let file = app.config_file.clone();
    let settings_file = app.settings_file.clone();
    let reader = app.transcripts.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let allowed = accounts::allowed(&settings::load(&settings_file), &home());
            work::transcript(&reader, &allowed, root, &id, &session)
        })
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
    blocking(move || {
        in_queue(&file, &queue, |root| {
            backlog::add(root, &text, "", "owner", &stamp)
        })
    })
    .await
}

/// The owner files backlog note `note` (its file name) as a task in Shaping; a note a session is
/// shaping (0097) is taken over, logged so.
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
    let settings_file = app.settings_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let lease = settings::load(&settings_file).lease_minutes;
            backlog::file_note(root, &note, &task, "owner", &stamp, lease)
        })
    })
    .await
}

/// The owner drops backlog note `note`, with the reason; a note a session is shaping (0097) is
/// taken over, said so. Returns where it went.
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
    let settings_file = app.settings_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            let lease = settings::load(&settings_file).lease_minutes;
            backlog::drop_note(root, &note, &why, "owner", &stamp, lease)
        })
    })
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
        in_queue(&file, &queue, |root| {
            backlog::shape(root, &note).map(|b| backlog::brief_text(&b))
        })
    })
    .await
}

/// The vision brief for task `id` (0029), as text for the clipboard.
#[tauri::command]
async fn vision_brief(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
) -> Result<String, String> {
    check_id(&id)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            vision::brief(root, &id).map(|b| vision::brief_text(&b))
        })
    })
    .await
}

/// The owner resolves vision conflict `which` (V2) of task `id`, saying how. Returns how many are
/// still open.
#[tauri::command]
async fn resolve_vision(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    which: String,
    how: String,
    stamp: String,
) -> Result<usize, String> {
    check_id(&id)?;
    check_stamp(&stamp)?;
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            vision::resolve(root, &id, &which, &how, "owner", &stamp)
        })
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
    blocking(move || in_queue(&file, &queue, |root| open_to_edit(root, &id))).await
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
    blocking(move || {
        in_queue(&file, &queue, |root| {
            save_body(root, &id, &body, &expected, &stamp)
        })
    })
    .await
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

/// The owner comments on a task (0017): any stage, whoever holds it. A comment that starts with
/// an @verb acts first (mu-101, `fields::comment`): what it did comes back in `did`.
#[tauri::command]
async fn add_comment(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    text: String,
    stamp: String,
) -> Result<fields::Commented, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| comment(root, &id, &text, &stamp))).await
}

/// What `add_comment` does once it has the queue: its arguments checked, then the core's rules.
pub(crate) fn comment(
    root: &std::path::Path,
    id: &str,
    text: &str,
    stamp: &str,
) -> Result<fields::Commented, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    fields::comment(root, id, text, "owner", stamp)
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
        in_queue(&file, &queue, |root| {
            let url = link_to_open(root, &id, &url)?;
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
        in_queue(&file, &queue, |root| {
            let root = root.to_path_buf();
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
        in_queue(&app.config_file, &queue, |root| {
            let root = root.to_path_buf();
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

/// A local queue's history (0109) for Settings › Projects, by project key: only the projects
/// whose queue keeps one. Off the window's thread: each reads its history's git.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectHistory {
    key: String,
    history: history::Status,
}

#[tauri::command]
async fn project_histories(app: tauri::State<'_, AppState>) -> Result<Vec<ProjectHistory>, String> {
    let file = app.config_file.clone();
    blocking(move || {
        Ok(projects::list(&base(), &projects::load(&file))
            .into_iter()
            .filter_map(|p| {
                let history = history::status(std::path::Path::new(&p.path));
                history.kept.then_some(ProjectHistory {
                    key: p.key,
                    history,
                })
            })
            .collect())
    })
    .await
}

/// The queue folder of the project Muster lists under `key`; the window never names a folder
/// itself.
fn project_queue(file: &std::path::Path, key: &str) -> Result<PathBuf, String> {
    projects::list(&base(), &projects::load(file))
        .into_iter()
        .find(|p| p.key == key)
        .map(|p| PathBuf::from(p.path))
        .ok_or_else(|| "that project isn't one Muster lists".to_string())
}

/// Settings › Projects' Restore (0109): `muster restore --all` for the project `key`, by the
/// owner. Returns how many files came back.
#[tauri::command]
async fn restore_history(app: tauri::State<'_, AppState>, key: String) -> Result<usize, String> {
    let file = app.config_file.clone();
    blocking(move || {
        let root = project_queue(&file, &key)?;
        history::restore(&root, &[], true, "owner").map(|v| v.len())
    })
    .await
}

/// Settings › Projects' Commit (0109): `muster history commit` for the project `key`. Returns
/// how many files it committed.
#[tauri::command]
async fn commit_history(app: tauri::State<'_, AppState>, key: String) -> Result<usize, String> {
    let file = app.config_file.clone();
    blocking(move || {
        let root = project_queue(&file, &key)?;
        history::commit_outside(&root).map(|v| v.len())
    })
    .await
}

/// Settings › Projects' Keep deleted (0109): `muster history removed --all` for the project
/// `key`, by the owner: files deleted on purpose are recorded as removed. Returns how many.
#[tauri::command]
async fn record_removed(app: tauri::State<'_, AppState>, key: String) -> Result<usize, String> {
    let file = app.config_file.clone();
    blocking(move || {
        let root = project_queue(&file, &key)?;
        history::removed(&root, &[], true, "owner").map(|v| v.len())
    })
    .await
}

/// What Settings shows (0036): this machine's settings with their bounds, and for About the
/// version, the config folder and the folders the app reads.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsInfo {
    settings: settings::Settings,
    /// The lease's and the LIVE window's bounds, in minutes: [least, most].
    lease_bounds: [u64; 2],
    live_bounds: [u64; 2],
    version: String,
    /// The home folder: the window shows paths under it as `~/…`.
    home: String,
    /// The app's config folder: projects.json, settings.json and the helper's folder.
    config_dir: String,
    /// Where queues are found on their own (`<base>/*/tasks` and `<base>/*/*/tasks`).
    base: String,
    /// The Claude account folders the session transcripts are read from (0025): the allowed ones
    /// (0099), never `~/.claude`.
    transcripts: Vec<String>,
    /// Only the projects under this folder have their transcripts read (0099): `~/playground`.
    transcripts_under: String,
    /// Whether Claude Code's session states are read (0048): "on (2 accounts)", or "not
    /// available: <why>" (`agents::Monitor::summary`).
    session_states: String,
}

/// About for the settings in `settings_file`, with the Claude accounts under `home`.
fn info_for(
    settings_file: &std::path::Path,
    version: &str,
    home: &std::path::Path,
    session_states: String,
) -> SettingsInfo {
    let shown = |p: &std::path::Path| p.to_string_lossy().to_string();
    let settings = settings::load(settings_file);
    let transcripts = accounts::allowed(&settings, home)
        .iter()
        .map(|a| shown(a.dir()))
        .collect();
    SettingsInfo {
        settings,
        lease_bounds: [settings::LEASE_MIN, settings::LEASE_MAX],
        live_bounds: [settings::LIVE_MIN, settings::LIVE_MAX],
        version: version.to_string(),
        home: shown(home),
        config_dir: settings_file.parent().map(shown).unwrap_or_default(),
        base: shown(&base()),
        transcripts,
        transcripts_under: shown(&home.join("playground")),
        session_states,
    }
}

#[tauri::command]
async fn settings_info(handle: tauri::AppHandle) -> Result<SettingsInfo, String> {
    let version = handle.package_info().version.to_string();
    blocking(move || {
        let app = handle.state::<AppState>();
        Ok(info_for(
            &app.settings_file,
            &version,
            &home(),
            app.agents.summary(),
        ))
    })
    .await
}

/// The role profiles (0112) for Settings › Agents › Roles and the names on cards: roles.toml,
/// written from the defaults on first read. A refused file is an error naming its line and key.
#[tauri::command]
async fn roles_info(handle: tauri::AppHandle) -> Result<Vec<roles::Role>, String> {
    blocking(move || roles::load(&handle.state::<AppState>().roles_file)).await
}

/// Change one field of one role (`roles::EDITABLE`) from Settings › Agents › Roles. The core
/// checks the role, the field and the whole team; a refusal says why and leaves the file.
#[tauri::command]
async fn set_role(
    handle: tauri::AppHandle,
    id: String,
    field: String,
    value: String,
) -> Result<Vec<roles::Role>, String> {
    if id.len() > 40 || field.len() > 40 || value.len() > 400 {
        return Err("that value is too long".into());
    }
    blocking(move || {
        let app = handle.state::<AppState>();
        let _guard = app.config.lock().map_err(|e| e.to_string())?;
        roles::edit(&app.roles_file, &id, &field, &value)
    })
    .await
}

/// Change one of this machine's settings, `lease` or `live`, to a span like `12h` or `5m` (empty
/// is the default), or a switch (`settings::set`). The core checks the name, the units and the
/// bounds; the next poll uses it, and the menu bar icon follows its switch at once.
#[tauri::command]
async fn set_setting(
    handle: tauri::AppHandle,
    name: String,
    value: String,
) -> Result<SettingsInfo, String> {
    let version = handle.package_info().version.to_string();
    blocking(move || {
        let app = handle.state::<AppState>();
        {
            let _guard = app.config.lock().map_err(|e| e.to_string())?;
            let now = settings::set(&app.settings_file, &name, &value)?;
            if name == "menu-bar" {
                // At once, with the latest count; the next read keeps it current.
                app.menu_bar.sync(now.menu_bar, None, &tray::Tray(&handle));
            }
        }
        Ok(info_for(
            &app.settings_file,
            &version,
            &home(),
            app.agents.summary(),
        ))
    })
    .await
}

/// Change a project's Your check limit (0050), by its key (queue folder), to a number from 1 to
/// 50, `off`, or empty for the default. The core checks the value; the next poll uses it.
#[tauri::command]
async fn set_your_check_limit(
    handle: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            known_key(c, &key)?;
            check_limit::set(&mut c.your_check, &key, &value)
        })
    })
    .await
}

/// Change projects.json under the lock: load, edit, save (`projects::edit`, whose file lock keeps
/// `muster projects set` from losing the app's edits, or the app the command's).
fn edit_config<T>(
    app: &AppState,
    f: impl FnOnce(&mut projects::Config) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = app.config.lock().map_err(|e| e.to_string())?;
    projects::edit(&app.config_file, f)
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
        "Choose the project's repo (its .muster/ goes inside)",
    )
    .await?
    else {
        return Ok(None);
    };
    let h = handle.clone();
    blocking(move || {
        let app = h.state::<AppState>();
        // Its prefix is stored with it (0033): the default beside every project's.
        let known = projects::list(&base(), &projects::load(&app.config_file));
        // As `muster init` makes it (0004): a repo's local .muster/, else the old tasks/.
        let (q, _) = projects::new_queue(&dir, None, None, &known, &tools())?;
        edit_config(&app, |c| projects::add(c, &base(), &q)).map(Some)
    })
    .await
}

/// Set a project's prefix (0033), stored with its queue: refused when it isn't 2 to 4 lowercase
/// letters or another project has it.
#[tauri::command]
async fn set_project_prefix(
    handle: tauri::AppHandle,
    key: String,
    prefix: String,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        // Under the config lock, so two changes can't both take one prefix.
        let _guard = app.config.lock().map_err(|e| e.to_string())?;
        let c = projects::load(&app.config_file);
        projects::set_prefix_by_key(&c, &base(), &key, prefix.trim()).map(|_| ())
    })
    .await
}

/// Set what the window calls a project (0044), by its key: at most 40 characters, control
/// characters stripped; empty shows its name again. Commands keep the name.
#[tauri::command]
async fn set_project_label(
    handle: tauri::AppHandle,
    key: String,
    label: String,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            known_key(c, &key)?;
            labels::set(&mut c.labels, &key, &label, labels::MAX_LABEL);
            Ok(())
        })
    })
    .await
}

/// Set the sidebar heading a project goes under (0044), by its key: at most 24 characters,
/// control characters stripped; empty puts it under its group's heading, or Other.
#[tauri::command]
async fn set_project_category(
    handle: tauri::AppHandle,
    key: String,
    category: String,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        // Under the lock with the write: the heading it goes under unfolds (arrange::put_under).
        edit_config(&app, |c| {
            let list = projects::list(&base(), c);
            arrange::set_category(c, &list, &key, &category)
        })
    })
    .await
}

/// Set a project's colour (0057), by its key: one of `arrange::COLOURS`, or "" for none.
#[tauri::command]
async fn set_project_colour(
    handle: tauri::AppHandle,
    key: String,
    colour: String,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            known_key(c, &key)?;
            arrange::set_colour(c, &key, &colour)
        })
    })
    .await
}

/// Keep the sidebar's order as the window shows it after a drag or ⌥↑/⌥↓ (0057): the projects by
/// key and the headings, first to last, and the project that moved under another heading, if one
/// did (`arrange::arrange`).
#[tauri::command]
async fn arrange_projects(
    handle: tauri::AppHandle,
    order: Vec<String>,
    categories: Vec<String>,
    moved: Option<arrange::Moved>,
) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            let list = projects::list(&base(), c);
            arrange::arrange(c, &list, &order, &categories, moved.as_ref())
        })
    })
    .await
}

/// Fold or unfold a sidebar heading (0070), by its name (`arrange::set_folded`).
#[tauri::command]
async fn fold_category(handle: tauri::AppHandle, name: String, folded: bool) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            let list = projects::list(&base(), c);
            arrange::set_folded(c, &list, &name, folded)
        })
    })
    .await
}

/// Rename a sidebar heading (0057): every project under it goes under the new name, which may be
/// another heading's, and then the two are one (`arrange::rename_category`).
#[tauri::command]
async fn rename_category(handle: tauri::AppHandle, from: String, to: String) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        edit_config(&app, |c| {
            let list = projects::list(&base(), c);
            arrange::rename_category(c, &list, &from, &to)
        })
    })
    .await
}

/// The window's title for the project it shows, named by its name ("all" for All): "<label> —
/// Muster", or "Muster".
#[tauri::command]
async fn show_project_title(handle: tauri::AppHandle, name: String) -> Result<(), String> {
    // The notifications hold back what the window, focused, shows (0052).
    handle.state::<AppState>().notify.viewing(&name);
    let h = handle.clone();
    let title = blocking(move || {
        let app = h.state::<AppState>();
        let list = projects::list(&base(), &projects::load(&app.config_file));
        Ok(labels::window_title(&list, &name))
    })
    .await?;
    match handle.get_webview_window("main") {
        Some(w) => w.set_title(&title).map_err(|e| e.to_string()),
        None => Ok(()),
    }
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
            c.labels.remove(&key);
            c.categories.remove(&key);
            c.colours.remove(&key);
            c.order.retain(|k| k != &key);
            c.hidden.retain(|h| h != &key);
            Ok(())
        })?;
        // Its run, if one goes, ends with it: the window can't name it any more (0079).
        app.runs.stop(&key);
        Ok(())
    })
    .await
}

/// Run (0079): starts the run command of the project named `project` in its main checkout,
/// restarting a run of it that's still going. The window gives the name only: the command and the
/// folder are the core's own, from projects.json and the project's queue.
#[tauri::command]
async fn run_start(handle: tauri::AppHandle, project: String) -> Result<run::View, String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let list = projects::list(&base(), &projects::load(&app.config_file));
        app.runs.start_named(&list, &project)
    })
    .await
}

/// Stop (0079): ends the run of the project named `project` and everything it started.
#[tauri::command]
async fn run_stop(handle: tauri::AppHandle, project: String) -> Result<run::View, String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let list = projects::list(&base(), &projects::load(&app.config_file));
        app.runs.stop_named(&list, &project)
    })
    .await
}

/// What Run knows of the project named `project` (0079): its command, its checkout, why it
/// can't run, and its latest run with the output kept.
#[tauri::command]
async fn run_state(handle: tauri::AppHandle, project: String) -> Result<run::View, String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let list = projects::list(&base(), &projects::load(&app.config_file));
        app.runs.view(&list, &project)
    })
    .await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(AppState {
                cache: Mutex::new(queue::Cache::default()),
                helpers: Mutex::new(HashMap::new()),
                config: Mutex::new(()),
                transcripts: Arc::new(transcripts::Reader::new(accounts::playground_key(&home()))),
                agents: agents::Monitor::app(home()),
                config_file: dir.join("projects.json"),
                settings_file: dir.join(settings::FILE),
                roles_file: dir.join(roles::FILE),
                badge: badge::Badge::default(),
                notify: notify::Notifier::new(dir.join(notify::FILE)),
                runs: run::Runs::new(run_events(app.handle().clone())),
                menu_bar: tray::MenuBar::default(),
            });
            // Up at launch, before the first read has a count, unless Settings turned it off.
            let state = app.state::<AppState>();
            let settings = settings::load(&state.settings_file);
            state
                .menu_bar
                .sync(settings.menu_bar, None, &tray::Tray(app.handle()));
            watch_badge(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            search_index,
            help,
            task_body,
            image,
            answer,
            answer_pick,
            projects,
            project_histories,
            restore_history,
            commit_history,
            record_removed,
            settings_info,
            set_setting,
            roles_info,
            set_role,
            set_your_check_limit,
            add_project,
            create_project,
            set_project_label,
            set_project_category,
            set_project_colour,
            arrange_projects,
            rename_category,
            fold_category,
            show_project_title,
            set_project_prefix,
            hide_project,
            remove_project,
            create_task,
            move_task,
            pass_task,
            accept_all,
            return_task,
            block_task,
            unblock_task,
            reclaim_task,
            set_stage,
            set_field,
            add_note,
            tick_criterion,
            task_sessions,
            session_transcript,
            add_backlog_note,
            file_note,
            drop_note,
            shape_note,
            vision_brief,
            resolve_vision,
            editable_body,
            edit_body,
            add_comment,
            open_link,
            attach_data,
            attach_pick,
            helper_cmds::helper_info,
            helper_cmds::helper_set_account,
            helper_cmds::helper_set_model,
            helper_cmds::helper_reset,
            helper_cmds::helper_stop,
            helper_cmds::helper_send,
            helper_cmds::helper_login,
            ping_cmds::ask_checkin,
            ping_cmds::message_session,
            pause_cmds::pause_task,
            pause_cmds::resume_task,
            pause_cmds::old_pauses,
            pause_cmds::fix_old_pauses,
            pause_cmds::set_old_pauses_offered,
            review_cmds::review_brief,
            review_cmds::lessons,
            review_cmds::decide_lessons,
            run_start,
            run_stop,
            run_state
        ])
        .build(tauri::generate_context!())
        .expect("error while building Muster")
        .run(|handle, event| {
            // Quitting ends every run and everything it started (0079). Hiding the window
            // doesn't: the app stays alive for the Dock badge, and so do the runs.
            if let tauri::RunEvent::Exit = event {
                handle.state::<AppState>().runs.stop_all();
            }
        });
}

/// A run's output and state to the window, as `run` events (0079).
fn run_events(handle: tauri::AppHandle) -> run::Emit {
    use tauri::Emitter;
    Arc::new(move |snap: &run::Snap| {
        let _ = handle.emit("run", snap);
    })
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
            fields::Commented {
                path: "doing/0007-t.md".into(),
                did: String::new(),
                unsaved: String::new()
            }
        );
        let task = queue::read_task(d.path(), "0007").unwrap();
        assert_eq!(
            (
                task.comments[0].who.as_str(),
                task.comments[0].text.as_str()
            ),
            ("owner", "Hi.")
        );
        // The app's comment is the owner's: an @verb acts (mu-101), an unknown one saves nothing.
        assert!(comment(d.path(), "0007", "@sendbak x", t)
            .unwrap_err()
            .contains("isn't an action"));
        assert_eq!(
            comment(d.path(), "0007", "@block Apple review", t)
                .unwrap()
                .did,
            "blocked (outside)"
        );
        let task = queue::read_task(d.path(), "0007").unwrap();
        assert_eq!(
            (task.blocked_kind.as_str(), task.comments.len()),
            ("outside", 2)
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
    fn search_index_takes_only_a_queue_muster_knows() {
        let d = tempfile::tempdir().unwrap();
        let config = d.path().join("projects.json");
        for bad in ["../x", "/tmp", "no such queue"] {
            let err = index_scope(&config, Some(bad)).unwrap_err();
            assert!(err.contains("no queue named"), "{bad:?}: {err}");
        }
    }

    #[test]
    fn accept_all_accepts_exactly_the_just_accept_tasks_each_logged() {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        let write = |file: &str, id: &str, stage: &str, criteria: &str| {
            let text = format!(
                "---\nid: {id}\nstage: {stage}\nstatus: doing\nclaimed-by:\nheld-by:\nwaiting_on:\ncommit: abc1234\n---\n\n# T\n\n## Acceptance criteria\n- [x] AC1: built — verify: command\n{criteria}\n## Log\n- 2026-10-12 09:00 capture: created\n"
            );
            std::fs::write(d.path().join(file), &text).unwrap();
            text
        };
        // None of the owner's; the owner's all ticked; one of theirs open; still in Testing.
        write("doing/0001-t.md", "0001", "your-check", "");
        write(
            "doing/0002-t.md",
            "0002",
            "your-check",
            "- [x] AC2: tried it — verify: owner\n",
        );
        let open = write(
            "doing/0003-t.md",
            "0003",
            "your-check",
            "- [ ] AC2: tried it — verify: owner\n",
        );
        let testing = write("doing/0004-t.md", "0004", "testing", "");
        let t = "2026-10-13 10:00";

        // Arguments first: one bad id or stamp and nothing is touched.
        assert_eq!(
            accept_all_in(d.path(), &["0001".into(), "../7".into()], t).unwrap_err(),
            "bad task id"
        );
        assert_eq!(
            accept_all_in(d.path(), &["0001".into()], "2026-10-13").unwrap_err(),
            "bad time stamp"
        );
        assert_eq!(
            queue::read_task(d.path(), "0001").unwrap().stage,
            "your-check"
        );
        assert_eq!(
            accept_all_in(d.path(), &[], t).unwrap(),
            AcceptAll::default()
        );

        let ids: Vec<String> = ["0001", "0002", "0003", "0004", "0001"]
            .into_iter()
            .map(String::from)
            .collect();
        let r = accept_all_in(d.path(), &ids, t).unwrap();
        assert_eq!(r.accepted, ["0001", "0002"]);
        let refused: Vec<&str> = r.refused.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(refused, ["0003", "0004", "0001"]);
        assert!(
            r.refused[0].1.contains("owner's criteria aren't ticked"),
            "{r:?}"
        );
        assert!(
            r.refused[1].1.contains("is now in Testing, not Your check"),
            "{r:?}"
        );
        // Asked twice in one go, it's accepted once.
        assert!(
            r.refused[2].1.contains("is now in Done, not Your check"),
            "{r:?}"
        );
        for id in ["0001", "0002"] {
            let task = queue::read_task(d.path(), id).unwrap();
            // Done, with the commit the merge recorded, and one log line: the stage move.
            assert_eq!(
                (task.stage.as_str(), task.commit.as_str()),
                ("done", "abc1234")
            );
            let file = d.path().join(&task.path);
            assert!(file.starts_with(d.path().join("done")), "{file:?}");
            let text = std::fs::read_to_string(file).unwrap();
            let log = text.split("## Log\n").nth(1).unwrap();
            assert_eq!(
                log,
                "- 2026-10-12 09:00 capture: created\n- 2026-10-13 10:00 owner: stage your-check → done\n",
                "{id}"
            );
        }
        // The refused ones are as they were.
        assert_eq!(
            std::fs::read_to_string(d.path().join("doing/0003-t.md")).unwrap(),
            open
        );
        assert_eq!(
            std::fs::read_to_string(d.path().join("doing/0004-t.md")).unwrap(),
            testing
        );
    }

    #[test]
    fn accept_all_leaves_a_task_with_an_unproven_criterion() {
        // mu-78 R1: the card and the pane say "not proven" for an agent's criterion left unticked
        // (a hand move into Your check), so Accept all doesn't take it either.
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        let write = |id: &str, criteria: &str| {
            let text = format!(
                "---\nid: {id}\nstage: your-check\nstatus: doing\nclaimed-by:\nheld-by:\nwaiting_on:\ncommit:\n---\n\n# T\n\n## Acceptance criteria\n{criteria}\n## Log\n- 2026-10-12 09:00 capture: created\n"
            );
            std::fs::write(d.path().join(format!("doing/{id}-t.md")), &text).unwrap();
            text
        };
        let unproven = write(
            "0001",
            "- [x] AC1: built — verify: command — proof: ok\n- [ ] AC2: also — verify: command\n- [ ] no label — verify: command\n",
        );
        // The owner's own, open, is the pass's to refuse; all ticked goes.
        write("0002", "- [x] AC1: built — verify: command — proof: ok\n");
        let ids: Vec<String> = ["0001", "0002"].into_iter().map(String::from).collect();
        let r = accept_all_in(d.path(), &ids, "2026-10-13 10:00").unwrap();
        assert_eq!(r.accepted, ["0002"]);
        assert_eq!(
            r.refused,
            [(
                "0001".to_string(),
                "0001 has criteria nobody has proven (AC2, criterion 3); send it back, or check them yourself and accept it on its own".to_string()
            )]
        );
        assert_eq!(
            std::fs::read_to_string(d.path().join("doing/0001-t.md")).unwrap(),
            unproven
        );
    }

    #[test]
    fn accept_all_leaves_goals_features_and_open_questions_for_the_owner() {
        let d = tempfile::tempdir().unwrap();
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir(d.path().join(f)).unwrap();
        }
        let write = |id: &str, front: &str, questions: &str| {
            let text = format!(
                "---\nid: {id}\nstage: your-check\nstatus: doing\nclaimed-by:\nheld-by:\nwaiting_on:\ncommit:\n{front}---\n\n# T\n\n## Questions\n{questions}\n## Acceptance criteria\n- [x] AC1: built — verify: command\n\n## Log\n- 2026-10-12 09:00 capture: created\n"
            );
            std::fs::write(d.path().join(format!("doing/{id}-t.md")), &text).unwrap();
            text
        };
        let escalated = "- [ ] **Q1 (wt-a, colour, 2026-10-12 09:00):** Which colour?\n  **Escalated to owner (capture, 2026-10-12 09:05).**\n";
        let answered = "- [x] **Q1 (wt-a, colour, 2026-10-12 09:00):** Which colour?\n  **Escalated to owner (capture, 2026-10-12 09:05).**\n";
        let asked_agent = "- [ ] **Q1 (wt-a, colour, 2026-10-12 09:00):** Which colour?\n";
        let goal = write("0001", "level: goal\n", "");
        let feature = write("0002", "level: feature\n", "");
        let open_q = write("0003", "", escalated);
        // An answered question, or one only for an agent, isn't the owner's to do.
        write("0004", "", answered);
        write("0005", "", asked_agent);
        let ids: Vec<String> = ["0001", "0002", "0003", "0004", "0005"]
            .into_iter()
            .map(String::from)
            .collect();
        let r = accept_all_in(d.path(), &ids, "2026-10-13 10:00").unwrap();
        assert_eq!(r.accepted, ["0004", "0005"]);
        assert_eq!(
            r.refused,
            [
                (
                    "0001".to_string(),
                    "0001 is a goal; Accept all takes tasks only".to_string()
                ),
                (
                    "0002".to_string(),
                    "0002 is a feature; Accept all takes tasks only".to_string()
                ),
                (
                    "0003".to_string(),
                    "0003 has a question for the owner (Q1); answer it first".to_string()
                ),
            ]
        );
        for (id, before) in [("0001", goal), ("0002", feature), ("0003", open_q)] {
            assert_eq!(
                std::fs::read_to_string(d.path().join(format!("doing/{id}-t.md"))).unwrap(),
                before,
                "{id}"
            );
        }
    }

    #[test]
    fn settings_info_shows_the_settings_their_bounds_and_where_things_are() {
        // The owner's Mac: the company account, ih, and mg and hg (the allowlist's default).
        let home = accounts::tests::fake_home();
        let h = home.path();
        let dir = h.join("Library/Application Support/ai.sweetlemon.muster");
        let file = dir.join(settings::FILE);
        let info = info_for(&file, "0.1.0", h, "on (1 account)".into());
        // Nothing saved yet: the defaults.
        assert_eq!(info.settings, settings::Settings::default());
        assert_eq!(
            (info.lease_bounds, info.live_bounds),
            ([60, 10080], [1, 60])
        );
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.config_dir, dir.to_string_lossy());
        // The transcripts are read from the allowed accounts' folders, as `~/…` paths, never
        // `~/.claude`; and only the projects under `~/playground`.
        let shown = |p: &str| h.join(p).to_string_lossy().to_string();
        assert_eq!(info.transcripts, [shown(".claude-mg"), shown(".claude-hg")]);
        assert_eq!(info.transcripts_under, shown("playground"));
        settings::set(&file, "live", "15m").unwrap();
        let json =
            serde_json::to_string(&info_for(&file, "0.1.0", h, "on (1 account)".into())).unwrap();
        assert!(
            json.contains(
                r#""settings":{"leaseMinutes":720,"liveMinutes":15,"notify":true,"menuBar":true,"oldPausesOffered":false,"claudeAccounts":["mg","hg"]}"#
            ),
            "{json}"
        );
        assert!(
            json.contains(r#""leaseBounds":[60,10080],"liveBounds":[1,60]"#),
            "{json}"
        );
        assert!(
            json.contains(&format!(
                r#""transcripts":[{:?},{:?}],"transcriptsUnder":{:?}"#,
                shown(".claude-mg"),
                shown(".claude-hg"),
                shown("playground")
            )),
            "{json}"
        );
        // An empty allowlist: no account is read, and About lists none.
        std::fs::write(&file, r#"{"claudeAccounts":[]}"#).unwrap();
        assert!(info_for(&file, "0.1.0", h, String::new())
            .transcripts
            .is_empty());
        // The home folder, so the window can show paths under it as `~/…`.
        assert_eq!(info.home, h.to_string_lossy());
        assert!(json.contains(r#""home":"#), "{json}");
        // Whether Claude Code's session states are read (0048), as the monitor says.
        assert!(
            json.contains(r#""sessionStates":"on (1 account)""#),
            "{json}"
        );
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
        let lease = settings::Settings::default().lease_minutes;
        assert_eq!(
            release(d.path(), "7", "2026-10-13 10:00", now, lease).unwrap_err(),
            "bad task id"
        );
        assert_eq!(
            release(d.path(), "0007", "2026-10-13T10:00", now, lease).unwrap_err(),
            "bad time stamp"
        );
        assert!(release(d.path(), "0007", "2026-10-12 21:59", now, lease)
            .unwrap_err()
            .contains("lease hasn't run out"));
        // This machine's lease (0036): 2 h releases it sooner.
        assert!(release(d.path(), "0007", "2026-10-12 12:00", now, 120)
            .unwrap_err()
            .contains("lease hasn't run out"));
        assert_eq!(
            release(d.path(), "0007", "2026-10-12 12:01", now, 120).unwrap(),
            "todo/0007-t.md"
        );
        let t = queue::read_task(d.path(), "0007").unwrap();
        assert_eq!((t.stage.as_str(), t.claimed_by.as_str()), ("ready", ""));
        assert!(t.log_tail.last().unwrap().ends_with(
            "owner: stage building → ready (reclaimed from wt-a (no heartbeat for 2 h))"
        ));
    }

    #[test]
    fn the_hidden_windows_badge_counts_what_needs_the_owner_in_the_shown_projects() {
        let base = tempfile::tempdir().unwrap();
        let put = |project: &str, name: &str, front: &str| {
            let q = base.path().join(project).join("tasks");
            std::fs::create_dir_all(q.join("todo")).unwrap();
            std::fs::write(q.join("README.md"), "q").unwrap();
            let text = format!("---\n{front}\n---\n\n# {name}\n\n## Log\n- 2026-10-13 captured\n");
            std::fs::write(q.join(name), text).unwrap();
        };
        put("alpha", "0001-buy.md", "id: 0001\nstage: ready\nfor: human");
        put("alpha", "0002-check.md", "id: 0002\nstage: your-check");
        put("alpha", "0003-build.md", "id: 0003\nstage: building");
        put("beta", "0001-check.md", "id: 0001\nstage: your-check");
        let config = tempfile::tempdir().unwrap();
        let config_file = config.path().join("projects.json");
        let settings_file = config.path().join(settings::FILE);
        let cache = Mutex::new(queue::Cache::default());
        // No task has a session stamp, so Claude Code is never asked.
        let fetch: agents::Fetch = Arc::new(|_: &accounts::Account| Err("not asked".to_string()));
        let monitor = agents::Monitor::new(fetch, in_place);
        let home = accounts::tests::fake_home();
        let read = || {
            waiting_in(
                base.path(),
                home.path(),
                &config_file,
                &settings_file,
                &cache,
                &monitor,
            )
            .unwrap()
            .0
        };
        let count = || {
            let s = read();
            badge::dock_count(&s.tasks, &s.queues)
        };
        assert_eq!(count(), 3);
        // Read with the tasks' keys and the projects' labels, for the notifications (0052).
        let s = read();
        let lines: Vec<String> = notify::waiting(&s.tasks, &s.queues)
            .into_iter()
            .map(|i| format!("{}: {}", i.label, i.line))
            .collect();
        assert_eq!(
            lines,
            [
                "alpha: al-1 0001-buy.md: your task",
                "alpha: al-2 0002-check.md: ready for your check",
                "beta: be-1 0001-check.md: ready for your check"
            ]
        );
        // Read again on the cache it put back: the same answer.
        assert_eq!(count(), 3);
        // A hidden project isn't on the board, so it isn't on the badge either.
        let mut c = projects::load(&config_file);
        let beta = projects::list(base.path(), &c)
            .into_iter()
            .find(|p| p.name == "beta")
            .unwrap();
        c.hidden.push(beta.key);
        projects::save(&config_file, &c).unwrap();
        assert_eq!(count(), 2);
    }

    fn in_place(job: Box<dyn FnOnce() + Send>) {
        job()
    }

    #[test]
    fn the_hidden_windows_badge_counts_a_task_whose_session_waits_for_input() {
        let base = tempfile::tempdir().unwrap();
        let q = base.path().join("alpha/tasks");
        std::fs::create_dir_all(q.join("todo")).unwrap();
        std::fs::write(q.join("README.md"), "q").unwrap();
        let front =
            "id: 0001\nstage: building\nclaimed-by: wt-a (wt-0048 · session 7b5e8856 · cc-hg)";
        let text = format!("---\n{front}\n---\n\n# Build\n\n## Log\n- 2026-10-13 captured\n");
        std::fs::write(q.join("0001-build.md"), text).unwrap();
        // Bare `claude` (the company account) and an account not on the list (0099): their
        // sessions wait too, but they're never asked about nor counted.
        for (id, who) in [
            ("0002", "wt-b (main · session 11111111)"),
            ("0003", "wt-c (main · session 22222222 · cc-ih)"),
        ] {
            let text = format!("---\nid: {id}\nstage: building\nclaimed-by: {who}\n---\n\n# Build\n\n## Log\n- 2026-10-13 captured\n");
            std::fs::write(q.join(format!("{id}-build.md")), text).unwrap();
        }
        let config = tempfile::tempdir().unwrap();
        let config_file = config.path().join("projects.json");
        let settings_file = config.path().join(settings::FILE);
        // Claude Code says every session is `state`, as the window's poll would apply it.
        let home = accounts::tests::fake_home();
        let read = |fetch: agents::Fetch| {
            let monitor = agents::Monitor::new(fetch, in_place);
            let cache = Mutex::new(queue::Cache::default());
            let (s, _) = waiting_in(
                base.path(),
                home.path(),
                &config_file,
                &settings_file,
                &cache,
                &monitor,
            )
            .unwrap();
            s
        };
        let count = |state: &'static str| {
            // Every allowed account is asked (0182); only hg lists the sessions.
            let s = read(Arc::new(move |account: &accounts::Account| {
                assert!(
                    matches!(account.label(), "mg" | "hg"),
                    "{}",
                    account.label()
                );
                if account.label() == "mg" {
                    return Ok(b"[]".to_vec());
                }
                Ok(format!(
                    r#"[{{"sessionId": "7b5e8856", "state": "{state}"}}, {{"sessionId": "11111111", "state": "{state}"}}, {{"sessionId": "22222222", "state": "{state}"}}]"#
                )
                .into_bytes())
            }));
            badge::dock_count(&s.tasks, &s.queues)
        };
        // Waiting for input (a permission prompt) waits on the owner, hidden window or not.
        assert_eq!(count("blocked"), 1);
        assert_eq!(count("working"), 0);
        // The allowlist comes from the settings file: emptied, nothing is asked and nothing waits.
        std::fs::write(&settings_file, r#"{"claudeAccounts":[]}"#).unwrap();
        let s = read(Arc::new(
            |_: &accounts::Account| -> Result<Vec<u8>, String> {
                panic!("no account is on the list, so Claude Code isn't asked")
            },
        ));
        assert_eq!(badge::dock_count(&s.tasks, &s.queues), 0);
        assert!(s
            .tasks
            .iter()
            .all(|t| t.session_state == agents::SessionState::Unknown));
    }

    /// The paused lane's offer lists and converts old pauses in the queues the app knows (0059):
    /// in a test, found under the base it gave (`projects::use_places`), else none, never this
    /// Mac's (task 0062).
    #[test]
    fn the_old_pauses_the_app_offers_are_in_the_places_a_test_gave() {
        let base = tempfile::tempdir().unwrap();
        let q = base.path().join("drover/tasks");
        for f in queue::STATUS_FOLDERS {
            std::fs::create_dir_all(q.join(f)).unwrap();
        }
        std::fs::write(q.join("README.md"), "q").unwrap();
        std::fs::write(
            q.join("blocked/0022-pets.md"),
            "---\nid: 0022\ntype: feature\nstatus: blocked\nclaimed-by:\nheartbeat:\nwaiting_on: owner (paused)\n---\n\n# Desk pets\n\n## Log\n- 2026-10-13 02:51 owner: Paused.\n",
        )
        .unwrap();
        let config_file = base.path().join("projects.json");
        let stamp = "2026-10-13 18:00";
        let listed = [old_pause::Listed {
            queue: "drover".into(),
            id: "0022".into(),
        }];
        assert!(queues(&config_file).is_empty(), "not given: none known");
        let fixed = old_pause::convert_listed(&queues(&config_file), &listed, "owner", stamp);
        assert_eq!(fixed.failed, ["drover: no queue named \"drover\""]);
        projects::use_places(projects::Places {
            base: base.path().to_path_buf(),
            config_file: config_file.clone(),
        });
        let found = old_pause::find_all(&queues(&config_file), stamp);
        let keys: Vec<&str> = found.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(
            keys,
            ["dr-22"],
            "the default prefix beside the given projects only"
        );
        let fixed = old_pause::convert_listed(&queues(&config_file), &listed, "owner", stamp);
        assert_eq!((fixed.converted.len(), fixed.failed.len()), (1, 0));
        assert!(!queue::read_task(&q, "0022").unwrap().blocked);
    }
}
