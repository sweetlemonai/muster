//! A project's label and category (task 0044): the name the window shows for it ("Muster",
//! "Drover — agents") and the heading the sidebar groups it under ("Apps", "Tools"). Kept in
//! `projects.json` by queue folder (`labels`, `categories`), beside its gate and Your check limit,
//! so they never edit the queue or the repo, and a queue that moves takes them along. A label is
//! only shown: the folder, the queue and `muster` commands keep the project's real name. A missing
//! or bad entry reads as none: the name, under its group's heading (a queue one level deeper,
//! `persianUtils`) or "Other".

use crate::projects::Project;
use crate::queue::QueueInfo;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// The longest label and category kept, in characters.
pub const MAX_LABEL: usize = 40;
pub const MAX_CATEGORY: usize = 24;
/// The heading of the projects with no category.
pub const OTHER: &str = "Other";

/// One line as it's kept: control characters out, the spaces around it trimmed, at most `max`
/// characters.
pub fn clean(s: &str, max: usize) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    let s: String = s.trim().chars().take(max).collect();
    s.trim_end().to_string()
}

/// projects.json's `labels` or `categories`, entry by entry, each cleaned: one that isn't text,
/// or is empty once cleaned, is left out, and anything but an object reads as none, so the rest
/// of the file still loads.
fn read_map<'de, D: Deserializer<'de>>(
    d: D,
    max: usize,
) -> Result<HashMap<String, String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(folder, v)| {
                    let s = clean(v.as_str()?, max);
                    (!s.is_empty()).then(|| (folder.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn read_labels<'de, D: Deserializer<'de>>(d: D) -> Result<HashMap<String, String>, D::Error> {
    read_map(d, MAX_LABEL)
}

pub fn read_categories<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<HashMap<String, String>, D::Error> {
    read_map(d, MAX_CATEGORY)
}

/// Set the entry of the queue folder `key` to `value`, cleaned to at most `max` characters; empty
/// takes it out (the name, or the default heading).
pub fn set(map: &mut HashMap<String, String>, key: &str, value: &str, max: usize) {
    let v = clean(value, max);
    if v.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), v);
    }
}

/// What the window calls the project: its label, else its name.
pub fn shown_label(p: &Project) -> String {
    if p.label.is_empty() {
        p.name.clone()
    } else {
        p.label.clone()
    }
}

/// The heading the project goes under: its category, else its group (the folder that holds a
/// queue one level deeper), else "Other", in any case written "Other".
pub fn shown_category(p: &Project) -> String {
    let c = if p.category.is_empty() {
        &p.group
    } else {
        &p.category
    };
    if c.is_empty() || c.eq_ignore_ascii_case(OTHER) {
        OTHER.to_string()
    } else {
        c.clone()
    }
}

/// Each queue's label and category in the state, from its project in `list` (the same folder).
pub fn apply(list: &[Project], queues: &mut [QueueInfo]) {
    for q in queues {
        if let Some(p) = list.iter().find(|p| p.path == q.path) {
            q.label = shown_label(p);
            q.category = shown_category(p);
        }
    }
}

/// The window's title with the project named `name` shown: "<label> — Muster"; "Muster" for All
/// and for a name that isn't a shown project.
pub fn window_title(list: &[Project], name: &str) -> String {
    match list.iter().find(|p| p.name == name && !p.hidden) {
        Some(p) => format!("{} — Muster", shown_label(p)),
        None => "Muster".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{self, Config};
    use std::fs;
    use std::path::Path;

    fn make_queue(dir: &Path) {
        fs::create_dir_all(dir.join("todo")).unwrap();
        fs::write(dir.join("README.md"), "q").unwrap();
    }

    fn config(json: &str) -> Config {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn clean_strips_control_characters_trims_and_keeps_40_or_24_characters() {
        assert_eq!(clean("  Muster\u{7}\n ", MAX_LABEL), "Muster");
        assert_eq!(clean("Dro\tver — agents", MAX_LABEL), "Drover — agents");
        assert_eq!(clean("\u{1b}[31m", MAX_LABEL), "[31m");
        let long = "é".repeat(50);
        assert_eq!(clean(&long, MAX_LABEL).chars().count(), 40);
        assert_eq!(clean(&long, MAX_CATEGORY).chars().count(), 24);
        // Cut at 40, no space left at the end.
        let spaced = format!("{} b", "a".repeat(39));
        assert_eq!(clean(&spaced, MAX_LABEL), "a".repeat(39));
        assert_eq!(clean(" \n\t ", MAX_LABEL), "");
        assert_eq!(clean("Apps 🍋", MAX_CATEGORY), "Apps 🍋");
    }

    #[test]
    fn old_files_without_labels_load_and_bad_entries_are_dropped_without_losing_the_rest() {
        let c = config(r#"{"names": {"/a": "A"}, "gate": {"/a": "bin/gate"}}"#);
        assert!(c.labels.is_empty() && c.categories.is_empty());
        assert_eq!(c.gate.len(), 1);
        let c = config(
            r#"{"names": {"/a": "A"},
                "labels": {"/a": "Muster\n", "/b": 5, "/c": null, "/d": "   ", "/e": ["x"], "/f": "Drover — agents"},
                "categories": {"/a": "Apps", "/b": true, "/c": "\u0007", "/d": "Muster family"}}"#,
        );
        assert_eq!(c.names.len(), 1, "the rest is kept");
        assert_eq!(
            c.labels,
            HashMap::from([
                ("/a".to_string(), "Muster".to_string()),
                ("/f".to_string(), "Drover — agents".to_string()),
            ])
        );
        assert_eq!(
            c.categories,
            HashMap::from([
                ("/a".to_string(), "Apps".to_string()),
                ("/d".to_string(), "Muster family".to_string()),
            ])
        );
        // Too long by hand: cut to the limit, as the window would have.
        let c = config(&format!(r#"{{"labels": {{"/a": "{}"}}}}"#, "x".repeat(90)));
        assert_eq!(c.labels["/a"].len(), 40);
        for broken in [
            r#"{"labels": "Muster", "categories": 3, "gate": {"/a": "g"}}"#,
            r#"{"labels": null, "categories": [], "gate": {"/a": "g"}}"#,
        ] {
            let c = config(broken);
            assert!(c.labels.is_empty() && c.categories.is_empty(), "{broken}");
            assert_eq!(c.gate.len(), 1, "{broken}: the rest is kept");
        }
    }

    #[test]
    fn labels_and_categories_are_saved_by_folder_and_read_back() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg/projects.json");
        projects::edit(&f, |c| {
            set(&mut c.labels, "/p/a/tasks", " Muster ", MAX_LABEL);
            set(&mut c.categories, "/p/a/tasks", "Apps", MAX_CATEGORY);
            Ok(())
        })
        .unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(saved["labels"], serde_json::json!({"/p/a/tasks": "Muster"}));
        assert_eq!(
            saved["categories"],
            serde_json::json!({"/p/a/tasks": "Apps"})
        );
        let c = projects::load(&f);
        assert_eq!(c.labels["/p/a/tasks"], "Muster");
        // Empty takes the entry out.
        projects::edit(&f, |c| {
            set(&mut c.labels, "/p/a/tasks", "\n", MAX_LABEL);
            set(&mut c.categories, "/p/a/tasks", "", MAX_CATEGORY);
            Ok(())
        })
        .unwrap();
        let c = projects::load(&f);
        assert!(c.labels.is_empty() && c.categories.is_empty());
        // A broken file falls back to none: folder names, under Other.
        fs::write(&f, "{ not json").unwrap();
        assert_eq!(projects::load(&f), Config::default());
    }

    #[test]
    fn a_label_is_only_shown_the_project_keeps_its_name_for_commands() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("muster/tasks"));
        make_queue(&base.path().join("persianUtils/colors/tasks"));
        make_queue(&base.path().join("drover/tasks"));
        let mut c = Config::default();
        let key = |c: &Config, name: &str| {
            projects::list(base.path(), c)
                .into_iter()
                .find(|p| p.name == name)
                .unwrap()
                .key
        };
        let mu = key(&c, "muster");
        set(&mut c.labels, &mu, "Muster", MAX_LABEL);
        set(&mut c.categories, &mu, "Apps", MAX_CATEGORY);
        let list = projects::list(base.path(), &c);
        let p = list.iter().find(|p| p.key == mu).unwrap();
        assert_eq!((p.name.as_str(), p.label.as_str()), ("muster", "Muster"));
        assert_eq!(shown_label(p), "Muster");
        assert_eq!(shown_category(p), "Apps");
        // Commands name it by its real name, the one `active` reads queues by.
        let names: Vec<String> = projects::active(base.path(), &c)
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names, ["drover", "muster", "colors"]);
        let json = serde_json::to_value(p).unwrap();
        assert_eq!(json["name"], "muster");
        assert_eq!(json["label"], "Muster");
        assert_eq!(json["category"], "Apps");
        // No label: the name; no category: its group, else Other.
        let colors = list.iter().find(|p| p.name == "colors").unwrap();
        assert_eq!(shown_label(colors), "colors");
        assert_eq!(shown_category(colors), "persianUtils");
        let drover = list.iter().find(|p| p.name == "drover").unwrap();
        assert_eq!((drover.label.as_str(), drover.category.as_str()), ("", ""));
        assert_eq!(shown_category(drover), OTHER);
        // "other" in any case is the Other heading, over a group too.
        let ck = key(&c, "colors");
        set(&mut c.categories, &ck, "other", MAX_CATEGORY);
        let list = projects::list(base.path(), &c);
        let colors = list.iter().find(|p| p.name == "colors").unwrap();
        assert_eq!(shown_category(colors), OTHER);
    }

    #[test]
    fn the_state_carries_each_queues_label_and_category() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("muster/tasks"));
        make_queue(&base.path().join("drover/tasks"));
        let mut c = Config::default();
        let list = projects::list(base.path(), &c);
        let mu = list
            .iter()
            .find(|p| p.name == "muster")
            .unwrap()
            .key
            .clone();
        set(&mut c.labels, &mu, "Muster", MAX_LABEL);
        set(&mut c.categories, &mu, "Tools", MAX_CATEGORY);
        let list = projects::list(base.path(), &c);
        let mut state =
            crate::queue::read_state(&projects::shown(&list), &mut crate::queue::Cache::default());
        // Before: the name and Other, as the CLI reads the state.
        let got = |s: &crate::queue::State| -> Vec<(String, String, String)> {
            s.queues
                .iter()
                .map(|q| (q.name.clone(), q.label.clone(), q.category.clone()))
                .collect()
        };
        assert_eq!(
            got(&state),
            [
                ("drover".into(), "drover".into(), "Other".into()),
                ("muster".into(), "muster".into(), "Other".into()),
            ]
        );
        apply(&list, &mut state.queues);
        assert_eq!(
            got(&state),
            [
                ("drover".into(), "drover".into(), "Other".into()),
                ("muster".into(), "Muster".into(), "Tools".into()),
            ]
        );
        let json = serde_json::to_value(&state.queues[1]).unwrap();
        assert_eq!(
            (json["label"].as_str(), json["category"].as_str()),
            (Some("Muster"), Some("Tools"))
        );
    }

    #[test]
    fn the_window_title_names_the_project_by_its_label() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("muster/tasks"));
        make_queue(&base.path().join("drover/tasks"));
        let mut c = Config::default();
        let list = projects::list(base.path(), &c);
        let mu = list
            .iter()
            .find(|p| p.name == "muster")
            .unwrap()
            .key
            .clone();
        set(&mut c.labels, &mu, "Muster", MAX_LABEL);
        let list = projects::list(base.path(), &c);
        assert_eq!(window_title(&list, "muster"), "Muster — Muster");
        assert_eq!(window_title(&list, "drover"), "drover — Muster");
        assert_eq!(window_title(&list, "all"), "Muster");
        assert_eq!(
            window_title(&list, "Muster"),
            "Muster",
            "by name, never by label"
        );
        assert_eq!(window_title(&list, "nowhere"), "Muster");
        c.hidden.push(mu);
        assert_eq!(
            window_title(&projects::list(base.path(), &c), "muster"),
            "Muster"
        );
    }
}
