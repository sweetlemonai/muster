//! The app's commands for the accounts and their use (task 0113, `profiles` and `usage`): the
//! header's meter and Settings › Accounts.
//!
//! Refresh: the profiles and each account's latest reading are kept in memory (`Accounts`) and
//! read again only when accounts.json, settings.json (the allowlist seeds the profiles) or a file
//! in usage/ changed, by size or modification time. The window's 2-second state poll compares
//! those few file times off the main thread (`notify`) and emits `accounts-changed` when they
//! changed since it last looked, so the window refetches `accounts_info` then and never polls it.
//! The status line writes at most one line per change of use (`usage::record`), and the poll's
//! interval is the debounce. The app has no file watcher to reuse, and the poll runs anyway.

use crate::profiles::{self, Profile};
use crate::usage::{self, Reading};
use crate::{accounts, blocking, home, settings, AppState};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use tauri::{Emitter, Manager};

/// The event the window refetches the accounts on.
pub(crate) const CHANGED: &str = "accounts-changed";

/// The files the accounts are read from, by name, each with its size and modification time.
type Stamp = Vec<(String, u64, u128)>;

/// The profiles with their latest readings, and the stamp they were read at.
type Snapshot = (Stamp, Vec<(Profile, Option<Reading>)>);

#[derive(Default)]
pub(crate) struct Accounts {
    /// The stamp the window was last told about; None before the first look.
    told: Mutex<Option<Stamp>>,
    read: Mutex<Option<Snapshot>>,
}

/// accounts.json, settings.json and every file in usage/ in the app's config folder `dir`.
fn stamp(dir: &Path) -> Stamp {
    let one = |p: PathBuf, out: &mut Stamp| {
        if let Ok(m) = fs::metadata(&p) {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            out.push((name.into_owned(), m.len(), mtime));
        }
    };
    let mut out = Stamp::new();
    one(dir.join(profiles::FILE), &mut out);
    one(dir.join(settings::FILE), &mut out);
    if let Ok(entries) = fs::read_dir(dir.join(usage::DIR)) {
        let mut files: Stamp = Vec::new();
        for e in entries.flatten() {
            one(e.path(), &mut files);
        }
        files.sort();
        out.extend(
            files
                .into_iter()
                .map(|(n, l, m)| (format!("usage/{n}"), l, m)),
        );
    }
    out
}

impl Accounts {
    /// Have the files in `dir` changed since the last look? The first look says yes, so a change
    /// between the window's first fetch and its first poll isn't missed.
    pub(crate) fn changed(&self, dir: &Path) -> bool {
        let now = stamp(dir);
        let Ok(mut told) = self.told.lock() else {
            return false;
        };
        if told.as_ref() == Some(&now) {
            return false;
        }
        *told = Some(now);
        true
    }

    /// The accounts in `dir` judged at `now`, from memory unless the files changed;
    /// `allowed` seeds the profiles when there's no accounts.json.
    pub(crate) fn info(
        &self,
        dir: &Path,
        allowed: impl FnOnce() -> Vec<accounts::Account>,
        now: i64,
    ) -> Result<profiles::Info, String> {
        let s = stamp(dir);
        let mut read = self.read.lock().map_err(|e| e.to_string())?;
        let fresh = matches!(&*read, Some((at, _)) if *at == s);
        if !fresh {
            *read = Some((s, profiles::read(dir, &allowed(), now)));
        }
        let accounts = read.as_ref().map(|(_, a)| a.as_slice()).unwrap_or_default();
        Ok(profiles::info_of(accounts, now))
    }
}

/// The app's config folder: accounts.json and usage/ are beside settings.json.
fn config_dir(app: &AppState) -> Result<PathBuf, String> {
    app.settings_file
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "the app has no config folder".to_string())
}

fn allowed(app: &AppState) -> Vec<accounts::Account> {
    accounts::allowed(&settings::load(&app.settings_file), &home())
}

/// Tell the window when the accounts' files changed since it was last told (`accounts-changed`).
/// The state poll calls it, off the main thread, and so does every change here.
pub(crate) fn notify(handle: &tauri::AppHandle) {
    let app = handle.state::<AppState>();
    if let Ok(dir) = config_dir(&app) {
        if app.accounts.changed(&dir) {
            let _ = handle.emit(CHANGED, ());
        }
    }
}

/// Every account with its latest reading (`profiles::Info`), for the header's meter and
/// Settings › Accounts.
#[tauri::command]
pub(crate) async fn accounts_info(handle: tauri::AppHandle) -> Result<profiles::Info, String> {
    blocking(move || {
        let app = handle.state::<AppState>();
        let dir = config_dir(&app)?;
        let now = crate::queue::now_secs() as i64;
        app.accounts.info(&dir, || allowed(&app), now)
    })
    .await
}

/// Run `f` on accounts.json under the config lock, then tell the window.
fn change<T>(
    handle: &tauri::AppHandle,
    f: impl FnOnce(&AppState, &Path) -> Result<T, String>,
) -> Result<(), String> {
    let app = handle.state::<AppState>();
    let file = config_dir(&app)?.join(profiles::FILE);
    {
        let _guard = app.config.lock().map_err(|e| e.to_string())?;
        f(&app, &file)?;
    }
    notify(handle);
    Ok(())
}

/// Add the account `label` (`mg` or `cc-mg`) of `provider` (claude or codex), refused with why
/// for the company account and one off the allowlist (`profiles::add`).
#[tauri::command]
pub(crate) async fn account_add(
    handle: tauri::AppHandle,
    label: String,
    provider: String,
) -> Result<(), String> {
    blocking(move || {
        change(&handle, |app, file| {
            let settings = settings::load(&app.settings_file);
            profiles::add(file, &settings, &home(), &label, &provider)
        })
    })
    .await
}

/// Remove the account `label`.
#[tauri::command]
pub(crate) async fn account_remove(handle: tauri::AppHandle, label: String) -> Result<(), String> {
    blocking(move || {
        change(&handle, |app, file| {
            profiles::remove(file, &allowed(app), &label)
        })
    })
    .await
}

/// Save Settings › Accounts' fields of one account, each checked (`profiles::save_edit`).
#[tauri::command]
pub(crate) async fn account_save(
    handle: tauri::AppHandle,
    profile: profiles::Edit,
) -> Result<(), String> {
    blocking(move || {
        change(&handle, |app, file| {
            profiles::save_edit(file, &allowed(app), &profile)
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::tests::{windows, NOW};
    use crate::usage::Source;

    #[test]
    fn a_change_to_the_files_is_told_once_and_the_readings_are_read_again_only_then() {
        let d = crate::testkit::tempdir().unwrap();
        let dir = d.path();
        let a = Accounts::default();
        assert!(a.changed(dir), "the first look");
        assert!(!a.changed(dir), "nothing changed");
        let seeded = || vec![];
        // The allowed accounts, mg and hg: another's readings go on read (usage::prune).
        let home = crate::accounts::tests::fake_home();
        let mine = || accounts::allowed(&settings::Settings::default(), home.path());
        assert!(a.info(dir, seeded, NOW).unwrap().accounts.is_empty());
        // A reading for an account with no profile isn't shown; adding the profile is a change.
        let r = Reading::new("mg", windows(35.0, 41.0), NOW, Source::Statusline);
        usage::record(dir, &r, NOW).unwrap();
        assert!(a.changed(dir), "a new readings file");
        assert!(!a.changed(dir));
        fs::write(
            dir.join(profiles::FILE),
            r#"{"accounts": [{"provider": "claude", "label": "mg"}]}"#,
        )
        .unwrap();
        assert!(a.changed(dir), "accounts.json");
        let info = a.info(dir, mine, NOW).unwrap();
        assert_eq!(
            info.accounts[0].five_hour.as_ref().unwrap().used_percentage,
            35.0
        );
        // From memory until a file changes: a new reading is a change, read on the next ask.
        let r2 = Reading::new("mg", windows(50.0, 41.0), NOW + 1, Source::Stream);
        usage::record(dir, &r2, NOW + 1).unwrap();
        assert!(a.changed(dir), "a new line");
        let info = a.info(dir, mine, NOW + 2).unwrap();
        assert_eq!(
            info.accounts[0].five_hour.as_ref().unwrap().used_percentage,
            50.0
        );
        assert_eq!(info.now, NOW + 2);
        // The same stamp: what's in memory, judged at the new time.
        let later = a.info(dir, mine, 1_792_035_600).unwrap();
        assert!(later.accounts[0].five_hour.as_ref().unwrap().stale);
        assert!(!a.changed(dir));
        // With no accounts.json the profiles are seeded: once per change, not once per ask.
        fs::remove_file(dir.join(profiles::FILE)).unwrap();
        let calls = std::cell::Cell::new(0);
        let seed = || {
            calls.set(calls.get() + 1);
            mine()
        };
        a.info(dir, seed, NOW).unwrap();
        a.info(dir, seed, NOW).unwrap();
        assert_eq!(calls.get(), 1, "read once, then from memory");
    }
}
