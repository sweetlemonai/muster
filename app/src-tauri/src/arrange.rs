//! The sidebar's own arrangement (task 0057): the projects in the owner's order (the Mine chip),
//! the category headings in theirs, and a colour per project. Kept in `projects.json` beside the
//! labels and categories (0044): by queue folder (`colours`, `order`) and by heading
//! (`categoryOrder`), so they're this Mac's and never touch a queue or a repo. Changes go through
//! `projects::edit`, under its lock. A missing or bad entry reads as none: no colour, and a
//! project or heading nobody placed comes after the placed ones, A to Z.

use crate::labels::{self, MAX_CATEGORY, OTHER};
use crate::projects::{Config, Project};
use crate::queue::QueueInfo;
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};

/// The colours a project can have, by the id projects.json keeps. The window has their shades
/// (themes.ts `PROJECT_COLOURS`, tested against the stage and priority colours in every theme).
pub const COLOURS: [&str; 5] = ["lime", "orchid", "pink", "tan", "mint"];
/// The most projects, and the most headings, an order keeps.
pub const MAX_ORDER: usize = 500;

/// A heading as the sidebar names it: cleaned as a category is, and "other" in any case is Other
/// (`labels::shown_category`).
fn heading(name: &str) -> String {
    let h = labels::clean(name, MAX_CATEGORY);
    if h.eq_ignore_ascii_case(OTHER) {
        OTHER.to_string()
    } else {
        h
    }
}

/// projects.json's `colours`: each entry that's one of `COLOURS`; anything else is left out, and
/// anything but an object reads as none, so the rest of the file still loads.
pub fn read_colours<'de, D: Deserializer<'de>>(d: D) -> Result<HashMap<String, String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(folder, v)| {
                    let c = v.as_str()?;
                    COLOURS
                        .contains(&c)
                        .then(|| (folder.clone(), c.to_string()))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// A list in projects.json: its text entries, each `f`'d, the empty ones and repeats left out, at
/// most `MAX_ORDER`; anything but a list reads as none.
fn read_list<'de, D: Deserializer<'de>>(
    d: D,
    f: fn(&str) -> String,
) -> Result<Vec<String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let mut seen = HashSet::new();
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(f))
                .filter(|s| !s.is_empty() && seen.insert(s.clone()))
                .take(MAX_ORDER)
                .collect()
        })
        .unwrap_or_default())
}

/// projects.json's `order`: queue folders, first to last.
pub fn read_order<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    read_list(d, str::to_string)
}

/// projects.json's `categoryOrder`: headings, first to last, each cleaned as a category is.
pub fn read_category_order<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    read_list(d, heading)
}

/// Set the colour of the project with the queue folder `key`: one of `COLOURS`, or "" for none.
pub fn set_colour(c: &mut Config, key: &str, colour: &str) -> Result<(), String> {
    let colour = colour.trim();
    if colour.is_empty() {
        c.colours.remove(key);
    } else if COLOURS.contains(&colour) {
        c.colours.insert(key.to_string(), colour.to_string());
    } else {
        return Err(format!(
            "no colour {colour:?}: it's one of {}",
            COLOURS.join(", ")
        ));
    }
    Ok(())
}

/// A project moved under another heading by an arrangement: its queue folder and the heading.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Moved {
    pub key: String,
    pub category: String,
}

/// Keep the order the window shows (`order`, queue folders; `categories`, headings), and move a
/// project under another heading with it (`moved`). A folder `list` doesn't know is left out; the
/// ones kept before that aren't in `order` (a hidden project's) stay, after it, as do headings.
/// Refused, with nothing changed, for more than `MAX_ORDER` of either, a project `list` doesn't
/// know, or a heading with no name.
pub fn arrange(
    c: &mut Config,
    list: &[Project],
    order: &[String],
    categories: &[String],
    moved: Option<&Moved>,
) -> Result<(), String> {
    if order.len() > MAX_ORDER || categories.len() > MAX_ORDER {
        return Err(format!(
            "at most {MAX_ORDER} projects and headings are kept in order"
        ));
    }
    let known: HashSet<&str> = list.iter().map(|p| p.key.as_str()).collect();
    if let Some(m) = moved {
        if !known.contains(m.key.as_str()) {
            return Err("no such project".into());
        }
        let to = heading(&m.category);
        if to.is_empty() {
            return Err("a category needs a name".into());
        }
        labels::set(&mut c.categories, &m.key, &to, MAX_CATEGORY);
    }
    let mut seen = HashSet::new();
    let sent: Vec<String> = order
        .iter()
        .filter(|k| known.contains(k.as_str()) && seen.insert(k.to_string()))
        .cloned()
        .collect();
    c.order = merged(sent, &c.order);
    let mut seen = HashSet::new();
    let sent: Vec<String> = categories
        .iter()
        .map(|h| heading(h))
        .filter(|h| !h.is_empty() && seen.insert(h.clone()))
        .collect();
    c.category_order = merged(sent, &c.category_order);
    Ok(())
}

/// `first`, then what `before` had that `first` hasn't, at most `MAX_ORDER`.
fn merged(first: Vec<String>, before: &[String]) -> Vec<String> {
    let have: HashSet<String> = first.iter().cloned().collect();
    let mut out = first;
    out.extend(before.iter().filter(|k| !have.contains(*k)).cloned());
    out.truncate(MAX_ORDER);
    out
}

/// Rename the heading `from` to `to`: every project under it (by its category, its group or
/// Other) goes under `to`, which may be another heading already, and then the two are one, in
/// `to`'s place. Refused for an empty `to` or a `from` no project in `list` is under.
pub fn rename_category(
    c: &mut Config,
    list: &[Project],
    from: &str,
    to: &str,
) -> Result<(), String> {
    let (from, to) = (heading(from), heading(to));
    if to.is_empty() {
        return Err("a category needs a name".into());
    }
    let under: Vec<&Project> = list
        .iter()
        .filter(|p| labels::shown_category(p) == from)
        .collect();
    if under.is_empty() {
        return Err(format!("no category named {from:?}"));
    }
    if from == to {
        return Ok(());
    }
    for p in under {
        labels::set(&mut c.categories, &p.key, &to, MAX_CATEGORY);
    }
    let had_to = c.category_order.contains(&to);
    if let Some(i) = c.category_order.iter().position(|h| *h == from) {
        if had_to {
            c.category_order.remove(i);
        } else {
            c.category_order[i] = to;
        }
    }
    Ok(())
}

/// Each queue's colour, its place in the Mine order and its heading's place, in the state, from
/// its project in `list` (the same folder) and `c`.
pub fn apply(c: &Config, list: &[Project], queues: &mut [QueueInfo]) {
    for q in queues {
        if let Some(p) = list.iter().find(|p| p.path == q.path) {
            q.colour = p.colour.clone();
            q.rank = c.order.iter().position(|k| *k == p.key);
            let h = labels::shown_category(p);
            q.category_rank = c.category_order.iter().position(|x| *x == h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects;
    use std::fs;
    use std::path::Path;

    fn make_queue(dir: &Path) {
        fs::create_dir_all(dir.join("todo")).unwrap();
        fs::write(dir.join("README.md"), "q").unwrap();
    }

    fn config(json: &str) -> Config {
        serde_json::from_str(json).unwrap()
    }

    /// Three found queues (one a group's, `persianUtils/colors`) and their list.
    fn three() -> (tempfile::TempDir, Config) {
        let base = tempfile::tempdir().unwrap();
        for q in ["muster/tasks", "drover/tasks", "persianUtils/colors/tasks"] {
            make_queue(&base.path().join(q));
        }
        (base, Config::default())
    }

    fn key(base: &Path, c: &Config, name: &str) -> String {
        projects::list(base, c)
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
            .key
    }

    #[test]
    fn an_old_file_without_them_loads_and_bad_entries_are_dropped_without_losing_the_rest() {
        let c = config(r#"{"labels": {"/a": "A"}, "gate": {"/a": "bin/gate"}}"#);
        assert!(c.colours.is_empty() && c.order.is_empty() && c.category_order.is_empty());
        assert_eq!(c.labels.len(), 1);
        let c = config(
            r#"{"gate": {"/a": "g"},
                "colours": {"/a": "lime", "/b": "red", "/c": 3, "/d": null, "/e": "Mint", "/f": "mint"},
                "order": ["/b", 4, "/a", "", "/b", null, "/c"],
                "categoryOrder": ["Tools", "other", " Apps\n", "Tools", "", 7, "OTHER"]}"#,
        );
        assert_eq!(c.gate.len(), 1, "the rest is kept");
        assert_eq!(
            c.colours,
            HashMap::from([
                ("/a".to_string(), "lime".to_string()),
                ("/f".to_string(), "mint".to_string()),
            ]),
            "an unknown colour, or one in another case, is none"
        );
        assert_eq!(c.order, ["/b", "/a", "/c"]);
        assert_eq!(c.category_order, ["Tools", "Other", "Apps"]);
        for broken in [
            r#"{"colours": ["lime"], "order": {"/a": 1}, "categoryOrder": "Tools", "gate": {"/a": "g"}}"#,
            r#"{"colours": null, "order": null, "categoryOrder": 5, "gate": {"/a": "g"}}"#,
        ] {
            let c = config(broken);
            assert!(
                c.colours.is_empty() && c.order.is_empty() && c.category_order.is_empty(),
                "{broken}"
            );
            assert_eq!(c.gate.len(), 1, "{broken}: the rest is kept");
        }
        // A hand-made list too long is cut at the limit.
        let long: Vec<String> = (0..MAX_ORDER + 20).map(|i| format!("/p/{i}")).collect();
        let c = config(&serde_json::json!({ "order": long }).to_string());
        assert_eq!(c.order.len(), MAX_ORDER);
    }

    #[test]
    fn a_colour_is_one_of_the_palette_or_none_and_is_saved_by_folder() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg/projects.json");
        projects::edit(&f, |c| set_colour(c, "/p/a/tasks", " orchid ")).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(
            saved["colours"],
            serde_json::json!({"/p/a/tasks": "orchid"})
        );
        assert_eq!(projects::load(&f).colours["/p/a/tasks"], "orchid");
        // Unknown: refused, and the file keeps what it had.
        let before = fs::read_to_string(&f).unwrap();
        let e = projects::edit(&f, |c| set_colour(c, "/p/a/tasks", "red")).unwrap_err();
        assert_eq!(
            e,
            "no colour \"red\": it's one of lime, orchid, pink, tan, mint"
        );
        assert_eq!(fs::read_to_string(&f).unwrap(), before);
        // Empty takes it out.
        projects::edit(&f, |c| set_colour(c, "/p/a/tasks", "")).unwrap();
        assert!(projects::load(&f).colours.is_empty());
    }

    #[test]
    fn the_list_carries_each_projects_colour() {
        let (base, mut c) = three();
        let mu = key(base.path(), &c, "muster");
        set_colour(&mut c, &mu, "tan").unwrap();
        let list = projects::list(base.path(), &c);
        let colour = |name: &str| list.iter().find(|p| p.name == name).unwrap().colour.clone();
        assert_eq!(colour("muster"), "tan");
        assert_eq!(colour("drover"), "");
        let json = serde_json::to_value(list.iter().find(|p| p.name == "muster").unwrap()).unwrap();
        assert_eq!(json["colour"], "tan");
    }

    #[test]
    fn an_arrangement_keeps_the_order_shown_and_what_it_didnt_send() {
        let (base, mut c) = three();
        let (mu, dr, co) = (
            key(base.path(), &c, "muster"),
            key(base.path(), &c, "drover"),
            key(base.path(), &c, "colors"),
        );
        let list = projects::list(base.path(), &c);
        // A hidden project's place, from before, stays after the ones sent.
        c.order = vec!["/gone/tasks".into(), co.clone()];
        c.category_order = vec!["Old".into()];
        arrange(
            &mut c,
            &list,
            &[dr.clone(), "/nowhere/tasks".into(), mu.clone(), dr.clone()],
            &[
                "Tools".into(),
                " other ".into(),
                "Tools".into(),
                "\n".into(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(
            c.order,
            [dr.clone(), mu.clone(), "/gone/tasks".into(), co.clone()]
        );
        assert_eq!(c.category_order, ["Tools", "Other", "Old"]);
        assert!(c.categories.is_empty(), "nothing moved");
        // Again: the new order first, nothing doubled.
        arrange(
            &mut c,
            &list,
            &[co.clone(), mu.clone()],
            &["Old".into()],
            None,
        )
        .unwrap();
        assert_eq!(
            c.order,
            [co.clone(), mu.clone(), dr.clone(), "/gone/tasks".into()]
        );
        assert_eq!(c.category_order, ["Old", "Tools", "Other"]);
    }

    #[test]
    fn an_arrangement_moves_a_project_under_another_heading_or_refuses_with_nothing_changed() {
        let (base, mut c) = three();
        let (mu, co) = (
            key(base.path(), &c, "muster"),
            key(base.path(), &c, "colors"),
        );
        let list = projects::list(base.path(), &c);
        let moved = Moved {
            key: mu.clone(),
            category: " Apps ".into(),
        };
        arrange(
            &mut c,
            &list,
            std::slice::from_ref(&mu),
            &["Apps".into()],
            Some(&moved),
        )
        .unwrap();
        assert_eq!(c.categories[&mu], "Apps");
        // Into Other, out of its group's heading: Other is said, so the group doesn't win.
        let moved = Moved {
            key: co.clone(),
            category: "other".into(),
        };
        arrange(&mut c, &list, &[], &[], Some(&moved)).unwrap();
        let list = projects::list(base.path(), &c);
        let colors = list.iter().find(|p| p.key == co).unwrap();
        assert_eq!(labels::shown_category(colors), OTHER);
        let before = c.clone();
        let nowhere = Moved {
            key: "/nowhere/tasks".into(),
            category: "Apps".into(),
        };
        assert_eq!(
            arrange(
                &mut c,
                &list,
                std::slice::from_ref(&co),
                &[],
                Some(&nowhere)
            )
            .unwrap_err(),
            "no such project"
        );
        let blank = Moved {
            key: mu.clone(),
            category: " \t".into(),
        };
        assert_eq!(
            arrange(&mut c, &list, std::slice::from_ref(&co), &[], Some(&blank)).unwrap_err(),
            "a category needs a name"
        );
        let many: Vec<String> = (0..=MAX_ORDER).map(|i| format!("/p/{i}")).collect();
        assert!(arrange(&mut c, &list, &many, &[], None)
            .unwrap_err()
            .starts_with("at most 500"));
        assert!(arrange(&mut c, &list, &[], &many, None).is_err());
        assert_eq!(c, before, "a refusal changes nothing");
    }

    #[test]
    fn renaming_a_heading_moves_every_project_under_it_and_merges_into_one_that_exists() {
        let (base, mut c) = three();
        let (mu, dr, co) = (
            key(base.path(), &c, "muster"),
            key(base.path(), &c, "drover"),
            key(base.path(), &c, "colors"),
        );
        labels::set(&mut c.categories, &mu, "Apps", MAX_CATEGORY);
        labels::set(&mut c.categories, &dr, "Tools", MAX_CATEGORY);
        c.category_order = vec!["Tools".into(), "persianUtils".into(), "Apps".into()];
        let list = projects::list(base.path(), &c);
        // A group's heading (colors has no category of its own): renamed, in its place.
        rename_category(&mut c, &list, "persianUtils", " Libraries ").unwrap();
        assert_eq!(c.categories[&co], "Libraries");
        assert_eq!(c.category_order, ["Tools", "Libraries", "Apps"]);
        // Into one that exists: one heading, in the place of the one kept.
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "Apps", "Tools").unwrap();
        assert_eq!(
            (c.categories[&mu].as_str(), c.categories[&dr].as_str()),
            ("Tools", "Tools")
        );
        assert_eq!(c.category_order, ["Tools", "Libraries"]);
        // The same name changes nothing; an empty one, or a heading nobody is under, is refused.
        let list = projects::list(base.path(), &c);
        let before = c.clone();
        rename_category(&mut c, &list, "Tools", "Tools").unwrap();
        assert_eq!(c, before);
        assert_eq!(
            rename_category(&mut c, &list, "Tools", " ").unwrap_err(),
            "a category needs a name"
        );
        assert_eq!(
            rename_category(&mut c, &list, "Nowhere", "Apps").unwrap_err(),
            "no category named \"Nowhere\""
        );
        assert_eq!(c, before);
        // Other, renamed, names its projects; renaming into "other" is the Other heading.
        let mut c = Config::default();
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "Other", "Mine").unwrap();
        assert_eq!(
            (c.categories[&mu].as_str(), c.categories[&dr].as_str()),
            ("Mine", "Mine")
        );
        assert!(!c.categories.contains_key(&co), "colors is under its group");
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "Mine", "OTHER").unwrap();
        let list = projects::list(base.path(), &c);
        assert!(list
            .iter()
            .filter(|p| p.key != co)
            .all(|p| labels::shown_category(p) == OTHER));
    }

    #[test]
    fn the_state_carries_each_queues_colour_and_places() {
        let (base, mut c) = three();
        let (mu, dr) = (
            key(base.path(), &c, "muster"),
            key(base.path(), &c, "drover"),
        );
        set_colour(&mut c, &mu, "lime").unwrap();
        labels::set(&mut c.categories, &mu, "Apps", MAX_CATEGORY);
        c.order = vec![dr.clone(), mu.clone()];
        c.category_order = vec!["persianUtils".into(), "Apps".into()];
        let list = projects::list(base.path(), &c);
        let mut state =
            crate::queue::read_state(&projects::shown(&list), &mut crate::queue::Cache::default());
        let got = |s: &crate::queue::State| -> Vec<(String, String, Option<usize>, Option<usize>)> {
            s.queues
                .iter()
                .map(|q| (q.name.clone(), q.colour.clone(), q.rank, q.category_rank))
                .collect()
        };
        // Before: none, as the CLI reads the state.
        assert!(got(&state)
            .iter()
            .all(|(_, c, r, h)| c.is_empty() && r.is_none() && h.is_none()));
        labels::apply(&list, &mut state.queues);
        apply(&c, &list, &mut state.queues);
        let mut rows = got(&state);
        rows.sort();
        assert_eq!(
            rows,
            [
                ("colors".into(), "".into(), None, Some(0)),
                ("drover".into(), "".into(), Some(0), None),
                ("muster".into(), "lime".into(), Some(1), Some(1)),
            ]
        );
        let q = state.queues.iter().find(|q| q.name == "muster").unwrap();
        let json = serde_json::to_value(q).unwrap();
        assert_eq!(
            (
                json["colour"].as_str(),
                json["rank"].as_u64(),
                json["categoryRank"].as_u64()
            ),
            (Some("lime"), Some(1), Some(1))
        );
    }

    #[test]
    fn writers_at_once_keep_every_colour_and_arrangement() {
        let (base, c) = three();
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("projects.json");
        let list = projects::list(base.path(), &c);
        let keys: Vec<String> = list.iter().map(|p| p.key.clone()).collect();
        let writers: Vec<_> = (0..4)
            .map(|w| {
                let (file, list, keys) = (file.clone(), list.clone(), keys.clone());
                std::thread::spawn(move || {
                    for i in 0..20 {
                        projects::edit(&file, |c| {
                            // Each writer's own folders get a colour; all of them arrange.
                            set_colour(c, &format!("/w{w}/{i}"), COLOURS[(w + i) % COLOURS.len()])?;
                            let mut order = keys.clone();
                            order.rotate_left((w + i) % keys.len());
                            arrange(c, &list, &order, &[format!("H{w}")], None)
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        let c = projects::load(&file);
        assert_eq!(c.colours.len(), 80, "every writer's every colour is kept");
        let mut order = c.order.clone();
        order.sort();
        let mut want = keys.clone();
        want.sort();
        assert_eq!(order, want, "each project once");
        let mut heads = c.category_order.clone();
        heads.sort();
        assert_eq!(heads, ["H0", "H1", "H2", "H3"]);
    }
}
