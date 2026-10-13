//! Search, filters and sort for `muster list` (task 0032), with the app's rules: search.ts for
//! the search, filters.ts for the groups, lib.ts `sortBoard` for the orders. A shared fixture
//! (`app/src/search-cases.json`) holds the CLI and the app to the same results.
//!
//! The rules, kept simple so the two can't drift:
//! - Fold: each character is lowercased on its own, combining accents (U+0300–U+036F) are
//!   dropped, the letters in `FOLD` lose their accent, and anything that isn't a letter or a
//!   digit (`char::is_alphanumeric`) becomes a space. "Café-RÉSUMÉ" → "cafe resume".
//!   There's no Unicode normalization (std has none): a letter outside `FOLD` keeps its accent
//!   when it's one character ("ếạ" stays) and loses it when it's written decomposed (e, U+0302,
//!   U+0301 → "e"). Digits of other scripts ("١٩", "²", "１９") are kept as letters of a word.
//! - A query is its folded words. A task matches when every word does, in any order.
//! - A word of digits only (0–9) is an id: it matches the task whose id is all digits and the
//!   same number (19, 0019 and 00019 all find 0019). It never matches text.
//! - A key (0033: a word between spaces that `key::parse` reads with a prefix, mu-12, MU-0012) is
//!   kept whole, as `mu-0012`: it matches the task with that prefix and number, and never text.
//!   Only spaces, tabs and line breaks separate a key from the words beside it.
//! - Any other word matches when it's part of the folded holder (`holder`) or of the task's
//!   `search` text: its title, tags and body (no front matter), each folded word once, words of
//!   digits only left out, cut at `MAX_SEARCH_BYTES`. So "flick" finds "flickers".
//! - Filters: within a group (stages, types, tags, holders) any listed value will do, exactly
//!   as written; every group given must match. `blocked` keeps blocked tasks only, those blocked
//!   only by the tasks they depend on included.

use crate::queue::Task;
use std::cmp::Ordering;
use std::collections::HashSet;

/// The most a task's search text holds. The window fetches it only while it searches
/// (`queue::search_index`), not with every poll.
pub const MAX_SEARCH_BYTES: usize = 4096;

/// Accented letters and what they fold to, after lowercasing. Mirrors search.ts FOLD.
const FOLD: &[(&str, &str)] = &[
    ("àáâãäåāăąǎ", "a"),
    ("çćĉċč", "c"),
    ("ďđð", "d"),
    ("èéêëēĕėęě", "e"),
    ("ĝğġģ", "g"),
    ("ĥħ", "h"),
    ("ìíîïĩīĭįıǐ", "i"),
    ("ĵ", "j"),
    ("ķ", "k"),
    ("ĺļľŀł", "l"),
    ("ñńņňŉ", "n"),
    ("òóôõöøōŏőǒ", "o"),
    ("ŕŗř", "r"),
    ("śŝşšș", "s"),
    ("ţťŧț", "t"),
    ("ùúûüũūŭůűųǔ", "u"),
    ("ŵ", "w"),
    ("ýÿŷ", "y"),
    ("źżž", "z"),
    ("ß", "ss"),
    ("æ", "ae"),
    ("œ", "oe"),
    ("þ", "th"),
];

fn fold_char(c: char, out: &mut String) {
    // Most text is ASCII: the same answer without the table.
    if c.is_ascii() {
        out.push(if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            ' '
        });
        return;
    }
    for l in c.to_lowercase() {
        if ('\u{300}'..='\u{36f}').contains(&l) {
            continue;
        }
        match FOLD.iter().find(|(from, _)| from.contains(l)) {
            Some((_, to)) => out.push_str(to),
            None if l.is_alphanumeric() => out.push(l),
            None => out.push(' '),
        }
    }
}

/// Lowercase, without accents, with a space for anything that isn't a letter or a digit.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        fold_char(c, &mut out);
    }
    out
}

fn is_id_word(w: &str) -> bool {
    !w.is_empty() && w.bytes().all(|b| b.is_ascii_digit())
}

fn same_id(w: &str, id: &str) -> bool {
    is_id_word(id) && w.trim_start_matches('0') == id.trim_start_matches('0')
}

/// A query's key word (`mu-0012`) as its prefix and number; folded words never have a `-`.
fn key_word(w: &str) -> Option<(&str, &str)> {
    w.split_once('-')
}

/// Each folded word of `parts` once, in order, without words of digits only, up to `cap` bytes
/// (a word that doesn't fit ends it).
fn words_of(parts: &[&str], cap: usize) -> String {
    let mut seen = HashSet::new();
    let mut out = String::new();
    for part in parts {
        for w in fold(part).split(' ') {
            if w.is_empty() || is_id_word(w) || seen.contains(w) {
                continue;
            }
            let need = w.len() + usize::from(!out.is_empty());
            if out.len() + need > cap {
                return out;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(w);
            seen.insert(w.to_string());
        }
    }
    out
}

/// A task's search text, built when its file is read: title, tags, then the body.
pub fn search_text(title: &str, tags: &[String], body: &str) -> String {
    let tags = tags.join(" ");
    words_of(&[title, &tags, body], MAX_SEARCH_BYTES)
}

/// A query's folded words, a key kept whole (`mu-0012`); none for an empty query, which finds
/// everything.
pub fn query_words(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in q.split(|c: char| c.is_ascii_whitespace()) {
        match crate::key::parse(raw) {
            Some(crate::key::Typed {
                prefix: Some(p),
                id,
            }) => out.push(format!("{p}-{id}")),
            _ => out.extend(
                fold(raw)
                    .split(' ')
                    .filter(|w| !w.is_empty())
                    .map(str::to_string),
            ),
        }
    }
    out
}

/// Who holds a task, without the session id, as the app shows it (lib.ts `holder`): whoever
/// `Task::holder` names in its stage (the claim in Ready and Building, `held-by` in Testing and
/// Your check, the owner's when that names nobody), cut before the first `(` or `,`. So a stamped
/// `wt-a (wt-0030 · session 7b5e8856 · cc-hg)` is `wt-a`, and a leftover `claimed-by` on a
/// stage that isn't a claim is nobody.
pub fn holder(t: &Task) -> &str {
    let who = t.holder().trim();
    if who.is_empty() {
        return "";
    }
    match who.split(['(', ',']).next().map(str::trim) {
        Some(name) if !name.is_empty() => name,
        _ => who,
    }
}

/// Whether a task has every word: digits as its id, a key as its prefix and id, the rest in its
/// holder or its search text.
pub fn task_matches(t: &Task, words: &[String]) -> bool {
    let mut hay: Option<String> = None;
    words.iter().all(|w| {
        if is_id_word(w) {
            return same_id(w, &t.id);
        }
        if let Some((prefix, n)) = key_word(w) {
            return t.prefix == prefix && same_id(n, &t.id);
        }
        hay.get_or_insert_with(|| format!("{} {}", fold(holder(t)), t.search))
            .contains(w.as_str())
    })
}

/// Whether a backlog note (title and text) has every word. A note has no id, so a word of
/// digits, or a key, never matches one.
pub fn note_matches(title: &str, text: &str, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let hay = words_of(&[title, text], usize::MAX);
    words
        .iter()
        .all(|w| !is_id_word(w) && key_word(w).is_none() && hay.contains(w.as_str()))
}

/// What `muster list` keeps: the search and the filter groups the app has as chips.
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub words: Vec<String>,
    pub stages: Vec<String>,
    pub kinds: Vec<String>,
    pub tags: Vec<String>,
    pub holders: Vec<String>,
    pub blocked: bool,
}

impl Filter {
    pub fn matches(&self, t: &Task) -> bool {
        let any = |want: &[String], has: &dyn Fn(&str) -> bool| {
            want.is_empty() || want.iter().any(|w| has(w))
        };
        any(&self.stages, &|v| t.stage == v)
            && any(&self.kinds, &|v| t.kind == v)
            && any(&self.tags, &|v| t.tags.iter().any(|g| g == v))
            && any(&self.holders, &|v| holder(t) == v)
            // Blocked by the flag, or only by the tasks it depends on (`blocked_kind` "tasks",
            // derived, 0024), as the app's blocked chip and red card (blockWords.ts isBlocked).
            && (!self.blocked || t.blocked || !t.blocked_kind.is_empty())
            && task_matches(t, &self.words)
    }

    /// A filter on something only tasks have (a backlog note has no type, tag, holder or
    /// blocked flag), or a stage filter without Backlog: notes don't show under it.
    pub fn hides_notes(&self) -> bool {
        !self.kinds.is_empty()
            || !self.tags.is_empty()
            || !self.holders.is_empty()
            || self.blocked
            || (!self.stages.is_empty() && !self.stages.iter().any(|s| s == "backlog"))
    }
}

/// The board's orders (lib.ts `BOARD_SORTS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Needs you first, then high to low (P0–P3 read as the board does), then id; Done after
    /// the open ones, latest changed first.
    Priority,
    /// The day it was captured, latest first; the same day, the higher id first. No date last.
    Newest,
    /// The day it was captured, earliest first, then id. No date last.
    Oldest,
    /// Latest changed (file time) first.
    Changed,
    /// Smallest estimate (time, then tokens) first; not estimated last.
    Estimate,
    /// By project, then id.
    Id,
}

impl Sort {
    pub const NAMES: [&'static str; 6] =
        ["priority", "newest", "oldest", "changed", "estimate", "id"];

    pub fn parse(s: &str) -> Option<Sort> {
        Some(match s {
            "priority" => Sort::Priority,
            "newest" => Sort::Newest,
            "oldest" => Sort::Oldest,
            "changed" => Sort::Changed,
            "estimate" => Sort::Estimate,
            "id" => Sort::Id,
            _ => return None,
        })
    }
}

/// The board's priority rank (lib.ts `PRIORITY`): P0 and P1 are high, P2 normal, P3 low, and
/// anything else is normal.
fn rank(p: &str) -> u8 {
    match p {
        "high" | "P0" | "P1" => 0,
        "low" | "P3" => 2,
        _ => 1,
    }
}

fn by_place(a: &Task, b: &Task) -> Ordering {
    a.queue.cmp(&b.queue).then_with(|| a.id.cmp(&b.id))
}

fn estimated(t: &Task) -> bool {
    t.est_minutes > 0 || t.est_token_count > 0
}

fn compare(by: Sort, a: &Task, b: &Task) -> Ordering {
    match by {
        Sort::Priority => {
            let (da, db) = (a.stage == "done", b.stage == "done");
            da.cmp(&db).then_with(|| {
                if da {
                    b.modified.cmp(&a.modified).then_with(|| by_place(a, b))
                } else {
                    b.waiting_for_owner
                        .cmp(&a.waiting_for_owner)
                        .then_with(|| rank(&a.priority).cmp(&rank(&b.priority)))
                        .then_with(|| by_place(a, b))
                }
            })
        }
        Sort::Newest => a
            .captured
            .is_empty()
            .cmp(&b.captured.is_empty())
            .then_with(|| b.captured.cmp(&a.captured))
            .then_with(|| b.id.cmp(&a.id))
            .then_with(|| a.queue.cmp(&b.queue)),
        Sort::Oldest => a
            .captured
            .is_empty()
            .cmp(&b.captured.is_empty())
            .then_with(|| a.captured.cmp(&b.captured))
            .then_with(|| by_place(a, b)),
        Sort::Changed => b.modified.cmp(&a.modified).then_with(|| by_place(a, b)),
        Sort::Estimate => (!estimated(a))
            .cmp(&!estimated(b))
            .then_with(|| a.est_minutes.cmp(&b.est_minutes))
            .then_with(|| a.est_token_count.cmp(&b.est_token_count))
            .then_with(|| by_place(a, b)),
        Sort::Id => by_place(a, b),
    }
}

pub fn sort(tasks: &mut [&Task], by: Sort) {
    tasks.sort_by(|a, b| compare(by, a, b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn task(fm: &str, body: &str) -> Task {
        crate::queue::parse_task(
            "q",
            Path::new("/q"),
            "",
            Path::new("/q/0001-x.md"),
            &format!("---\n{fm}\n---\n\n{body}"),
            0,
        )
        .expect("front matter")
    }
    fn words(q: &str) -> Vec<String> {
        query_words(q)
    }

    #[test]
    fn fold_lowercases_drops_accents_and_punctuation() {
        assert_eq!(fold("Café RÉSUMÉ"), "cafe resume");
        assert_eq!(fold("Cafe\u{301}"), "cafe");
        assert_eq!(
            fold("Ærøskøbing ß Œuvre þorn"),
            "aeroskobing ss oeuvre thorn"
        );
        assert_eq!(fold("İstanbul ŁÓDŹ"), "istanbul lodz");
        assert_eq!(fold("tech-debt/#UI!"), "tech debt  ui ");
        assert_eq!(fold("Задача 任务"), "задача 任务");
        assert_eq!(fold(""), "");
    }

    #[test]
    fn the_search_text_is_title_tags_and_body_folded_once_without_digit_words() {
        let t = task(
            "id: 0001\nstage: ready\ntags: ui, tech-debt\nclaimed-by: someone",
            "# Board FLICKERS\n\nThe board flickers on résumé.\n\n## Log\n- 2026-10-19 09:00 capture: created\n",
        );
        assert_eq!(
            t.search,
            "board flickers ui tech debt the on resume log capture created"
        );
        // Front matter isn't text: its values reach search only as tags.
        assert!(!t.search.contains("someone"));
        assert!(!t.search.contains("stage"));
    }

    #[test]
    fn the_search_text_is_capped_at_a_word() {
        let body: String = (0..2000).map(|i| format!("word{i} ")).collect();
        let s = search_text("Title", &[], &body);
        assert!(s.len() <= MAX_SEARCH_BYTES, "{}", s.len());
        assert!(s.len() > MAX_SEARCH_BYTES - 12);
        assert!(s.starts_with("title word0 word1 "));
        assert!(!s.ends_with(' '));
        // A word is kept whole or not at all, so a multi-byte one is never cut inside.
        assert_eq!(search_text(&"日".repeat(2000), &[], "after"), "");
        assert_eq!(search_text("日本", &[], ""), "日本");
    }

    #[test]
    fn a_task_matches_every_word_in_any_order() {
        let t = task(
            "id: 0019\nstage: building\ntags: ui\nclaimed-by: drover/main (session 3fba2222)",
            "# Board flickers on resize\n\nThe columns jump.\n",
        );
        for (q, want) in [
            ("flickers", true),
            ("FLICK", true),
            ("jump columns", true),
            ("ui", true),
            ("drover", true),
            ("board crash", false),
            ("", true),
            ("  -- ", true),
            ("19", true),
            ("0019", true),
            ("00019", true),
            ("19 jump", true),
            ("20 jump", false),
            ("session", false),
        ] {
            assert_eq!(task_matches(&t, &words(q)), want, "{q:?}");
        }
        let other = task("id: 0119\nstage: ready", "# Other\n\nfixed on 2026-10-19\n");
        assert!(!task_matches(&other, &words("19")));
        assert!(task_matches(
            &task("id: 0000\nstage: ready", "# Zero\n"),
            &words("0")
        ));
        let accents = task("id: 0002\nstage: ready", "# Café résumé\n");
        assert!(task_matches(&accents, &words("cafe RESUME")));
        assert!(task_matches(
            &task("id: 0003\nstage: ready", "# cafe resume\n"),
            &words("Café")
        ));
        // A holder counts only while the task is held.
        let done = task("id: 0004\nstage: done\nclaimed-by: drover/main", "# Done\n");
        assert!(!task_matches(&done, &words("drover")));
    }

    #[test]
    fn holder_is_the_name_before_the_session() {
        let h = |fm: &str| holder(&task(fm, "# T\n")).to_string();
        assert_eq!(
            h("id: 0001\nstage: building\nclaimed-by: drover/main (session 3fba2222)"),
            "drover/main"
        );
        assert_eq!(
            h("id: 0001\nstage: ready\nclaimed-by: capture (drover/main, session 1)"),
            "capture"
        );
        assert_eq!(
            h("id: 0001\nstage: ready\nclaimed-by: wt-i18n, session 41f7fe18"),
            "wt-i18n"
        );
        assert_eq!(h("id: 0001\nstage: testing\nclaimed-by: drover/main"), "");
        assert_eq!(h("id: 0001\nstage: ready\nclaimed-by: (odd)"), "(odd)");
        // Session stamps (0030), and who holds Testing and Your check (`held-by`).
        assert_eq!(
            h("id: 0001\nstage: building\nclaimed-by: wt-a (wt-0030 · session 7b5e8856 · cc-hg)"),
            "wt-a"
        );
        assert_eq!(
            h("id: 0001\nstage: testing\nheld-by: qa (main-29 · session 1a2b3c4d)"),
            "qa"
        );
        assert_eq!(h("id: 0001\nstage: your-check"), "owner");
        assert_eq!(h("id: 0001\nstage: your-check\nheld-by: wt-b"), "wt-b");
        assert_eq!(h("id: 0001\nstage: done\nheld-by: wt-b"), "");
    }

    #[test]
    fn notes_match_on_title_and_text_never_on_digits() {
        let w = |q: &str| words(q);
        assert!(note_matches(
            "Flicker",
            "Flicker\nthe columns jump",
            &w("COLUMNS flick")
        ));
        assert!(!note_matches("Flicker", "Flicker", &w("export")));
        assert!(!note_matches("Release 19", "Release 19", &w("19")));
        assert!(note_matches("", "", &w("")));
    }

    #[test]
    fn filter_groups_any_within_every_across() {
        let a = task("id: 0001\ntype: bug\nstage: building\ntags: ui, design\nclaimed-by: drover/main (session 1)", "# A\n");
        let b = task(
            "id: 0002\ntype: task\nstage: ready\nwaiting_on: owner\ntags: docs",
            "# B\n",
        );
        let f = |f: Filter| {
            [&a, &b]
                .iter()
                .filter(|t| f.matches(t))
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
        };
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(f(Filter::default()), ["0001", "0002"]);
        assert_eq!(
            f(Filter {
                kinds: v(&["bug", "code"]),
                ..Default::default()
            }),
            ["0001", "0002"]
        );
        assert_eq!(
            f(Filter {
                kinds: v(&["bug"]),
                tags: v(&["docs"]),
                ..Default::default()
            }),
            Vec::<String>::new()
        );
        assert_eq!(
            f(Filter {
                stages: v(&["ready"]),
                ..Default::default()
            }),
            ["0002"]
        );
        assert_eq!(
            f(Filter {
                holders: v(&["drover/main"]),
                ..Default::default()
            }),
            ["0001"]
        );
        assert_eq!(
            f(Filter {
                blocked: true,
                ..Default::default()
            }),
            ["0002"]
        );
        assert_eq!(
            f(Filter {
                tags: v(&["design", "docs"]),
                words: words("b"),
                ..Default::default()
            }),
            ["0002"]
        );
        assert!(!Filter::default().hides_notes());
        assert!(!Filter {
            stages: v(&["backlog", "ready"]),
            words: words("x"),
            ..Default::default()
        }
        .hides_notes());
        assert!(Filter {
            stages: v(&["ready"]),
            ..Default::default()
        }
        .hides_notes());
        assert!(Filter {
            blocked: true,
            ..Default::default()
        }
        .hides_notes());
        assert!(Filter {
            tags: v(&["ui"]),
            ..Default::default()
        }
        .hides_notes());
    }

    #[test]
    fn blocked_keeps_a_task_blocked_only_by_the_tasks_it_depends_on() {
        // 0024: a block by tasks is derived from `depends:` (no flag, no waiting_on), and the
        // card shows it red, so `--blocked` keeps it too.
        let mut tasks = vec![
            task("id: 0001\nstage: building\nclaimed-by: wt-a", "# A\n"),
            task("id: 0002\nstage: ready\ndepends: 0001", "# B\n"),
            task("id: 0003\nstage: ready\ndepends: 0004", "# C\n"),
            task("id: 0004\nstage: done", "# D\n"),
        ];
        crate::block::derive(&mut tasks);
        assert!(!tasks[1].blocked);
        let f = Filter {
            blocked: true,
            ..Default::default()
        };
        let kept: Vec<&str> = tasks
            .iter()
            .filter(|t| f.matches(t))
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(kept, ["0002"]);
    }

    #[test]
    fn sorts_like_the_board() {
        let mk = |fm: &str, modified: u64| {
            let mut t = task(fm, "# T\n");
            t.modified = modified;
            t
        };
        let list = [
            mk(
                "id: 0001\nstage: ready\npriority: low\ncaptured: 2026-10-01\nest-time: 2h",
                30,
            ),
            mk(
                "id: 0002\nstage: ready\npriority: normal\ncaptured: 2026-10-03",
                10,
            ),
            mk(
                "id: 0003\nstage: your-check\ncaptured: 2026-10-03\nest-time: 30m",
                20,
            ),
            mk(
                "id: 0004\nstage: ready\npriority: P0\nest-time: 30m\nest-tokens: 5k",
                40,
            ),
            mk("id: 0005\nstage: done\npriority: high", 1),
            mk("id: 0006\nstage: done", 9),
        ];
        let order = |by: &str| {
            let mut v: Vec<&Task> = list.iter().collect();
            sort(&mut v, Sort::parse(by).expect("a sort"));
            v.iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(order("priority"), "0003 0004 0002 0001 0006 0005");
        assert_eq!(order("newest"), "0003 0002 0001 0006 0005 0004");
        assert_eq!(order("oldest"), "0001 0002 0003 0004 0005 0006");
        assert_eq!(order("changed"), "0004 0001 0003 0002 0006 0005");
        assert_eq!(order("estimate"), "0003 0004 0001 0002 0005 0006");
        assert_eq!(order("id"), "0001 0002 0003 0004 0005 0006");
        assert_eq!(Sort::parse("sideways"), None);
        for n in Sort::NAMES {
            assert!(Sort::parse(n).is_some(), "{n}");
        }
    }
}
