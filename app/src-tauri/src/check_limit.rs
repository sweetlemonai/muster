//! A limit on Your check per project (task 0050): how many of the agents' tasks may wait for the
//! owner's check before Muster warns. Kept in `projects.json` by queue folder (`yourCheck`),
//! beside the gate and the display name, so a rename keeps it and a queue that moves takes it
//! along. 5 unless set to 1–50 or Off; a missing or bad value reads as 5, and never stops the
//! rest of the file being read. It only warns: nothing is refused because of it.

use crate::queue;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// The limit's bounds, in tasks, and its default.
pub const MIN: usize = 1;
pub const MAX: usize = 50;
pub const DEFAULT: usize = 5;

/// A project's limit: a number of tasks, or Off. Saved as the number, or `"off"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    At(usize),
    Off,
}

impl Serialize for Limit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Limit::At(n) => s.serialize_u64(*n as u64),
            Limit::Off => s.serialize_str("off"),
        }
    }
}

/// A saved limit: a whole number within the bounds, or `"off"`.
fn read(v: &serde_json::Value) -> Option<Limit> {
    if v.as_str() == Some("off") {
        return Some(Limit::Off);
    }
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (MIN..=MAX).contains(n))
        .map(Limit::At)
}

/// projects.json's `yourCheck`, entry by entry: a bad one is left out (that folder has the
/// default), and anything but an object reads as none, so the rest of the file still loads.
pub fn read_limits<'de, D: Deserializer<'de>>(d: D) -> Result<HashMap<String, Limit>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(folder, v)| Some((folder.clone(), read(v)?)))
                .collect()
        })
        .unwrap_or_default())
}

/// The limit of the queue at `root`: its own (the same folder, through symlinks too), else the
/// default; None when it's Off.
pub fn limit_for(limits: &HashMap<String, Limit>, root: &Path) -> Option<usize> {
    let own = limits.get(root.to_string_lossy().as_ref()).or_else(|| {
        let canon = |p: &Path| std::fs::canonicalize(p).ok();
        let root = canon(root)?;
        limits
            .iter()
            .find(|(folder, _)| canon(Path::new(folder.as_str())).as_ref() == Some(&root))
            .map(|(_, l)| l)
    });
    match own {
        Some(Limit::At(n)) => Some(*n),
        Some(Limit::Off) => None,
        None => Some(DEFAULT),
    }
}

/// Each queue's limit in the state, from projects.json (`queue::read_state` leaves the default).
pub fn apply(limits: &HashMap<String, Limit>, queues: &mut [queue::QueueInfo]) {
    for q in queues {
        q.your_check_limit = limit_for(limits, Path::new(&q.path));
    }
}

/// Set the limit of the queue folder `key` to a number from 1 to 50 or `off` (any case); empty is
/// the default, so its entry goes. A value that isn't one changes nothing.
pub fn set(limits: &mut HashMap<String, Limit>, key: &str, value: &str) -> Result<(), String> {
    let limit = match value.trim().to_lowercase().as_str() {
        "" => None,
        "off" => Some(Limit::Off),
        n => Some(
            n.parse::<usize>()
                .ok()
                .filter(|n| (MIN..=MAX).contains(n))
                .map(Limit::At)
                .ok_or_else(|| {
                    format!(
                        "the Your check limit can be from {MIN} to {MAX}, or off, not {value:?}"
                    )
                })?,
        ),
    };
    match limit {
        Some(l) => limits.insert(key.to_string(), l),
        None => limits.remove(key),
    };
    Ok(())
}

/// How many tasks wait in Your check, as the limit counts them: agents' tasks, not goals or
/// features (theirs roll up from their tasks) and not the owner's own (`for: human`), which skip
/// Your check.
pub fn waiting(tasks: &[queue::Task]) -> usize {
    tasks
        .iter()
        .filter(|t| t.stage == "your-check" && t.level == "task" && !t.is_human())
        .count()
}

/// What `next`, `start`, `ready --parallel` and `list` say on stderr when more than `limit` tasks
/// wait in Your check; None within it or with the limit Off. It never refuses anything.
pub fn over_line(waiting: usize, limit: Option<usize>) -> Option<String> {
    let limit = limit?;
    (waiting > limit).then(|| {
        format!("muster: {waiting} tasks wait in Your check (limit {limit}): the owner's review is the bottleneck.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::Config;
    use std::fs;

    /// A task of `level`, `for` and `stage`, as the queue reads it.
    fn task(id: &str, stage: &str, level: &str, for_who: &str) -> queue::Task {
        let text =
            format!("---\nid: {id}\nstage: {stage}\nlevel: {level}\nfor: {for_who}\n---\n\n# T\n");
        queue::parse_task("q", Path::new("/q"), "", Path::new("/q/x.md"), &text, 0).unwrap()
    }

    #[test]
    fn only_agents_tasks_waiting_in_your_check_count() {
        let tasks = [
            task("0001", "your-check", "task", "ai"),
            task("0002", "your-check", "task", "ai"),
            // The owner's own task, a feature and a goal don't count; nor do other stages.
            task("0003", "your-check", "task", "human"),
            task("0004", "your-check", "feature", "ai"),
            task("0005", "your-check", "goal", "ai"),
            task("0006", "testing", "task", "ai"),
            task("0007", "done", "task", "ai"),
        ];
        assert_eq!(waiting(&tasks), 2);
        assert_eq!(waiting(&[]), 0);
    }

    #[test]
    fn the_over_line_says_how_many_and_the_limit_only_when_over() {
        assert_eq!(
            over_line(7, Some(5)).as_deref(),
            Some(
                "muster: 7 tasks wait in Your check (limit 5): the owner's review is the bottleneck."
            )
        );
        assert!(over_line(6, Some(5)).is_some());
        assert_eq!(over_line(5, Some(5)), None, "at the limit isn't over");
        assert_eq!(over_line(0, Some(5)), None);
        assert_eq!(over_line(40, None), None, "off");
    }

    /// projects.json with `json` in it, read as the app and the command line read it.
    fn config(json: &str) -> Config {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("projects.json");
        fs::write(&f, json).unwrap();
        crate::projects::load(&f)
    }

    #[test]
    fn the_limit_is_kept_by_queue_folder_in_projects_json_and_defaults_to_5() {
        let c = config(
            r#"{"gate": {"/p/a/tasks": "bin/gate"}, "yourCheck": {"/p/a/tasks": 8, "/p/b/tasks": "off", "/p/c/tasks": 1, "/p/d/tasks": 50}}"#,
        );
        assert_eq!(c.gate.len(), 1, "the rest of the file is read as before");
        assert_eq!(limit_for(&c.your_check, Path::new("/p/a/tasks")), Some(8));
        assert_eq!(limit_for(&c.your_check, Path::new("/p/b/tasks")), None);
        assert_eq!(limit_for(&c.your_check, Path::new("/p/c/tasks")), Some(1));
        assert_eq!(limit_for(&c.your_check, Path::new("/p/d/tasks")), Some(50));
        assert_eq!(
            limit_for(&c.your_check, Path::new("/p/e/tasks")),
            Some(5),
            "not set"
        );
        assert_eq!(
            limit_for(&Config::default().your_check, Path::new("/x")),
            Some(5)
        );
    }

    #[test]
    fn a_bad_limit_reads_as_the_default_and_never_loses_the_rest_of_projects_json() {
        let c = config(
            r#"{"names": {"/p/a/tasks": "A"}, "yourCheck": {"/a": 0, "/b": 51, "/c": -1, "/d": 2.5, "/e": "5", "/f": "OFF", "/g": null, "/h": true, "/i": [3], "/ok": 7}}"#,
        );
        assert_eq!(c.names.get("/p/a/tasks").map(String::as_str), Some("A"));
        for p in ["/a", "/b", "/c", "/d", "/e", "/f", "/g", "/h", "/i"] {
            assert_eq!(limit_for(&c.your_check, Path::new(p)), Some(5), "{p}");
        }
        assert_eq!(limit_for(&c.your_check, Path::new("/ok")), Some(7));
        assert_eq!(c.your_check.len(), 1, "only the good one is kept");
        for broken in [
            r#"{"yourCheck": 5}"#,
            r#"{"yourCheck": "off"}"#,
            r#"{"yourCheck": null}"#,
        ] {
            let c = config(&format!("{{\"names\": {{\"/x\": \"X\"}}, {}", &broken[1..]));
            assert!(c.your_check.is_empty(), "{broken}");
            assert_eq!(c.names.len(), 1, "{broken}: the rest is kept");
        }
    }

    #[test]
    fn set_takes_1_to_50_or_off_and_empty_is_the_default() {
        let mut m = HashMap::new();
        set(&mut m, "/p/a/tasks", " 8 ").unwrap();
        set(&mut m, "/p/b/tasks", "Off").unwrap();
        assert_eq!(limit_for(&m, Path::new("/p/a/tasks")), Some(8));
        assert_eq!(limit_for(&m, Path::new("/p/b/tasks")), None);
        // Saved as the number or "off", which is how projects.json reads them back.
        let c = Config {
            your_check: m.clone(),
            ..Default::default()
        };
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("projects.json");
        crate::projects::save(&f, &c).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(
            saved["yourCheck"],
            serde_json::json!({"/p/a/tasks": 8, "/p/b/tasks": "off"})
        );
        assert_eq!(crate::projects::load(&f), c);
        set(&mut m, "/p/a/tasks", "1").unwrap();
        assert_eq!(limit_for(&m, Path::new("/p/a/tasks")), Some(1));
        set(&mut m, "/p/a/tasks", "50").unwrap();
        assert_eq!(limit_for(&m, Path::new("/p/a/tasks")), Some(50));
        set(&mut m, "/p/a/tasks", "").unwrap();
        assert!(
            !m.contains_key("/p/a/tasks"),
            "empty: the default, its entry gone"
        );
    }

    #[test]
    fn set_refuses_the_rest_and_changes_nothing() {
        let mut m = HashMap::new();
        set(&mut m, "/p/a/tasks", "8").unwrap();
        for bad in ["0", "51", "-1", "2.5", "five", "on", "1e1"] {
            assert_eq!(
                set(&mut m, "/p/a/tasks", bad).unwrap_err(),
                format!("the Your check limit can be from 1 to 50, or off, not {bad:?}")
            );
        }
        assert_eq!(limit_for(&m, Path::new("/p/a/tasks")), Some(8));
    }

    #[test]
    fn a_limit_follows_the_folder_through_a_rename_and_never_another_project_of_the_same_name() {
        let base = tempfile::tempdir().unwrap();
        let mk = |rel: &str| {
            let q = base.path().join(rel);
            fs::create_dir_all(q.join("todo")).unwrap();
            fs::write(q.join("README.md"), "q").unwrap();
            q
        };
        let a = mk("one/app/tasks");
        let b = mk("two/app/tasks");
        let mut c = Config::default();
        let list = crate::projects::list(base.path(), &c);
        let key_of = |q: &Path| {
            list.iter()
                .find(|p| Path::new(&p.path) == q)
                .unwrap()
                .key
                .clone()
        };
        set(&mut c.your_check, &key_of(&a), "2").unwrap();
        // Renamed: the limit stays with its folder.
        c.names.insert(key_of(&a), "Renamed".into());
        // Two projects shown under one name keep their own limits.
        c.names.insert(key_of(&b), "Renamed".into());
        let mut state = crate::queue::read_state(
            &crate::projects::active(base.path(), &c),
            &mut crate::queue::Cache::default(),
        );
        apply(&c.your_check, &mut state.queues);
        let limit = |q: &Path| {
            state
                .queues
                .iter()
                .find(|i| Path::new(&i.path) == q)
                .unwrap()
                .your_check_limit
        };
        assert_eq!(limit(&a), Some(2));
        assert_eq!(limit(&b), Some(5));
        // The command line finds it by the folder however it's spelled (a symlink here).
        let link = base.path().join("link");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert_eq!(limit_for(&c.your_check, &link), Some(2));
        assert_eq!(limit_for(&c.your_check, &b), Some(5));
    }
}
