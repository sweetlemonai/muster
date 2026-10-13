//! The app's commands for pausing and resuming a task (task 0045, `pause`), as the owner. Kept
//! beside `pause` so the app's command list grows by two lines.

use crate::{blocking, check_id, check_stamp, in_queue, pause, AppState};

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
        let d = tempfile::tempdir().unwrap();
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
