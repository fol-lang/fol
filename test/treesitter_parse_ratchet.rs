//! The tree-sitter grammar is a hand-written second parser, so it drifts from
//! the compiler's silently — nothing in the suite parsed a `.fol` file with it
//! until this test. Every file that still produces `ERROR` nodes is listed in
//! `tree-sitter/test/known-parse-gaps.txt`, and that list is a **ratchet**: a new
//! entry fails the build, and a fixed entry must be removed from the list.
//!
//! Regenerating the grammar cannot hide a regression behind "it still compiles".

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn known_gaps(root: &Path) -> BTreeSet<String> {
    let listing = root.join("lang/tooling/fol-editor/tree-sitter/test/known-parse-gaps.txt");
    std::fs::read_to_string(&listing)
        .expect("the known-parse-gaps listing should be readable")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Every `.fol` file worth parsing, as a repo-relative path.
fn fol_sources(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack: Vec<PathBuf> = ["examples", "lang/library", "test", "book"]
        .iter()
        .map(|dir| root.join(dir))
        .collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `.fol` build directories hold generated copies of the sources.
                if path.file_name().is_some_and(|name| name == ".fol") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "fol") {
                if let Ok(relative) = path.strip_prefix(root) {
                    found.push(relative.to_string_lossy().into_owned());
                }
            }
        }
    }
    found.sort();
    found
}

#[test]
fn treesitter_parse_ratchet() {
    let root = repo_root();
    let listed = known_gaps(&root);

    let mut newly_broken = Vec::new();
    let mut newly_fixed = Vec::new();
    for relative in fol_sources(&root) {
        let Ok(summary) = fol_editor::editor_parse_file(&root.join(&relative)) else {
            newly_broken.push(relative);
            continue;
        };
        // `editor_parse_file` reports `error_count=N` for the tree-sitter parse.
        let has_error = !summary
            .details
            .iter()
            .any(|detail| detail == "error_count=0");
        match (has_error, listed.contains(&relative)) {
            (true, false) => newly_broken.push(relative),
            (false, true) => newly_fixed.push(relative),
            _ => {}
        }
    }

    assert!(
        newly_broken.is_empty(),
        "the grammar no longer parses these files:\n  {}",
        newly_broken.join("\n  ")
    );
    assert!(
        newly_fixed.is_empty(),
        "these now parse — remove them from tree-sitter/test/known-parse-gaps.txt:\n  {}",
        newly_fixed.join("\n  ")
    );
}
