//! Lessons from reviews (task 0027): a kind of mistake the review of record keeps finding becomes
//! a one-line rule for the next agents, and one it keeps dismissing becomes something reviewers
//! don't flag. When findings of one kind (`review::Finding::kind`) are confirmed on two tasks or
//! more, muster proposes a lesson for builders; dismissed as false positives on two or more, one
//! for reviewers. Nothing an agent wrote becomes an instruction without the owner's yes: the owner
//! accepts or rejects proposals in one batch (`decide`), never one at a time by necessity.
//!
//! Accepted lessons for builders go into `muster start`'s brief for the areas they name (the
//! folders their findings were in, matched against the task's `touches:` as `ready --parallel`
//! matches them), and both kinds into the review brief. At most `MAX_LIVE` are live: accepting
//! more expires the ones that mattered least recently. A lesson that hasn't mattered (no finding
//! of its kind recorded, and not accepted) in `EXPIRE_DAYS` days expires on its own and leaves the
//! briefs; a new finding of its kind brings it back.
//!
//! `reviews/lessons.json` in the queue's folder is the truth, written only here; `lessons.md`
//! there is rendered from it on every change, for people to read. `layout` says where: the old
//! queue's folder, or `.muster/reviews/` and `.muster/lessons.md` (task 0005; a local queue keeps
//! them out of git like the rest).

use crate::queue::{actor, with_lock, write_atomic};
use crate::review::{self, Finding, Review};
use crate::session::is_owner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Live lessons (accepted, not expired), at most.
pub const MAX_LIVE: usize = 40;
/// A lesson that hasn't mattered in this many days expires.
pub const EXPIRE_DAYS: i64 = 60;
/// Distinct tasks a kind must recur on before a lesson is proposed.
const RECUR: usize = 2;
/// Areas one lesson names, at most.
const MAX_AREAS: usize = 5;
const MAX_STORE: u64 = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    #[serde(default)]
    pub lessons: Vec<Lesson>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    /// L1, L2…
    pub id: String,
    pub kind: String,
    /// "builders" (from confirmed findings) or "reviewers" (from false positives: don't flag).
    #[serde(rename = "for")]
    pub for_who: String,
    /// proposed, accepted, rejected or expired (over the cap). An accepted lesson that hasn't
    /// mattered in `EXPIRE_DAYS` reads as expired without being written so (`expired`).
    pub state: String,
    pub rule: String,
    #[serde(default)]
    pub bad: String,
    #[serde(default)]
    pub good: String,
    /// The folders its findings were in (`app/src-tauri/src/`), or the files at the top.
    #[serde(default)]
    pub areas: Vec<String>,
    /// The tasks and findings it came from: `0027 R1`.
    #[serde(default)]
    pub from: Vec<String>,
    pub proposed: String,
    #[serde(default)]
    pub decided: String,
    #[serde(default)]
    pub decided_by: String,
    /// Why the owner rejected it, or why it expired.
    #[serde(default)]
    pub why: String,
    /// When the cap expired it (state `expired`), "" otherwise: its kind is proposed again only
    /// once a finding of it comes after this.
    #[serde(default)]
    pub expired: String,
}

fn path(root: &Path) -> PathBuf {
    crate::layout::reviews_dir(root).join("lessons.json")
}

/// The queue's lessons; none before the first proposal. A file that doesn't read is an error,
/// so nothing writes over it.
pub fn load(root: &Path) -> Result<Book, String> {
    let p = path(root);
    let Ok(meta) = fs::metadata(&p) else {
        return Ok(Book::default());
    };
    if meta.len() > MAX_STORE {
        return Err(format!(
            "{} is over 2 MB; muster never writes one that big",
            p.display()
        ));
    }
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&text).map_err(|e| {
        format!(
            "{} doesn't read ({e}); only muster writes it, so don't edit it by hand",
            p.display()
        )
    })
}

fn save(root: &Path, book: &Book, reviews: &[Review], now: &str) -> Result<(), String> {
    fs::create_dir_all(crate::layout::reviews_dir(root)).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(book).map_err(|e| e.to_string())?;
    write_atomic(&path(root), &format!("{json}\n"))?;
    write_atomic(
        &crate::layout::lessons_file(root),
        &render(book, reviews, now),
    )
}

/// The findings of `kind` across every review, with their task.
fn of_kind<'a>(reviews: &'a [Review], kind: &str) -> Vec<(&'a str, &'a Finding)> {
    reviews
        .iter()
        .flat_map(|r| r.findings.iter().map(move |f| (r.task.as_str(), f)))
        .filter(|(_, f)| f.kind == kind)
        .collect()
}

/// When lesson `l` last mattered: the latest finding of its kind, or its acceptance.
pub fn mattered(l: &Lesson, reviews: &[Review]) -> String {
    of_kind(reviews, &l.kind)
        .iter()
        .map(|(_, f)| f.found_at().to_string())
        .chain([l.decided.clone(), l.proposed.clone()])
        .max()
        .unwrap_or_default()
}

/// Has lesson `l` expired by `now`: over the cap, or accepted and not mattered in
/// `EXPIRE_DAYS`?
pub fn expired(l: &Lesson, reviews: &[Review], now: &str) -> bool {
    if l.state == "expired" {
        return true;
    }
    if l.state != "accepted" {
        return false;
    }
    match (
        crate::ops::stamp_minutes(&mattered(l, reviews)),
        crate::ops::stamp_minutes(now),
    ) {
        (Some(m), Some(n)) => n - m > EXPIRE_DAYS * 24 * 60,
        _ => false,
    }
}

fn live<'a>(book: &'a Book, reviews: &[Review], now: &str, for_who: &str) -> Vec<&'a Lesson> {
    book.lessons
        .iter()
        .filter(|l| l.state == "accepted" && l.for_who == for_who && !expired(l, reviews, now))
        .collect()
}

/// The lessons for a builder whose task touches `touches`: live, for builders, with an area
/// that overlaps one of them (as `ready --parallel` reads touches). A task that doesn't say where
/// it works gets them all, and so does a lesson that names no area.
pub fn for_builders(book: &Book, reviews: &[Review], touches: &[String], now: &str) -> Vec<Lesson> {
    live(book, reviews, now, "builders")
        .into_iter()
        .filter(|l| {
            touches.is_empty()
                || l.areas.is_empty()
                || crate::parallel::touches_overlap(&l.areas, touches)
        })
        .cloned()
        .collect()
}

/// The lessons for reviewers: what not to flag.
pub fn for_reviewers(book: &Book, reviews: &[Review], now: &str) -> Vec<Lesson> {
    live(book, reviews, now, "reviewers")
        .into_iter()
        .cloned()
        .collect()
}

/// Every kind of mistake named so far, in findings or lessons, for reviewers to reuse.
pub fn kinds(book: &Book, reviews: &[Review]) -> Vec<String> {
    let mut k: Vec<String> = reviews
        .iter()
        .flat_map(|r| r.findings.iter().map(|f| f.kind.clone()))
        .chain(book.lessons.iter().map(|l| l.kind.clone()))
        .filter(|k| !k.is_empty())
        .collect();
    k.sort();
    k.dedup();
    k.truncate(100);
    k
}

/// A lesson on one line, for a brief: `L3 (unbounded-output): <rule> Bad: … Good: …`.
pub fn line(l: &Lesson) -> String {
    let mut s = format!("{} ({}): {}", l.id, l.kind, l.rule);
    if !l.bad.is_empty() {
        s.push_str(&format!(" Bad: {}", l.bad));
    }
    if !l.good.is_empty() {
        s.push_str(&format!(" Good: {}", l.good));
    }
    s
}

/// The folder a finding's file is in, with its slash (`app/src/`), or the file at the top.
fn area(file: &str) -> String {
    match file.trim_start_matches("./").rfind('/') {
        Some(i) => file.trim_start_matches("./")[..=i].to_string(),
        None => file.to_string(),
    }
}

/// Propose a lesson for every kind that has recurred and has none yet (proposed, accepted or
/// rejected; an expired one may come back), under the queue's lock the caller holds. Returns
/// the new proposals.
pub(crate) fn propose_locked(root: &Path, stamp: &str) -> Result<Vec<Lesson>, String> {
    let reviews = review::all(root);
    let mut book = load(root)?;
    // (kind, for) → its findings that count, in the order they were found.
    let mut groups: BTreeMap<(String, &str), Vec<(&str, &Finding)>> = BTreeMap::new();
    for r in &reviews {
        for f in &r.findings {
            let for_who = match f.state.as_str() {
                "confirmed" => "builders",
                "false-positive" => "reviewers",
                _ => continue,
            };
            if !f.kind.is_empty() {
                groups
                    .entry((f.kind.clone(), for_who))
                    .or_default()
                    .push((r.task.as_str(), f));
            }
        }
    }
    let mut new = Vec::new();
    let mut next = book
        .lessons
        .iter()
        .filter_map(|l| l.id.strip_prefix('L')?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    for ((kind, for_who), mut found) in groups {
        let mut tasks: Vec<&str> = found.iter().map(|(t, _)| *t).collect();
        tasks.sort();
        tasks.dedup();
        let latest = found.iter().map(|(_, f)| f.found_at()).max().unwrap_or("");
        let known = book.lessons.iter().any(|l| {
            l.kind == kind
                && l.for_who == for_who
                && match l.state.as_str() {
                    "proposed" | "accepted" | "rejected" => true,
                    // Expired by the cap: back only when its kind was found again since.
                    "expired" => latest <= l.expired.as_str(),
                    _ => false,
                }
        });
        if tasks.len() < RECUR || known {
            continue;
        }
        found.sort_by(|a, b| a.1.found_at().cmp(b.1.found_at()));
        let latest = found.last().map(|(_, f)| *f).ok_or("no findings")?;
        let taught = found.iter().rev().find_map(|(_, f)| f.lesson.as_ref());
        let rule = match (taught, for_who) {
            (Some(t), _) => t.rule.clone(),
            (None, "builders") => latest.title.clone(),
            (None, _) => format!("Don't flag: {}", latest.title),
        };
        let mut areas: Vec<String> = found.iter().map(|(_, f)| area(&f.file)).collect();
        areas.sort();
        areas.dedup();
        areas.truncate(MAX_AREAS);
        let l = Lesson {
            id: format!("L{next}"),
            kind,
            for_who: for_who.into(),
            state: "proposed".into(),
            rule,
            bad: taught.map(|t| t.bad.clone()).unwrap_or_default(),
            good: taught.map(|t| t.good.clone()).unwrap_or_default(),
            areas,
            from: found.iter().map(|(t, f)| format!("{t} {}", f.id)).collect(),
            proposed: stamp.into(),
            ..Default::default()
        };
        next += 1;
        book.lessons.push(l.clone());
        new.push(l);
    }
    if !new.is_empty() {
        save(root, &book, &reviews, stamp)?;
    }
    Ok(new)
}

/// What `decide` did.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Decided {
    pub accepted: Vec<String>,
    pub rejected: Vec<String>,
    /// Live lessons expired to stay under the cap, the ones that mattered least recently.
    pub expired: Vec<String>,
}

/// The owner's batch: accept `accept` (every proposal when `all`) and reject `reject` with `why`.
/// Only the owner: a lesson becomes an instruction to future agents. Every id named must be a
/// proposal; nothing changes when one isn't.
pub fn decide(
    root: &Path,
    accept: &[String],
    all: bool,
    reject: &[String],
    why: &str,
    who: &str,
    stamp: &str,
) -> Result<Decided, String> {
    let who = actor(who)?;
    if !is_owner(&who) {
        return Err("only the owner accepts or rejects lessons, since they become instructions to the next agents (--as owner)".into());
    }
    let why = if reject.is_empty() {
        String::new()
    } else {
        crate::ops::one_line(why, 500, "why they're rejected")?
    };
    with_lock(root, || {
        let reviews = review::all(root);
        let mut book = load(root)?;
        let proposals: Vec<String> = book
            .lessons
            .iter()
            .filter(|l| l.state == "proposed")
            .map(|l| l.id.clone())
            .collect();
        let accept: Vec<String> = if all {
            proposals
                .iter()
                .filter(|p| !reject.contains(p))
                .cloned()
                .collect()
        } else {
            accept.to_vec()
        };
        if accept.is_empty() && reject.is_empty() {
            return Err("no lessons are waiting for you".into());
        }
        for id in accept.iter().chain(reject) {
            if !proposals.contains(id) {
                return Err(match book.lessons.iter().find(|l| l.id == *id) {
                    Some(l) => format!("{id} is {}, not a proposal", l.state),
                    None => format!("there's no lesson {id}"),
                });
            }
        }
        if let Some(both) = accept.iter().find(|a| reject.contains(a)) {
            return Err(format!("{both} is both accepted and rejected"));
        }
        let mut done = Decided::default();
        for l in book.lessons.iter_mut() {
            let (state, list) = if accept.contains(&l.id) {
                ("accepted", &mut done.accepted)
            } else if reject.contains(&l.id) {
                ("rejected", &mut done.rejected)
            } else {
                continue;
            };
            l.state = state.into();
            l.decided = stamp.into();
            l.decided_by = who.clone();
            if state == "rejected" {
                l.why = why.clone();
            }
            list.push(l.id.clone());
        }
        // Over the cap, the live ones that mattered least recently expire.
        let mut live: Vec<(String, String)> = book
            .lessons
            .iter()
            .filter(|l| l.state == "accepted" && !expired(l, &reviews, stamp))
            .map(|l| (mattered(l, &reviews), l.id.clone()))
            .collect();
        live.sort();
        let over = live.len().saturating_sub(MAX_LIVE);
        for (_, id) in live.into_iter().take(over) {
            if let Some(l) = book.lessons.iter_mut().find(|l| l.id == id) {
                l.state = "expired".into();
                l.expired = stamp.into();
                l.why = format!("over the cap of {MAX_LIVE}: it mattered least recently");
                done.expired.push(id);
            }
        }
        save(root, &book, &reviews, stamp)?;
        Ok(done)
    })
}

/// The proposals by `lessons.json`'s path, each its id and rule, with the size and time it was
/// read at: the window asks on every poll, and an unchanged file isn't parsed again (as task files
/// are cached).
type Seen = (u64, std::time::SystemTime, Vec<(String, String)>);
static WAITING: std::sync::Mutex<BTreeMap<PathBuf, Seen>> = std::sync::Mutex::new(BTreeMap::new());

/// How many proposals wait for the owner.
pub fn waiting(root: &Path) -> usize {
    with_proposed(root, <[_]>::len)
}

/// The proposals waiting for the owner, each its id and rule (for the notifications, 0052).
pub fn proposed(root: &Path) -> Vec<(String, String)> {
    with_proposed(root, <[_]>::to_vec)
}

/// `f` of the proposals in `root`'s `lessons.json`, parsed only when it changed; none when it's
/// missing or doesn't read.
fn with_proposed<T>(root: &Path, f: impl FnOnce(&[(String, String)]) -> T) -> T {
    let p = path(root);
    let Some((len, at)) = fs::metadata(&p)
        .ok()
        .and_then(|m| Some((m.len(), m.modified().ok()?)))
    else {
        return f(&[]);
    };
    if let Ok(c) = WAITING.lock() {
        if let Some((l, a, ps)) = c.get(&p) {
            if (*l, *a) == (len, at) {
                return f(ps);
            }
        }
    }
    let ps = read_proposed(root);
    let out = f(&ps);
    if let Ok(mut c) = WAITING.lock() {
        c.insert(p, (len, at, ps));
    }
    out
}

fn read_proposed(root: &Path) -> Vec<(String, String)> {
    load(root)
        .map(|b| {
            b.lessons
                .into_iter()
                .filter(|l| l.state == "proposed")
                .map(|l| (l.id, l.rule))
                .collect()
        })
        .unwrap_or_default()
}

/// One lesson in `lessons.md`.
fn md_item(l: &Lesson, reviews: &[Review], now: &str) -> String {
    let one = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect()
    };
    let status = match l.state.as_str() {
        "accepted" if expired(l, reviews, now) => format!(
            "expired: nothing of its kind since {}",
            mattered(l, reviews)
        ),
        "accepted" => format!("accepted {} by {}", l.decided, one(&l.decided_by)),
        "proposed" => format!("proposed {}", l.proposed),
        "rejected" => format!("rejected {}: {}", l.decided, one(&l.why)),
        other => format!("{other}: {}", one(&l.why)),
    };
    let mut s = format!(
        "- **{}** `{}` · {status} · from {}\n  - {}\n",
        l.id,
        l.kind,
        l.from.join(", "),
        one(&l.rule)
    );
    if !l.bad.is_empty() {
        s.push_str(&format!("  - Bad: {}\n", one(&l.bad)));
    }
    if !l.good.is_empty() {
        s.push_str(&format!("  - Good: {}\n", one(&l.good)));
    }
    s
}

/// `lessons.md`: live lessons for builders by area, then for reviewers, then the proposals, then
/// what expired or was rejected.
pub fn render(book: &Book, reviews: &[Review], now: &str) -> String {
    let mut s = String::from("# Lessons from reviews\n\nWritten by muster from the reviews of record (task 0027); don't edit it by hand. Lessons for builders go into `muster start`'s brief for the areas they name, and lessons for reviewers into the review brief, as what not to flag. Proposals wait for the owner, who accepts or rejects them in one batch: `muster lessons accept --all --as owner`.\n");
    let live_b = live(book, reviews, now, "builders");
    s.push_str("\n## For builders\n");
    if live_b.is_empty() {
        s.push_str("\nNone yet.\n");
    }
    let mut by_area: BTreeMap<String, Vec<&Lesson>> = BTreeMap::new();
    for l in live_b {
        let a = l
            .areas
            .first()
            .cloned()
            .unwrap_or_else(|| "everywhere".into());
        by_area.entry(a).or_default().push(l);
    }
    for (a, ls) in by_area {
        s.push_str(&format!("\n### {a}\n\n"));
        for l in ls {
            s.push_str(&md_item(l, reviews, now));
        }
    }
    let mut section = |title: &str, keep: &dyn Fn(&Lesson) -> bool| {
        let ls: Vec<&Lesson> = book.lessons.iter().filter(|l| keep(l)).collect();
        s.push_str(&format!("\n## {title}\n\n"));
        if ls.is_empty() {
            s.push_str("None.\n");
        }
        for l in ls {
            s.push_str(&md_item(l, reviews, now));
        }
    };
    section("For reviewers: don't flag", &|l| {
        l.state == "accepted" && l.for_who == "reviewers" && !expired(l, reviews, now)
    });
    section("Proposed, waiting for the owner", &|l| {
        l.state == "proposed"
    });
    section("Expired or rejected", &|l| {
        l.state == "rejected" || expired(l, reviews, now)
    });
    s
}

/// `muster lessons`: what waits for the owner, then what's live.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub proposed: Vec<Lesson>,
    pub builders: Vec<Lesson>,
    pub reviewers: Vec<Lesson>,
    pub expired_or_rejected: usize,
    pub file: String,
}

pub fn listing(root: &Path, now: &str) -> Result<Listing, String> {
    let book = load(root)?;
    let reviews = review::all(root);
    let pick = |f: &dyn Fn(&Lesson) -> bool| -> Vec<Lesson> {
        book.lessons.iter().filter(|l| f(l)).cloned().collect()
    };
    Ok(Listing {
        proposed: pick(&|l| l.state == "proposed"),
        builders: live(&book, &reviews, now, "builders")
            .into_iter()
            .cloned()
            .collect(),
        reviewers: live(&book, &reviews, now, "reviewers")
            .into_iter()
            .cloned()
            .collect(),
        expired_or_rejected: book
            .lessons
            .iter()
            .filter(|l| l.state == "rejected" || expired(l, &reviews, now))
            .count(),
        file: crate::layout::lessons_file(root).display().to_string(),
    })
}

pub fn listing_text(l: &Listing) -> String {
    let mut out = String::new();
    let block = |title: &str, ls: &[Lesson], out: &mut String| {
        out.push_str(&format!("{title}:\n"));
        if ls.is_empty() {
            out.push_str("  (none)\n");
        }
        for x in ls {
            let areas = if x.areas.is_empty() {
                String::new()
            } else {
                format!(" [{}]", x.areas.join(", "))
            };
            out.push_str(&format!(
                "  {}{areas} · from {}\n",
                crate::backlog::one_line_name(&line(x)),
                x.from.join(", ")
            ));
        }
    };
    block("Proposed, waiting for the owner", &l.proposed, &mut out);
    if !l.proposed.is_empty() {
        out.push_str("  Accept or reject them in one batch: muster lessons accept --all --as owner (or name them: accept L1,L2; reject L3 \"<why>\")\n");
    }
    block(
        "For builders (in muster start's brief)",
        &l.builders,
        &mut out,
    );
    block("For reviewers (don't flag)", &l.reviewers, &mut out);
    if l.expired_or_rejected > 0 {
        out.push_str(&format!(
            "Expired or rejected: {} (see {})\n",
            l.expired_or_rejected, l.file
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{LessonText, Step};

    fn found(
        task: &str,
        id: &str,
        kind: &str,
        state: &str,
        file: &str,
        at: &str,
    ) -> (String, Finding) {
        (
            task.to_string(),
            Finding {
                id: id.into(),
                round: 1,
                lens: "correctness".into(),
                severity: "should".into(),
                file: file.into(),
                title: format!("{kind} in {file}"),
                scenario: "x".into(),
                kind: kind.into(),
                state: state.into(),
                lesson: (kind == "unbounded-output").then(|| LessonText {
                    rule: "Cap every external program's output.".into(),
                    bad: "cmd.output()".into(),
                    good: "take(1 MB)".into(),
                }),
                history: vec![Step {
                    at: at.into(),
                    by: "qa".into(),
                    what: "found".into(),
                    text: String::new(),
                }],
                ..Default::default()
            },
        )
    }

    /// Reviews written as `review` stores them, one per task.
    fn write(root: &Path, items: &[(String, Finding)]) {
        let mut by: BTreeMap<String, Review> = BTreeMap::new();
        for (t, f) in items {
            let r = by.entry(t.clone()).or_insert_with(|| Review {
                task: t.clone(),
                ..Default::default()
            });
            r.findings.push(f.clone());
        }
        fs::create_dir_all(root.join("reviews")).unwrap();
        for (t, r) in by {
            fs::write(
                review::path_of(root, &t),
                serde_json::to_string(&r).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn a_kind_confirmed_on_two_tasks_is_proposed_once_and_needs_the_owners_yes() {
        let d = crate::testkit::tempdir().unwrap();
        let root = d.path();
        write(
            root,
            &[found(
                "0001",
                "R1",
                "unbounded-output",
                "confirmed",
                "app/src-tauri/src/a.rs",
                "2026-10-13 10:00",
            )],
        );
        assert!(
            propose_locked(root, "2026-10-13 10:00").unwrap().is_empty(),
            "one task isn't a pattern"
        );
        write(
            root,
            &[
                found(
                    "0001",
                    "R1",
                    "unbounded-output",
                    "confirmed",
                    "app/src-tauri/src/a.rs",
                    "2026-10-13 10:00",
                ),
                found(
                    "0002",
                    "R4",
                    "unbounded-output",
                    "confirmed",
                    "app/src-tauri/src/b.rs",
                    "2026-10-13 11:00",
                ),
                // Open and disputed findings aren't labels yet.
                found(
                    "0002",
                    "R5",
                    "missing-timeout",
                    "open",
                    "app/x.rs",
                    "2026-10-13 11:00",
                ),
                found(
                    "0003",
                    "R1",
                    "missing-timeout",
                    "disputed",
                    "app/x.rs",
                    "2026-10-13 11:00",
                ),
                // Dismissed on two tasks: one for reviewers.
                found(
                    "0002",
                    "R6",
                    "style-taste",
                    "false-positive",
                    "app/src/y.ts",
                    "2026-10-13 11:00",
                ),
                found(
                    "0003",
                    "R2",
                    "style-taste",
                    "false-positive",
                    "app/src/z.ts",
                    "2026-10-13 11:00",
                ),
            ],
        );
        let p = propose_locked(root, "2026-10-13 12:00").unwrap();
        let ids: Vec<(&str, &str, &str)> = p
            .iter()
            .map(|l| (l.id.as_str(), l.kind.as_str(), l.for_who.as_str()))
            .collect();
        assert_eq!(
            ids,
            [
                ("L1", "style-taste", "reviewers"),
                ("L2", "unbounded-output", "builders")
            ]
        );
        let l2 = &p[1];
        assert_eq!(l2.rule, "Cap every external program's output.");
        assert_eq!(
            (l2.bad.as_str(), l2.good.as_str()),
            ("cmd.output()", "take(1 MB)")
        );
        assert_eq!(l2.areas, ["app/src-tauri/src/"]);
        assert_eq!(l2.from, ["0001 R1", "0002 R4"]);
        assert_eq!(p[0].rule, "Don't flag: style-taste in app/src/z.ts");
        assert!(
            propose_locked(root, "2026-10-13 12:05").unwrap().is_empty(),
            "proposed once"
        );
        assert_eq!(waiting(root), 2);
        let md = fs::read_to_string(crate::layout::lessons_file(root)).unwrap();
        assert!(
            md.contains("## Proposed, waiting for the owner\n\n- **L1** `style-taste`"),
            "{md}"
        );
        // Proposals aren't instructions yet.
        let book = load(root).unwrap();
        let reviews = review::all(root);
        assert!(for_builders(&book, &reviews, &[], "2026-10-13 12:10").is_empty());
        // Only the owner decides; an id that isn't a proposal changes nothing.
        assert!(decide(root, &[], true, &[], "", "wt-a", "2026-10-13 12:10")
            .unwrap_err()
            .contains("only the owner"));
        assert!(decide(
            root,
            &["L9".into()],
            false,
            &[],
            "",
            "owner",
            "2026-10-13 12:10"
        )
        .unwrap_err()
        .contains("no lesson L9"));
        assert!(
            decide(
                root,
                &[],
                false,
                &["L1".into()],
                " ",
                "owner",
                "2026-10-13 12:10"
            )
            .is_err(),
            "a rejection says why"
        );
        let done = decide(
            root,
            &[],
            true,
            &["L1".into()],
            "taste, not a defect",
            "owner",
            "2026-10-13 12:10",
        )
        .unwrap();
        assert_eq!(
            (done.accepted, done.rejected),
            (vec!["L2".to_string()], vec!["L1".to_string()])
        );
        assert!(decide(
            root,
            &["L2".into()],
            false,
            &[],
            "",
            "owner",
            "2026-10-13 12:11"
        )
        .unwrap_err()
        .contains("accepted, not a proposal"));
        assert_eq!(waiting(root), 0);
        let book = load(root).unwrap();
        let mine = |touches: &[&str], now: &str| -> Vec<String> {
            let t: Vec<String> = touches.iter().map(|s| s.to_string()).collect();
            for_builders(&book, &reviews, &t, now)
                .into_iter()
                .map(|l| l.id)
                .collect()
        };
        assert_eq!(
            mine(&["app/src-tauri/src/review.rs"], "2026-10-13 13:00"),
            ["L2"]
        );
        assert_eq!(
            mine(&["app/src/"], "2026-10-13 13:00"),
            Vec::<String>::new(),
            "another area"
        );
        assert_eq!(
            mine(&[], "2026-10-13 13:00"),
            ["L2"],
            "no touches: every lesson"
        );
        // A rejected kind isn't proposed again.
        assert!(propose_locked(root, "2026-10-14 10:00").unwrap().is_empty());
        let md = fs::read_to_string(crate::layout::lessons_file(root)).unwrap();
        assert!(md.contains("### app/src-tauri/src/\n\n- **L2** `unbounded-output` · accepted 2026-10-13 12:10 by owner"), "{md}");
        assert!(
            md.contains("rejected 2026-10-13 12:10: taste, not a defect"),
            "{md}"
        );
    }

    #[test]
    fn the_waiting_count_is_read_again_only_when_lessons_json_changes() {
        let d = crate::testkit::tempdir().unwrap();
        let root = d.path();
        assert_eq!(waiting(root), 0, "no file");
        fs::create_dir_all(crate::layout::reviews_dir(root)).unwrap();
        let file = path(root);
        let book = |n: usize| {
            let ls: Vec<String> = (1..=n)
                .map(|i| format!(r#"{{"id":"L{i}","kind":"k{i}","for":"builders","state":"proposed","rule":"r","proposed":"2026-10-13 10:00"}}"#))
                .collect();
            format!(r#"{{"lessons":[{}]}}"#, ls.join(","))
        };
        fs::write(&file, book(1)).unwrap();
        assert_eq!(waiting(root), 1);
        // The same size and time: the poll doesn't parse it again (garbage of the same length
        // still reads as 1).
        let at = fs::metadata(&file).unwrap().modified().unwrap();
        let same = "x".repeat(book(1).len());
        fs::write(&file, &same).unwrap();
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(at)
            .unwrap();
        assert_eq!(waiting(root), 1);
        // The proposals themselves, for the notifications (0052), from the same read.
        assert_eq!(proposed(root), [("L1".to_string(), "r".to_string())]);
        // Changed: read again.
        fs::write(&file, book(2)).unwrap();
        assert_eq!(waiting(root), 2);
        assert_eq!(proposed(root).len(), 2);
        fs::remove_file(&file).unwrap();
        assert_eq!(waiting(root), 0);
        assert!(proposed(root).is_empty());
    }

    #[test]
    fn a_lesson_the_cap_expired_isnt_proposed_again_until_its_kind_comes_back() {
        let d = crate::testkit::tempdir().unwrap();
        let root = d.path();
        // 41 kinds, each confirmed on two tasks.
        let mut items = Vec::new();
        for k in 1..=MAX_LIVE + 1 {
            let kind = format!("kind-{k:02}");
            let at = format!("2026-10-{:02} 10:00", 1 + k % 28);
            items.push(found(
                "0001",
                &format!("R{k}"),
                &kind,
                "confirmed",
                "a.rs",
                &at,
            ));
            items.push(found(
                "0002",
                &format!("R{k}"),
                &kind,
                "confirmed",
                "a.rs",
                &at,
            ));
        }
        write(root, &items);
        assert_eq!(
            propose_locked(root, "2026-10-29 10:00").unwrap().len(),
            MAX_LIVE + 1
        );
        let done = decide(root, &[], true, &[], "", "owner", "2026-10-29 11:00").unwrap();
        assert_eq!(done.expired.len(), 1, "41 accepted, one over the cap");
        // The next record proposes nothing: the expired one isn't back.
        assert!(propose_locked(root, "2026-10-29 12:00").unwrap().is_empty());
        assert!(propose_locked(root, "2026-10-29 13:00").unwrap().is_empty());
        // A new finding of its kind after it expired brings it back as a proposal.
        let kind = load(root)
            .unwrap()
            .lessons
            .iter()
            .find(|l| l.state == "expired")
            .map(|l| l.kind.clone())
            .unwrap();
        items.push(found(
            "0003",
            "R1",
            &kind,
            "confirmed",
            "a.rs",
            "2026-10-30 09:00",
        ));
        write(root, &items);
        let again = propose_locked(root, "2026-10-30 10:00").unwrap();
        assert_eq!(
            again.iter().map(|l| l.kind.as_str()).collect::<Vec<_>>(),
            [kind.as_str()]
        );
    }

    #[test]
    fn a_lesson_expires_after_sixty_quiet_days_and_the_cap_expires_the_stalest() {
        let d = crate::testkit::tempdir().unwrap();
        let root = d.path();
        let mut book = Book::default();
        for i in 1..=MAX_LIVE + 1 {
            book.lessons.push(Lesson {
                id: format!("L{i}"),
                kind: format!("kind-{i}"),
                for_who: "builders".into(),
                state: if i == MAX_LIVE + 1 {
                    "proposed"
                } else {
                    "accepted"
                }
                .into(),
                rule: "r".into(),
                proposed: "2026-08-01 10:00".into(),
                decided: format!("2026-08-{:02} 10:00", i.min(28)),
                ..Default::default()
            });
        }
        // L1's kind was found again recently: it mattered last, so it stays.
        write(
            root,
            &[found(
                "0001",
                "R1",
                "kind-1",
                "open",
                "a.rs",
                "2026-09-20 10:00",
            )],
        );
        fs::create_dir_all(root.join("reviews")).unwrap();
        fs::write(path(root), serde_json::to_string(&book).unwrap()).unwrap();
        let done = decide(
            root,
            &["L41".into()],
            false,
            &[],
            "",
            "owner",
            "2026-09-25 10:00",
        )
        .unwrap();
        assert_eq!(done.expired, ["L2"], "the stalest of 41 live ones");
        let book = load(root).unwrap();
        let reviews = review::all(root);
        let alive = |now: &str| for_builders(&book, &reviews, &[], now).len();
        assert_eq!(alive("2026-09-25 10:00"), MAX_LIVE);
        // Sixty days after their acceptance the quiet ones expire; L1 (found 2026-09-20) and L41
        // (accepted 2026-09-25) are still live.
        let late: Vec<String> = for_builders(&book, &reviews, &[], "2026-11-19 10:00")
            .into_iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(late, ["L1", "L41"]);
    }
}
