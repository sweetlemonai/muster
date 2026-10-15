//! The shaping lenses (task 0076): the concerns every project shares, which a shaped task answers
//! with one line each under `**Lenses:**` in its body: what the task must do for the lens, or
//! "n/a" and why. Every lens applies unless the project skips it explicitly, with the reason:
//!
//! - a `.muster` queue, in `config.toml`:
//!
//!   ```toml
//!   [shaping]
//!   skip = ["payments"]
//!
//!   [shaping.reasons]
//!   payments = "a free tool: no paid features, licences or prices"
//!   ```
//!
//! - an old queue, in README.md's front matter: `shaping-skip: payments, themes`, and one
//!   `shaping-reason-<lens>: <why>` line for each lens it skips.
//!
//! A skip naming no lens, or with no reason, doesn't count (the lens still applies), and
//! `muster check` reports it. A project's `docs/VISION.md` says which lenses it needs on a
//! `**Lenses needed:**` line; the vision check flags a task that calls one of them "n/a".
//!
//! Config and task text are data: read with caps, cleaned to one line where they're shown.

use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::Path;

/// One concern a shaped task answers.
#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lens {
    /// What a project's skip and its VISION's line call it.
    pub id: &'static str,
    /// The label of the task's line.
    pub name: &'static str,
    /// What the line covers.
    pub write: &'static str,
    /// The words a line's label may name it by.
    #[serde(skip)]
    words: &'static [&'static str],
}

/// Every lens, in the order a task's Lenses block lists them.
pub const LENSES: [Lens; 10] = [
    Lens {
        id: "accessibility",
        name: "Accessibility",
        write: "keyboard, screen reader labels, contrast, reduced motion",
        words: &["accessibility", "a11y"],
    },
    Lens {
        id: "i18n",
        name: "i18n and RTL",
        write: "strings, Persian and right-to-left layout, digits, dates",
        words: &[
            "i18n",
            "rtl",
            "l10n",
            "translation",
            "localisation",
            "localization",
        ],
    },
    Lens {
        id: "themes",
        name: "Themes",
        write: "light and dark, the project's tokens",
        words: &["themes", "theme", "theming"],
    },
    Lens {
        id: "payments",
        name: "Payments and entitlements",
        write: "does it touch a paid feature, a licence or a price",
        words: &["payments", "payment", "entitlements", "entitlement"],
    },
    Lens {
        id: "architecture",
        name: "Architecture",
        write: "which layer and module, new dependencies, the data model, migrations",
        words: &["architecture"],
    },
    Lens {
        id: "security",
        name: "Security",
        write: "untrusted input, secrets, permissions (the security baseline)",
        words: &["security"],
    },
    Lens {
        id: "privacy",
        name: "Privacy",
        write: "what user data it reads, keeps or sends (the privacy baseline)",
        words: &["privacy"],
    },
    Lens {
        id: "performance",
        name: "Performance",
        write: "the main thread, startup, sizes, caches (the performance baseline)",
        words: &["performance", "perf"],
    },
    Lens {
        id: "tests",
        name: "Tests",
        write: "which tests prove it: unit, render, end to end, by hand",
        words: &["tests", "test", "testing"],
    },
    Lens {
        id: "docs",
        name: "Docs",
        write: "which docs change: help, manual, AS_BUILT, PROTOCOL, DESIGN",
        words: &["docs", "doc", "documentation"],
    },
];

/// The line in a body that starts its Lenses block.
pub const BLOCK: &str = "**Lenses:**";
/// The line in docs/VISION.md that names the lenses the project needs.
pub const NEEDED: &str = "**Lenses needed:**";
/// A skip's reason, at most, as shown.
const MAX_WHY: usize = 300;
/// A task's line, at most, as the vision check quotes it.
const MAX_SAYS: usize = 300;
/// The most of README.md read for its front matter (as `key::stored_prefix` reads it).
const README_HEAD: u64 = 64 * 1024;
/// The most of config.toml and docs/VISION.md read.
const FILE_CAP: u64 = 1024 * 1024;

/// The lens with id `id`.
pub fn by_id(id: &str) -> Option<&'static Lens> {
    LENSES.iter().find(|l| l.id == id)
}

/// Does `text` name lens `id`: is one of its words one of the lens's?
pub fn names(text: &str, id: &str) -> bool {
    let Some(lens) = by_id(id) else {
        return false;
    };
    text.split(|c: char| !c.is_alphanumeric())
        .any(|w| lens.words.contains(&w.to_lowercase().as_str()))
}

/// A lens the project skips, and why.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub lens: &'static str,
    pub name: &'static str,
    pub why: String,
}

/// A project's skips that count, and what's wrong with the rest (for `muster check`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shaping {
    pub skipped: Vec<Skipped>,
    pub problems: Vec<String>,
}

/// Every lens but the skipped ones, in order.
pub fn active(skipped: &[Skipped]) -> Vec<Lens> {
    LENSES
        .iter()
        .filter(|l| !skipped.iter().any(|s| s.lens == l.id))
        .copied()
        .collect()
}

/// `text` on one line (control characters and runs of space as one space), cut to `max`
/// characters with "…".
fn short(text: &str, max: usize) -> String {
    let t: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > max {
        let cut: String = t.chars().take(max - 1).collect();
        format!("{cut}…")
    } else {
        t
    }
}

/// The first `cap` bytes of a file as text, or None when it can't be read.
fn read_head(path: &Path, cap: u64) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(cap)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

// ---- The project's skips ----

/// What a project wrote, before it's checked: the lenses it skips, as written, and each reason.
#[derive(Default)]
struct Raw {
    skip: Vec<String>,
    reasons: BTreeMap<String, String>,
    problems: Vec<String>,
}

/// The skips that count from `raw`: a known lens with a reason. `src` says where they're written
/// and `reason_at` how a lens's reason is, for the problems.
fn resolve(raw: Raw, src: &str, reason_at: &dyn Fn(&str) -> String) -> Shaping {
    let mut out = Shaping {
        skipped: Vec::new(),
        problems: raw.problems,
    };
    let ids: Vec<&str> = LENSES.iter().map(|l| l.id).collect();
    let mut named: Vec<String> = Vec::new();
    for s in &raw.skip {
        let id = s.trim().to_lowercase();
        if id.is_empty() || named.contains(&id) {
            continue;
        }
        named.push(id.clone());
        let Some(lens) = by_id(&id) else {
            out.problems.push(format!(
                "{src}: skips {:?}, which isn't a lens (the lenses: {})",
                short(s, 40),
                ids.join(", ")
            ));
            continue;
        };
        let why = raw
            .reasons
            .iter()
            .find(|(k, _)| k.trim().to_lowercase() == id)
            .map(|(_, v)| short(v, MAX_WHY))
            .unwrap_or_default();
        if why.is_empty() {
            out.problems.push(format!(
                "{src}: skips {id} with no reason, so it still applies; say why with {}",
                reason_at(&id)
            ));
            continue;
        }
        out.skipped.push(Skipped {
            lens: lens.id,
            name: lens.name,
            why,
        });
    }
    for k in raw.reasons.keys() {
        let id = k.trim().to_lowercase();
        if !named.contains(&id) {
            out.problems.push(format!(
                "{src}: has a reason for {:?}, which it doesn't skip",
                short(k, 40)
            ));
        }
    }
    // The block's order, whatever order they were written in.
    out.skipped
        .sort_by_key(|s| LENSES.iter().position(|l| l.id == s.lens));
    out
}

/// A TOML string at the start of `s`, basic (`"…"`, with its escapes) or literal (`'…'`), and
/// what's after it; None when it isn't one.
fn toml_string(s: &str) -> Option<(String, &str)> {
    let mut chars = s.char_indices();
    let (_, quote) = chars.next()?;
    if quote == '\'' {
        let end = s[1..].find('\'')? + 1;
        return Some((s[1..end].to_string(), &s[end + 1..]));
    }
    if quote != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 1..])),
            '\\' => {
                let (_, e) = chars.next()?;
                match e {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' | 'U' => {
                        let n = if e == 'u' { 4 } else { 8 };
                        let hex: String = (0..n)
                            .filter_map(|_| chars.next().map(|(_, h)| h))
                            .collect();
                        if hex.chars().count() != n {
                            return None;
                        }
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    _ => return None,
                }
            }
            c => out.push(c),
        }
    }
    None
}

/// A TOML line without its comment: from a `#` that isn't in a string.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return &line[..i],
            None => {}
        }
    }
    line
}

/// A TOML array of strings (`["a", 'b',]`), the whole of `s`; None when it isn't one.
fn toml_strings(s: &str) -> Option<Vec<String>> {
    let mut rest = s.trim().strip_prefix('[')?;
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix(']') {
            return after.trim().is_empty().then_some(out);
        }
        let (v, after) = toml_string(rest)?;
        out.push(v);
        rest = after.trim_start();
        if let Some(after) = rest.strip_prefix(',') {
            rest = after;
        } else if !rest.starts_with(']') {
            return None;
        }
    }
}

/// A `key = value` line's key (bare or quoted) and value; None without an `=`.
fn key_value(line: &str) -> Option<(String, &str)> {
    if line.starts_with('"') || line.starts_with('\'') {
        let (k, rest) = toml_string(line)?;
        return Some((k, rest.trim_start().strip_prefix('=')?.trim()));
    }
    let (k, v) = line.split_once('=')?;
    Some((k.trim().to_string(), v.trim()))
}

/// `config.toml`'s `[shaping]` table (`skip = […]`, one line or several) and
/// `[shaping.reasons]` (`<lens> = "<why>"`; `reasons.<lens> = "<why>"` in `[shaping]` too).
/// Every other table is someone else's.
fn raw_toml(text: &str) -> Raw {
    const SRC: &str = "config.toml [shaping]";
    let mut raw = Raw::default();
    let mut table = String::new();
    let mut open: Option<String> = None;
    let skip_value = |raw: &mut Raw, v: &str| match toml_strings(v) {
        Some(list) => raw.skip.extend(list),
        None => raw.problems.push(format!(
            "{SRC}: skip must be a list of lens names, like skip = [\"payments\"]"
        )),
    };
    for line in text.lines() {
        let line = strip_comment(line).trim();
        if let Some(buf) = open.as_mut() {
            buf.push(' ');
            buf.push_str(line);
            if line.contains(']') {
                let v = open.take().unwrap_or_default();
                skip_value(&mut raw, &v);
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if let Some(head) = line.strip_prefix('[') {
            table = head
                .strip_suffix(']')
                .map(|h| h.split('.').map(str::trim).collect::<Vec<_>>().join("."))
                .unwrap_or_default();
            continue;
        }
        let reason = |raw: &mut Raw, lens: &str, v: &str| match toml_string(v) {
            Some((why, rest)) if rest.trim().is_empty() => {
                raw.reasons.insert(lens.to_string(), why);
            }
            _ => raw.problems.push(format!(
                "{SRC}: the reason for {:?} must be a quoted string",
                short(lens, 40)
            )),
        };
        match table.as_str() {
            "shaping" => match key_value(line) {
                Some((k, v)) if k == "skip" => {
                    if v.starts_with('[') && !v.contains(']') {
                        open = Some(v.to_string());
                    } else {
                        skip_value(&mut raw, v);
                    }
                }
                Some((k, v)) if k.starts_with("reasons.") => {
                    let lens = k["reasons.".len()..].trim().trim_matches('"').to_string();
                    reason(&mut raw, &lens, v);
                }
                Some((k, _)) => raw.problems.push(format!(
                    "{SRC}: {:?} isn't a setting (skip = [\"<lens>\"] here, the reasons in [shaping.reasons])",
                    short(&k, 40)
                )),
                None => raw.problems.push(format!(
                    "{SRC}: {:?} isn't a key = value line",
                    short(line, 40)
                )),
            },
            "shaping.reasons" => match key_value(line) {
                Some((k, v)) => reason(&mut raw, &k, v),
                None => raw.problems.push(format!(
                    "{SRC}: {:?} isn't a <lens> = \"<why>\" line",
                    short(line, 40)
                )),
            },
            _ => {}
        }
    }
    if open.is_some() {
        raw.problems
            .push(format!("{SRC}: skip's list has no closing ]"));
    }
    raw
}

/// README.md's front matter: `shaping-skip: <lens>, <lens>` and `shaping-reason-<lens>: <why>`.
fn raw_readme(text: &str) -> Raw {
    let mut raw = Raw::default();
    let Some((fields, _)) = crate::queue::front_matter(text) else {
        return raw;
    };
    let mut keys: Vec<&String> = fields.keys().collect();
    keys.sort();
    for k in keys {
        let v = &fields[k];
        if k == "shaping-skip" {
            let list = v.trim().trim_start_matches('[').trim_end_matches(']');
            raw.skip.extend(
                list.split(',')
                    .map(|s| s.trim().trim_matches(['"', '\'']).to_string()),
            );
        } else if let Some(lens) = k.strip_prefix("shaping-reason-") {
            raw.reasons.insert(lens.to_string(), v.clone());
        } else if k.starts_with("shaping-") {
            raw.problems.push(format!(
                "README.md: {:?} isn't a setting (shaping-skip: <lens>, … and shaping-reason-<lens>: <why>)",
                short(k, 40)
            ));
        }
    }
    raw
}

/// The `[shaping]` skips in a `.muster` queue's `config.toml` text.
pub fn from_toml(text: &str) -> Shaping {
    resolve(raw_toml(text), "config.toml [shaping]", &|id| {
        format!("[shaping.reasons] {id} = \"<why>\"")
    })
}

/// The skips in an old queue's README.md text (its front matter).
pub fn from_readme(text: &str) -> Shaping {
    resolve(raw_readme(text), "README.md", &|id| {
        format!("shaping-reason-{id}: <why> in its front matter")
    })
}

/// The skips of the queue at `root`: a `.muster` queue's in its `config.toml`, an old queue's in
/// README.md's front matter. A file that isn't there skips nothing.
pub fn read(root: &Path) -> Shaping {
    if crate::layout::is_muster(root) {
        read_head(&root.join(crate::layout::CONFIG), FILE_CAP)
            .map(|t| from_toml(&t))
            .unwrap_or_default()
    } else {
        read_head(&root.join("README.md"), README_HEAD)
            .map(|t| from_readme(&t))
            .unwrap_or_default()
    }
}

/// What `muster check` says is wrong with the queue's skips, each line starting with its name.
pub fn problems(queue: &str, root: &Path) -> Vec<String> {
    read(root)
        .problems
        .into_iter()
        .map(|p| format!("{queue}: {p}"))
        .collect()
}

// ---- A task's Lenses block ----

/// The lenses a line's label names: every word in it (outside parentheses) a lens's word or
/// "and", and at least one of them; else none.
fn label_lenses(label: &str) -> Vec<&'static str> {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in label.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 => plain.push(c),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for w in plain
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let w = w.to_lowercase();
        if w == "and" {
            continue;
        }
        match LENSES.iter().find(|l| l.words.contains(&w.as_str())) {
            Some(l) if !out.contains(&l.id) => out.push(l.id),
            Some(_) => {}
            None => return Vec::new(),
        }
    }
    out
}

/// One labelled line in a task's Lenses block.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The ids of the lenses its label names.
    pub lenses: Vec<&'static str>,
    /// The line as written, label and all.
    pub text: String,
    /// It says "n/a".
    pub na: bool,
    /// It's still the template's `<…>`.
    pub unfilled: bool,
}

/// A Markdown heading: one to six `#` and a space (or nothing after them).
fn is_heading(line: &str) -> bool {
    let t = line.trim_start();
    let hashes = t.bytes().take_while(|&b| b == b'#').count();
    (1..=6).contains(&hashes) && t[hashes..].chars().next().is_none_or(char::is_whitespace)
}

/// The body's Lenses block: from its `**Lenses:**` line (what follows the label on it included)
/// to the next heading, or to a blank line that isn't followed by a list item or an indented line.
/// Fenced code is never read (`merge_log::Fences`): a `**Lenses:**` example in a fence isn't the
/// block, and a fence inside the block is left out of it.
fn block(body: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    let mut fences = crate::merge_log::Fences::default();
    let code: Vec<bool> = lines.iter().map(|l| fences.fenced(l.as_bytes())).collect();
    let start = (0..lines.len()).find(|&i| !code[i] && lines[i].trim_start().starts_with(BLOCK))?;
    let mut out = vec![lines[start].trim_start()[BLOCK.len()..].trim().to_string()];
    let mut i = start + 1;
    while i < lines.len() {
        let l = lines[i];
        if code[i] {
            i += 1;
            continue;
        }
        if is_heading(l) {
            break;
        }
        if l.trim().is_empty() {
            let next = lines[i + 1..].iter().find(|n| !n.trim().is_empty());
            let goes_on = next.is_some_and(|n| {
                let t = n.trim_start();
                n.starts_with([' ', '\t']) || t.starts_with("- ") || t.starts_with("* ")
            });
            if !goes_on {
                break;
            }
        }
        out.push(l.to_string());
        i += 1;
    }
    Some(out.join("\n"))
}

/// The labelled lines in the body's Lenses block: each list item, and each sentence (`. ` or
/// `; ` ends one) that starts with a label naming lenses and a colon, like `**Security:** …`,
/// `i18n/RTL: …` or `Security, privacy: n/a.`
pub fn lines(body: &str) -> Vec<Line> {
    let Some(block) = block(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for raw in block.lines() {
        // A sub-item (indented) says more about the item above it.
        if raw.starts_with([' ', '\t']) {
            continue;
        }
        let mut line = raw.trim();
        for marker in ["- ", "* ", "+ "] {
            if let Some(rest) = line.strip_prefix(marker) {
                line = rest.trim_start();
                break;
            }
        }
        for clause in line.split(". ").flat_map(|s| s.split("; ")) {
            let clause = clause.trim();
            let Some((label, value)) = clause.split_once(':') else {
                continue;
            };
            let lenses = label_lenses(label);
            if lenses.is_empty() {
                continue;
            }
            let value = value.trim_start_matches(['*', '_']).trim();
            let lower = value.to_lowercase();
            out.push(Line {
                lenses,
                text: clause.to_string(),
                na: lower.starts_with("n/a"),
                unfilled: value.starts_with('<') && value.ends_with('>'),
            });
        }
    }
    out
}

/// The `active` lenses the body has no line for (or only the template's `<…>`), by name.
pub fn missing(body: &str, active: &[Lens]) -> Vec<&'static str> {
    let lines = lines(body);
    active
        .iter()
        .filter(|l| {
            !lines
                .iter()
                .any(|x| !x.unfilled && x.lenses.contains(&l.id))
        })
        .map(|l| l.name)
        .collect()
}

/// The lenses the body says "n/a" for and nothing else, each with its line (one line, capped).
pub fn said_na(body: &str) -> Vec<(&'static str, String)> {
    let lines = lines(body);
    let answered: Vec<&str> = lines
        .iter()
        .filter(|l| !l.na && !l.unfilled)
        .flat_map(|l| l.lenses.iter().copied())
        .collect();
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for l in lines.iter().filter(|l| l.na) {
        for id in &l.lenses {
            if !answered.contains(id) && !out.iter().any(|(x, _)| x == id) {
                out.push((id, short(&l.text, MAX_SAYS)));
            }
        }
    }
    out
}

/// The lenses a VISION's first `**Lenses needed:**` line names, as a list (`,` or `;` between
/// them; an item that isn't a lens's name or words is passed over), with the line's number (from
/// 1) and its text; None without one.
pub fn needed(vision: &str) -> Option<(usize, String, Vec<&'static str>)> {
    let (n, line) = vision
        .lines()
        .enumerate()
        .find(|(_, l)| l.trim_start().starts_with(NEEDED))?;
    let rest = &line.trim_start()[NEEDED.len()..];
    let mut ids = Vec::new();
    for id in rest.split([',', ';']).flat_map(label_lenses) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Some((n + 1, short(line, MAX_SAYS), ids))
}

/// The project's `docs/VISION.md` with a `**Lenses needed:**` line: the first of the queue's
/// project places (`backlog::project_places`) that has one, as (its path relative to the place,
/// the line's number, the line, the lenses).
pub fn vision_needs(root: &Path) -> Option<(String, usize, String, Vec<&'static str>)> {
    crate::backlog::project_places(root)
        .into_iter()
        .find_map(|place| {
            let text = read_head(&place.join("docs/VISION.md"), FILE_CAP)?;
            let (n, line, ids) = needed(&text)?;
            Some(("docs/VISION.md".to_string(), n, line, ids))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ids(s: &Shaping) -> Vec<(&str, &str)> {
        s.skipped.iter().map(|x| (x.lens, x.why.as_str())).collect()
    }

    #[test]
    fn every_lens_has_an_id_a_name_and_what_to_write() {
        let all: Vec<&str> = LENSES.iter().map(|l| l.id).collect();
        assert_eq!(
            all,
            [
                "accessibility",
                "i18n",
                "themes",
                "payments",
                "architecture",
                "security",
                "privacy",
                "performance",
                "tests",
                "docs"
            ]
        );
        for l in &LENSES {
            assert!(!l.name.is_empty() && !l.write.is_empty(), "{}", l.id);
            assert!(l.words.contains(&l.id), "{} is one of its own words", l.id);
            // A name, read back as a label, is its own lens and no other.
            assert_eq!(label_lenses(l.name), [l.id], "{}", l.name);
        }
        // No word is two lenses'.
        let mut words: Vec<&str> = LENSES
            .iter()
            .flat_map(|l| l.words.iter().copied())
            .collect();
        let n = words.len();
        words.sort();
        words.dedup();
        assert_eq!(words.len(), n);
    }

    #[test]
    fn config_toml_skips_a_lens_with_its_reason() {
        let text = "local = true\nprefix = \"mu\"\n\n[shaping]\nskip = [\"payments\", 'Themes'] # two\n\n[shaping.reasons]\npayments = \"Muster has no paid features # none\"\n\"themes\" = 'one look'\n\n[other]\nskip = 3\n";
        let s = from_toml(text);
        assert_eq!(s.problems, Vec::<String>::new());
        assert_eq!(
            ids(&s),
            [
                ("themes", "one look"),
                ("payments", "Muster has no paid features # none")
            ]
        );
        assert_eq!(s.skipped[1].name, "Payments and entitlements");
        let names: Vec<&str> = active(&s.skipped).iter().map(|l| l.id).collect();
        assert_eq!(names.len(), 8);
        assert!(!names.contains(&"payments") && !names.contains(&"themes"));
        // A list over several lines, a reason with escapes, and reasons.<lens> in [shaping].
        let text = "[shaping]\nskip = [\n  \"payments\",  # paid\n  \"docs\",\n]\nreasons.payments = \"no \\\"paid\\\" \\u00e9dition\"\n[shaping.reasons]\ndocs = \"the README is\\nthe doc\"\n";
        let s = from_toml(text);
        assert_eq!(s.problems, Vec::<String>::new());
        assert_eq!(
            ids(&s),
            [
                ("payments", "no \"paid\" édition"),
                ("docs", "the README is the doc")
            ]
        );
        // No [shaping]: every lens applies.
        let s = from_toml("local = true\n");
        assert_eq!(s, Shaping::default());
        assert_eq!(active(&s.skipped).len(), LENSES.len());
    }

    #[test]
    fn bad_skips_are_problems_and_the_lens_still_applies() {
        let text = "[shaping]\nskip = [\"paymnts\", \"payments\", \"themes\", \"themes\", \"docs\"]\nskips = [\"x\"]\n[shaping.reasons]\npayments = \"   \"\ndocs = 3\nprivacy = \"why\"\n";
        let s = from_toml(text);
        assert_eq!(s.skipped, Vec::<Skipped>::new(), "{:?}", s.problems);
        let all = s.problems.join("\n");
        for want in [
            "config.toml [shaping]: \"skips\" isn't a setting",
            "config.toml [shaping]: the reason for \"docs\" must be a quoted string",
            "config.toml [shaping]: skips \"paymnts\", which isn't a lens (the lenses: accessibility, i18n, themes, payments, architecture, security, privacy, performance, tests, docs)",
            "config.toml [shaping]: skips payments with no reason, so it still applies; say why with [shaping.reasons] payments = \"<why>\"",
            "config.toml [shaping]: skips themes with no reason",
            "config.toml [shaping]: skips docs with no reason",
            "config.toml [shaping]: has a reason for \"privacy\", which it doesn't skip",
        ] {
            assert!(all.contains(want), "{want}\n---\n{all}");
        }
        assert_eq!(all.matches("skips themes").count(), 1, "named once: {all}");
        assert_eq!(active(&s.skipped).len(), LENSES.len());
        for (text, want) in [
            (
                "[shaping]\nskip = \"payments\"\n",
                "skip must be a list of lens names, like skip = [\"payments\"]",
            ),
            (
                "[shaping]\nskip = [\"payments\" \"docs\"]\n",
                "skip must be a list",
            ),
            (
                "[shaping]\nskip = [\"payments\",\n\"docs\"\n",
                "skip's list has no closing ]",
            ),
            ("[shaping]\nskip = [\"a\\q\"]\n", "skip must be a list"),
            (
                "[shaping]\njust words\n",
                "\"just words\" isn't a key = value line",
            ),
            (
                "[shaping.reasons]\npayments = \"open\n",
                "the reason for \"payments\" must be a quoted string",
            ),
            (
                "[shaping.reasons]\npayments = \"x\" y\n",
                "the reason for \"payments\" must be a quoted string",
            ),
        ] {
            let p = from_toml(text).problems.join("\n");
            assert!(p.contains(want), "{text:?}: {p}");
        }
    }

    #[test]
    fn an_old_queues_readme_front_matter_skips_a_lens_with_its_reason() {
        let text = "---\nprefix: dr\nshaping-skip: payments, Themes,\nshaping-reason-payments: Drover is free  # for now\nshaping-reason-themes: one look\n---\n\n# Drover\n\nshaping-skip: docs\n";
        let s = from_readme(text);
        assert_eq!(s.problems, Vec::<String>::new());
        assert_eq!(
            ids(&s),
            [("themes", "one look"), ("payments", "Drover is free")]
        );
        let s = from_readme("---\nshaping-skip: [paid, docs]\nshaping-reasons-docs: x\n---\n");
        let all = s.problems.join("\n");
        for want in [
            "README.md: skips \"paid\", which isn't a lens",
            "README.md: skips docs with no reason, so it still applies; say why with shaping-reason-docs: <why> in its front matter",
            "README.md: \"shaping-reasons-docs\" isn't a setting",
        ] {
            assert!(all.contains(want), "{want}\n---\n{all}");
        }
        assert!(s.skipped.is_empty());
        assert_eq!(
            from_readme("# no front matter\nshaping-skip: docs\n"),
            Shaping::default()
        );
    }

    #[test]
    fn a_queue_reads_its_own_layouts_skips_and_check_names_the_queue() {
        let d = crate::testkit::tempdir().unwrap();
        // A .muster queue: config.toml, never a README there.
        let m = d.path().join("repo/.muster");
        fs::create_dir_all(&m).unwrap();
        fs::write(
            m.join("config.toml"),
            "local = true\n[shaping]\nskip = [\"payments\", \"nope\"]\n[shaping.reasons]\npayments = \"free\"\n",
        )
        .unwrap();
        fs::write(m.join("README.md"), "---\nshaping-skip: docs\n---\n").unwrap();
        let s = read(&m);
        assert_eq!(ids(&s), [("payments", "free")]);
        assert_eq!(
            problems("mu", &m),
            ["mu: config.toml [shaping]: skips \"nope\", which isn't a lens (the lenses: accessibility, i18n, themes, payments, architecture, security, privacy, performance, tests, docs)"]
        );
        // An old queue: README.md's front matter, never a config.toml there.
        let old = d.path().join("proj/tasks");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("config.toml"), "[shaping]\nskip = [\"x\"]\n").unwrap();
        fs::write(
            old.join("README.md"),
            "---\nprefix: dr\nshaping-skip: docs\n---\n",
        )
        .unwrap();
        assert!(read(&old).skipped.is_empty());
        assert_eq!(problems("drover", &old).len(), 1);
        assert!(
            problems("drover", &old)[0].starts_with("drover: README.md: skips docs with no reason")
        );
        // Nothing to read: nothing skipped, nothing wrong.
        let bare = d.path().join("bare/tasks");
        fs::create_dir_all(&bare).unwrap();
        assert_eq!(read(&bare), Shaping::default());
    }

    #[test]
    fn the_queues_problems_carry_a_bad_skip_in_either_layout() {
        let d = crate::testkit::tempdir().unwrap();
        let old = d.path().join("proj/tasks");
        for f in crate::queue::STATUS_FOLDERS {
            fs::create_dir_all(old.join(f)).unwrap();
        }
        fs::write(old.join("README.md"), "---\nshaping-skip: paid\n---\n").unwrap();
        let m = d.path().join("repo/.muster");
        fs::create_dir_all(m.join("tasks")).unwrap();
        fs::write(
            m.join("config.toml"),
            "local = false\n[shaping]\nskip = [\"docs\"]\n",
        )
        .unwrap();
        let s = crate::queue::read_state_at(
            &[("drover".into(), old), ("mu".into(), m)],
            &mut crate::queue::Cache::default(),
            "2026-10-14 06:00",
        );
        let bad: Vec<&String> = s.problems.iter().filter(|p| p.contains("skips")).collect();
        assert_eq!(bad.len(), 2, "{:?}", s.problems);
        assert!(bad[0].starts_with("drover: README.md: skips \"paid\", which isn't a lens"));
        assert!(bad[1].starts_with("mu: config.toml [shaping]: skips docs with no reason"));
        // Problems, not warnings: `muster check` fails on them.
        assert!(bad.iter().all(|p| !crate::queue::is_warning(p)));
    }

    #[test]
    fn a_long_or_multi_line_reason_is_shown_on_one_line_and_capped() {
        let long = "w".repeat(400);
        let s = from_toml(&format!(
            "[shaping]\nskip = [\"docs\"]\n[shaping.reasons]\ndocs = \"a\\tb\\u001b[2J {long}\"\n"
        ));
        let why = &s.skipped[0].why;
        assert!(why.starts_with("a b [2J ww"), "{why}");
        assert_eq!(why.chars().count(), MAX_WHY);
        assert!(why.ends_with('…'));
    }

    /// The Lenses block as real tasks write it: one item each, with sub-items (mu-78's shape).
    const LISTED: &str = "**Done looks like:** x\n\n**Lenses:**\n- **Accessibility:**\n  - The block is a labelled group.\n  - Tests: not a lens line, it's under Accessibility\n- **i18n and RTL:** summaries can hold Persian ('پیام بعدی').\n- **Themes:** existing tokens only.\n- **Payments and entitlements:** n/a. Muster has none.\n- **Architecture:**\n  - Core: `queue.rs` reads it.\n  - CLI: `pass --summary`.\n- **Security:** stored, never run.\n- **Privacy:** N/A, nothing leaves the Mac.\n- **Performance:** one small map.\n\n## Acceptance criteria\n- [ ] AC1: Docs: x — verify: command\n";

    /// One paragraph (mu-83's shape).
    const INLINE: &str = "**Lenses:** accessibility: strips are buttons; the stage colour isn't the only cue. i18n/RTL: labels mirror in RTL. Themes: every theme, light and dark. Payments: n/a. Architecture: CSS only. Security, privacy: n/a. Performance: no layout thrash. Tests: render tests. Docs: DESIGN.md (board), manual 04.\n\n## Acceptance criteria\n";

    #[test]
    fn lines_are_found_in_a_listed_block_and_in_a_paragraph() {
        let all = LENSES.to_vec();
        // The listed block has no Tests or Docs line: an AC or a sub-item doesn't count.
        assert_eq!(missing(LISTED, &all), ["Tests", "Docs"]);
        assert_eq!(missing(INLINE, &all), Vec::<&str>::new());
        assert_eq!(
            said_na(LISTED),
            [
                ("payments", "**Payments and entitlements:** n/a".to_string()),
                (
                    "privacy",
                    "**Privacy:** N/A, nothing leaves the Mac.".to_string()
                ),
            ]
        );
        assert_eq!(
            said_na(INLINE),
            [
                ("payments", "Payments: n/a".to_string()),
                ("security", "Security, privacy: n/a".to_string()),
                ("privacy", "Security, privacy: n/a".to_string()),
            ]
        );
        // A lens answered on a line of its own isn't n/a, whatever a shared line says.
        let both = "**Lenses:** Security, privacy: n/a. Security: the input is checked.\n";
        assert_eq!(
            said_na(both),
            [("privacy", "Security, privacy: n/a".to_string())]
        );
        // Only the active lenses are asked for.
        let without: Vec<Lens> = all
            .iter()
            .filter(|l| l.id != "docs" && l.id != "tests")
            .copied()
            .collect();
        assert_eq!(missing(LISTED, &without), Vec::<&str>::new());
    }

    #[test]
    fn a_lenses_example_in_a_code_fence_is_not_the_block() {
        let all = LENSES.to_vec();
        let only_security: Vec<Lens> = all.iter().filter(|l| l.id == "security").copied().collect();
        // The format shown in a fence before the real block: the real one is read.
        let body = "Write it like this:\n\n```\n**Lenses:**\n- **Accessibility:** n/a\n```\n\n**Lenses:** Security: the input is checked.\n";
        assert_eq!(missing(body, &only_security), Vec::<&str>::new());
        assert_eq!(
            said_na(body),
            Vec::<(&str, String)>::new(),
            "the example's n/a isn't the task's"
        );
        // A ~~~ fence, and one that a shorter fence doesn't close.
        let tilde = "~~~~md\n**Lenses:** Security: n/a\n~~~\nstill code\n~~~~\n\n**Lenses:** Security: checked.\n";
        assert_eq!(said_na(tilde), Vec::<(&str, String)>::new());
        assert_eq!(missing(tilde, &only_security), Vec::<&str>::new());
        // Only a fenced example: no block at all, so every lens is missing.
        let names: Vec<&str> = LENSES.iter().map(|l| l.name).collect();
        assert_eq!(missing("```\n**Lenses:** Security: x\n```\n", &all), names);
        // A fence inside the block is left out of it, and a heading quoted there doesn't end it.
        let inside = "**Lenses:**\n- **Security:** checked, as in:\n```\n## not a heading\n- **Privacy:** n/a\n```\n- **Themes:** every theme\n";
        let m = missing(inside, &all);
        assert!(!m.contains(&"Security") && !m.contains(&"Themes") && m.contains(&"Privacy"));
        assert_eq!(said_na(inside), Vec::<(&str, String)>::new());
    }

    #[test]
    fn what_is_not_a_lens_line_does_not_count() {
        let all = LENSES.to_vec();
        // No block: every lens is missing.
        let names: Vec<&str> = LENSES.iter().map(|l| l.name).collect();
        assert_eq!(missing("", &all), names);
        assert_eq!(
            missing("Security: fine, but no Lenses block\n", &all),
            names
        );
        // A label with other words, a quoted block, the template's placeholders.
        let body = "> **Lenses:** Security: quoted\n\n**Lenses:**\n- **The security model:** x\n- **Themes:** <light and dark, the project's tokens>\n- **Tests (unit, render):** the parser's\n- Docs and help: x\n\n**Depends on:** Privacy: not in the block\n";
        let m = missing(body, &all);
        assert!(
            m.contains(&"Security")
                && m.contains(&"Themes")
                && m.contains(&"Privacy")
                && m.contains(&"Docs"),
            "{m:?}"
        );
        assert!(
            !m.contains(&"Tests"),
            "a parenthesis in the label is fine: {m:?}"
        );
        // A heading ends the block; a line that only starts with # doesn't.
        let body = "**Lenses:** Security: x.\n#3 is the issue. Privacy: y.\n## Notes\nThemes: z\n";
        let m = missing(body, &all);
        assert!(
            !m.contains(&"Security") && !m.contains(&"Privacy") && m.contains(&"Themes"),
            "{m:?}"
        );
        // The block goes on past a blank line only into a list.
        let body = "**Lenses:**\n- **Security:** x\n\n- **Privacy:** y\n\nThemes: z\n";
        let m = missing(body, &all);
        assert!(
            !m.contains(&"Security") && !m.contains(&"Privacy") && m.contains(&"Themes"),
            "{m:?}"
        );
        // Unicode in the values and a label in another script are read as they are.
        let body = "**Lenses:** Security: پیام ✓. دسترسی: x\n";
        let m = missing(body, &all);
        assert!(
            !m.contains(&"Security") && m.contains(&"Accessibility"),
            "{m:?}"
        );
    }

    #[test]
    fn a_vision_names_the_lenses_it_needs_on_one_line() {
        let v = "# Vision\n\nIntro.\n\n**Lenses needed:** accessibility, i18n and RTL, Payments.; not themes words like colour, a11y\n\n**Lenses needed:** docs\n";
        assert_eq!(
            needed(v),
            Some((
                5,
                "**Lenses needed:** accessibility, i18n and RTL, Payments.; not themes words like colour, a11y".to_string(),
                vec!["accessibility", "i18n", "payments"]
            ))
        );
        assert_eq!(needed("# Vision\nLenses needed: docs\n"), None);
        let d = crate::testkit::tempdir().unwrap();
        let root = d.path().join("proj/tasks");
        fs::create_dir_all(&root).unwrap();
        assert_eq!(vision_needs(&root), None);
        let repo = d.path().join("proj/main");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("docs")).unwrap();
        fs::write(
            repo.join("docs/VISION.md"),
            "x\n**Lenses needed:** privacy\n",
        )
        .unwrap();
        let (at, n, _, ids) = vision_needs(&root).unwrap();
        assert_eq!(
            (at.as_str(), n, ids),
            ("docs/VISION.md", 2, vec!["privacy"])
        );
    }
}
