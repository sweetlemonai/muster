//! The Dock badge (task 0006): how many tasks need the owner, on Muster's icon, so the count shows
//! without opening the window. It's set from here, never from the window, so the window needs no
//! permission for it. While the window polls (every 2 s, only while it's visible), each poll sets
//! it; once the window is hidden and stops polling, `lib.rs` reads the queues every `HIDDEN_EVERY`
//! instead. Nothing leaves the Mac.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::queue::Task;

/// How often the badge is read while the window doesn't poll (hidden or minimized). A warm read of
/// every queue took 7 ms for 662 tasks (release build, 2026-10-13).
pub const HIDDEN_EVERY: Duration = Duration::from_secs(15);

/// Does a task need the owner? Their own task (`for: human`) until it's done, unless they paused
/// it (0045), or an agent's that waits on them (`waiting_for_owner`, `queue::needs_owner`, and a
/// session waiting for input, `agents::apply`), paused or not. The window's `needsYou` (human.ts)
/// is the same rule; `app/src/needs-cases.json` holds the two to the same answers.
pub fn needs_you(for_who: &str, stage: &str, waiting_for_owner: bool, paused: bool) -> bool {
    if for_who == "human" {
        stage != "done" && !paused
    } else {
        waiting_for_owner
    }
}

/// What the Dock counts: the tasks that need the owner, and the lessons from reviews waiting for
/// their yes (0027), as the window's needs-you count adds them.
pub fn dock_count(tasks: &[Task], queues: &[crate::queue::QueueInfo]) -> usize {
    needs_you_count(tasks) + queues.iter().map(|q| q.lessons_waiting).sum::<usize>()
}

/// How many tasks need the owner: the count the window's sidebar shows for every project.
pub fn needs_you_count(tasks: &[Task]) -> usize {
    tasks
        .iter()
        .filter(|t| {
            needs_you(
                &t.for_who,
                &t.stage,
                t.waiting_for_owner,
                !t.paused.is_empty(),
            )
        })
        .count()
}

/// What the Dock shows for a count: no badge at 0.
pub fn label(n: usize) -> Option<i64> {
    (n > 0).then(|| i64::try_from(n).unwrap_or(i64::MAX))
}

/// What the Dock shows now, and when the window last polled.
#[derive(Default)]
pub struct Badge {
    seen: Mutex<Seen>,
}

#[derive(Default)]
struct Seen {
    shown: Option<usize>,
    polled: Option<Instant>,
}

impl Badge {
    /// The window polled at `at`: it's visible and sets the badge itself.
    pub fn polled(&self, at: Instant) {
        self.seen().polled = Some(at);
    }

    /// Is a read of the badge's own due at `now`: the window hasn't polled for `HIDDEN_EVERY`?
    pub fn due(&self, now: Instant) -> bool {
        self.seen()
            .polled
            .is_none_or(|at| now.saturating_duration_since(at) >= HIDDEN_EVERY)
    }

    /// Shows `n` through `set` unless the Dock already shows it; true when `set` ran and took it.
    /// A `set` that fails leaves the count to be shown again next time. The lock is held through
    /// `set`, so of a poll and a hidden read at once, the Dock ends on the one recorded last.
    pub fn show(&self, n: usize, set: impl FnOnce(Option<i64>) -> bool) -> bool {
        let mut seen = self.seen();
        if seen.shown == Some(n) {
            return false;
        }
        let took = set(label(n));
        seen.shown = took.then_some(n);
        took
    }

    /// The state behind the lock. Nothing panics while holding it, but a poisoned lock still
    /// holds plain values, so it's used as is.
    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::cell::RefCell;
    use std::fs;

    const CASES: &str = include_str!("../../src/needs-cases.json");

    #[test]
    fn needs_you_gives_the_shared_cases_answers() {
        let cases: Value = serde_json::from_str(CASES).expect("needs-cases.json is JSON");
        let cases = cases["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        for c in cases {
            let s = |i: usize| c[i].as_str().unwrap();
            let b = |i: usize| c[i].as_bool().unwrap();
            assert_eq!(needs_you(s(0), s(1), b(2), b(4)), b(3), "{c}");
        }
    }

    #[test]
    fn counts_every_task_that_needs_the_owner_as_the_core_reads_them() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("README.md"), "queue").unwrap();
        let put = |name: &str, front: &str| {
            let text = format!("---\n{front}\n---\n\n# {name}\n\n## Log\n- 2026-10-13 captured\n");
            fs::write(d.path().join(name), text).unwrap();
        };
        put("0001-buy.md", "id: 0001\nstage: ready\nfor: human");
        put("0002-paid.md", "id: 0002\nstage: done\nfor: human");
        put("0003-check.md", "id: 0003\nstage: your-check");
        put(
            "0004-build.md",
            "id: 0004\nstage: building\nclaimed-by: wt-a (session aaaa1111)",
        );
        put(
            "0005-ask.md",
            "id: 0005\nstage: building\nwaiting_on: owner: which colour?",
        );
        // The owner paused their own task (0045): out of the count until they resume it.
        put(
            "0006-later.md",
            "id: 0006\nstage: ready\nfor: human\npaused: 2026-10-13 10:00",
        );
        let s = crate::queue::read_state(
            &[("q".into(), d.path().to_path_buf())],
            &mut crate::queue::Cache::default(),
        );
        assert_eq!(s.tasks.len(), 6, "{:?}", s.problems);
        assert_eq!(needs_you_count(&s.tasks), 3);
        assert_eq!(needs_you_count(&[]), 0);
        assert_eq!(dock_count(&s.tasks, &s.queues), 3, "no lessons waiting");
        // Two lessons from reviews proposed and waiting for the owner's yes (0027) count too; an
        // accepted one doesn't.
        fs::create_dir(d.path().join("reviews")).unwrap();
        let lesson = |id: &str, state: &str| {
            format!(
                r#"{{"id":"{id}","kind":"k-{id}","for":"builders","state":"{state}","rule":"r","proposed":"2026-10-13 10:00"}}"#
            )
        };
        fs::write(
            d.path().join("reviews/lessons.json"),
            format!(
                r#"{{"lessons":[{},{},{}]}}"#,
                lesson("L1", "proposed"),
                lesson("L2", "accepted"),
                lesson("L3", "proposed")
            ),
        )
        .unwrap();
        let s = crate::queue::read_state(
            &[("q".into(), d.path().to_path_buf())],
            &mut crate::queue::Cache::default(),
        );
        assert_eq!(s.queues[0].lessons_waiting, 2);
        assert_eq!(dock_count(&s.tasks, &s.queues), 5);
    }

    #[test]
    fn the_dock_shows_no_badge_at_zero_and_the_count_otherwise() {
        assert_eq!(label(0), None);
        assert_eq!(label(1), Some(1));
        assert_eq!(label(120), Some(120));
        assert_eq!(label(usize::MAX), Some(i64::MAX));
    }

    #[test]
    fn shows_a_count_only_when_it_changes_and_again_after_a_failed_set() {
        let b = Badge::default();
        let sets = RefCell::new(Vec::new());
        let set = |ok: bool| {
            let sets = &sets;
            move |l: Option<i64>| {
                sets.borrow_mut().push(l);
                ok
            }
        };
        // The first count is shown, even 0: an earlier run may have left a badge.
        assert!(b.show(0, set(true)));
        assert!(!b.show(0, set(true)));
        assert!(b.show(2, set(true)));
        assert!(!b.show(2, set(true)));
        // A set that fails is tried again with the next count, even the same one.
        assert!(!b.show(3, set(false)));
        assert!(b.show(3, set(true)));
        assert!(b.show(0, set(true)));
        assert_eq!(*sets.borrow(), [None, Some(2), Some(3), Some(3), None]);
    }

    #[test]
    fn reads_on_its_own_only_while_the_window_has_not_polled_for_a_while() {
        let b = Badge::default();
        let t0 = Instant::now();
        // No poll yet (the window hasn't loaded, or never shows): due.
        assert!(b.due(t0));
        b.polled(t0);
        assert!(!b.due(t0));
        assert!(!b.due(t0 + HIDDEN_EVERY - Duration::from_millis(1)));
        assert!(b.due(t0 + HIDDEN_EVERY));
        // A later poll puts it off again.
        b.polled(t0 + HIDDEN_EVERY);
        assert!(!b.due(t0 + HIDDEN_EVERY + Duration::from_secs(1)));
    }
}
