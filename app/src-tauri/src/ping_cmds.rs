//! The app's commands for reaching the session working on a task (task 0039, `ping`), as the
//! owner. Kept beside `ping` so the app's command list grows by two lines.

use crate::{blocking, check_id, check_stamp, in_queue, ping, AppState};

/// The owner asks the session holding a task in Building for a check-in.
#[tauri::command]
pub(crate) async fn ask_checkin(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stamp: String,
) -> Result<ping::Pinged, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| ask(root, &id, &stamp))).await
}

/// The owner sends the session holding a task in Building or Testing a message.
#[tauri::command]
pub(crate) async fn message_session(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    text: String,
    stamp: String,
) -> Result<ping::Pinged, String> {
    let file = app.config_file.clone();
    blocking(move || in_queue(&file, &queue, |root| message(root, &id, &text, &stamp))).await
}

/// What `ask_checkin` does once it has the queue: its arguments checked, then the core's rules.
pub(crate) fn ask(root: &std::path::Path, id: &str, stamp: &str) -> Result<ping::Pinged, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    ping::ask_checkin(root, id, "owner", stamp)
}

/// What `message_session` does once it has the queue.
pub(crate) fn message(
    root: &std::path::Path,
    id: &str,
    text: &str,
    stamp: &str,
) -> Result<ping::Pinged, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    ping::message(root, id, text, "owner", stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_app_pings_as_the_owner_with_its_arguments_checked() {
        let d = crate::testkit::tempdir().unwrap();
        fs::create_dir(d.path().join("doing")).unwrap();
        fs::write(d.path().join("README.md"), "q").unwrap();
        fs::write(
            d.path().join("doing/0022-t.md"),
            "---\nid: 0022\nstage: building\nstatus: doing\nclaimed-by: wt-a (session 7b5e8856)\nheartbeat:\nwaiting_on:\n---\n\n# T\n\n## Log\n- 2026-10-13 08:00 capture: created\n",
        )
        .unwrap();
        assert_eq!(
            ask(d.path(), "22", "2026-10-13 10:00").unwrap_err(),
            "bad task id"
        );
        assert!(ask(d.path(), "0022", "now").is_err());
        assert!(message(d.path(), "0022", "hi", "10:00").is_err());
        assert!(!d.path().join("mail/7b5e8856").exists());
        assert_eq!(
            ask(d.path(), "0022", "2026-10-13 10:00").unwrap().session,
            "7b5e8856"
        );
        message(d.path(), "0022", "look at the test", "2026-10-13 10:01").unwrap();
        let body = fs::read_to_string(d.path().join("doing/0022-t.md")).unwrap();
        assert!(
            body.contains("- 2026-10-13 10:00 owner: asked for a check-in (session 7b5e8856)\n")
        );
        assert!(body
            .contains("- 2026-10-13 10:01 owner: messaged session 7b5e8856: look at the test\n"));
    }
}
