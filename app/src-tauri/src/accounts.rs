//! The Claude accounts Muster reads (task 0099): one allowlist, `claude_accounts` in Settings,
//! default cc-mg and cc-hg. `~/.claude` is the company account and is never read: not its
//! sessions (`claude agents`), not its transcripts, not its settings or login. Everything that
//! reads an account goes through `allowed`: the app's session states (`agents`), the transcripts
//! (`transcripts::Reader`), the helper, and `muster list --json`.
//!
//! An `Account` can only be made here, from a valid label whose folder is a real directory that
//! isn't `~/.claude` or inside it, so a caller holding one holds an allowed account.
//!
//! The account folders' `projects` folders may be links to one shared folder (on the owner's Mac
//! all three are), so transcripts are also filtered by project: only a folder whose name is the
//! key of a path under `~/playground` (`playground_key`, `is_under`) is read.

use crate::session::MAX_ACCOUNT;
use crate::settings::Settings;
use std::fs;
use std::path::{Path, PathBuf};

/// The default allowlist: the owner's personal accounts.
pub const DEFAULT: [&str; 2] = ["mg", "hg"];

/// Is `label` an account label as `session.rs` reads one back: 1 to 12 lowercase letters and
/// digits? `""`, `.claude`, `../x` and `MG` aren't.
pub fn is_label(label: &str) -> bool {
    (1..=MAX_ACCOUNT).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The default allowlist as owned labels.
pub fn default_labels() -> Vec<String> {
    DEFAULT.iter().map(|s| s.to_string()).collect()
}

/// An allowed Claude account: its label and its config folder (`~/.claude-<label>`, as
/// `CLAUDE_CONFIG_DIR` gets it). Only `allowed` and `resolve` make one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    label: String,
    dir: PathBuf,
    real: PathBuf,
}

impl Account {
    /// The label: `mg` for `~/.claude-mg`.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The config folder as `CLAUDE_CONFIG_DIR` is set to it: `<home>/.claude-<label>`.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The folder's name under the home folder: `.claude-mg` (the helper's saved account).
    pub fn folder_name(&self) -> String {
        format!(".claude-{}", self.label)
    }

    /// The config folder, canonical.
    pub fn real(&self) -> &Path {
        &self.real
    }
}

/// The accounts in `settings`' allowlist under `home` (`resolve`).
pub fn allowed(settings: &Settings, home: &Path) -> Vec<Account> {
    resolve(&settings.claude_accounts, home)
}

/// Each of `labels`, once and in order, as an `Account` under `home`: a valid label (`is_label`)
/// whose `~/.claude-<label>`, canonicalized, is a directory that isn't `~/.claude` itself,
/// anything inside it, or a folder holding it, like home (so `~/.claude-x → ~/.claude`,
/// `~/.claude-x → ~` and `..` tricks are refused). The rest are left out. Cheap: one `canonicalize` per label, so it's resolved on every read, and a folder
/// made after the app started is picked up.
pub fn resolve(labels: &[String], home: &Path) -> Vec<Account> {
    let company = company_dir(home);
    let mut out: Vec<Account> = Vec::new();
    for label in labels {
        if !is_label(label) || out.iter().any(|a| &a.label == label) {
            continue;
        }
        let dir = home.join(format!(".claude-{label}"));
        let Ok(real) = fs::canonicalize(&dir) else {
            continue;
        };
        let company_owned = company
            .as_ref()
            .is_some_and(|c| real.starts_with(c) || c.starts_with(&real));
        if real.is_dir() && !company_owned {
            out.push(Account {
                label: label.clone(),
                dir,
                real,
            });
        }
    }
    out
}

/// Where `~/.claude` is, canonical, or would be: a missing one still can't be reached through a
/// link.
fn company_dir(home: &Path) -> Option<PathBuf> {
    fs::canonicalize(home.join(".claude"))
        .or_else(|_| fs::canonicalize(home).map(|h| h.join(".claude")))
        .ok()
}

/// Is the canonical folder `real` the company's `~/.claude` under `home`, inside it, or a folder
/// holding it? `resolve`'s rule, for a Codex account's folder too (task 0113).
pub fn is_company(real: &Path, home: &Path) -> bool {
    company_dir(home).is_some_and(|c| real.starts_with(&c) || c.starts_with(real))
}

/// The allowed account a `CLAUDE_CONFIG_DIR` names (task 0113, the status line's shim): a folder
/// named `.claude-<label>` whose label is on `settings`' allowlist and which, canonicalized, is
/// that account's own folder under `home`. None for `~/.claude`, a folder of the same name
/// elsewhere, and a label off the list.
pub fn from_config_dir(dir: &Path, settings: &Settings, home: &Path) -> Option<Account> {
    let label = dir.file_name()?.to_str()?.strip_prefix(".claude-")?;
    if !settings.claude_accounts.iter().any(|l| l == label) {
        return None;
    }
    let account = resolve(&[label.to_string()], home).pop()?;
    (fs::canonicalize(dir).ok()? == account.real).then_some(account)
}

/// Is `label` one of `allowed`, exactly?
pub fn is_allowed(label: &str, allowed: &[Account]) -> bool {
    allowed.iter().any(|a| a.label == label)
}

/// The account `label` among `allowed`, exactly.
pub fn find<'a>(label: &str, allowed: &'a [Account]) -> Option<&'a Account> {
    allowed.iter().find(|a| a.label == label)
}

/// A path as Claude Code names its project folder: every character but ASCII letters and digits
/// becomes `-` (`/Users/x/playground/a.b` → `-Users-x-playground-a-b`).
pub fn key(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The key of the personal root, `<home>/playground`, with the home folder canonicalized
/// (`-Users-x-playground`).
pub fn playground_key(home: &Path) -> String {
    let home = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    key(&home.join("playground"))
}

/// Is the project folder `name` the key of a path at or under the root whose key is `root`? The
/// root itself, or the root followed by `-`: `-Users-x-playground2-…` isn't under
/// `-Users-x-playground`. An empty root matches nothing.
pub fn is_under(name: &str, root: &str) -> bool {
    !root.is_empty()
        && name
            .strip_prefix(root)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn labels(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    /// A home as the owner's Mac has it: `~/.claude` (the company account, with sessions,
    /// transcripts and settings), `~/.claude-ih`, and `~/.claude-mg` and `~/.claude-hg` whose
    /// `projects` are links to `~/.claude/projects`. Transcripts in a code-keyed and a
    /// playground-keyed project folder.
    pub(crate) fn fake_home() -> crate::testkit::TestDir {
        let d = crate::testkit::tempdir().unwrap();
        let h = d.path();
        let projects = h.join(".claude/projects");
        fs::create_dir_all(projects.join("-Users-x-code-app")).unwrap();
        fs::create_dir_all(projects.join("-Users-x-playground-muster-main")).unwrap();
        fs::write(h.join(".claude/settings.json"), "{}").unwrap();
        fs::write(h.join(".claude.json"), "{}").unwrap();
        fs::create_dir_all(h.join(".claude-ih/projects")).unwrap();
        for a in ["mg", "hg"] {
            fs::create_dir(h.join(format!(".claude-{a}"))).unwrap();
            symlink(&projects, h.join(format!(".claude-{a}/projects"))).unwrap();
        }
        d
    }

    #[test]
    fn labels_are_one_to_twelve_lowercase_letters_and_digits() {
        for ok in ["mg", "hg", "mg2", "a", "abcdefghijkl"] {
            assert!(is_label(ok), "{ok}");
        }
        for bad in [
            "",
            ".claude",
            "../x",
            "MG",
            "abcdefghijklm",
            "m g",
            "cc-mg",
            "mg/",
            "é",
        ] {
            assert!(!is_label(bad), "{bad:?}");
        }
    }

    #[test]
    fn allowed_resolves_each_listed_folder_once_in_order() {
        let d = fake_home();
        let got = resolve(&labels(&["hg", "mg", "hg"]), d.path());
        let names: Vec<&str> = got.iter().map(Account::label).collect();
        assert_eq!(names, ["hg", "mg"]);
        assert_eq!(got[0].dir(), d.path().join(".claude-hg"));
        assert_eq!(got[0].folder_name(), ".claude-hg");
        assert_eq!(
            got[0].real(),
            fs::canonicalize(d.path().join(".claude-hg")).unwrap()
        );
        // The default list, through the settings.
        let s = Settings::default();
        assert_eq!(s.claude_accounts, ["mg", "hg"]);
        let names: Vec<String> = allowed(&s, d.path())
            .iter()
            .map(|a| a.label().to_string())
            .collect();
        assert_eq!(names, ["mg", "hg"]);
        assert!(is_allowed("mg", &got) && !is_allowed("ih", &got) && !is_allowed("", &got));
        assert!(!is_allowed("m", &got), "an exact match, not a prefix");
        assert_eq!(find("hg", &got), Some(&got[0]));
    }

    #[test]
    fn the_company_account_and_links_into_it_are_never_allowed() {
        let d = fake_home();
        let h = d.path();
        // A label whose folder is a link to ~/.claude, or to a folder inside it.
        symlink(h.join(".claude"), h.join(".claude-x")).unwrap();
        fs::create_dir(h.join(".claude/inner")).unwrap();
        symlink(h.join(".claude/inner"), h.join(".claude-y")).unwrap();
        // A link to a folder that holds ~/.claude: claude would read ~/.claude.json through it.
        symlink(h, h.join(".claude-z")).unwrap();
        symlink("/", h.join(".claude-r")).unwrap();
        // A file, not a folder; and a label with no folder.
        fs::write(h.join(".claude-f"), "").unwrap();
        let got = resolve(
            &labels(&[
                "x",
                "y",
                "z",
                "r",
                "f",
                "none",
                "",
                ".claude",
                "../claude",
                "MG",
                "ih",
            ]),
            h,
        );
        let names: Vec<&str> = got.iter().map(Account::label).collect();
        assert_eq!(names, ["ih"], "only a real folder of its own");
        assert!(resolve(&[], h).is_empty(), "an empty list reads nothing");
    }

    #[test]
    fn a_config_dir_names_an_account_only_when_it_is_that_allowed_accounts_own_folder() {
        let d = fake_home();
        let h = d.path();
        let s = Settings::default();
        let got = from_config_dir(&h.join(".claude-mg"), &s, h).unwrap();
        assert_eq!(got.label(), "mg");
        // A trailing slash is the same folder.
        let slash = PathBuf::from(format!("{}/", h.join(".claude-hg").display()));
        assert_eq!(from_config_dir(&slash, &s, h).unwrap().label(), "hg");
        // The company account, one off the list, and a folder of the same name elsewhere.
        let elsewhere = crate::testkit::tempdir().unwrap();
        fs::create_dir(elsewhere.path().join(".claude-mg")).unwrap();
        for dir in [
            h.join(".claude"),
            h.join(".claude-ih"),
            h.join(".claude-none"),
            elsewhere.path().join(".claude-mg"),
            h.to_path_buf(),
            PathBuf::new(),
        ] {
            assert!(from_config_dir(&dir, &s, h).is_none(), "{}", dir.display());
        }
        // A link to ~/.claude under an allowed label.
        let s2 = Settings {
            claude_accounts: labels(&["x"]),
            ..Settings::default()
        };
        symlink(h.join(".claude"), h.join(".claude-x")).unwrap();
        assert!(from_config_dir(&h.join(".claude-x"), &s2, h).is_none());
        // The company folder rule, as Codex folders are checked.
        let company = fs::canonicalize(h.join(".claude")).unwrap();
        assert!(is_company(&company, h));
        assert!(is_company(&company.join("projects"), h));
        assert!(
            is_company(&fs::canonicalize(h).unwrap(), h),
            "a folder holding it"
        );
        assert!(!is_company(
            &fs::canonicalize(h.join(".claude-mg")).unwrap(),
            h
        ));
    }

    #[test]
    fn a_project_key_is_under_the_playground_only_at_a_folder_boundary() {
        assert_eq!(
            key(Path::new("/Users/x/playground/muster/main/.claude/work")),
            "-Users-x-playground-muster-main--claude-work"
        );
        let root = "-Users-x-playground";
        for ok in [
            "-Users-x-playground",
            "-Users-x-playground-muster-main",
            "-Users-x-playground--claude",
        ] {
            assert!(is_under(ok, root), "{ok}");
        }
        for bad in [
            "-Users-x-playground2-muster",
            "-Users-x-playgroundx",
            "-Users-x-code-app",
            "-Users-x-code-playground-x",
            "x-Users-x-playground-a",
            "-Users-xy-playground-a",
            "",
        ] {
            assert!(!is_under(bad, root), "{bad}");
        }
        assert!(!is_under("-Users-x-playground", ""), "no root, nothing");
        let d = crate::testkit::tempdir().unwrap();
        let canon = fs::canonicalize(d.path()).unwrap();
        assert_eq!(playground_key(d.path()), key(&canon.join("playground")));
        assert!(playground_key(d.path()).ends_with("-playground"));
    }
}
