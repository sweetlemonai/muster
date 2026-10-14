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
/// project under another heading with it (`moved`), which unfolds that heading (0070). A folder `list` doesn't know is left out; the
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
        // A project moved under a folded heading unfolds it, so it stays in sight, whichever
        // window moved it; a fold left by a heading that was emptied and is made again goes too
        // (0070 review R1).
        c.folded.retain(|h| *h != to);
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

/// Rename the heading `from` to `to`: every project under it goes under `to`, which may be another
/// heading already, and then the two are one, in `to`'s place, folded if either was (0070). A
/// project there by its group's heading (no category of its own) moves by the group's display
/// name (0070), so the group's projects, and any it gains later, need no category; `to` being
/// the group folder's own name clears that. A project there by a category of its own (or by
/// nothing, under Other) takes `to` as its category. Refused for an empty `to` or a `from` no
/// project in `list` is under.
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
        .filter(|p| heading(&labels::shown_category(p)) == from)
        .collect();
    if under.is_empty() {
        return Err(format!("no category named {from:?}"));
    }
    if from == to {
        return Ok(());
    }
    for p in under {
        if p.category.is_empty() && !p.group.is_empty() {
            if heading(&p.group) == to {
                c.group_names.remove(&p.group);
            } else {
                labels::set(&mut c.group_names, &p.group, &to, MAX_CATEGORY);
            }
        } else {
            labels::set(&mut c.categories, &p.key, &to, MAX_CATEGORY);
        }
    }
    renamed(&mut c.category_order, &from, &to);
    renamed(&mut c.folded, &from, &to);
    Ok(())
}

/// `from` in a list of headings (an order, the folded ones) renamed `to`: in its place, or gone
/// when `to` is there already.
fn renamed(list: &mut Vec<String>, from: &str, to: &str) {
    let had_to = list.iter().any(|h| h == to);
    if let Some(i) = list.iter().position(|h| h == from) {
        if had_to {
            list.remove(i);
        } else {
            list[i] = to.to_string();
        }
    }
}

/// Fold (or unfold) the heading `name` in the sidebar (0070), and drop the folds of headings no
/// project in `list` is under any more. Refused, with nothing changed, for no name or a heading no
/// project is under.
pub fn set_folded(
    c: &mut Config,
    list: &[Project],
    name: &str,
    folded: bool,
) -> Result<(), String> {
    let h = heading(name);
    if h.is_empty() {
        return Err("a category needs a name".into());
    }
    let shown: HashSet<String> = list
        .iter()
        .map(|p| heading(&labels::shown_category(p)))
        .collect();
    if !shown.contains(&h) {
        return Err(format!("no category named {h:?}"));
    }
    // The folds of headings no project is under any more go as well (review R1).
    c.folded.retain(|x| *x != h && shown.contains(x));
    if folded {
        c.folded.push(h);
        c.folded.truncate(MAX_ORDER);
    }
    Ok(())
}

/// Each queue's colour, its place in the Mine order, its heading's place and whether it's folded
/// (0070), in the state, from
/// its project in `list` (the same folder) and `c`.
pub fn apply(c: &Config, list: &[Project], queues: &mut [QueueInfo]) {
    for q in queues {
        if let Some(p) = list.iter().find(|p| p.path == q.path) {
            q.colour = p.colour.clone();
            q.rank = c.order.iter().position(|k| *k == p.key);
            let h = heading(&labels::shown_category(p));
            q.category_rank = c.category_order.iter().position(|x| *x == h);
            q.category_folded = c.folded.contains(&h);
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
        // A group's heading (colors has no category of its own): renamed, in its place, by the
        // group's display name (0070), not by a category of colors' own.
        rename_category(&mut c, &list, "persianUtils", " Libraries ").unwrap();
        assert_eq!(c.group_names["persianUtils"], "Libraries");
        assert!(!c.categories.contains_key(&co));
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
    fn a_group_folder_longer_than_a_heading_can_be_renamed_ranked_and_dropped_onto() {
        // Review R4: the heading the window shows, sends back and the core keeps are one form.
        let base = tempfile::tempdir().unwrap();
        let group = "a-very-long-group-folder-name-x";
        assert_eq!(group.chars().count(), 31);
        make_queue(&base.path().join(group).join("colors/tasks"));
        make_queue(&base.path().join("muster/tasks"));
        let mut c = Config::default();
        let shown = |c: &Config| -> Vec<(String, String, Option<usize>)> {
            let list = projects::list(base.path(), c);
            let mut s = crate::queue::read_state(
                &projects::shown(&list),
                &mut crate::queue::Cache::default(),
            );
            labels::apply(&list, &mut s.queues);
            apply(c, &list, &mut s.queues);
            let mut rows: Vec<_> = s
                .queues
                .iter()
                .map(|q| (q.name.clone(), q.category.clone(), q.category_rank))
                .collect();
            rows.sort();
            rows
        };
        // The window is sent the heading cut as a category is, never longer.
        let heading_of = |c: &Config| shown(c)[0].1.clone();
        let h = heading_of(&c);
        assert_eq!(h, "a-very-long-group-folder");
        assert_eq!(h.chars().count(), MAX_CATEGORY);
        // Ranked: the heading's place comes back with the state.
        let list = projects::list(base.path(), &c);
        arrange(&mut c, &list, &[], &[h.clone(), OTHER.into()], None).unwrap();
        assert_eq!(shown(&c)[0], ("colors".into(), h.clone(), Some(0)));
        // A project dropped onto it goes under the same heading, not a second one.
        let mu = key(base.path(), &c, "muster");
        let moved = Moved {
            key: mu,
            category: h.clone(),
        };
        arrange(&mut c, &list, &[], &[], Some(&moved)).unwrap();
        assert_eq!(
            shown(&c),
            [
                ("colors".into(), h.clone(), Some(0)),
                ("muster".into(), h.clone(), Some(0))
            ]
        );
        // Renamed by the name the window shows, or by the whole folder name an older window sent.
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, group, "Libraries").unwrap();
        assert_eq!(c.category_order, ["Libraries", OTHER]);
        assert!(shown(&c)
            .iter()
            .all(|(_, h, r)| h == "Libraries" && *r == Some(0)));
    }

    /// A group of two (`persianUtils/colors`, `persianUtils/dates`), two projects of their own with
    /// the group's folder name set by hand as their category (`muster`, `drover`), and one under
    /// `Tools` (`alpha`): synthetic, after this Mac's shape (0070).
    fn grouped() -> (tempfile::TempDir, Config) {
        let base = tempfile::tempdir().unwrap();
        for q in [
            "persianUtils/colors/tasks",
            "persianUtils/dates/tasks",
            "muster/tasks",
            "drover/tasks",
            "alpha/tasks",
        ] {
            make_queue(&base.path().join(q));
        }
        let mut c = Config::default();
        for (name, cat) in [
            ("muster", "persianUtils"),
            ("drover", "persianUtils"),
            ("alpha", "Tools"),
        ] {
            let k = key(base.path(), &c, name);
            labels::set(&mut c.categories, &k, cat, MAX_CATEGORY);
        }
        (base, c)
    }

    /// Each shown queue's name, heading and whether its heading is folded, as the state says.
    fn headings_of(base: &Path, c: &Config) -> Vec<(String, String, bool)> {
        let list = projects::list(base, c);
        let mut s =
            crate::queue::read_state(&projects::shown(&list), &mut crate::queue::Cache::default());
        labels::apply(&list, &mut s.queues);
        apply(c, &list, &mut s.queues);
        let mut rows: Vec<_> = s
            .queues
            .iter()
            .map(|q| (q.name.clone(), q.category.clone(), q.category_folded))
            .collect();
        rows.sort();
        rows
    }

    #[test]
    fn a_heading_folds_and_unfolds_and_the_state_says_so() {
        let (base, mut c) = grouped();
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, " persianUtils ", true).unwrap();
        set_folded(&mut c, &list, "persianUtils", true).unwrap();
        assert_eq!(c.folded, ["persianUtils"], "once");
        let rows = headings_of(base.path(), &c);
        assert_eq!(
            rows,
            [
                ("alpha".into(), "Tools".into(), false),
                ("colors".into(), "persianUtils".into(), true),
                ("dates".into(), "persianUtils".into(), true),
                ("drover".into(), "persianUtils".into(), true),
                ("muster".into(), "persianUtils".into(), true),
            ]
        );
        let json = serde_json::to_value(crate::queue::QueueInfo {
            category_folded: true,
            ..sample_queue()
        })
        .unwrap();
        assert_eq!(json["categoryFolded"], true);
        set_folded(&mut c, &list, "persianUtils", false).unwrap();
        assert!(c.folded.is_empty());
        // A heading no project is under, or no name, is refused with nothing changed.
        assert_eq!(
            set_folded(&mut c, &list, "Nowhere", true).unwrap_err(),
            "no category named \"Nowhere\""
        );
        assert_eq!(
            set_folded(&mut c, &list, " ", true).unwrap_err(),
            "a category needs a name"
        );
        assert!(c.folded.is_empty());
        // Other folds too.
        let mut c = Config::default();
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "other", true).unwrap();
        assert_eq!(c.folded, [OTHER]);
    }

    fn sample_queue() -> crate::queue::QueueInfo {
        crate::queue::QueueInfo {
            name: "q".into(),
            path: "/q".into(),
            prefix: "q".into(),
            counts: HashMap::new(),
            your_check: 0,
            your_check_limit: None,
            label: "q".into(),
            category: OTHER.into(),
            colour: String::new(),
            rank: None,
            category_rank: None,
            category_folded: false,
            lessons_waiting: 0,
        }
    }

    #[test]
    fn a_project_moved_under_a_folded_heading_unfolds_it_from_any_window() {
        // Review R1: the core keeps the rule, so Settings' ↑ ↓ unfolds too, not only the sidebar.
        let (base, mut c) = grouped();
        let co = key(base.path(), &c, "colors");
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "Tools", true).unwrap();
        // A reorder that moves nobody under another heading leaves the fold alone.
        arrange(&mut c, &list, std::slice::from_ref(&co), &[], None).unwrap();
        assert_eq!(c.folded, ["Tools"]);
        let moved = Moved {
            key: co.clone(),
            category: "Tools".into(),
        };
        arrange(&mut c, &list, std::slice::from_ref(&co), &[], Some(&moved)).unwrap();
        assert!(c.folded.is_empty());
        assert!(headings_of(base.path(), &c).iter().all(|(_, _, f)| !f));
    }

    #[test]
    fn a_fold_left_by_an_emptied_heading_never_folds_one_made_again_by_that_name() {
        // Review R1: Tools folded, then emptied (its one project's category changed by hand).
        let (base, mut c) = grouped();
        let (al, dr) = (
            key(base.path(), &c, "alpha"),
            key(base.path(), &c, "drover"),
        );
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "Tools", true).unwrap();
        labels::set(&mut c.categories, &al, "Apps", MAX_CATEGORY);
        // Folding something else drops the fold of a heading no project is under any more.
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "Apps", true).unwrap();
        assert_eq!(c.folded, ["Apps"]);
        // And made again by a move, a heading with a stale fold opens.
        c.folded.push("Tools".into());
        let moved = Moved {
            key: dr,
            category: "Tools".into(),
        };
        arrange(&mut c, &list, &[], &[], Some(&moved)).unwrap();
        assert_eq!(c.folded, ["Apps"]);
        let rows = headings_of(base.path(), &c);
        assert!(
            rows.iter()
                .any(|(n, h, f)| n == "drover" && h == "Tools" && !f),
            "{rows:?}"
        );
    }

    #[test]
    fn folds_and_group_names_are_saved_and_read_leniently() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg/projects.json");
        projects::edit(&f, |c| {
            c.folded = vec!["DashAfzar".into()];
            c.group_names
                .insert("persianUtils".into(), "DashAfzar".into());
            Ok(())
        })
        .unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(saved["folded"], serde_json::json!(["DashAfzar"]));
        assert_eq!(
            saved["groupNames"],
            serde_json::json!({"persianUtils": "DashAfzar"})
        );
        assert_eq!(projects::load(&f).group_names["persianUtils"], "DashAfzar");
        // An old file has neither; junk entries go, the rest of the file stays.
        let c = config(r#"{"gate": {"/a": "g"}}"#);
        assert!(c.folded.is_empty() && c.group_names.is_empty());
        let c = config(&format!(
            r#"{{"gate": {{"/a": "g"}}, "folded": ["Tools", 3, "other", "", "Tools", " Apps\n"],
                "groupNames": {{"persianUtils": " DashAfzar\u0007 ", "x": 4, "y": "  ", "z": "{}"}}}}"#,
            "n".repeat(40)
        ));
        assert_eq!(c.folded, ["Tools", OTHER, "Apps"]);
        assert_eq!(c.group_names.len(), 2);
        assert_eq!(c.group_names["persianUtils"], "DashAfzar");
        assert_eq!(c.group_names["z"].chars().count(), MAX_CATEGORY);
        for broken in [
            r#"{"folded": "Tools", "groupNames": ["x"], "gate": {"/a": "g"}}"#,
            r#"{"folded": null, "groupNames": 3, "gate": {"/a": "g"}}"#,
        ] {
            let c = config(broken);
            assert!(c.folded.is_empty() && c.group_names.is_empty(), "{broken}");
            assert_eq!(c.gate.len(), 1, "{broken}: the rest is kept");
        }
    }

    #[test]
    fn renaming_a_group_heading_names_the_group_and_takes_the_hand_set_ones_along() {
        let (base, mut c) = grouped();
        let (mu, co) = (
            key(base.path(), &c, "muster"),
            key(base.path(), &c, "colors"),
        );
        c.category_order = vec!["Tools".into(), "persianUtils".into()];
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "persianUtils", true).unwrap();
        rename_category(&mut c, &list, "persianUtils", "DashAfzar").unwrap();
        // The group gets the name; its own projects need no category of their own.
        assert_eq!(
            c.group_names,
            HashMap::from([("persianUtils".to_string(), "DashAfzar".to_string())])
        );
        assert!(!c.categories.contains_key(&co));
        // The ones set to the old name by hand stay under the heading, as before.
        assert_eq!(c.categories[&mu], "DashAfzar");
        // Its place and its fold go with it.
        assert_eq!(c.category_order, ["Tools", "DashAfzar"]);
        assert_eq!(c.folded, ["DashAfzar"]);
        let under = |c: &Config| -> Vec<String> {
            headings_of(base.path(), c)
                .into_iter()
                .filter(|(_, h, _)| h == "DashAfzar")
                .map(|(n, _, f)| {
                    assert!(f, "{n} folded");
                    n
                })
                .collect()
        };
        assert_eq!(under(&c), ["colors", "dates", "drover", "muster"]);
        // A project the group gains later goes under the name too, with nothing set for it.
        make_queue(&base.path().join("persianUtils/fonts/tasks"));
        assert_eq!(under(&c), ["colors", "dates", "drover", "fonts", "muster"]);
        let list = projects::list(base.path(), &c);
        let fonts = list.iter().find(|p| p.name == "fonts").unwrap();
        assert_eq!(
            (fonts.category.as_str(), fonts.group_name.as_str()),
            ("", "DashAfzar")
        );
        assert_eq!(
            serde_json::to_value(fonts).unwrap()["groupName"],
            "DashAfzar"
        );
    }

    #[test]
    fn renaming_a_group_heading_back_to_its_folder_clears_the_name() {
        let (base, mut c) = grouped();
        let mu = key(base.path(), &c, "muster");
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "persianUtils", "DashAfzar").unwrap();
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "DashAfzar", true).unwrap();
        rename_category(&mut c, &list, "DashAfzar", " persianUtils").unwrap();
        assert!(
            c.group_names.is_empty(),
            "the folder's own name is no display name"
        );
        assert_eq!(c.categories[&mu], "persianUtils");
        assert_eq!(c.folded, ["persianUtils"]);
        assert!(headings_of(base.path(), &c)
            .iter()
            .filter(|(n, _, _)| n != "alpha")
            .all(|(_, h, f)| h == "persianUtils" && *f));
    }

    #[test]
    fn renaming_a_group_heading_onto_another_heading_merges_them_folded_if_either_was() {
        let (base, mut c) = grouped();
        c.category_order = vec!["persianUtils".into(), "Tools".into()];
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "persianUtils", true).unwrap();
        rename_category(&mut c, &list, "persianUtils", "Tools").unwrap();
        assert_eq!(c.group_names["persianUtils"], "Tools");
        assert_eq!(
            c.category_order,
            ["Tools"],
            "one heading, in the kept one's place"
        );
        assert_eq!(c.folded, ["Tools"]);
        assert!(headings_of(base.path(), &c)
            .iter()
            .all(|(_, h, f)| h == "Tools" && *f));
        // The other way: an unfolded one onto a folded one stays folded, its entry not doubled.
        let (base, mut c) = grouped();
        let list = projects::list(base.path(), &c);
        set_folded(&mut c, &list, "Tools", true).unwrap();
        rename_category(&mut c, &list, "persianUtils", "Tools").unwrap();
        assert_eq!(c.folded, ["Tools"]);
        assert!(headings_of(base.path(), &c)
            .iter()
            .all(|(_, h, f)| h == "Tools" && *f));
        // And a hand-set heading renamed onto the group's name joins the group's heading.
        let (base, mut c) = grouped();
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "persianUtils", "DashAfzar").unwrap();
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "Tools", "DashAfzar").unwrap();
        assert!(headings_of(base.path(), &c)
            .iter()
            .all(|(_, h, _)| h == "DashAfzar"));
    }

    #[test]
    fn a_long_group_folder_takes_a_display_name_and_gives_it_back() {
        let base = tempfile::tempdir().unwrap();
        let group = "a-very-long-group-folder-name-x";
        make_queue(&base.path().join(group).join("colors/tasks"));
        let mut c = Config::default();
        let list = projects::list(base.path(), &c);
        let cut = labels::shown_category(&list[0]);
        assert_eq!(cut.chars().count(), MAX_CATEGORY);
        rename_category(&mut c, &list, &cut, "Long ones").unwrap();
        assert_eq!(c.group_names[group], "Long ones");
        assert_eq!(headings_of(base.path(), &c)[0].1, "Long ones");
        // Back by the cut name the window showed: no display name left.
        let list = projects::list(base.path(), &c);
        rename_category(&mut c, &list, "Long ones", &cut).unwrap();
        assert!(c.group_names.is_empty());
        assert_eq!(headings_of(base.path(), &c)[0].1, cut);
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
