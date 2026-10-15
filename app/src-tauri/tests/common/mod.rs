//! Test folders with a private parent, for the integration tests (the crate's own tests use
//! `testkit`, which these can't see). A queue's project is its parent folder, and every repo
//! beside the queue is one of its checkouts (`backlog::project_places`), so a queue in a bare
//! `tempfile::tempdir()` would take other sessions' checkouts in the shared temp folder as its
//! own and read their now pages. Every folder here is `<tmp>/proj/q`, and `<tmp>/proj` holds
//! nothing else.

use std::path::{Path, PathBuf};

/// A test folder at `<tmp>/proj/q`; the whole `<tmp>` goes when it's dropped.
#[derive(Debug)]
pub struct TestDir {
    _tmp: tempfile::TempDir,
    path: PathBuf,
}

impl TestDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A new empty folder with a private parent, in place of `tempfile::tempdir()`.
pub fn tempdir() -> std::io::Result<TestDir> {
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("proj").join("q");
    std::fs::create_dir_all(&path)?;
    Ok(TestDir { _tmp: tmp, path })
}
