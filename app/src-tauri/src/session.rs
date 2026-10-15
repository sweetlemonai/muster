//! Who a session is, as `muster` stamps it next to `--as` (task 0030), so an agent never types it:
//! `wt-0019 (main-29 · session 7b5e8856 · cc-hg)`. The stamp holds only the Claude Code session's
//! short id, its name when the harness gives one, the checkout's folder name and the account label.
//! Nothing else: no paths, no user names, no tokens (the privacy baseline).
//!
//! A holder is its `--as` label plus, when it has one, its session. Two holders are the same when
//! their labels match and their sessions don't differ, so a claim made before stamps (or by the
//! owner in a terminal) keeps working, while two sessions under one label stay apart.
//!
//! A teammate (a subagent) shares its lead's session id, so its stamp also carries its agent id
//! (task 0090): `asghar (main · session 7b5e8856 · agent a1b2c3d4 · cc-mg)`. A stamp without an
//! agent part is the lead's, so a lead and each of its teammates are different holders.
//!
//! A claim made for a role's slot (task 0112) carries the slot's role id and number after the
//! session and agent: `Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)`. Views show the
//! role's name from `roles.toml` at read time, so renaming Kaveh renames every stamp that has one.

use std::path::Path;

/// Caps per part of a stamp. The session id is always 8 characters.
pub const MAX_NAME: usize = 32;
pub const MAX_CHECKOUT: usize = 40;
pub const MAX_ACCOUNT: usize = 12;

/// The session a command runs in, every part validated and capped.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Session {
    /// The first 8 characters of `CLAUDE_CODE_SESSION_ID`, lowercase hex.
    pub id: String,
    /// `CLAUDE_CODE_SESSION_NAME` (set by the harness for some sessions), or "".
    pub name: String,
    /// The folder name of the git checkout (or worktree) the command runs in, or "".
    pub checkout: String,
    /// The account label from `CLAUDE_CONFIG_DIR` (`~/.claude-hg` → `hg`), or "".
    pub account: String,
    /// A teammate's agent id, the first 8 characters of `MUSTER_AGENT` (`--agent`), lowercase hex;
    /// None for the lead.
    pub agent: Option<String>,
}

/// A stamp read back from a holder: the name and checkout (in that order, either may be missing),
/// the session id and the account label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stamp {
    pub parts: Vec<String>,
    pub id: String,
    /// The agent id, "" for a lead's stamp.
    pub agent: String,
    /// The role slot (0112), `coder-2`; "" when the claim wasn't made for one.
    pub slot: String,
    pub account: String,
}

impl Stamp {
    /// The holder's mailbox under `mail/` (contract rule 4): `<session8>` for a lead,
    /// `<session8>/<agent8>` for a teammate.
    pub fn mailbox(&self) -> String {
        if self.agent.is_empty() {
            self.id.clone()
        } else {
            format!("{}/{}", self.id, self.agent)
        }
    }
}

impl Session {
    /// The session `get` (an environment) and `cwd` describe, or None outside a session: no
    /// `CLAUDE_CODE_SESSION_ID`, or one that isn't a session id. Then the caller is the owner, in
    /// the app or a terminal, and nothing is stamped.
    pub fn from_env(get: &dyn Fn(&str) -> Option<String>, cwd: Option<&Path>) -> Option<Session> {
        let id = short_id(&get("CLAUDE_CODE_SESSION_ID")?)?;
        let home = get("HOME").filter(|h| !h.is_empty());
        // The login name, only to keep it out: any part that contains it is dropped (privacy).
        let login = home_login(home.as_deref());
        let keep = |part: String| {
            if names_login(&part, &login) {
                String::new()
            } else {
                part
            }
        };
        // A session name with a path in it is dropped whole: squashed, it would still spell one.
        let name = get("CLAUDE_CODE_SESSION_NAME")
            .filter(|n| !n.contains(['/', '\\']))
            .map(|n| keep(token(&n, MAX_NAME)))
            .unwrap_or_default();
        let checkout = keep(
            cwd.and_then(|c| checkout(c, home.as_deref().map(Path::new)))
                .unwrap_or_default(),
        );
        let account = keep(
            get("CLAUDE_CONFIG_DIR")
                .and_then(|d| account(&d))
                .unwrap_or_default(),
        );
        let agent = get("MUSTER_AGENT").and_then(|a| agent_id(&a));
        Some(Session {
            id,
            name,
            checkout,
            account,
            agent,
        })
    }
}

/// The home folder's name as a lowercase token (the login name), "" when unknown.
fn home_login(home: Option<&str>) -> String {
    home.and_then(|h| {
        Path::new(h)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
    })
    .map(|n| token(&n, MAX_CHECKOUT).to_ascii_lowercase())
    .unwrap_or_default()
}

/// Does `part` contain the login name? Only for logins of 3+ characters, so a short login doesn't
/// wipe every part.
fn names_login(part: &str, login: &str) -> bool {
    login.len() >= 3 && part.to_ascii_lowercase().contains(login)
}

/// A session id's short form: its first 8 characters, when the whole id is lowercase hex and
/// dashes (a UUID) of a sane length and those 8 are hex.
pub fn short_id(raw: &str) -> Option<String> {
    let s = raw.trim().to_ascii_lowercase();
    let ok = (8..=64).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        && s.bytes().take(8).all(|b| b.is_ascii_hexdigit());
    ok.then(|| s[..8].to_string())
}

/// An agent id's short form (0090): its first 8 characters lowercased, when the whole id is
/// letters, digits and dashes of a sane length and those 8 are hex. None for anything else.
pub fn agent_id(raw: &str) -> Option<String> {
    let s = raw.trim().to_ascii_lowercase();
    let ok = (8..=64).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && s.bytes().take(8).all(|b| b.is_ascii_hexdigit());
    ok.then(|| s[..8].to_string())
}

/// Is `s` 8 lowercase hex characters, as a stamp writes a session or agent id?
pub fn is_hex8(s: &str) -> bool {
    s.len() == 8
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A name as one stamp part: letters, digits, `.`, `_` and `-` kept, spaces made `-`, anything
/// else dropped, trimmed of `-` and `.`, capped at `max` characters. "" when nothing is left.
pub fn token(raw: &str, max: usize) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_') {
            out.push(c);
        } else if (c == '-' || c.is_whitespace()) && !out.ends_with('-') {
            out.push('-');
        }
    }
    let t: String = out.trim_matches(['-', '.']).chars().take(max).collect();
    t.trim_end_matches(['-', '.']).to_string()
}

fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_CHECKOUT.max(MAX_NAME)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The checkout `cwd` is in: the nearest folder up from it with a `.git` (a repo or a worktree).
/// Only its name is kept. None outside one, at or above the home folder, in a task queue (its own
/// git, not a code checkout), or when it's named like the home folder (the login name). `USER`
/// is never read.
pub(crate) fn checkout(cwd: &Path, home: Option<&Path>) -> Option<String> {
    let login = home
        .and_then(Path::file_name)
        .map(|n| token(&n.to_string_lossy(), MAX_CHECKOUT))
        .unwrap_or_default();
    for dir in cwd.ancestors() {
        if home.is_some_and(|h| dir == h) {
            return None;
        }
        if dir.join(".git").exists() {
            if crate::projects::looks_like_queue(dir) {
                return None;
            }
            let name = token(&dir.file_name()?.to_string_lossy(), MAX_CHECKOUT);
            let is_login = !login.is_empty() && name.eq_ignore_ascii_case(&login);
            return (!name.is_empty() && !is_login).then_some(name);
        }
    }
    None
}

/// The account label of a Claude config folder: `.claude-hg` or `cc-hg` → `hg`; the default
/// `.claude` has none. Only 1 to 12 lowercase letters and digits.
fn account(dir: &str) -> Option<String> {
    let last = Path::new(dir.trim())
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    let name = last.trim_start_matches('.');
    let label = name
        .strip_prefix("claude-")
        .or_else(|| name.strip_prefix("cc-"))?;
    let ok = (1..=MAX_ACCOUNT).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    ok.then(|| label.to_string())
}

/// The words of a label, for leaving out a stamp part the label already says (`--as wt-0030` in
/// the checkout `wt-0030`).
fn says(label: &str, part: &str) -> bool {
    label
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .any(|w| w == part)
}

/// `label` as the session acts: the label (any stamp typed into it replaced) with the session's
/// stamp after it. Without a session, the label as is.
pub fn stamped(label: &str, session: Option<&Session>) -> String {
    stamped_slot(label, session, None)
}

/// `stamped`, with the role slot the label acts as (0112, `coder-2`) after the session and agent.
pub fn stamped_slot(label: &str, session: Option<&Session>, slot: Option<&str>) -> String {
    // No session, or a blank label (refused by actor, never made valid by a stamp): as is.
    let Some(s) = session.filter(|_| !label.trim().is_empty()) else {
        return label.to_string();
    };
    let (label, _) = split(label.trim());
    let mut parts: Vec<String> = Vec::new();
    for p in [&s.name, &s.checkout] {
        if !p.is_empty() && !says(label, p) && !parts.contains(p) {
            parts.push(p.clone());
        }
    }
    parts.push(format!("session {}", s.id));
    if let Some(agent) = &s.agent {
        parts.push(format!("agent {agent}"));
    }
    if let Some(slot) = slot.filter(|x| crate::roles::Slot::parse(x).is_some()) {
        parts.push(format!("slot {slot}"));
    }
    if !s.account.is_empty() {
        parts.push(format!("cc-{}", s.account));
    }
    format!("{label} ({})", parts.join(" · "))
}

/// A holder as (label, stamp): the stamp is a last ` (… · session <8 hex> …)` group in exactly
/// the shape `stamped` writes, an `agent <8 hex>` part right after the session when it has one,
/// then a `slot <role>-<n>` part when it has one (0112). Anything else (a hand-typed `(drover/main, session 349bc9c3)`) is part of the label.
pub fn split(who: &str) -> (&str, Option<Stamp>) {
    let parsed = || -> Option<(usize, Stamp)> {
        let at = who.rfind(" (")?;
        let inner = who[at + 2..].strip_suffix(')')?;
        let mut stamp = Stamp::default();
        // Only the part right after the session may be the agent.
        let mut after_session = false;
        for part in inner.split(" · ") {
            let next_to_session = std::mem::take(&mut after_session);
            if let Some(id) = part.strip_prefix("session ") {
                if !stamp.id.is_empty() || !is_hex8(id) {
                    return None;
                }
                stamp.id = id.to_string();
                after_session = true;
            } else if let Some(agent) = part.strip_prefix("agent ").filter(|_| next_to_session) {
                if !is_hex8(agent) {
                    return None;
                }
                stamp.agent = agent.to_string();
            } else if let Some(slot) = part.strip_prefix("slot ").filter(|_| !stamp.id.is_empty()) {
                // Once, after the session (and agent), before the account.
                if !stamp.slot.is_empty()
                    || !stamp.account.is_empty()
                    || crate::roles::Slot::parse(slot).is_none()
                {
                    return None;
                }
                stamp.slot = slot.to_string();
            } else if stamp.id.is_empty() {
                if !is_token(part) || stamp.parts.len() == 2 {
                    return None;
                }
                stamp.parts.push(part.to_string());
            } else {
                let label = part.strip_prefix("cc-")?;
                let ok = stamp.account.is_empty()
                    && (1..=MAX_ACCOUNT).contains(&label.len())
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
                if !ok {
                    return None;
                }
                stamp.account = label.to_string();
            }
        }
        (!stamp.id.is_empty() && at > 0).then_some((at, stamp))
    };
    match parsed() {
        Some((at, stamp)) => (&who[..at], Some(stamp)),
        None => (who, None),
    }
}

/// Is `a` the same holder as `b`? The same text; or the same label, with sessions that don't
/// differ (one without a session matches any, as every claim did before stamps) and the same
/// agent (none on both is the lead; a teammate is never its lead, nor another teammate). Two
/// stamps of one role slot (0112) in one session are one holder whatever their labels say, so a
/// slot acting as `coder-2` after a claim as `Kaveh-2`, or after a rename, is still itself.
pub fn same_holder(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let ((la, sa), (lb, sb)) = (split(a), split(b));
    let one_slot =
        matches!((&sa, &sb), (Some(x), Some(y)) if !x.slot.is_empty() && x.slot == y.slot);
    (la == lb || one_slot)
        && match (sa, sb) {
            (Some(x), Some(y)) => x.id == y.id && x.agent == y.agent,
            _ => true,
        }
}

/// One session (and agent) is one role slot (0112): two stamps of the same session and agent
/// where either acts as a slot are one holder for one claim per holder, whatever their labels
/// say (`Kaveh-1 (… slot coder-1)` and `Kaveh-2 (… slot coder-2)`, or a plain `akbar (…)`).
pub fn one_session_slot(a: &str, b: &str) -> bool {
    match (split(a).1, split(b).1) {
        (Some(x), Some(y)) => {
            x.id == y.id && x.agent == y.agent && (!x.slot.is_empty() || !y.slot.is_empty())
        }
        _ => false,
    }
}

/// The role slot a holder's stamp acts as (0112, `coder-2`); empty without one.
pub fn slot_of(who: &str) -> String {
    split(who).1.map(|s| s.slot).unwrap_or_default()
}

/// The session and agent of a holder's stamp, for telling two holders' sessions apart whatever
/// their labels; None without a stamp.
pub fn session_of(who: &str) -> Option<(String, String)> {
    split(who).1.map(|s| (s.id, s.agent))
}

/// `who` with the role slot of `from`'s stamp (0112), for a holder that's rewritten: the pass
/// into Testing from the claim, a takeover from the old claim. Carried when `who` is a stamp
/// without a slot and is `from`'s holder or its successor under the same label, so a slot acting
/// as its old name after a rename (`Kaveh-2`, now کاوه) still holds it. Anything else as is.
pub fn keep_slot(from: &str, who: &str) -> String {
    let (from_label, from_stamp) = split(from);
    let (label, stamp) = split(who);
    match (from_stamp, stamp) {
        (Some(f), Some(mut w))
            if !f.slot.is_empty()
                && w.slot.is_empty()
                && (label == from_label || same_holder(from, who)) =>
        {
            w.slot = f.slot;
            written(label, &w)
        }
        _ => who.to_string(),
    }
}

/// `who` without its role slot (0112): the same label and stamp, its session and account kept, for
/// a holder coming back to a task after its slot went to another session.
pub fn without_slot(who: &str) -> String {
    match split(who) {
        (label, Some(mut s)) if !s.slot.is_empty() => {
            s.slot.clear();
            written(label, &s)
        }
        _ => who.to_string(),
    }
}

/// The claim `claim` as it reads once `who`, its holder, acts on it (mu-184), or None when it
/// reads right already. A claim that names no session (a bare label: made outside Claude Code, or
/// given back by label before mu-184) takes `who`'s stamp, without a slot, as a takeover of it
/// would. A claim of `who`'s own session and agent stamped under another account (the session
/// moved from cc-mg to cc-hg, say) takes `who`'s account. Only for a stamped `who` that isn't the
/// owner and holds the claim (`same_holder`).
pub fn restamped(claim: &str, who: &str) -> Option<String> {
    if claim.trim().is_empty() || is_owner(who) || !same_holder(claim, who) {
        return None;
    }
    let mine = split(who).1?;
    match split(claim) {
        (_, None) => Some(without_slot(who)),
        (label, Some(mut old))
            if old.id == mine.id
                && old.agent == mine.agent
                && !mine.account.is_empty()
                && old.account != mine.account =>
        {
            old.account = mine.account;
            Some(written(label, &old))
        }
        _ => None,
    }
}

/// A label and a stamp as `stamped` writes them.
fn written(label: &str, s: &Stamp) -> String {
    let mut parts = s.parts.clone();
    parts.push(format!("session {}", s.id));
    if !s.agent.is_empty() {
        parts.push(format!("agent {}", s.agent));
    }
    if !s.slot.is_empty() {
        parts.push(format!("slot {}", s.slot));
    }
    if !s.account.is_empty() {
        parts.push(format!("cc-{}", s.account));
    }
    format!("{label} ({})", parts.join(" · "))
}

/// Is this holder the owner (`owner`, with or without a session)?
pub fn is_owner(who: &str) -> bool {
    split(who).0 == "owner"
}

#[cfg(test)]
mod tests {
    #[test]
    fn one_session_acting_as_a_slot_is_one_holder() {
        use super::{one_session_slot, slot_of};
        let one = "Kaveh-1 (main · session 7b5e8856 · slot coder-1 · cc-hg)";
        let two = "Kaveh-2 (wt-0112 · session 7b5e8856 · slot coder-2 · cc-hg)";
        let plain = "akbar (main · session 7b5e8856 · cc-hg)";
        assert!(one_session_slot(one, two));
        assert!(one_session_slot(one, plain) && one_session_slot(plain, one));
        // Neither a slot: labels still tell holders apart, as before slots.
        assert!(!one_session_slot(plain, "asghar (main · session 7b5e8856)"));
        // Another session, another agent, or no stamp: not the same.
        assert!(!one_session_slot(
            one,
            "Kaveh-2 (main · session 0f0f0f0f · slot coder-2)"
        ));
        assert!(!one_session_slot(
            one,
            "Kaveh-2 (main · session 7b5e8856 · agent a1b2c3d4 · slot coder-2)"
        ));
        assert!(!one_session_slot(one, "Kaveh-1"));
        assert_eq!(slot_of(one), "coder-1");
        assert_eq!(slot_of(plain), "");
        assert_eq!(slot_of("Kaveh-1"), "");
    }

    #[test]
    fn a_holders_slot_is_carried_to_its_next_stamp() {
        use super::keep_slot;
        let claim = "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)";
        // The holder acting as its old name after a rename: the slot, before the account.
        assert_eq!(
            keep_slot(claim, "Kaveh-2 (wt-0112 · session 7b5e8856 · cc-hg)"),
            "Kaveh-2 (wt-0112 · session 7b5e8856 · slot coder-2 · cc-hg)"
        );
        // A successor under the same label (a takeover), a teammate's agent kept.
        assert_eq!(
            keep_slot(claim, "Kaveh-2 (session 11111111 · agent a1b2c3d4)"),
            "Kaveh-2 (session 11111111 · agent a1b2c3d4 · slot coder-2)"
        );
        // The round trip reads back.
        let carried = keep_slot(claim, "Kaveh-2 (session 7b5e8856)");
        assert_eq!(super::split(&carried).1.unwrap().slot, "coder-2");
        for (from, who) in [
            // Another label in another session, the owner, no stamp, a slot already, no slot to carry.
            (claim, "wt-a (session 11111111)"),
            (claim, "owner (session 11111111 · cc-mg)"),
            (claim, "Kaveh-2"),
            (claim, "coder-3 (session 7b5e8856 · slot coder-3)"),
            ("wt-a (session 7b5e8856)", "wt-a (session 7b5e8856)"),
            ("Kaveh-2", "Kaveh-2 (session 7b5e8856)"),
        ] {
            assert_eq!(keep_slot(from, who), who, "{from} → {who}");
        }
    }

    #[test]
    fn a_stamp_without_its_slot_keeps_the_rest() {
        use super::{slot_of, split, without_slot};
        let claim = "Kaveh-2 (main · session 7b5e8856 · agent a1b2c3d4 · slot coder-2 · cc-hg)";
        let bare = without_slot(claim);
        assert_eq!(
            bare,
            "Kaveh-2 (main · session 7b5e8856 · agent a1b2c3d4 · cc-hg)"
        );
        assert_eq!(slot_of(&bare), "");
        let (label, stamp) = split(&bare);
        let stamp = stamp.unwrap();
        assert_eq!(
            (label, stamp.id.as_str(), stamp.agent.as_str()),
            ("Kaveh-2", "7b5e8856", "a1b2c3d4")
        );
        for who in [
            "akbar (main · session 7b5e8856 · cc-hg)",
            "Kaveh-2",
            "owner",
            "",
        ] {
            assert_eq!(without_slot(who), who);
        }
    }

    #[test]
    fn a_part_containing_the_login_name_is_dropped_wherever_it_comes_from() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("jdoe");
        let repo = home.join("code").join("jdoe.github.io");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let env = |k: &str| -> Option<String> {
            match k {
                "CLAUDE_CODE_SESSION_ID" => Some("7b5e8856-aaaa-bbbb-cccc-0123456789ab".into()),
                "CLAUDE_CODE_SESSION_NAME" => Some("fix /Users/jdoe/notes".into()),
                "CLAUDE_CONFIG_DIR" => Some("/x/.claude-jdoe".into()),
                "HOME" => Some(home.to_string_lossy().to_string()),
                _ => None,
            }
        };
        let s = Session::from_env(&env, Some(&repo)).unwrap();
        assert_eq!(
            (s.name.as_str(), s.checkout.as_str(), s.account.as_str()),
            ("", "", "")
        );
        let written = stamped("wt-a", Some(&s));
        assert_eq!(written, "wt-a (session 7b5e8856)");
        assert!(!written.to_lowercase().contains("jdoe"));
        // A name with the login in it, no path, is dropped too; an unrelated one stays.
        let named = |n: &'static str| {
            let env2 = |k: &str| {
                if k == "CLAUDE_CODE_SESSION_NAME" {
                    Some(n.to_string())
                } else {
                    env(k)
                }
            };
            Session::from_env(&env2, None).unwrap().name
        };
        assert_eq!(named("jdoe-fixes"), "");
        assert_eq!(named("main-29"), "main-29");
    }

    #[test]
    fn a_blank_label_gets_no_stamp_so_it_is_still_refused() {
        let s = Session {
            id: "7b5e8856".into(),
            name: String::new(),
            checkout: String::new(),
            account: String::new(),
            agent: None,
        };
        assert_eq!(stamped("", Some(&s)), "");
        assert_eq!(stamped(" \t", Some(&s)), " \t");
        assert!(crate::queue::actor(&stamped("", Some(&s))).is_err());
    }

    use super::*;
    use std::collections::HashMap;
    use std::fs;

    const ID: &str = "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394";

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| m.get(k).cloned()
    }

    /// `<tmp>/home/code/wt-0030/app/src`, with `wt-0030` a checkout (`.git` is a file in a worktree).
    fn tree() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        let wt = home.join("code/wt-0030");
        fs::create_dir_all(wt.join("app/src")).unwrap();
        fs::write(wt.join(".git"), "gitdir: /elsewhere").unwrap();
        (d, home, wt)
    }

    #[test]
    fn no_session_id_is_no_session() {
        let (_d, home, wt) = tree();
        let h = home.to_string_lossy().to_string();
        let e = env(&[("HOME", &h), ("CLAUDE_CONFIG_DIR", "/x/.claude-hg")]);
        assert_eq!(Session::from_env(&e, Some(&wt)), None);
        for bad in [
            "",
            "   ",
            "nothex!!-1234",
            "7b5e885",
            "7b5e8856\nsession 00000000",
            "7b5e8856: stage ready → done",
            "7b5e8856 (x)",
            "zzzzzzzz-1234",
            &"a".repeat(65),
        ] {
            let e = env(&[("CLAUDE_CODE_SESSION_ID", bad)]);
            assert_eq!(Session::from_env(&e, None), None, "{bad:?}");
        }
        assert_eq!(stamped("wt-a", None), "wt-a");
    }

    #[test]
    fn a_session_is_its_short_id_name_checkout_and_account() {
        let (_d, home, wt) = tree();
        let h = home.to_string_lossy().to_string();
        let e = env(&[
            ("CLAUDE_CODE_SESSION_ID", &ID.to_uppercase()),
            ("CLAUDE_CODE_SESSION_NAME", "main-29"),
            ("CLAUDE_CONFIG_DIR", "/Users/someone/.claude-hg"),
            ("HOME", &h),
            ("USER", "someone-else"),
        ]);
        let s = Session::from_env(&e, Some(&wt.join("app/src"))).unwrap();
        assert_eq!(
            s,
            Session {
                id: "7b5e8856".into(),
                name: "main-29".into(),
                checkout: "wt-0030".into(),
                account: "hg".into(),
                agent: None,
            }
        );
        assert_eq!(
            stamped("muster/wt-0030 (who's on it)", Some(&s)),
            "muster/wt-0030 (who's on it) (main-29 · session 7b5e8856 · cc-hg)",
            "the checkout the label already says is left out"
        );
        assert_eq!(
            stamped("qa", Some(&s)),
            "qa (main-29 · wt-0030 · session 7b5e8856 · cc-hg)"
        );
        // Only the id: no name, no checkout, the default config folder.
        let e = env(&[
            ("CLAUDE_CODE_SESSION_ID", ID),
            ("CLAUDE_CONFIG_DIR", "/Users/someone/.claude"),
        ]);
        let s = Session::from_env(&e, None).unwrap();
        assert_eq!(stamped("wt-0030", Some(&s)), "wt-0030 (session 7b5e8856)");
    }

    #[test]
    fn hostile_names_are_cut_to_safe_tokens_and_capped() {
        for (raw, want) in [
            ("main-29", "main-29"),
            ("my session\nname", "my-session-name"),
            ("x: stage ready → done", "x-stage-ready-done"),
            ("a (session 00000000)", "a-session-00000000"),
            ("../../etc/passwd", "etcpasswd"),
            ("  --.hidden.--  ", "hidden"),
            ("ÄÖÜ", ""),
            (":::", ""),
        ] {
            assert_eq!(token(raw, MAX_NAME), want, "{raw:?}");
        }
        let long = token(&"n".repeat(500), MAX_NAME);
        assert_eq!(long.len(), MAX_NAME);
        assert_eq!(token("abc-def", 4), "abc", "no dash left at the cut");
        let e = env(&[
            ("CLAUDE_CODE_SESSION_ID", ID),
            (
                "CLAUDE_CODE_SESSION_NAME",
                "evil: stage ready → building (claimed)\n- x",
            ),
            ("CLAUDE_CONFIG_DIR", "/x/.claude-hg: stage"),
        ]);
        let s = Session::from_env(&e, None).unwrap();
        let who = stamped("wt-a", Some(&s));
        assert_eq!(
            who,
            "wt-a (evil-stage-ready-building-claime · session 7b5e8856)"
        );
        assert!(!who.contains(": stage ") && !who.contains(" → ") && !who.contains('\n'));
    }

    #[test]
    fn accounts_come_only_from_the_config_folders_name() {
        let a = |d: &str| account(d);
        assert_eq!(a("/Users/x/.claude-hg").as_deref(), Some("hg"));
        assert_eq!(a("/Users/x/.claude-mg/").as_deref(), Some("mg"));
        assert_eq!(a("/Users/x/cc-ih").as_deref(), Some("ih"));
        assert_eq!(a("/Users/x/.claude"), None);
        assert_eq!(a("/Users/x/secret-token-abc"), None);
        assert_eq!(a("/Users/x/.claude-Way-Too-Long-Name"), None);
        assert_eq!(a("/Users/x/.claude-h g"), None);
        assert_eq!(a(""), None);
    }

    #[test]
    fn the_checkout_is_the_nearest_git_folder_never_home_a_queue_or_the_login() {
        let (_d, home, wt) = tree();
        assert_eq!(
            checkout(&wt.join("app/src"), Some(&home)).as_deref(),
            Some("wt-0030")
        );
        // Outside any checkout, and a git folder at home, are nothing.
        let loose = home.join("loose");
        fs::create_dir_all(&loose).unwrap();
        fs::create_dir_all(home.join(".git")).unwrap();
        assert_eq!(checkout(&loose, Some(&home)), None);
        assert_eq!(checkout(&home, Some(&home)), None);
        // A checkout named like the home folder (the login name): nothing, rather than that name.
        let mine = home.join("code/Home");
        fs::create_dir_all(mine.join(".git")).unwrap();
        assert_eq!(checkout(&mine, Some(&home)), None);
        // A task queue has its own git; it isn't a checkout of code.
        let q = home.join("code/tasks");
        fs::create_dir_all(q.join(".git")).unwrap();
        fs::create_dir_all(q.join("todo")).unwrap();
        fs::write(q.join("README.md"), "q").unwrap();
        assert_eq!(checkout(&q, Some(&home)), None);
    }

    #[test]
    fn a_holder_splits_into_its_label_and_stamp() {
        let (l, s) = split("wt-0030 (main-29 · session 7b5e8856 · cc-hg)");
        assert_eq!(l, "wt-0030");
        assert_eq!(
            s.unwrap(),
            Stamp {
                parts: vec!["main-29".into()],
                id: "7b5e8856".into(),
                agent: String::new(),
                slot: String::new(),
                account: "hg".into()
            }
        );
        assert_eq!(split("drover/main (session 3fba2222)").0, "drover/main");
        // Older free-form claims are labels: no stamp in the shape the tool writes.
        for who in [
            "wt-a",
            "capture (drover/main, session 349bc9c3)",
            "worktree-i18n (.claude/worktrees/i18n), session 41f7fe18",
            "muster/wt-0019 (dependency graph)",
            "x (session 7B5E8856)",
            "x (session 7b5e885)",
            "x (session 7b5e8856 · session 7b5e8856)",
            "x (a · b · c · session 7b5e8856)",
            "x (session 7b5e8856 · hg)",
            "(session 7b5e8856)",
            "x (session 7b5e8856) trailing",
        ] {
            assert_eq!(split(who), (who, None), "{who}");
        }
    }

    #[test]
    fn a_typed_stamp_is_replaced_by_the_real_one() {
        let s = Session {
            id: "7b5e8856".into(),
            ..Default::default()
        };
        assert_eq!(
            stamped("wt-a (session 00000000)", Some(&s)),
            "wt-a (session 7b5e8856)"
        );
        let once = stamped("wt-a", Some(&s));
        assert_eq!(
            stamped(&once, Some(&s)),
            once,
            "stamping twice changes nothing"
        );
    }

    #[test]
    fn holders_match_by_label_and_session() {
        let a = "wt-a (wt-0030 · session 7b5e8856)";
        assert!(same_holder(a, a));
        assert!(
            same_holder(a, "wt-a (main · session 7b5e8856 · cc-hg)"),
            "another folder, same session"
        );
        assert!(
            same_holder(a, "wt-a"),
            "unstamped (before 0030, or the owner's terminal)"
        );
        assert!(same_holder("wt-a", a));
        assert!(
            !same_holder(a, "wt-a (session 11111111)"),
            "another session under the label"
        );
        assert!(
            !same_holder(a, "wt-b (session 7b5e8856)"),
            "another label in the session"
        );
        assert!(!same_holder("wt-a", "wt-b"));
        assert!(!same_holder("", "wt-a"));
        assert!(is_owner("owner") && is_owner("owner (session 7b5e8856)"));
        assert!(!is_owner("owner-ish") && !is_owner("wt-a"));
    }

    // Task 0090: a teammate's stamp carries its agent id.

    fn teammate(agent: Option<&str>) -> Session {
        Session {
            id: "7b5e8856".into(),
            checkout: "main".into(),
            account: "mg".into(),
            agent: agent.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn an_agent_id_is_its_first_8_hex_characters_or_nothing() {
        assert_eq!(agent_id("a1B2c3D4e5f6a7b8c").as_deref(), Some("a1b2c3d4"));
        assert_eq!(agent_id("  a1b2c3d4  ").as_deref(), Some("a1b2c3d4"));
        for bad in [
            "",
            "a1b2c3d",
            "g1b2c3d4",
            "a1b2c3d4; rm -rf x",
            "a1b2c3d4 x",
            "a1b2c3d4\nsession 00000000",
            "../a1b2c3",
            "a1b2c3d4/../x",
            &"a".repeat(65),
        ] {
            assert_eq!(agent_id(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_teammate_is_stamped_with_its_agent_from_muster_agent() {
        let e = env(&[
            ("CLAUDE_CODE_SESSION_ID", ID),
            ("MUSTER_AGENT", "A1B2C3D4E5F6A7B8C"),
            ("CLAUDE_CONFIG_DIR", "/x/.claude-mg"),
        ]);
        let s = Session::from_env(&e, None).unwrap();
        assert_eq!(s.agent.as_deref(), Some("a1b2c3d4"));
        assert_eq!(
            stamped("asghar", Some(&s)),
            "asghar (session 7b5e8856 · agent a1b2c3d4 · cc-mg)"
        );
        // An id that isn't one is no agent (the CLI refuses it before it gets here).
        let e = env(&[("CLAUDE_CODE_SESSION_ID", ID), ("MUSTER_AGENT", "nope")]);
        assert_eq!(Session::from_env(&e, None).unwrap().agent, None);
    }

    #[test]
    fn a_stamp_with_an_agent_round_trips_under_a_persian_label() {
        let who = stamped("اصغر", Some(&teammate(Some("a1b2c3d4"))));
        assert_eq!(
            who,
            "اصغر (main · session 7b5e8856 · agent a1b2c3d4 · cc-mg)"
        );
        let (label, stamp) = split(&who);
        assert_eq!(label, "اصغر");
        let stamp = stamp.unwrap();
        assert_eq!(
            stamp,
            Stamp {
                parts: vec!["main".into()],
                id: "7b5e8856".into(),
                agent: "a1b2c3d4".into(),
                slot: String::new(),
                account: "mg".into(),
            }
        );
        assert_eq!(stamp.mailbox(), "7b5e8856/a1b2c3d4");
        assert_eq!(stamped(&who, Some(&teammate(Some("a1b2c3d4")))), who);
        // The lead's stamp, as before: no agent part, the session's own mailbox.
        let (_, lead) = split("اصغر (main · session 7b5e8856 · cc-mg)");
        assert_eq!(lead.unwrap().mailbox(), "7b5e8856");
        // The agent part only right after the session, once, as 8 lowercase hex.
        for who in [
            "x (agent a1b2c3d4 · session 7b5e8856)",
            "x (session 7b5e8856 · cc-mg · agent a1b2c3d4)",
            "x (session 7b5e8856 · agent a1b2c3d4 · agent a1b2c3d4)",
            "x (session 7b5e8856 · agent A1B2C3D4)",
            "x (session 7b5e8856 · agent a1b2c3)",
            "x (session 7b5e8856 · agent ../../ab)",
            "x (agent a1b2c3d4)",
        ] {
            assert_eq!(split(who), (who, None), "{who}");
        }
    }

    #[test]
    fn same_holder_tells_a_lead_and_its_teammates_apart() {
        let lead = stamped("asghar", Some(&teammate(None)));
        let one = stamped("asghar", Some(&teammate(Some("a1b2c3d4"))));
        let two = stamped("asghar", Some(&teammate(Some("0f0f0f0f"))));
        assert!(same_holder(&one, &one));
        assert!(
            same_holder(&one, "asghar (wt-0090 · session 7b5e8856 · agent a1b2c3d4)"),
            "the same teammate in another folder"
        );
        assert!(!same_holder(&lead, &one), "a lead isn't its teammate");
        assert!(!same_holder(&one, &lead), "nor the other way round");
        assert!(!same_holder(&one, &two), "two teammates of one lead");
        assert!(
            !same_holder(&one, "asghar (session 11111111 · agent a1b2c3d4)"),
            "the same agent id under another session"
        );
        assert!(
            same_holder(&one, "asghar") && same_holder("asghar", &one),
            "unstamped matches any"
        );
        assert!(!same_holder(
            &one,
            "gholi (session 7b5e8856 · agent a1b2c3d4)"
        ));
    }

    #[test]
    fn same_holder_reads_old_stamps_as_before() {
        let old = "wt-a (main · session 7b5e8856 · cc-hg)";
        assert!(same_holder(old, "wt-a (session 7b5e8856)"));
        assert!(!same_holder(old, "wt-a (session 11111111)"));
        assert_eq!(split(old).1.unwrap().agent, "");
    }

    // Task 0112: a claim made for a role's slot carries the slot in its stamp.

    #[test]
    fn a_slot_stamp_round_trips_role_slot_and_session() {
        let s = Session {
            id: "7b5e8856".into(),
            checkout: "main".into(),
            account: "hg".into(),
            ..Default::default()
        };
        let who = stamped_slot("Kaveh-2", Some(&s), Some("coder-2"));
        assert_eq!(
            who,
            "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)"
        );
        let (label, stamp) = split(&who);
        assert_eq!(label, "Kaveh-2");
        let stamp = stamp.unwrap();
        assert_eq!(
            (
                stamp.id.as_str(),
                stamp.slot.as_str(),
                stamp.account.as_str()
            ),
            ("7b5e8856", "coder-2", "hg")
        );
        let roles = crate::roles::defaults();
        let slot = crate::roles::slot_of_holder(&roles, &who).unwrap();
        assert_eq!((slot.role.as_str(), slot.n), ("coder", 2));
        // Stamping again changes nothing; a slot that isn't one is left out.
        assert_eq!(stamped_slot(&who, Some(&s), Some("coder-2")), who);
        assert_eq!(
            stamped_slot("x", Some(&s), Some("coder-02")),
            "x (main · session 7b5e8856 · cc-hg)"
        );
        // A teammate's: the slot after its agent.
        let t = Session {
            agent: Some("a1b2c3d4".into()),
            ..s.clone()
        };
        let mate = stamped_slot("Sohrab-1", Some(&t), Some("security-1"));
        assert_eq!(
            mate,
            "Sohrab-1 (main · session 7b5e8856 · agent a1b2c3d4 · slot security-1 · cc-hg)"
        );
        assert_eq!(split(&mate).1.unwrap().slot, "security-1");
    }

    #[test]
    fn a_slot_stamp_with_a_persian_name_parses_back() {
        let s = Session {
            id: "7b5e8856".into(),
            account: "hg".into(),
            ..Default::default()
        };
        let who = stamped_slot("کاوه-2", Some(&s), Some("coder-2"));
        assert_eq!(who, "کاوه-2 (session 7b5e8856 · slot coder-2 · cc-hg)");
        let (label, stamp) = split(&who);
        assert_eq!(label, "کاوه-2");
        assert_eq!(stamp.unwrap().slot, "coder-2");
        // Without a stamp, the Persian name and the ASCII number after it find the role; a stamp
        // without the slot part holds none (it came back after its slot went to another session).
        let roles = crate::roles::parse("[coder]\nname = \"کاوه\"\n").unwrap();
        let slot = crate::roles::slot_of_holder(&roles, "کاوه-2").unwrap();
        assert_eq!(slot.id(), "coder-2");
        assert_eq!(
            crate::roles::slot_of_holder(&roles, "کاوه-2 (session 7b5e8856 · cc-hg)"),
            None
        );
    }

    #[test]
    fn a_slot_part_only_after_the_session_once_and_before_the_account() {
        for who in [
            "x (slot coder-2 · session 7b5e8856)",
            "x (session 7b5e8856 · cc-hg · slot coder-2)",
            "x (session 7b5e8856 · slot coder-2 · slot coder-2)",
            "x (session 7b5e8856 · slot coder-02)",
            "x (session 7b5e8856 · slot Coder-2)",
            "x (session 7b5e8856 · slot coder)",
            "x (session 7b5e8856 · slot ../coder-2)",
        ] {
            assert_eq!(split(who), (who, None), "{who}");
        }
    }

    #[test]
    fn old_stamps_read_as_before_and_one_slot_in_one_session_is_one_holder() {
        let old = "wt-a (main · session 7b5e8856 · cc-hg)";
        let (label, stamp) = split(old);
        assert_eq!((label, stamp.unwrap().slot), ("wt-a", String::new()));
        assert_eq!(
            crate::roles::slot_of_holder(&crate::roles::defaults(), old),
            None
        );
        let kaveh = "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)";
        assert!(same_holder(
            kaveh,
            "coder-2 (wt-0112 · session 7b5e8856 · slot coder-2)"
        ));
        assert!(
            same_holder(kaveh, "کاوه-2 (session 7b5e8856 · slot coder-2)"),
            "renamed"
        );
        assert!(
            !same_holder(kaveh, "coder-2 (session 11111111 · slot coder-2)"),
            "another session"
        );
        assert!(
            !same_holder(kaveh, "Kaveh-3 (session 7b5e8856 · slot coder-3)"),
            "another slot"
        );
        assert!(
            same_holder(kaveh, "Kaveh-2"),
            "unstamped matches any, as before"
        );
        assert!(
            !same_holder(kaveh, "coder-2"),
            "another label without a stamp"
        );
    }

    #[test]
    fn a_holder_acting_with_a_stamp_restamps_a_bare_or_moved_claim() {
        use super::restamped;
        let me = "asghar (main · session 7b5e8856 · cc-hg)";
        // A bare label takes the acting session's stamp; without a slot, as a takeover gives it.
        assert_eq!(restamped("asghar", me).as_deref(), Some(me));
        assert_eq!(
            restamped(
                "Kaveh-2",
                "Kaveh-2 (main · session 7b5e8856 · slot coder-2 · cc-hg)"
            )
            .as_deref(),
            Some("Kaveh-2 (main · session 7b5e8856 · cc-hg)")
        );
        // The same session under another account takes the new account, nothing else.
        assert_eq!(
            restamped("asghar (wt-0184 · session 7b5e8856 · cc-mg)", me).as_deref(),
            Some("asghar (wt-0184 · session 7b5e8856 · cc-hg)")
        );
        // Already right, or not this holder's, or nothing to stamp with: None.
        assert_eq!(restamped(me, me), None);
        assert_eq!(
            restamped("asghar (wt-0184 · session 7b5e8856 · cc-hg)", me),
            None
        );
        assert_eq!(
            restamped(
                "asghar (main · session 7b5e8856 · cc-hg)",
                "asghar (main · session 7b5e8856)"
            ),
            None
        );
        assert_eq!(restamped("asghar", "asghar"), None);
        assert_eq!(restamped("gholi", me), None);
        assert_eq!(
            restamped("asghar (main · session 0f0f0f0f · cc-mg)", me),
            None
        );
        assert_eq!(restamped("", me), None);
        assert_eq!(
            restamped("owner", "owner (main · session 7b5e8856 · cc-hg)"),
            None
        );
        // A teammate's claim is its own agent's: the lead's stamp never restamps it.
        assert_eq!(
            restamped(
                "asghar (main · session 7b5e8856 · agent a1b2c3d4 · cc-mg)",
                me
            ),
            None
        );
    }
}
