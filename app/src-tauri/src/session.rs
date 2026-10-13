//! Who a session is, as `muster` stamps it next to `--as` (task 0030), so an agent never types it:
//! `wt-0019 (main-29 · session 7b5e8856 · cc-hg)`. The stamp holds only the Claude Code session's
//! short id, its name when the harness gives one, the checkout's folder name and the account label.
//! Nothing else: no paths, no user names, no tokens (the privacy baseline).
//!
//! A holder is its `--as` label plus, when it has one, its session. Two holders are the same when
//! their labels match and their sessions don't differ, so a claim made before stamps (or by the
//! owner in a terminal) keeps working, while two sessions under one label stay apart.

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
}

/// A stamp read back from a holder: the name and checkout (in that order, either may be missing),
/// the session id and the account label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stamp {
    pub parts: Vec<String>,
    pub id: String,
    pub account: String,
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
        Some(Session {
            id,
            name,
            checkout,
            account,
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
fn checkout(cwd: &Path, home: Option<&Path>) -> Option<String> {
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
    if !s.account.is_empty() {
        parts.push(format!("cc-{}", s.account));
    }
    format!("{label} ({})", parts.join(" · "))
}

/// A holder as (label, stamp): the stamp is a last ` (… · session <8 hex> …)` group in exactly
/// the shape `stamped` writes. Anything else (a hand-typed `(drover/main, session 349bc9c3)`) is
/// part of the label.
pub fn split(who: &str) -> (&str, Option<Stamp>) {
    let parsed = || -> Option<(usize, Stamp)> {
        let at = who.rfind(" (")?;
        let inner = who[at + 2..].strip_suffix(')')?;
        let mut stamp = Stamp::default();
        for part in inner.split(" · ") {
            if let Some(id) = part.strip_prefix("session ") {
                let hex = id.len() == 8
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
                if !stamp.id.is_empty() || !hex {
                    return None;
                }
                stamp.id = id.to_string();
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
/// differ (one without a session matches any, as every claim did before stamps).
pub fn same_holder(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let ((la, sa), (lb, sb)) = (split(a), split(b));
    la == lb
        && match (sa, sb) {
            (Some(x), Some(y)) => x.id == y.id,
            _ => true,
        }
}

/// Is this holder the owner (`owner`, with or without a session)?
pub fn is_owner(who: &str) -> bool {
    split(who).0 == "owner"
}

#[cfg(test)]
mod tests {
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
}
