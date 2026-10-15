//! Test folders with a private parent. A queue's project is its parent folder, and every repo
//! beside the queue is one of its checkouts (`backlog::project_places`). A bare
//! `tempfile::tempdir()` sits straight in the shared temp folder, so other sessions' checkouts
//! there would count as the test queue's project. Every test folder here is `<tmp>/proj/q`, and
//! `<tmp>/proj` holds nothing else. A test that wants checkouts beside its queue plants them in
//! `path().parent()`.

use std::path::{Path, PathBuf};

/// A test folder at `<tmp>/proj/q`; the whole `<tmp>` goes when it's dropped.
#[derive(Debug)]
pub(crate) struct TestDir {
    _tmp: tempfile::TempDir,
    path: PathBuf,
}

impl TestDir {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// A new empty folder with a private parent, in place of `tempfile::tempdir()`.
pub(crate) fn tempdir() -> std::io::Result<TestDir> {
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("proj").join("q");
    std::fs::create_dir_all(&path)?;
    Ok(TestDir { _tmp: tmp, path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backlog, now, ops, queue};
    use std::fs;

    /// A Muster checkout with a now page covering everything, planted in `dir`.
    fn decoy(dir: &Path) -> PathBuf {
        let c = dir.join("decoy");
        fs::create_dir_all(c.join(".git")).unwrap();
        fs::create_dir_all(c.join("docs/now")).unwrap();
        fs::write(
            c.join("docs/now/all.md"),
            "---\ncovers: app/, docs/, bin/\n---\n# all\n- x\n",
        )
        .unwrap();
        c
    }

    fn queue_in(d: &TestDir) {
        for f in queue::STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "---\nprefix: pj\n---\nq").unwrap();
    }

    #[test]
    fn the_folder_is_empty_and_its_parent_holds_only_it() {
        let d = tempdir().unwrap();
        assert!(d.path().is_dir());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
        let parent = d.path().parent().unwrap();
        let in_parent: Vec<PathBuf> = fs::read_dir(parent)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(in_parent, vec![d.path().to_path_buf()]);
        let tmp = parent.parent().unwrap().to_path_buf();
        drop(d);
        assert!(!tmp.exists());
    }

    #[test]
    fn a_checkout_beside_the_private_parent_is_not_the_queues_project() {
        let d = tempdir().unwrap();
        let parent = d.path().parent().unwrap();
        decoy(parent.parent().unwrap());
        queue_in(&d);
        assert_eq!(
            backlog::project_places(d.path()),
            vec![parent.canonicalize().unwrap()]
        );
        assert!(now::places(d.path()).is_empty());
        let id = ops::create_task(
            d.path(),
            &ops::NewTask {
                title: "Touch the app".into(),
                touches: "app/src/x.ts".into(),
                ..Default::default()
            },
            "owner",
            "2026-10-15 09:00",
        )
        .unwrap()
        .id;
        let t = queue::read_task(d.path(), &id).unwrap();
        assert_eq!(t.touches, vec!["app/src/x.ts".to_string()]);
        assert!(t.worktree.is_empty());
        assert_eq!(now::pass_check(d.path(), &t), now::Pass::Free);
    }

    #[test]
    fn a_checkout_planted_in_the_private_parent_is_the_queues_project() {
        let d = tempdir().unwrap();
        let parent = d.path().parent().unwrap();
        let c = decoy(parent);
        assert_eq!(
            backlog::project_places(d.path()),
            vec![parent.canonicalize().unwrap(), c.canonicalize().unwrap()]
        );
    }

    /// Where `tempfile` is named in the `.rs` files under `dir`, outside the two helpers
    /// (`testkit.rs` here, `common/mod.rs` for the integration tests).
    fn bare_tempdirs(dir: &Path) -> Vec<String> {
        // Split, so this file's own patterns aren't what they look for.
        let banned = [
            concat!("tempfile", "::tempdir("),
            concat!("tempfile", "::TempDir"),
            concat!("tempfile", "::Builder"),
            concat!("TempDir", "::new"),
            concat!("use ", "tempfile"),
        ];
        let mut files = Vec::new();
        let mut dirs = vec![dir.to_path_buf()];
        while let Some(d) = dirs.pop() {
            for e in fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") {
                    files.push(p);
                }
            }
        }
        files.sort();
        let mut out = Vec::new();
        for f in files {
            let rel = f.strip_prefix(dir).unwrap();
            if rel == Path::new("testkit.rs") || rel == Path::new("common/mod.rs") {
                continue;
            }
            let text = fs::read_to_string(&f).unwrap();
            for (n, line) in text.lines().enumerate() {
                if banned.iter().any(|b| line.contains(b)) {
                    out.push(format!("{}:{}: {}", rel.display(), n + 1, line.trim()));
                }
            }
        }
        out
    }

    #[test]
    fn test_queues_have_a_private_parent() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let found: Vec<String> = ["src", "tests"]
            .iter()
            .flat_map(|d| {
                bare_tempdirs(&crate_dir.join(d))
                    .into_iter()
                    .map(move |f| format!("{d}/{f}"))
            })
            .collect();
        assert!(
            found.is_empty(),
            "use crate::testkit::tempdir() (tests/: common::tempdir()), not a bare temp folder:\n{}",
            found.join("\n")
        );
    }

    #[test]
    fn the_source_check_finds_a_bare_tempdir() {
        let d = tempdir().unwrap();
        fs::write(
            d.path().join("x.rs"),
            concat!("let d = tempfile", "::tempdir().unwrap();\n"),
        )
        .unwrap();
        fs::write(
            d.path().join("testkit.rs"),
            concat!("tempfile", "::tempdir()\n"),
        )
        .unwrap();
        fs::write(
            d.path().join("y.rs"),
            "let d = crate::testkit::tempdir().unwrap();\n",
        )
        .unwrap();
        // The integration tests' helper may name it; a file beside it, or below it, may not.
        fs::create_dir_all(d.path().join("common/deeper")).unwrap();
        fs::write(
            d.path().join("common/mod.rs"),
            concat!("use ", "tempfile;\n"),
        )
        .unwrap();
        fs::write(
            d.path().join("common/deeper/z.rs"),
            concat!("fn q() -> tempfile", "::TempDir {}\n"),
        )
        .unwrap();
        assert_eq!(
            bare_tempdirs(d.path()),
            vec![
                concat!("common/deeper/z.rs:1: fn q() -> tempfile", "::TempDir {}").to_string(),
                concat!("x.rs:1: let d = tempfile", "::tempdir().unwrap();").to_string(),
            ]
        );
    }
}
