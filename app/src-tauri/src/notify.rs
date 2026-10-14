//! Mac notifications (task 0052): one notification when something new waits for the owner, so
//! they hear of it without watching the Dock badge. What waits is the badge's rule (`badge.rs`:
//! the needs-you count plus the lessons to approve), split into items, one for each reason a task
//! waits (in Your check, a question escalated to them, blocked on them or by a permission, its
//! session waiting for input, their own task) and one for each lesson. An item is notified once
//! and never again. What waited when the app started, when its project first showed or when
//! notifications were turned on again, counts as seen, so nothing floods; so does a session the
//! first read of Claude Code finds waiting for input, and what comes in while the window is
//! focused on its project. News from another project waits while the window is focused (macOS
//! shows no banner then) and comes once it isn't. While Settings has notifications off, nothing is
//! read for them. Several new at once go in one notification.
//! The keys seen are kept, bounded, in `notified.json` beside `settings.json`, so a restart
//! doesn't notify again. Posted from the core through `tauri-plugin-notification`, so the window
//! needs no permission for it. Local only: nothing leaves the Mac.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::agents::{SessionState, Snapshot};
use crate::queue::{QueueInfo, State, Task};

/// The file of keys seen, in the app's config folder.
pub const FILE: &str = "notified.json";
/// How many keys are kept: the oldest go past this, unless they still wait.
pub const KEEP: usize = 500;
/// The most of the file read; a bigger one reads as none.
const MAX_FILE: u64 = 1024 * 1024;
/// How many items a notification names; the rest are "and n more".
const LINES: usize = 3;
/// How much of a task's title or a lesson's rule a line shows.
const SHORT: usize = 80;
/// How much of one line of a notification shows, title included: a `SHORT` title fits with its
/// key, why and project, and nothing it names (a task id, a project's name) runs on.
const LONG: usize = 200;

/// One thing waiting on the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What names it, never what it says: its project, its task and why, or its lesson
    /// (`muster/0012/check@2026-10-13 10:00`, `muster/lesson/L3`).
    pub key: String,
    /// Its project's name, as the window names the project it shows.
    pub queue: String,
    /// Its project's label (0044), for the notification.
    pub label: String,
    /// What the notification says about it: `mu-12 Fix the board: ready for your check`.
    pub line: String,
    /// Its session waits for input: Claude Code's word (0048), known only once it was read.
    pub input: bool,
}

/// What waits on the owner in a read: the tasks the needs-you count counts, by why, and the
/// lessons waiting for their yes (read from each queue that has some).
pub fn waiting(tasks: &[Task], queues: &[QueueInfo]) -> Vec<Item> {
    let labels: HashMap<&str, &str> = queues
        .iter()
        .map(|q| (q.name.as_str(), q.label.as_str()))
        .collect();
    let mut items: Vec<Item> = tasks
        .iter()
        .flat_map(|t| {
            let label = labels.get(t.queue.as_str()).copied().unwrap_or(&t.queue);
            task_items(t, label)
        })
        .collect();
    for q in queues.iter().filter(|q| q.lessons_waiting > 0) {
        // Parsed only when `lessons.json` changed, as for the count. One that doesn't read has
        // nothing to say here; `muster check` reports it.
        items.extend(lesson_items(
            q,
            &crate::lessons::proposed(Path::new(&q.path)),
        ));
    }
    items
}

/// A task's items: none unless it needs the owner (`badge::needs_you`), else one for each reason.
fn task_items(t: &Task, label: &str) -> Vec<Item> {
    if !crate::badge::needs_you(
        &t.for_who,
        &t.stage,
        t.waiting_for_owner,
        !t.paused.is_empty(),
    ) {
        return Vec::new();
    }
    let mut why: Vec<(String, &str)> = Vec::new();
    if t.for_who == "human" {
        why.push(("own".into(), "your task"));
    } else {
        // `queue::needs_owner`'s reasons, and a session waiting for input (`agents::apply`).
        if t.stage == "your-check" {
            // Named by when it came in: back in Your check after a return is news again.
            why.push((format!("check@{}", t.stage_since), "ready for your check"));
        }
        for q in t.questions.iter().filter(|q| !q.ticked && q.escalated) {
            let n = q.n.map(|n| n.to_string()).unwrap_or_default();
            why.push((format!("q{n}"), "asks you a question"));
        }
        match t.blocked_kind.as_str() {
            "you" => why.push(("blocked".into(), "blocked on you")),
            "permission" => why.push(("blocked".into(), "needs your permission")),
            _ => {}
        }
        if t.session_state == SessionState::NeedsInput {
            why.push(("input".into(), "its session waits for your input"));
        }
        if why.is_empty() {
            why.push(("waiting".into(), "waits for you"));
        }
    }
    let key = crate::key::key(&t.prefix, &t.id);
    why.into_iter()
        .map(|(k, words)| Item {
            input: k == "input",
            key: format!("{}/{}/{k}", t.queue, t.id),
            queue: t.queue.clone(),
            label: label.to_string(),
            line: format!("{key} {}: {words}", short(&t.title)),
        })
        .collect()
}

/// The lessons of `q` waiting for the owner's yes (0027), by id and rule, one item each.
fn lesson_items(q: &QueueInfo, proposed: &[(String, String)]) -> Vec<Item> {
    proposed
        .iter()
        .map(|(id, rule)| Item {
            key: format!("{}/lesson/{id}", q.name),
            queue: q.name.clone(),
            label: q.label.clone(),
            line: format!("A lesson to approve: {}", short(rule)),
            input: false,
        })
        .collect()
}

/// The first `SHORT` characters of `text`'s first line, with … when there was more.
fn short(text: &str) -> String {
    cut(text.lines().next().unwrap_or("").trim(), SHORT)
}

/// The first `n` characters of `line`, with … when there was more.
fn cut(line: &str, n: usize) -> String {
    let mut cut: String = line.chars().take(n).collect();
    if cut.len() < line.len() {
        cut.push('…');
    }
    cut
}

/// What to notify after a read of `queues` (by name) found `items` waiting: the items not seen
/// before, each taken as seen now. What waited before Muster could know of it is seen, not news:
/// in a project the last read didn't have (on the first read since the app started, none had),
/// and a session waiting for input until a read before this one knew the sessions (`sessions`,
/// `agents::known`). While the window is `focused`, macOS shows no banner, so news is never
/// posted then: what the window shows (the project named `viewing`, or every one for "all") is
/// seen there, and the rest waits, neither posted nor seen, for the first read once it isn't
/// focused. Also says whether the keys seen changed, to save them. Past `KEEP` keys, the oldest
/// go, unless they still wait.
pub fn decide(
    seen: &mut Seen,
    items: &[Item],
    queues: &[String],
    sessions: bool,
    focused: &dyn Fn() -> bool,
    viewing: &str,
) -> (Vec<Item>, bool) {
    let before = seen.queues.replace(queues.iter().cloned().collect());
    let sessions_before = seen.sessions;
    seen.sessions |= sessions;
    let mut new = Vec::new();
    let mut changed = false;
    for i in items {
        if seen.contains(&i.key) {
            continue;
        }
        let read_before = before.as_ref().is_some_and(|b| b.contains(&i.queue));
        let news = read_before && (sessions_before || !i.input);
        if news && focused() && viewing != "all" && viewing != i.queue {
            continue;
        }
        seen.add(&i.key);
        changed = true;
        if news && !focused() {
            new.push(i.clone());
        }
    }
    if seen.order.len() > KEEP {
        let still: HashSet<&str> = items.iter().map(|i| i.key.as_str()).collect();
        let mut over = seen.order.len() - KEEP;
        let Seen { order, keys, .. } = seen;
        order.retain(|k| {
            if over == 0 || still.contains(k.as_str()) {
                return true;
            }
            over -= 1;
            keys.remove(k);
            false
        });
    }
    (new, changed)
}

/// What a notification of `items` says: its title and its body. One item is named in full;
/// several are counted, and the first `LINES` named, each with its project when they're from
/// more than one. Each line, the title too, shows at most `LONG` characters.
pub fn message(items: &[Item]) -> (String, String) {
    let Some(first) = items.first() else {
        return (String::new(), String::new());
    };
    let one_project = items.iter().all(|i| i.queue == first.queue);
    let title = match (items.len(), one_project) {
        (1, _) => format!("New for you in {}", first.label),
        (n, true) => format!("{n} new for you in {}", first.label),
        (n, false) => format!("{n} new for you"),
    };
    let mut lines: Vec<String> = items
        .iter()
        .take(LINES)
        .map(|i| {
            if one_project {
                cut(&i.line, LONG)
            } else {
                cut(&format!("{}: {}", i.label, i.line), LONG)
            }
        })
        .collect();
    if items.len() > LINES {
        lines.push(format!("and {} more", items.len() - LINES));
    }
    (cut(&title, LONG), lines.join("\n"))
}

/// What has been seen: every key, oldest first, the projects of the last read, and whether a read
/// knew the sessions.
#[derive(Debug, Default)]
pub struct Seen {
    order: VecDeque<String>,
    keys: HashSet<String>,
    /// None before the first read since the app started, or since notifications were turned on.
    queues: Option<HashSet<String>>,
    /// Some read since the app started knew what the sessions are doing (`agents::known`).
    sessions: bool,
}

impl Seen {
    /// Takes `key` as seen; false when it already was.
    fn add(&mut self, key: &str) -> bool {
        if !self.keys.insert(key.to_string()) {
            return false;
        }
        self.order.push_back(key.to_string());
        true
    }

    pub fn contains(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    #[serde(default)]
    seen: Vec<String>,
}

/// The keys seen in `file`; none for a missing, broken or oversized one.
pub fn load(file: &Path) -> Seen {
    let mut s = Seen::default();
    let saved = std::fs::metadata(file)
        .ok()
        .filter(|m| m.len() <= MAX_FILE)
        .and_then(|_| std::fs::read_to_string(file).ok())
        .and_then(|t| serde_json::from_str::<Saved>(&t).ok())
        .unwrap_or_default();
    for k in &saved.seen {
        s.add(k);
    }
    s
}

/// Saves the keys seen to `file`, oldest first.
pub fn save(file: &Path, seen: &Seen) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let saved = Saved {
        seen: seen.order.iter().cloned().collect(),
    };
    crate::queue::write_atomic(
        file,
        &(serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())? + "\n"),
    )
}

/// Where a notification goes, and whether the window has the focus: the Mac, or a test's fake.
pub trait Post {
    /// Does Muster's window have the Mac's focus (not hidden, minimized or behind another app)?
    fn focused(&self) -> bool;
    /// Whether Muster may notify; true when it may.
    fn permit(&self) -> bool;
    /// Shows one notification.
    fn post(&self, title: &str, body: &str) -> Result<(), String>;
}

/// The Mac's notifications, through `tauri-plugin-notification`, called from the core only.
pub struct Mac<'a, R: tauri::Runtime>(pub &'a tauri::AppHandle<R>);

impl<R: tauri::Runtime> Post for Mac<'_, R> {
    fn focused(&self) -> bool {
        use tauri::Manager;
        self.0
            .get_webview_window("main")
            .is_some_and(|w| w.is_focused().unwrap_or(false))
    }

    /// The plugin grants it on desktop without asking anyone (2.6.0); whether a notification
    /// shows is up to System Settings › Notifications.
    fn permit(&self) -> bool {
        use tauri_plugin_notification::NotificationExt;
        matches!(
            self.0.notification().request_permission(),
            Ok(tauri::plugin::PermissionState::Granted)
        )
    }

    fn post(&self, title: &str, body: &str) -> Result<(), String> {
        use tauri_plugin_notification::NotificationExt;
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| e.to_string())
    }
}

/// What the notifications remember while the app runs.
pub struct Notifier {
    file: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    /// None until the first read loads the file.
    seen: Option<Seen>,
    /// The project the window shows, by name; "all" for All (`show_project_title`).
    viewing: String,
    /// Whether Muster may notify (`Post::permit`), the first time there's something to say.
    permitted: Option<bool>,
}

impl Notifier {
    /// Keeps what it has seen in `file`.
    pub fn new(file: PathBuf) -> Self {
        Notifier {
            file,
            inner: Mutex::new(Inner {
                seen: None,
                viewing: "all".into(),
                permitted: None,
            }),
        }
    }

    /// The window shows the project named `name` ("all" for All).
    pub fn viewing(&self, name: &str) {
        self.inner().viewing = name.to_string();
    }

    /// Notifications are off: nothing is read for them, and the first read once they're on again
    /// takes what waits then as seen, so what came in meanwhile doesn't come up.
    pub fn off(&self) {
        if let Some(seen) = &mut self.inner().seen {
            seen.queues = None;
        }
    }

    /// After a read of `queues` (by name) found `items` waiting, knowing the sessions or not
    /// (`agents::known`): notifies through `post` what's new, and returns what it notified.
    pub fn read(
        &self,
        items: &[Item],
        queues: &[String],
        sessions: bool,
        post: &dyn Post,
    ) -> Vec<Item> {
        let mut inner = self.inner();
        let Inner {
            seen,
            viewing,
            permitted,
        } = &mut *inner;
        let seen = seen.get_or_insert_with(|| load(&self.file));
        // Asked only when something is new, and once.
        let focused = std::cell::OnceCell::new();
        let focused = || *focused.get_or_init(|| post.focused());
        let (new, changed) = decide(seen, items, queues, sessions, &focused, viewing);
        if changed {
            // Kept in memory either way; a save that fails only loses them across a restart, and
            // what waits then is seen at start, so it brings no flood.
            let _ = save(&self.file, seen);
        }
        if new.is_empty() || !*permitted.get_or_insert_with(|| post.permit()) {
            return Vec::new();
        }
        let (title, body) = message(&new);
        match post.post(&title, &body) {
            Ok(()) => new,
            // Taken as seen all the same: never twice, even when the first didn't show.
            Err(_) => Vec::new(),
        }
    }

    /// The state behind the lock. Nothing panics while holding it, but a poisoned lock still
    /// holds plain values, so it's used as is.
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A Mac notification through `post` of what's new for the owner in the read `s`, whose
/// sessions Claude Code's `sessions` told (0048), unless Settings turned them off: then nothing
/// is read for them. Returns what it notified.
pub fn after_read(n: &Notifier, s: &State, sessions: &Snapshot, post: &dyn Post) -> Vec<Item> {
    if !s.settings.notify {
        n.off();
        return Vec::new();
    }
    let queues: Vec<String> = s.queues.iter().map(|q| q.name.clone()).collect();
    n.read(
        &waiting(&s.tasks, &s.queues),
        &queues,
        crate::agents::known(&s.tasks, sessions),
        post,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;

    /// A queue with one task for each way to wait on the owner, and some that don't.
    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("README.md"), "queue").unwrap();
        let put = |name: &str, front: &str, title: &str, more: &str| {
            let text =
                format!("---\n{front}\n---\n\n# {title}\n\n{more}## Log\n- 2026-10-13 captured\n");
            fs::write(d.path().join(name), text).unwrap();
        };
        put(
            "0001-buy.md",
            "id: 0001\nstage: ready\nfor: human",
            "Buy the domain",
            "",
        );
        put(
            "0002-paid.md",
            "id: 0002\nstage: done\nfor: human",
            "Pay",
            "",
        );
        put(
            "0003-check.md",
            "id: 0003\nstage: your-check",
            "Fix the board",
            "",
        );
        put(
            "0004-build.md",
            "id: 0004\nstage: building\nclaimed-by: wt-a (session aaaa1111)",
            "Build it",
            "",
        );
        put(
            "0005-ask.md",
            "id: 0005\nstage: building\nblocked-kind: you\nwaiting_on: which colour?",
            "Pick a colour",
            "",
        );
        put(
            "0006-later.md",
            "id: 0006\nstage: ready\nfor: human\npaused: 2026-10-13 10:00",
            "Later",
            "",
        );
        put(
            "0007-perm.md",
            "id: 0007\nstage: building\nblocked-kind: permission\nwaiting_on: write access",
            "Needs access",
            "",
        );
        put(
            "0008-q.md",
            "id: 0008\nstage: building",
            "Ask a question",
            "## Questions\n- [ ] **Q1 (wt-a, 2026-10-13 10:00):** Which port?\n- [ ] **Q2 (wt-a, 2026-10-13 10:05):** Product call?\n  **Escalated to owner (capture, 2026-10-13 10:06).**\n\n",
        );
        put(
            "0009-input.md",
            "id: 0009\nstage: building\nclaimed-by: wt-b (session bbbb2222)",
            "Waits for input",
            "",
        );
        d
    }

    fn read(d: &tempfile::TempDir) -> crate::queue::State {
        let mut s = crate::queue::read_state_keyed(
            &[("muster".into(), d.path().to_path_buf())],
            &mut crate::queue::Cache::default(),
            &crate::settings::Settings::default(),
            &|_| "mu".to_string(),
        );
        for q in &mut s.queues {
            q.label = "Muster app".into();
        }
        // Claude Code says this one's session waits for input (0048, `agents::apply`).
        let t = s.tasks.iter_mut().find(|t| t.id == "0009").unwrap();
        t.session_state = SessionState::NeedsInput;
        t.waiting_for_owner = true;
        s
    }

    fn lessons(d: &tempfile::TempDir, ids: &[(&str, &str)]) {
        fs::create_dir_all(d.path().join("reviews")).unwrap();
        let all: Vec<String> = ids
            .iter()
            .map(|(id, state)| {
                format!(
                    r#"{{"id":"{id}","kind":"k-{id}","for":"builders","state":"{state}","rule":"Say why in {id}","proposed":"2026-10-13 10:00"}}"#
                )
            })
            .collect();
        fs::write(
            d.path().join("reviews/lessons.json"),
            format!(r#"{{"lessons":[{}]}}"#, all.join(",")),
        )
        .unwrap();
    }

    fn item(key: &str, queue: &str) -> Item {
        Item {
            key: key.into(),
            queue: queue.into(),
            label: queue.to_uppercase(),
            line: format!("{key} line"),
            input: key.ends_with("/input"),
        }
    }

    const AWAY: &dyn Fn() -> bool = &|| false;

    #[test]
    fn items_are_what_the_badge_counts_one_for_each_reason_and_lesson() {
        let d = queue();
        lessons(
            &d,
            &[("L1", "proposed"), ("L2", "accepted"), ("L3", "proposed")],
        );
        let s = read(&d);
        let items = waiting(&s.tasks, &s.queues);
        let lines: Vec<(&str, &str)> = items
            .iter()
            .map(|i| (i.key.as_str(), i.line.as_str()))
            .collect();
        assert_eq!(
            lines,
            [
                ("muster/0001/own", "mu-1 Buy the domain: your task"),
                (
                    "muster/0003/check@",
                    "mu-3 Fix the board: ready for your check"
                ),
                ("muster/0005/blocked", "mu-5 Pick a colour: blocked on you"),
                (
                    "muster/0007/blocked",
                    "mu-7 Needs access: needs your permission"
                ),
                ("muster/0008/q2", "mu-8 Ask a question: asks you a question"),
                (
                    "muster/0009/input",
                    "mu-9 Waits for input: its session waits for your input"
                ),
                ("muster/lesson/L1", "A lesson to approve: Say why in L1"),
                ("muster/lesson/L3", "A lesson to approve: Say why in L3"),
            ]
        );
        assert!(items
            .iter()
            .all(|i| i.queue == "muster" && i.label == "Muster app"));
        // The same tasks the Dock badge counts, and the same lessons.
        let tasks: HashSet<&str> = items
            .iter()
            .filter(|i| !i.key.contains("/lesson/"))
            .map(|i| i.key.split('/').nth(1).unwrap())
            .collect();
        assert_eq!(
            tasks.len() + 2,
            crate::badge::dock_count(&s.tasks, &s.queues)
        );
    }

    #[test]
    fn a_task_waiting_for_several_reasons_is_an_item_for_each_and_a_new_your_check_is_new() {
        let d = queue();
        fs::write(
            d.path().join("0003-check.md"),
            "---\nid: 0003\nstage: your-check\nblocked-kind: you\nwaiting_on: try it\n---\n\n# Fix the board\n\n## Log\n- 2026-10-13 captured\n- 2026-10-13 11:00 wt-a: stage testing → your-check\n",
        )
        .unwrap();
        let s = read(&d);
        let keys: Vec<String> = waiting(&s.tasks, &s.queues)
            .into_iter()
            .filter(|i| i.key.starts_with("muster/0003/"))
            .map(|i| i.key)
            .collect();
        // Your check is named by when it came in, so coming back after a return is new.
        assert_eq!(
            keys,
            ["muster/0003/check@2026-10-13 11:00", "muster/0003/blocked"]
        );
    }

    #[test]
    fn a_long_title_is_cut_on_a_character() {
        let d = queue();
        let long = "é".repeat(SHORT + 5);
        fs::write(
            d.path().join("0001-buy.md"),
            format!("---\nid: 0001\nstage: ready\nfor: human\n---\n\n# {long}\n\n## Log\n- 2026-10-13 captured\n"),
        )
        .unwrap();
        let s = read(&d);
        let own = waiting(&s.tasks, &s.queues)
            .into_iter()
            .find(|i| i.key == "muster/0001/own")
            .unwrap();
        assert_eq!(own.line, format!("mu-1 {}…: your task", "é".repeat(SHORT)));
    }

    #[test]
    fn nothing_waits_in_an_empty_read() {
        assert!(waiting(&[], &[]).is_empty());
    }

    #[test]
    fn the_first_read_and_a_project_that_just_showed_notify_nothing() {
        let mut seen = Seen::default();
        let a = [item("a/1/own", "a")];
        let qs = ["a".to_string()];
        // At start: what's there is seen, not new.
        let (new, changed) = decide(&mut seen, &a, &qs, true, AWAY, "all");
        assert!(new.is_empty());
        assert!(changed && seen.contains("a/1/own"));
        // A project added or shown again brings what waits in it: seen, not new.
        let both = [item("a/1/own", "a"), item("b/1/own", "b")];
        let qs2 = ["a".to_string(), "b".to_string()];
        let (new, changed) = decide(&mut seen, &both, &qs2, true, AWAY, "all");
        assert!(new.is_empty());
        assert!(changed && seen.contains("b/1/own"));
        // Then something new in it is news.
        let more = [
            item("a/1/own", "a"),
            item("b/1/own", "b"),
            item("b/2/own", "b"),
        ];
        let (new, _) = decide(&mut seen, &more, &qs2, true, AWAY, "all");
        assert_eq!(new, [item("b/2/own", "b")]);
    }

    #[test]
    fn each_new_item_is_notified_once_and_never_again() {
        let mut seen = Seen::default();
        let qs = ["a".to_string()];
        decide(&mut seen, &[], &qs, true, AWAY, "all");
        let one = [item("a/1/q1", "a")];
        let (new, changed) = decide(&mut seen, &one, &qs, true, AWAY, "all");
        assert_eq!(new, one);
        assert!(changed);
        let (new, changed) = decide(&mut seen, &one, &qs, true, AWAY, "all");
        assert!(new.is_empty() && !changed, "still waiting");
        // Gone, then back: the same item, not news.
        decide(&mut seen, &[], &qs, true, AWAY, "all");
        let (new, _) = decide(&mut seen, &one, &qs, true, AWAY, "all");
        assert!(new.is_empty());
        // Several new in one read come together, once each.
        let three = [
            item("a/1/q1", "a"),
            item("a/2/own", "a"),
            item("a/3/input", "a"),
            item("a/3/input", "a"),
        ];
        let (new, _) = decide(&mut seen, &three, &qs, true, AWAY, "all");
        assert_eq!(new, [item("a/2/own", "a"), item("a/3/input", "a")]);
    }

    const FOCUSED: &dyn Fn() -> bool = &|| true;

    #[test]
    fn the_focused_window_sees_its_project_and_the_rest_waits_until_it_loses_the_focus() {
        let mut seen = Seen::default();
        let qs = ["a".to_string(), "b".to_string()];
        decide(&mut seen, &[], &qs, true, AWAY, "all");
        // Focused on B: B's news is seen there; A's isn't posted (macOS shows no banner while
        // Muster is in front) and isn't seen either.
        let items = [item("a/1/own", "a"), item("b/1/own", "b")];
        let (new, changed) = decide(&mut seen, &items, &qs, true, FOCUSED, "b");
        assert!(new.is_empty());
        assert!(changed && seen.contains("b/1/own"));
        assert!(!seen.contains("a/1/own"), "waits, not seen");
        // Still focused on B: still waiting.
        let (new, changed) = decide(&mut seen, &items, &qs, true, FOCUSED, "b");
        assert!(new.is_empty() && !changed);
        assert!(!seen.contains("a/1/own"));
        // Not focused any more: posted once, and B's isn't brought up.
        let (new, _) = decide(&mut seen, &items, &qs, true, AWAY, "b");
        assert_eq!(new, [item("a/1/own", "a")]);
        let (new, _) = decide(&mut seen, &items, &qs, true, AWAY, "b");
        assert!(new.is_empty(), "never twice");
        // Focused on All: everything new is seen in the window, never notified later.
        let more = [item("a/2/own", "a"), item("b/2/own", "b")];
        let (new, _) = decide(&mut seen, &more, &qs, true, FOCUSED, "all");
        assert!(new.is_empty());
        assert!(seen.contains("a/2/own") && seen.contains("b/2/own"));
        let (new, _) = decide(&mut seen, &more, &qs, true, AWAY, "all");
        assert!(new.is_empty());
    }

    #[test]
    fn a_session_waiting_for_input_before_claude_code_was_first_read_is_seen_not_news() {
        let mut seen = Seen::default();
        let qs = ["a".to_string()];
        // At start, before the first `claude agents` read lands: no session states yet.
        decide(&mut seen, &[], &qs, false, AWAY, "all");
        // The first read lands: what it says waits for input waited already.
        let first = [item("a/1/input", "a"), item("a/2/own", "a")];
        let (new, changed) = decide(&mut seen, &first, &qs, true, AWAY, "all");
        assert_eq!(
            new,
            [item("a/2/own", "a")],
            "only the session's is held back"
        );
        assert!(changed && seen.contains("a/1/input"));
        // From then on a session that comes to wait is news.
        let later = [item("a/1/input", "a"), item("a/3/input", "a")];
        let (new, _) = decide(&mut seen, &later, &qs, true, AWAY, "all");
        assert_eq!(new, [item("a/3/input", "a")]);
        // Known once is enough: a later read that doesn't know them doesn't hold news back.
        let more = [item("a/4/input", "a")];
        let (new, _) = decide(&mut seen, &more, &qs, false, AWAY, "all");
        assert_eq!(new, more);
    }

    #[test]
    fn keeps_at_most_keep_keys_the_oldest_going_first_but_never_one_still_waiting() {
        let mut seen = Seen::default();
        let qs = ["a".to_string()];
        decide(&mut seen, &[], &qs, true, AWAY, "all");
        // The first key seen still waits through it all.
        let first = item("a/0/own", "a");
        decide(
            &mut seen,
            std::slice::from_ref(&first),
            &qs,
            true,
            AWAY,
            "all",
        );
        for n in 1..=KEEP + 10 {
            let items = [first.clone(), item(&format!("a/{n}/own"), "a")];
            let (new, _) = decide(&mut seen, &items, &qs, true, AWAY, "all");
            assert_eq!(new.len(), 1);
        }
        assert_eq!(seen.len(), KEEP);
        assert!(seen.contains("a/0/own"), "still waiting, so kept");
        assert!(!seen.contains("a/1/own") && !seen.contains("a/11/own"));
        assert!(seen.contains("a/12/own") && seen.contains(&format!("a/{}/own", KEEP + 10)));
        // More waiting at once than it keeps: every one is kept, so none comes up again.
        let many: Vec<Item> = (0..KEEP + 5)
            .map(|n| item(&format!("b/{n}/own"), "a"))
            .collect();
        decide(&mut seen, &many, &qs, true, AWAY, "all");
        assert_eq!(seen.len(), KEEP + 5);
        let (new, _) = decide(&mut seen, &many, &qs, true, AWAY, "all");
        assert!(new.is_empty());
    }

    #[test]
    fn one_notification_names_one_item_or_counts_several() {
        assert_eq!(message(&[]), (String::new(), String::new()));
        let one = [item("a/1/own", "a")];
        assert_eq!(
            message(&one),
            ("New for you in A".to_string(), "a/1/own line".to_string())
        );
        let two = [item("a/1/own", "a"), item("a/2/own", "a")];
        assert_eq!(
            message(&two),
            (
                "2 new for you in A".to_string(),
                "a/1/own line\na/2/own line".to_string()
            )
        );
        // From more than one project: each line says which, and past three, how many more.
        let five = [
            item("a/1/own", "a"),
            item("b/1/own", "b"),
            item("a/2/own", "a"),
            item("a/3/own", "a"),
            item("a/4/own", "a"),
        ];
        assert_eq!(
            message(&five),
            (
                "5 new for you".to_string(),
                "A: a/1/own line\nB: b/1/own line\nA: a/2/own line\nand 2 more".to_string()
            )
        );
    }

    #[test]
    fn every_line_is_cut_on_a_character_however_long_what_it_names() {
        // A task id, a project name or a label has no bound of its own.
        let long = Item {
            key: "a/1/own".into(),
            queue: "q".repeat(LONG + 5),
            label: "ü".repeat(LONG + 5),
            line: format!("mu-{} Title: your task", "9".repeat(LONG + 5)),
            input: false,
        };
        let (title, body) = message(std::slice::from_ref(&long));
        assert_eq!(title, cut(&format!("New for you in {}", long.label), LONG));
        assert_eq!(title.chars().count(), LONG + 1);
        assert!(title.ends_with('…'));
        assert_eq!(body, format!("mu-{}…", "9".repeat(LONG - 3)));
        let two = [long.clone(), item("b/1/own", "b")];
        let (_, body) = message(&two);
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines[0], format!("{}…", "ü".repeat(LONG)));
        assert_eq!(lines[1], "B: b/1/own line");
        // A line that fits stays whole.
        assert_eq!(cut("mu-1 Fix: your task", LONG), "mu-1 Fix: your task");
    }

    /// A stand-in for the Mac's notifications: what it was asked and what it posted.
    #[derive(Default)]
    struct Fake {
        focused: bool,
        refuse: bool,
        fail: bool,
        asked: RefCell<usize>,
        posted: RefCell<Vec<(String, String)>>,
    }

    impl Post for Fake {
        fn focused(&self) -> bool {
            self.focused
        }

        fn permit(&self) -> bool {
            *self.asked.borrow_mut() += 1;
            !self.refuse
        }

        fn post(&self, title: &str, body: &str) -> Result<(), String> {
            if self.fail {
                return Err("no".into());
            }
            self.posted
                .borrow_mut()
                .push((title.to_string(), body.to_string()));
            Ok(())
        }
    }

    #[test]
    fn asks_leave_at_the_first_thing_to_say_posts_one_batch_and_remembers_across_a_restart() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("ai.sweetlemon.muster").join(FILE);
        let n = Notifier::new(file.clone());
        let fake = Fake::default();
        let qs = ["a".to_string()];
        let start = [item("a/1/own", "a")];
        assert!(n.read(&start, &qs, true, &fake).is_empty());
        assert_eq!(*fake.asked.borrow(), 0, "nothing to say yet: not asked");
        let more = [
            item("a/1/own", "a"),
            item("a/2/q1", "a"),
            item("a/3/own", "a"),
        ];
        assert_eq!(n.read(&more, &qs, true, &fake).len(), 2);
        assert_eq!(*fake.asked.borrow(), 1);
        assert_eq!(
            *fake.posted.borrow(),
            [(
                "2 new for you in A".to_string(),
                "a/2/q1 line\na/3/own line".to_string()
            )]
        );
        let later = [item("a/1/own", "a"), item("a/4/own", "a")];
        assert_eq!(n.read(&later, &qs, true, &fake).len(), 1);
        assert_eq!(*fake.asked.borrow(), 1, "asked once");
        // Restarted: what it saw is remembered; what came while it was closed is seen at start.
        let n = Notifier::new(file.clone());
        let fake = Fake::default();
        let back = [item("a/2/q1", "a"), item("a/5/own", "a")];
        assert!(n.read(&back, &qs, true, &fake).is_empty());
        assert!(n.read(&back, &qs, true, &fake).is_empty());
        let saved = load(&file);
        assert_eq!(saved.len(), 5);
        assert!(fake.posted.borrow().is_empty());
        // A broken file reads as nothing seen.
        fs::write(&file, "not json").unwrap();
        assert!(load(&file).is_empty());
    }

    #[test]
    fn the_focused_window_sees_what_it_shows_and_the_rest_is_posted_once_it_loses_the_focus() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join(FILE);
        let n = Notifier::new(file.clone());
        let focused = Fake {
            focused: true,
            ..Fake::default()
        };
        let qs = ["a".to_string(), "b".to_string()];
        n.read(&[], &qs, true, &focused);
        // All shown, focused: the owner sees every project.
        assert!(n
            .read(&[item("a/1/own", "a")], &qs, true, &focused)
            .is_empty());
        assert!(load(&file).contains("a/1/own"));
        // Focused on B: A's news is neither posted nor seen.
        n.viewing("b");
        let items = [
            item("a/1/own", "a"),
            item("a/2/own", "a"),
            item("b/1/own", "b"),
        ];
        assert!(n.read(&items, &qs, true, &focused).is_empty());
        assert!(n.read(&items, &qs, true, &focused).is_empty());
        assert!(focused.posted.borrow().is_empty());
        let saved = load(&file);
        assert!(saved.contains("b/1/own") && !saved.contains("a/2/own"));
        // Not focused (another app, hidden, minimized): posted at the first read, once.
        let away = Fake::default();
        assert_eq!(n.read(&items, &qs, true, &away), [item("a/2/own", "a")]);
        assert!(n.read(&items, &qs, true, &away).is_empty());
        assert_eq!(away.posted.borrow().len(), 1);
        let items = [item("b/2/own", "b")];
        assert_eq!(n.read(&items, &qs, true, &away), items);
    }

    #[test]
    fn off_reads_and_posts_nothing_and_on_again_doesnt_bring_up_what_came_meanwhile() {
        let d = queue();
        let dir = tempfile::tempdir().unwrap();
        let n = Notifier::new(dir.path().join(FILE));
        let fake = Fake::default();
        let mut s = read(&d);
        let known = Snapshot {
            accounts: vec![(String::new(), Ok(1))],
            ..Snapshot::default()
        };
        let all = s.tasks.clone();
        // Started with one task; the rest come in later.
        s.tasks.retain(|t| t.id == "0001");
        assert!(after_read(&n, &s, &known, &fake).is_empty());
        // Off: the rest comes in, and nothing is posted or kept.
        s.settings.notify = false;
        s.tasks = all.clone();
        assert!(after_read(&n, &s, &known, &fake).is_empty());
        assert!(fake.posted.borrow().is_empty());
        assert!(!load(&dir.path().join(FILE)).contains("muster/0005/blocked"));
        // On again: what came while it was off is seen, not news.
        s.settings.notify = true;
        assert!(after_read(&n, &s, &known, &fake).is_empty());
        assert!(fake.posted.borrow().is_empty());
        // Then what's new is.
        let t = s.tasks.iter_mut().find(|t| t.id == "0004").unwrap();
        t.waiting_for_owner = true;
        let new = after_read(&n, &s, &known, &fake);
        assert_eq!(
            new.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
            ["muster/0004/waiting"]
        );
        assert_eq!(fake.posted.borrow().len(), 1);
    }

    #[test]
    fn a_session_waiting_for_input_when_claude_code_is_first_read_isnt_posted() {
        let d = queue();
        let dir = tempfile::tempdir().unwrap();
        let n = Notifier::new(dir.path().join(FILE));
        let fake = Fake::default();
        let s = read(&d);
        // 0009's holder is stamped, so its session is read; until that read lands it's unknown.
        let mut before = s.clone();
        for t in &mut before.tasks {
            t.session_state = SessionState::Unknown;
        }
        before.tasks.retain(|t| t.id != "0009");
        assert!(!crate::agents::known(&before.tasks, &Snapshot::default()));
        assert!(after_read(&n, &before, &Snapshot::default(), &fake).is_empty());
        // The read lands 2-10 s later: the session waited already.
        let known = Snapshot {
            accounts: vec![(String::new(), Ok(1))],
            ..Snapshot::default()
        };
        assert!(after_read(&n, &s, &known, &fake).is_empty());
        assert!(fake.posted.borrow().is_empty());
    }

    #[test]
    fn refused_leave_or_a_failed_post_notifies_nothing_and_asks_no_more_this_run() {
        let d = tempfile::tempdir().unwrap();
        let qs = ["a".to_string()];
        let n = Notifier::new(d.path().join(FILE));
        let refuse = Fake {
            refuse: true,
            ..Fake::default()
        };
        n.read(&[], &qs, true, &refuse);
        assert!(n
            .read(&[item("a/1/own", "a")], &qs, true, &refuse)
            .is_empty());
        assert!(n
            .read(&[item("a/2/own", "a")], &qs, true, &refuse)
            .is_empty());
        assert_eq!(*refuse.asked.borrow(), 1);
        assert!(refuse.posted.borrow().is_empty());
        let n = Notifier::new(d.path().join("other.json"));
        let fail = Fake {
            fail: true,
            ..Fake::default()
        };
        n.read(&[], &qs, true, &fail);
        assert!(n.read(&[item("a/1/own", "a")], &qs, true, &fail).is_empty());
    }
}
