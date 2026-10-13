//! Task keys (task 0033): a task is shown and typed as `mu-12`, its project's prefix and its
//! number without the leading zeros, while its file, `depends:`, `parent:` and the ids in JSON keep
//! `0012`. Everything that shows or reads a key goes through here; taskKey.ts is the window's
//! half, and `app/src/key-cases.json` holds the two to the same answers.
//!
//! A project's prefix is 2 to 4 lowercase ASCII letters. It's stored in an old queue's README.md
//! front matter (`prefix: mu`), and in a `.muster` queue's `config.toml` (`prefix = "mu"`, task
//! 0005), which only `stored_prefix` reads and `store_prefix` writes. Without one, it's the
//! first 2 letters of the project's name, or 3, then 4, when another project Muster knows would
//! start the same (`prefixes`).

use crate::{projects, queue};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

/// What a command was given for a task: `mu-12`, `MU-12`, `12` or `0012`. `prefix` is the
/// project's when the key names one; `id` is always the file's four digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed {
    pub prefix: Option<String>,
    pub id: String,
}

/// `mu-12` for task `0012` of the project with the prefix `mu`. Without a prefix, or for an id
/// that isn't digits, the id as it is.
pub fn key(prefix: &str, id: &str) -> String {
    if prefix.is_empty() || id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return id.to_string();
    }
    match id.trim_start_matches('0') {
        "" => format!("{prefix}-0"),
        n => format!("{prefix}-{n}"),
    }
}

/// A prefix as stored and typed: 2 to 4 lowercase ASCII letters.
pub fn check_prefix(p: &str) -> Result<(), String> {
    if (2..=4).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_lowercase()) {
        Ok(())
    } else {
        Err(format!(
            "a prefix is 2 to 4 lowercase letters, a to z (mu, brn), got {p:?}"
        ))
    }
}

/// `mu-12`, `MU-12`, `mu-0012`, `12` or `0012` (spaces around it aside); None for anything else.
pub fn parse(input: &str) -> Option<Typed> {
    let s = input.trim();
    let (prefix, number) = match s.split_once('-') {
        Some((p, n)) => (Some(p.to_ascii_lowercase()), n),
        None => (None, s),
    };
    let digits =
        !number.is_empty() && number.len() <= 4 && number.bytes().all(|b| b.is_ascii_digit());
    if !digits || prefix.as_deref().is_some_and(|p| check_prefix(p).is_err()) {
        return None;
    }
    Some(Typed {
        prefix,
        id: format!("{number:0>4}"),
    })
}

/// `parse`, or the error a command gives for what isn't a task.
pub fn typed(input: &str) -> Result<Typed, String> {
    parse(input).ok_or_else(|| format!("a task is named like mu-12, 12 or 0012, got {input:?}"))
}

/// Tasks of one project (`depends:`, `parent:`, `block --on`), typed as keys or ids (`mu-14, 16`),
/// as their ids (`0014`, `0016`). None when a word isn't a task, or there are none: the value is
/// left as it is, for the command's own check. Refused when a key names another project than
/// `project`'s (`prefix`).
pub fn same_project_ids(
    value: &str,
    prefix: &str,
    project: &str,
) -> Result<Option<Vec<String>>, String> {
    let words: Vec<&str> = value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split([',', ' ', '\t'])
        .filter(|w| !w.is_empty())
        .collect();
    let mut ids = Vec::new();
    for w in &words {
        let Some(t) = parse(w) else {
            return Ok(None);
        };
        if t.prefix.as_deref().is_some_and(|p| p != prefix) {
            return Err(format!(
                "{w} isn't a task of {project} ({prefix}): depends, parents and blocks name tasks in the same project"
            ));
        }
        ids.push(t.id);
    }
    Ok((!ids.is_empty()).then_some(ids))
}

/// The tasks a block's `--on` starts with (`mu-14, mu-16 waiting for review`, `14,16`), as their
/// ids, and the words after them as they were typed: the words up to the first that isn't a task
/// of `project` (`prefix`), split at commas and spaces. None when it doesn't start with a task;
/// refused when one names another project.
pub fn leading_tasks(
    value: &str,
    prefix: &str,
    project: &str,
) -> Result<Option<(Vec<String>, String)>, String> {
    let mut ids = Vec::new();
    let mut rest = value.trim_start();
    loop {
        let word_end = rest.find([',', ' ', '\t', '\n']).unwrap_or(rest.len());
        let word = &rest[..word_end];
        let Some(t) = parse(word).filter(|_| !word.is_empty()) else {
            break;
        };
        if t.prefix.as_deref().is_some_and(|p| p != prefix) {
            return Err(format!(
                "{word} isn't a task of {project} ({prefix}): depends, parents and blocks name tasks in the same project"
            ));
        }
        ids.push(t.id);
        rest = rest[word_end..].trim_start_matches([',', ' ', '\t', '\n']);
    }
    Ok((!ids.is_empty()).then(|| (ids, rest.trim_end().to_string())))
}

/// What the core says (a refusal, an error), with each task of the queue at `root` it names by
/// its id (`0012`) named by its key (`mu-12`): what a command prints and the window shows. Only
/// ids of tasks that are there, plus `also` (what the command was given), standing alone: not in a
/// file name (`0012-x.md`), a path, a date or a longer number.
pub fn name_tasks(root: &Path, text: &str, also: &[&str]) -> String {
    let mut ids: HashSet<String> = queue::task_files(root)
        .into_iter()
        .filter_map(|(_, p)| {
            let name = p.file_name()?.to_str()?.to_string();
            name.get(..4).map(String::from)
        })
        .collect();
    ids.extend(also.iter().map(|s| s.to_string()));
    if ids.is_empty() || !text.bytes().any(|b| b.is_ascii_digit()) {
        return text.to_string();
    }
    name_ids(text, &prefix_for(root), &ids)
}

/// `name_tasks` with the prefix and the ids given.
fn name_ids(text: &str, prefix: &str, ids: &HashSet<String>) -> String {
    if prefix.is_empty() {
        return text.to_string();
    }
    let joins = |c: char| c.is_alphanumeric() || "-_/.:".contains(c);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        let run = chars[i..]
            .iter()
            .take_while(|(_, c)| c.is_ascii_digit())
            .count();
        let before = i.checked_sub(1).map(|j| chars[j].1);
        if c.is_ascii_digit() && run == 4 && !before.is_some_and(joins) {
            let after = chars.get(i + 4).map(|&(_, c)| c);
            let next = chars.get(i + 5).map(|&(_, c)| c);
            // A full stop ends a sentence; one before a letter or digit is a file's extension.
            let ends = match after {
                None => true,
                Some('.') => !next.is_some_and(char::is_alphanumeric),
                Some(a) => !(a.is_alphanumeric() || "-_/".contains(a)),
            };
            let end = chars.get(i + 4).map_or(text.len(), |&(e, _)| e);
            let id = &text[at..end];
            if ends && ids.contains(id) {
                out.push_str(&key(prefix, id));
                i += 4;
                continue;
            }
        }
        // Not an id: the digits as they are, all of them, so a longer number stays whole.
        let take = if c.is_ascii_digit() { run } else { 1 };
        for &(_, c) in &chars[i..i + take] {
            out.push(c);
        }
        i += take;
    }
    out
}

/// A name's letters as a prefix starts with them: a to z only, lowercased, padded with x to 2.
fn letters(name: &str) -> String {
    let mut l: String = name
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    while l.len() < 2 {
        l.push('x');
    }
    l
}

/// The starts a default prefix can be: the first 2, 3 and 4 letters (as many as there are).
fn starts(letters: &str) -> impl Iterator<Item = &str> {
    // ASCII only (`letters`), so every byte is a character.
    (2..=4.min(letters.len())).map(move |n| &letters[..n])
}

/// The starts, then the first two letters and one more, a to z: what a prefix falls back to.
fn fallbacks(letters: &str) -> impl Iterator<Item = String> + '_ {
    starts(letters)
        .map(String::from)
        .chain(('a'..='z').map(move |c| format!("{}{c}", &letters[..2])))
}

/// Every project's prefix, for (name, stored prefix) in the order Muster lists them. A stored one
/// is kept. Every other project takes the shortest start of its name's letters (2, 3, then 4) that
/// no stored prefix is and that no other such project starts with too; one with none of those
/// takes, in list order, the first of its starts (then its first two letters and one more, a to
/// z) not taken yet. A name with fewer than 2 letters is padded with x.
pub fn prefixes(projects: &[(String, Option<String>)]) -> Vec<String> {
    let stored: Vec<Option<String>> = projects
        .iter()
        .map(|(_, s)| {
            s.as_deref()
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| check_prefix(s).is_ok())
        })
        .collect();
    let names: Vec<String> = projects.iter().map(|(n, _)| letters(n)).collect();
    let mut taken: HashSet<String> = stored.iter().flatten().cloned().collect();
    let open: Vec<usize> = (0..projects.len())
        .filter(|&i| stored[i].is_none())
        .collect();
    let shared = |i: usize, s: &str| {
        open.iter()
            .any(|&j| j != i && names[j].len() >= s.len() && names[j][..s.len()] == *s)
    };
    let mut out = stored;
    // Picks of one length differ (none is shared) and picks of two lengths do too, so they're
    // unique without looking at each other.
    for &i in &open {
        out[i] = starts(&names[i])
            .find(|s| !taken.contains(*s) && !shared(i, s))
            .map(String::from);
    }
    taken.extend(out.iter().flatten().cloned());
    for &i in &open {
        if out[i].is_none() {
            let p = fallbacks(&names[i])
                .find(|c| !taken.contains(c))
                .unwrap_or_else(|| names[i][..2].to_string());
            taken.insert(p.clone());
            out[i] = Some(p);
        }
    }
    out.into_iter().map(Option::unwrap_or_default).collect()
}

/// The prefix a project named `name` would get beside `taken` (the prefixes Muster's projects
/// have), without changing theirs: what a new queue stores, and a folder Muster doesn't list
/// shows.
pub fn prefix_beside(name: &str, taken: &[String]) -> String {
    let l = letters(name);
    let free = fallbacks(&l).find(|c| !taken.contains(c));
    free.unwrap_or_else(|| l[..2].to_string())
}

/// The most of README.md read for its front matter.
const README_HEAD: u64 = 64 * 1024;

/// The prefix stored for the queue at `root`, if it's a valid one: a `.muster` queue's in its
/// `config.toml` (0005; a README there doesn't count), an old queue's in README.md's front matter.
pub fn stored_prefix(root: &Path) -> Option<String> {
    let raw = if crate::layout::is_muster(root) {
        crate::layout::config_prefix(root)?
    } else {
        let mut text = String::new();
        std::fs::File::open(root.join("README.md"))
            .ok()?
            .take(README_HEAD)
            .read_to_string(&mut text)
            .ok()?;
        let (fields, _) = queue::front_matter(&text)?;
        fields.get("prefix")?.clone()
    };
    let p = raw.trim().to_ascii_lowercase();
    check_prefix(&p).ok().map(|()| p)
}

/// Where a queue keeps its prefix exists: a `.muster` queue's `config.toml`, an old queue's
/// README.md.
fn has_prefix_home(root: &Path) -> bool {
    if crate::layout::is_muster(root) {
        root.join(crate::layout::CONFIG).is_file()
    } else {
        root.join("README.md").is_file()
    }
}

/// Store `prefix` for the queue at `root`: in a `.muster` queue's `config.toml` (0005), else in
/// README.md's front matter (made when it has none). The caller has checked it.
pub fn store_prefix(root: &Path, prefix: &str) -> Result<(), String> {
    if crate::layout::is_muster(root) {
        return crate::layout::store_config_prefix(root, prefix);
    }
    let file = root.join("README.md");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let text = if queue::front_matter(&text).is_some() {
        crate::ops::set_front(&text, "prefix", prefix)?
    } else {
        format!("---\nprefix: {prefix}\n---\n\n{text}")
    };
    queue::write_atomic(&file, &text)
}

/// Store the queue's current prefix when it has none stored, so its keys stay what they are now
/// whatever projects come along later. Called under the queue's lock by every write. A queue
/// with nowhere to keep it (an old one with no README.md, a `.muster` with no `config.toml`) is
/// left alone, and a file that can't be written only means the default is worked out again next
/// time: the write itself goes on either way.
pub fn pin_prefix(root: &Path) {
    if stored_prefix(root).is_none() && has_prefix_home(root) {
        let _ = store_prefix(root, &prefix_for(root));
    }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}

/// Every project Muster knows, with its prefix (`muster projects`).
pub fn known() -> Vec<projects::Project> {
    projects::list(
        &crate::cli::base(),
        &projects::load(&crate::cli::config_file()),
    )
}

/// The prefix of the queue at `root`: the stored one, else its project's in `muster projects`,
/// else (a folder Muster doesn't list) the default it would get beside those, without changing
/// theirs.
pub fn prefix_for(root: &Path) -> String {
    if let Some(p) = stored_prefix(root) {
        return p;
    }
    let list = known();
    if let Some(p) = list.iter().find(|p| same_folder(Path::new(&p.path), root)) {
        return p.prefix.clone();
    }
    let taken: Vec<String> = list.into_iter().map(|p| p.prefix).collect();
    prefix_beside(&projects::default_name(root), &taken)
}

/// Refuse `p` for the project at `root` when it isn't a prefix or another project has it.
pub fn check_new_prefix(p: &str, root: &Path, list: &[projects::Project]) -> Result<(), String> {
    check_prefix(p)?;
    match list
        .iter()
        .find(|o| o.prefix == p && !same_folder(Path::new(&o.path), root))
    {
        Some(o) => Err(format!(
            "{} has {p}: a prefix names one project (see `muster projects`)",
            o.name
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const CASES: &str = include_str!("../../src/key-cases.json");

    fn cases() -> Value {
        serde_json::from_str(CASES).expect("key-cases.json is JSON")
    }

    fn s(v: &Value) -> &str {
        v.as_str().unwrap_or_else(|| panic!("{v} is a string"))
    }

    #[test]
    fn formats_the_shared_cases() {
        for c in cases()["format"].as_array().unwrap() {
            assert_eq!(key(s(&c[0]), s(&c[1])), s(&c[2]), "{c}");
        }
    }

    #[test]
    fn parses_the_shared_cases() {
        let c = cases();
        for p in c["parse"].as_array().unwrap() {
            let want = Typed {
                prefix: p[1].as_str().map(String::from),
                id: s(&p[2]).to_string(),
            };
            assert_eq!(parse(s(&p[0])), Some(want), "{p}");
        }
        for b in c["bad"].as_array().unwrap() {
            assert_eq!(parse(s(b)), None, "{b}");
            assert!(typed(s(b)).unwrap_err().contains("mu-12, 12 or 0012"));
        }
    }

    #[test]
    fn lists_of_tasks_take_keys_of_their_own_project() {
        let ids = |v: &str| same_project_ids(v, "mu", "muster");
        let owned = |xs: &[&str]| Some(xs.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(ids("mu-14, 16").unwrap(), owned(&["0014", "0016"]));
        assert_eq!(ids("[MU-14,0016]").unwrap(), owned(&["0014", "0016"]));
        assert_eq!(ids("0014").unwrap(), owned(&["0014"]));
        assert_eq!(ids("").unwrap(), None, "nothing to change");
        assert_eq!(ids("you pick").unwrap(), None, "not tasks: as it was");
        assert_eq!(ids("mu-14 later").unwrap(), None);
        let e = ids("mu-14, br-7").unwrap_err();
        assert!(e.contains("br-7 isn't a task of muster (mu)"), "{e}");
    }

    #[test]
    fn the_state_gets_each_projects_prefix() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a/tasks"), d.path().join("b/tasks"));
        for q in [&a, &b] {
            queue_at(q, "q");
            std::fs::write(
                q.join("todo/0012-t.md"),
                "---\nid: 0012\nstatus: todo\ndepends: 0099\n---\n\n# T\n",
            )
            .unwrap();
        }
        let prefix_of = |name: &str| if name == "a" { "aa" } else { "bb" }.to_string();
        let s = queue::read_state_keyed(
            &[("a".into(), a.clone()), ("b".into(), b)],
            &mut queue::Cache::default(),
            &crate::settings::Settings::default(),
            &prefix_of,
        );
        let prefixes: Vec<&str> = s.queues.iter().map(|q| q.prefix.as_str()).collect();
        assert_eq!(prefixes, ["aa", "bb"]);
        let keys: Vec<String> = s.tasks.iter().map(|t| key(&t.prefix, &t.id)).collect();
        assert_eq!(keys, ["aa-12", "bb-12"]);
        // What the queue check says names tasks by their keys too; files keep their names.
        for want in [
            "a: todo/0012-t.md depends on aa-99, which doesn't exist",
            "b: todo/0012-t.md depends on bb-99, which doesn't exist",
        ] {
            assert!(s.problems.iter().any(|p| p == want), "{:?}", s.problems);
        }
        // The rules' own reads leave the prefix out.
        let plain = queue::read_state(&[("a".into(), a)], &mut queue::Cache::default());
        assert!(
            plain
                .problems
                .iter()
                .any(|p| p == "a: todo/0012-t.md depends on 0099, which doesn't exist"),
            "{:?}",
            plain.problems
        );
    }

    #[test]
    fn a_block_starts_with_its_tasks_and_the_rest_is_the_reason() {
        let on = |v: &str| leading_tasks(v, "mu", "muster");
        let got = |ids: &[&str], rest: &str| {
            Some((
                ids.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                rest.to_string(),
            ))
        };
        assert_eq!(on("mu-2, mu-3").unwrap(), got(&["0002", "0003"], ""));
        assert_eq!(on("2, 3").unwrap(), got(&["0002", "0003"], ""));
        assert_eq!(on("0014,0016").unwrap(), got(&["0014", "0016"], ""));
        assert_eq!(
            on(" MU-2 ,3 waiting for review, then 4 ").unwrap(),
            got(&["0002", "0003"], "waiting for review, then 4")
        );
        assert_eq!(on("you: pick a colour").unwrap(), None);
        assert_eq!(on("question Q2").unwrap(), None);
        assert_eq!(on("").unwrap(), None);
        assert!(on("mu-2, br-7")
            .unwrap_err()
            .contains("br-7 isn't a task of muster (mu)"));
    }

    #[test]
    fn what_the_core_says_names_tasks_by_their_keys() {
        let ids: HashSet<String> = ["0003", "0012", "2026", "4000"]
            .into_iter()
            .map(String::from)
            .collect();
        let named = |t: &str| name_ids(t, "mu", &ids);
        assert_eq!(
            named("0003 is the owner's to do; agents never take a human task"),
            "mu-3 is the owner's to do; agents never take a human task"
        );
        assert_eq!(
            named("you already hold 0012; pass, return or block it first"),
            "you already hold mu-12; pass, return or block it first"
        );
        assert_eq!(named("blocked by 0003, 0012."), "blocked by mu-3, mu-12.");
        assert_eq!(named("(0012)"), "(mu-12)");
        // Not ids: files, paths, dates, times, longer numbers and ids of no task here.
        for same in [
            "todo/0012-x.md",
            "0012-x.md",
            "attachments/0012/a.png",
            "0012.md",
            "on 2026-10-13 09:00",
            "x0012",
            "00120",
            "12345 and 40000",
            "no task 0099",
            "mu-12 already",
            "",
        ] {
            assert_eq!(named(same), same, "{same:?}");
        }
        // No prefix (a queue Muster can't name): as it was.
        assert_eq!(name_ids("0003 is done", "", &ids), "0003 is done");
        // Unicode around it stays whole.
        assert_eq!(named("é0003 ✓ 0003 ✓"), "é0003 ✓ mu-3 ✓");
    }

    #[test]
    fn errors_name_the_queues_tasks_and_what_was_typed() {
        let d = tempfile::tempdir().unwrap();
        queue_at(d.path(), "---\nprefix: nt\n---\n");
        std::fs::write(d.path().join("todo/0007-x.md"), "---\nid: 0007\n---\n").unwrap();
        assert_eq!(
            name_tasks(d.path(), "0007 waits on 0099", &[]),
            "nt-7 waits on 0099"
        );
        assert_eq!(
            name_tasks(d.path(), "no task 0099", &["0099"]),
            "no task nt-99"
        );
    }

    #[test]
    fn a_prefix_is_two_to_four_lowercase_letters() {
        for ok in ["mu", "mus", "abcd"] {
            assert!(check_prefix(ok).is_ok(), "{ok}");
        }
        for bad in [
            "", "m", "abcde", "Mu", "MU", "m1", "mü", " mu", "mu ", "m-u", "١٢",
        ] {
            let e = check_prefix(bad).unwrap_err();
            assert!(e.contains("2 to 4 lowercase letters"), "{bad}: {e}");
        }
    }

    fn defaults(projects: &[(&str, Option<&str>)]) -> Vec<String> {
        let p: Vec<(String, Option<String>)> = projects
            .iter()
            .map(|(n, s)| (n.to_string(), s.map(String::from)))
            .collect();
        prefixes(&p)
    }

    #[test]
    fn defaults_take_two_letters_and_more_only_on_a_clash() {
        assert_eq!(
            defaults(&[("braindump", None), ("drover", None), ("muster", None)]),
            ["br", "dr", "mu"]
        );
        // Both start "mu": both take 3, whatever their order.
        assert_eq!(
            defaults(&[("mumble", None), ("muster", None)]),
            ["mum", "mus"]
        );
        assert_eq!(
            defaults(&[("muster", None), ("mumble", None)]),
            ["mus", "mum"]
        );
        // 2 and 3 clash, 4 doesn't.
        assert_eq!(
            defaults(&[("musket", None), ("muster", None)]),
            ["musk", "must"]
        );
        // Even 4 clash: in list order, the first start not taken yet.
        assert_eq!(
            defaults(&[("mustard", None), ("muster", None)]),
            ["mu", "mus"]
        );
        // Capitals, digits and punctuation aside; fewer than 2 letters padded with x.
        assert_eq!(defaults(&[("Hood-2", None)]), ["ho"]);
        assert_eq!(defaults(&[("127", None), ("q", None)]), ["xx", "qx"]);
        // A name that is all one start: its own letters, then one more.
        assert_eq!(defaults(&[("ab", None), ("abc", None)]), ["ab", "abc"]);
        assert_eq!(
            defaults(&[("alpha", None), ("alpha-2", None), ("alpha-3", None)]),
            ["al", "alp", "alph"]
        );
        assert_eq!(
            defaults(&[
                ("alpha", None),
                ("alpha-2", None),
                ("alpha-3", None),
                ("alpha-4", None)
            ]),
            ["al", "alp", "alph", "ala"]
        );
    }

    #[test]
    fn a_stored_prefix_is_kept_and_taken() {
        // Muster stores mu: mumble can't have it, and takes 3 letters.
        assert_eq!(
            defaults(&[("mumble", None), ("muster", Some("mu"))]),
            ["mum", "mu"]
        );
        // Drover stores mu: muster takes mus, though nothing else starts "mus".
        assert_eq!(
            defaults(&[("drover", Some("mu")), ("muster", None)]),
            ["mu", "mus"]
        );
        // A stored prefix doesn't make a project that starts the same take more letters.
        assert_eq!(
            defaults(&[("drover", Some("dx")), ("dragon", None)]),
            ["dx", "dr"]
        );
        // Two stored the same (by hand: setting it refuses that): both keep it.
        assert_eq!(
            defaults(&[("a", Some("zz")), ("b", Some("zz"))]),
            ["zz", "zz"]
        );
    }

    #[test]
    fn a_new_project_takes_what_the_others_leave() {
        let taken = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(prefix_beside("mumble", &taken(&["mu", "dr"])), "mum");
        assert_eq!(prefix_beside("germ", &taken(&["mu", "dr"])), "ge");
        assert_eq!(prefix_beside("ab", &taken(&["ab"])), "aba");
        assert_eq!(prefix_beside("", &taken(&[])), "xx");
    }

    fn queue_at(dir: &Path, readme: &str) {
        std::fs::create_dir_all(dir.join("todo")).unwrap();
        std::fs::write(dir.join("README.md"), readme).unwrap();
    }

    #[test]
    fn the_stored_prefix_is_readmes_front_matter() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(stored_prefix(d.path()), None, "no README");
        queue_at(d.path(), "# q\n");
        assert_eq!(stored_prefix(d.path()), None, "no front matter");
        queue_at(d.path(), "---\nprefix: mu\n---\n\n# q\n");
        assert_eq!(stored_prefix(d.path()).as_deref(), Some("mu"));
        queue_at(d.path(), "---\nowner: x\nprefix:  MU  \n---\n# q\n");
        assert_eq!(stored_prefix(d.path()).as_deref(), Some("mu"), "case aside");
        for bad in ["m", "m1", "abcde", ""] {
            queue_at(d.path(), &format!("---\nprefix: {bad}\n---\n# q\n"));
            assert_eq!(stored_prefix(d.path()), None, "{bad:?} isn't a prefix");
        }
    }

    #[test]
    fn storing_a_prefix_keeps_the_rest_of_the_readme() {
        let d = tempfile::tempdir().unwrap();
        let readme = "# q — task queue\n\nThe rules.\n";
        queue_at(d.path(), readme);
        store_prefix(d.path(), "qu").unwrap();
        let text = std::fs::read_to_string(d.path().join("README.md")).unwrap();
        assert_eq!(text, format!("---\nprefix: qu\n---\n\n{readme}"));
        assert_eq!(stored_prefix(d.path()).as_deref(), Some("qu"));
        store_prefix(d.path(), "qq").unwrap();
        let text = std::fs::read_to_string(d.path().join("README.md")).unwrap();
        assert_eq!(
            text,
            format!("---\nprefix: qq\n---\n\n{readme}"),
            "replaced, not added"
        );
        queue_at(d.path(), "---\nowner: x\n---\n# q\n");
        store_prefix(d.path(), "ow").unwrap();
        let text = std::fs::read_to_string(d.path().join("README.md")).unwrap();
        assert_eq!(text, "---\nowner: x\nprefix: ow\n---\n# q\n");
        let none = tempfile::tempdir().unwrap();
        assert!(
            store_prefix(none.path(), "ab").is_err(),
            "no README to store it in"
        );
    }

    #[test]
    fn the_first_write_pins_the_default_prefix() {
        let d = tempfile::tempdir().unwrap();
        let q = d.path().join("zqxpin/tasks");
        let readme = "# zqxpin — task queue\n\nThe rules.\n";
        queue_at(&q, readme);
        let default = prefix_for(&q);
        assert_eq!(stored_prefix(&q), None);
        queue::with_lock(&q, || Ok(())).unwrap();
        assert_eq!(stored_prefix(&q).as_deref(), Some(default.as_str()));
        let text = std::fs::read_to_string(q.join("README.md")).unwrap();
        assert_eq!(text, format!("---\nprefix: {default}\n---\n\n{readme}"));
        // Once stored, a write leaves it be, and so does a refused one.
        queue::with_lock(&q, || Err::<(), _>("refused".to_string())).unwrap_err();
        let again = std::fs::read_to_string(q.join("README.md")).unwrap();
        assert_eq!(again, text);
        // A queue with no README has nowhere to keep it, and isn't given one.
        let flat = tempfile::tempdir().unwrap();
        queue::with_lock(flat.path(), || Ok(())).unwrap();
        assert!(!flat.path().join("README.md").exists());
    }

    #[test]
    fn a_new_prefix_is_refused_when_bad_or_taken() {
        let base = tempfile::tempdir().unwrap();
        queue_at(&base.path().join("muster/tasks"), "q");
        queue_at(&base.path().join("drover/tasks"), "---\nprefix: dv\n---\nq");
        queue_at(&base.path().join("braindump/tasks"), "q");
        let list = projects::list(base.path(), &projects::Config::default());
        let prefixes: Vec<&str> = list.iter().map(|p| p.prefix.as_str()).collect();
        assert_eq!(prefixes, ["br", "dv", "mu"]);
        let muster = base.path().join("muster/tasks");
        let e = check_new_prefix("br", &muster, &list).unwrap_err();
        assert!(e.contains("braindump has br"), "{e}");
        let e = check_new_prefix("dv", &muster, &list).unwrap_err();
        assert!(e.contains("drover has dv"), "{e}");
        assert!(check_new_prefix("Mu!", &muster, &list)
            .unwrap_err()
            .contains("2 to 4 lowercase letters"));
        assert!(check_new_prefix("mu", &muster, &list).is_ok(), "its own");
        assert!(check_new_prefix("mus", &muster, &list).is_ok());
    }

    #[test]
    fn a_queues_prefix_is_stored_or_its_default() {
        let d = tempfile::tempdir().unwrap();
        let q = d.path().join("zqxproject/tasks");
        queue_at(&q, "---\nprefix: zqx\n---\n# q\n");
        assert_eq!(prefix_for(&q), "zqx");
    }
}
