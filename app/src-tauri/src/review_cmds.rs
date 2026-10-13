//! The app's commands for the review of record and its lessons (task 0027, `review`, `lessons`),
//! as the owner: the brief to hand a session, the lessons waiting, and the owner's batch. Kept
//! beside them so the app's command list grows by a few lines.

use crate::{blocking, check_id, check_stamp, in_queue, lessons, review, AppState};

/// The brief for task `id`'s next review round, as text for the clipboard, the project named by
/// its label as the window names it (0044).
#[tauri::command]
pub(crate) async fn review_brief(
    app: tauri::State<'_, AppState>,
    queue: String,
    id: String,
    stamp: String,
) -> Result<String, String> {
    let file = app.config_file.clone();
    blocking(move || {
        let list = crate::projects::list(&crate::base(), &crate::projects::load(&file));
        in_queue(&file, &queue, |root| {
            let label = list
                .iter()
                .find(|p| std::path::Path::new(&p.path) == root)
                .map(crate::labels::shown_label)
                .unwrap_or_default();
            brief_in(root, &id, &stamp, &label)
        })
    })
    .await
}

/// The queue's lessons: proposals waiting for the owner, and what's live.
#[tauri::command]
pub(crate) async fn lessons(
    app: tauri::State<'_, AppState>,
    queue: String,
    stamp: String,
) -> Result<lessons::Listing, String> {
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            check_stamp(&stamp)?;
            lessons::listing(root, &stamp)
        })
    })
    .await
}

/// The owner's batch: accept `accept` (every proposal when `all`), reject `reject` with `why`.
#[tauri::command]
pub(crate) async fn decide_lessons(
    app: tauri::State<'_, AppState>,
    queue: String,
    accept: Vec<String>,
    all: bool,
    reject: Vec<String>,
    why: String,
    stamp: String,
) -> Result<lessons::Decided, String> {
    let file = app.config_file.clone();
    blocking(move || {
        in_queue(&file, &queue, |root| {
            decide_in(root, &accept, all, &reject, &why, &stamp)
        })
    })
    .await
}

/// The brief, the project called `label` when there is one, else by its name.
pub(crate) fn brief_in(
    root: &std::path::Path,
    id: &str,
    stamp: &str,
    label: &str,
) -> Result<String, String> {
    check_id(id)?;
    check_stamp(stamp)?;
    let span = review::span(root, id)?;
    let mut b = review::brief(root, id, &span, stamp)?;
    if !label.is_empty() {
        b.queue = label.to_string();
    }
    Ok(review::brief_text(&b))
}

/// The ids the window sends: `L` and up to four digits each, twenty at most.
fn lesson_ids(ids: &[String]) -> Result<(), String> {
    let ok = |s: &String| {
        s.len() >= 2
            && s.len() <= 5
            && s.starts_with('L')
            && s[1..].bytes().all(|b| b.is_ascii_digit())
    };
    if ids.len() <= 20 && ids.iter().all(ok) {
        Ok(())
    } else {
        Err("bad lesson ids".into())
    }
}

pub(crate) fn decide_in(
    root: &std::path::Path,
    accept: &[String],
    all: bool,
    reject: &[String],
    why: &str,
    stamp: &str,
) -> Result<lessons::Decided, String> {
    lesson_ids(accept)?;
    lesson_ids(reject)?;
    check_stamp(stamp)?;
    lessons::decide(root, accept, all, reject, why, "owner", stamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_decides_lessons_as_the_owner_with_its_arguments_checked() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            decide_in(d.path(), &["1".into()], false, &[], "", "2026-10-13 10:00").unwrap_err(),
            "bad lesson ids"
        );
        assert!(decide_in(d.path(), &[], true, &[], "", "now").is_err());
        assert_eq!(
            decide_in(d.path(), &[], true, &[], "", "2026-10-13 10:00").unwrap_err(),
            "no lessons are waiting for you"
        );
        assert!(brief_in(d.path(), "27", "2026-10-13 10:00", "").is_err());
    }

    #[test]
    fn the_apps_review_brief_names_the_project_by_its_label() {
        use crate::parallel::tests::{commit, fx, task};
        let f = fx();
        let id = task(&f.root, "Labelled", "l/");
        let s = crate::parallel::start(
            &f.root,
            &id,
            "wt-a",
            "2026-10-13 10:00",
            &f.repo,
            &crate::parallel::Place::Default,
        )
        .unwrap();
        commit(&s.worktree, "l/x.txt", "x\n", "build");
        let t = brief_in(&f.root, &id, "2026-10-13 11:00", "Muster app").unwrap();
        assert!(t.starts_with("Review brief: pj-1 in Muster app ("), "{t}");
        // No label: its name, as the command line says it.
        let t = brief_in(&f.root, &id, "2026-10-13 11:00", "").unwrap();
        assert!(t.starts_with("Review brief: pj-1 in proj ("), "{t}");
    }
}
