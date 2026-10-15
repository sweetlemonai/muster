//! No conflict markers left in the docs (mu-209): a merge resolved by hand that kept a `<<<<<<< `,
//! `||||||| `, `=======` or `>>>>>>> ` line turns the gate red. It reads every `docs/**/*.md` and
//! top-level `*.md` of this repo with `merge_log::stray_markers`, which skips fenced code, where
//! docs may quote markers.

mod common;

use muster_lib::merge_log::stray_markers;
use std::fs;
use std::path::{Path, PathBuf};

/// The `.md` files under `dir`, every level down, symlinks not followed, sorted.
fn md_files(dir: &Path, out: &mut Vec<PathBuf>, deep: bool) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(kind) = e.file_type() else { continue };
        let path = e.path();
        if kind.is_dir() && deep {
            md_files(&path, out, true);
        } else if kind.is_file() && path.extension().is_some_and(|x| x == "md") {
            out.push(path);
        }
    }
}

/// Every marker line in `root`'s docs, as `<path>:<line>: <text>`, and how many files it read.
fn markers_in(root: &Path) -> (Vec<String>, usize) {
    let mut files = Vec::new();
    md_files(root, &mut files, false);
    md_files(&root.join("docs"), &mut files, true);
    let mut found = Vec::new();
    for f in &files {
        let text = fs::read(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
        for n in stray_markers(&text) {
            let shown = String::from_utf8_lossy(lines[n - 1]);
            let name = f.strip_prefix(root).unwrap_or(f);
            found.push(format!("{}:{n}: {}", name.display(), shown.trim_end()));
        }
    }
    (found, files.len())
}

#[test]
fn conflict_markers_none_in_the_repos_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (found, files) = markers_in(&root);
    assert!(
        files >= 20,
        "only {files} .md files read: the wrong folder?"
    );
    assert!(
        found.is_empty(),
        "conflict markers left from a merge:\n{}",
        found.join("\n")
    );
}

#[test]
fn conflict_markers_found_in_top_level_and_nested_docs_not_in_code() {
    let dir = common::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("docs/now/deep")).unwrap();
    fs::write(root.join("README.md"), "# R\n<<<<<<< ours\n").unwrap();
    fs::write(
        root.join("docs/WORKLOG.md"),
        "## 2026-10-15 — A\n||||||| base\n",
    )
    .unwrap();
    fs::write(
        root.join("docs/now/deep/x.md"),
        "a\n=======\n>>>>>>> theirs\n",
    )
    .unwrap();
    // Quoted in a fence, a setext rule of another length, and files that aren't .md or top-level.
    fs::write(
        root.join("docs/quoted.md"),
        "```\n<<<<<<< ours\n=======\n```\nTitle\n========\n",
    )
    .unwrap();
    fs::write(root.join("docs/notes.txt"), "<<<<<<< ours\n").unwrap();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/README.md"), "<<<<<<< ours\n").unwrap();
    let (found, files) = markers_in(root);
    assert_eq!(files, 4);
    assert_eq!(
        found,
        [
            "README.md:2: <<<<<<< ours",
            "docs/WORKLOG.md:2: ||||||| base",
            "docs/now/deep/x.md:2: =======",
            "docs/now/deep/x.md:3: >>>>>>> theirs",
        ]
    );
}
