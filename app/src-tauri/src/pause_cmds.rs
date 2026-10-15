//! The app's commands for pausing and resuming a task (task 0045, `pause`), as the owner, and for
//! turning the old pauses into pauses (0059, `old_pause`). Kept beside `pause` so the app's
//! command list grows by a few lines.

use crate::{blocking, check_id, check_stamp, in_queue, old_pause, pause, queue, settings};
use crate::{queues, AppState};
use tauri::Manager;

/// The owner pauses a task, with an optional reason.
#[tauri::command]
pub(crate) async fn pause_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    why: String,
    stamp: String,
) -> Result<pause::Paused, String> {
    let file = app.config_file.clone();
    // What the core refuses with names tasks by their keys (0033).
    blocking(move || in_queue(&file, &queue, |root| pause_in(root, &id, &why, &stamp))).await
}

/// The owner resumes a paused task.
#[tauri::command]
pub(crate) async fn resume_task(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stamp: String,
) -> Result<pause::Paused, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| resume_in(root, &id, &stamp))).await
}

/// Every old pause (0059) in the queues the app shows, for the offer it makes on load.
#[tauri::command]
pub(crate) async fn old_pauses(
    app: tauri::State<'_, AppState>,
) -> Result<Vec<old_pause::OldPause>, String> {
    let file = app.config_file.clone();
    blocking(move || Ok(old_pause::find_all(&queues(&file), &queue::local_stamp()))).await
}

/// The owner said yes to the list they were shown (`items`, each its queue and id): each one
/// becomes a pause, with its reason and time, by the owner, and nothing that wasn't listed. A
/// queue the app doesn't know, an id that isn't one, or one that isn't an old pause any more is
/// in `failed`. Returns what it converted, and what it couldn't.
#[tauri::command]
pub(crate) async fn fix_old_pauses(
    app: tauri::State<'_, AppState>,
    items: Vec<old_pause::Listed>,
) -> Result<old_pause::Fixed, String> {
    let file = app.config_file.clone();
    blocking(move || {
        Ok(old_pause::convert_listed(
            &queues(&file),
            &items,
            "owner",
            &queue::local_stamp(),
        ))
    })
    .await
}

/// The app made its offer (whatever the owner said), so it doesn't make it again on this Mac:
/// `oldPausesOffered` in this machine's settings, as `set_setting("old-pauses-offered", "yes")`.
#[tauri::command]
pub(crate) async fn set_old_pauses_offered(handle: tauri::AppHandle) -> Result<(), String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let _guard = app.config.lock().map_err(|e| e.to_string())?;
        settings::set(&app.settings_file, "old-pauses-offered", "yes").map(|_| ())
    })
    .await
}

/// What `pause_task` does once it has the queue: its arguments checked, then the core's rules.
pub(crate) fn pause_in(
    root: &std::path::Path,
    id: &str,
    why: &str,
    stamp: &str,
) -> Result<pause::Paused, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    pause::pause(root, id, why, "owner", stamp)
}

/// What `resume_task` does once it has the queue.
pub(crate) fn resume_in(
    root: &std::path::Path,
    id: &str,
    stamp: &str,
) -> Result<pause::Paused, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    pause::resume(root, id, "owner", stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_app_pauses_and_resumes_as_the_owner_with_its_arguments_checked() {
        let d = crate::testkit::tempdir().unwrap();
        fs::create_dir(d.path().join("doing")).unwrap();
        fs::write(d.path().join("README.md"), "q").unwrap();
        fs::write(
            d.path().join("doing/0022-t.md"),
            "---\nid: 0022\nstage: building\nstatus: doing\nclaimed-by: wt-a (session 7b5e8856)\nheartbeat:\nwaiting_on:\n---\n\n# T\n\n## Log\n- 2026-10-13 08:00 capture: created\n",
        )
        .unwrap();
        assert_eq!(
            pause_in(d.path(), "22", "", "2026-10-13 10:00").unwrap_err(),
            "bad task id"
        );
        assert!(pause_in(d.path(), "0022", "", "now").is_err());
        assert!(!d.path().join("mail/7b5e8856").exists());
        let p = pause_in(d.path(), "0022", "later", "2026-10-13 10:00").unwrap();
        assert_eq!(p.session, "7b5e8856");
        assert!(resume_in(d.path(), "0022", "10:00").is_err());
        resume_in(d.path(), "0022", "2026-10-13 10:30").unwrap();
        let body = fs::read_to_string(d.path().join("doing/0022-t.md")).unwrap();
        assert!(body.contains(
            "- 2026-10-13 10:00 owner: paused (later); asked session 7b5e8856 to stop\n"
        ));
        assert!(body.contains(
            "- 2026-10-13 10:30 owner: resumed after 30 min; asked session 7b5e8856 to carry on\n"
        ));
    }
}
