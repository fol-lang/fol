//! The book's `fol` code blocks are never compiled, so they rot silently as the
//! language moves. These lock the failure modes that have actually happened: a
//! stale `std::` module path left behind by a library change, an `ali` without
//! its terminator, and a routine exported from `std` that no chapter lists. Each
//! was a real defect, found by extracting all 306 blocks and type-checking them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every markdown file under `book/src`.
fn book_pages(root: &Path) -> Vec<PathBuf> {
    let mut pages = Vec::new();
    let mut stack = vec![root.join("book/src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                pages.push(path);
            }
        }
    }
    pages.sort();
    pages
}

/// The lines inside ```fol fences, paired with their 1-based line number.
fn fol_block_lines(text: &str) -> Vec<(usize, String)> {
    let mut inside = false;
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_end();
        if trimmed == "```fol" {
            inside = true;
            continue;
        }
        if trimmed == "```" && inside {
            inside = false;
            continue;
        }
        if inside {
            lines.push((index + 1, line.to_string()));
        }
    }
    lines
}

fn page_label(root: &Path, page: &Path) -> String {
    page.strip_prefix(root)
        .unwrap_or(page)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn book_code_names_only_std_modules_that_exist() {
    let root = repo_root();
    let std_root = root.join("lang/library/std");
    let available: BTreeSet<String> = std::fs::read_dir(&std_root)
        .expect("the bundled std library should be readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        available.contains("fmt"),
        "sanity: std should expose an 'fmt' module, found {available:?}"
    );

    let mut stale = Vec::new();
    for page in book_pages(&root) {
        let text = std::fs::read_to_string(&page).expect("book page should be readable");
        // Every line, not just ```fol blocks: plenty of book code sits in plain
        // ``` fences, and a `std::` path named in prose has to be right too.
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            for module in referenced_std_modules(line) {
                if !available.contains(&module) {
                    stale.push(format!(
                        "{}:{line_number} names std::{module}, which is not a module of the bundled std",
                        page_label(&root, &page)
                    ));
                }
            }
        }
    }
    assert!(
        stale.is_empty(),
        "book code references std modules that do not exist:\n{}",
        stale.join("\n")
    );
}

/// Every `std::<module>::<routine>` path on one line, as (module, routine).
fn referenced_std_routines(line: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("std::") {
        rest = &rest[at + "std::".len()..];
        let module: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let after = &rest[module.len()..];
        let Some(tail) = after.strip_prefix("::") else {
            continue;
        };
        let routine: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        // A third `::` means a nested path, which the module test already reports.
        if !module.is_empty() && !routine.is_empty() && !tail[routine.len()..].starts_with("::") {
            found.push((module, routine));
        }
    }
    found
}

#[test]
fn book_code_names_only_std_routines_that_exist() {
    let root = repo_root();
    let mut stale = Vec::new();
    for page in book_pages(&root) {
        let text = std::fs::read_to_string(&page).expect("book page should be readable");
        for (index, line) in text.lines().enumerate() {
            // Rust's own `std::` paths appear when the book describes what a FOL
            // surface is built on; those name Rust items, not FOL ones.
            if line.contains("Rust's") {
                continue;
            }
            for (module, routine) in referenced_std_routines(line) {
                let dir = root.join("lang/library/std").join(&module);
                if !dir.is_dir() {
                    continue; // the module test owns this case
                }
                let declared = std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "fol"))
                    .any(|entry| {
                        let source = std::fs::read_to_string(entry.path()).unwrap_or_default();
                        source.lines().any(|declaration| {
                            let declaration = declaration.trim_start();
                            ["fun", "pro", "log", "con", "typ", "ali"]
                                .iter()
                                .any(|kind| {
                                    declaration.starts_with(kind)
                                        && declaration.contains(&format!(" {routine}("))
                                        || declaration.starts_with(kind)
                                            && declaration.contains(&format!(" {routine}:"))
                                })
                        })
                    });
                if !declared {
                    stale.push(format!(
                        "{}:{} names std::{module}::{routine}, which that module does not declare",
                        page_label(&root, &page),
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(
        stale.is_empty(),
        "book code references std routines that do not exist:\n{}",
        stale.join("\n")
    );
}

/// The module segments of each `std::...` path on one line.
///
/// A path is `std::<module>::<routine>`: bundled std is flat, so every segment
/// before the routine is a module and there is only ever one. Returning the whole
/// chain is what catches `std::fmt::math::answer()`, where the first segment
/// exists and the second does not — checking only the first segment misses it.
fn referenced_std_modules(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("std::") {
        rest = &rest[at + "std::".len()..];
        loop {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let after = &rest[name.len()..];
            // A segment followed by `::` is a module; the one that is not is the
            // routine, and ends this path.
            if name.is_empty() || !after.starts_with("::") {
                break;
            }
            found.push(name);
            rest = &after["::".len()..];
        }
    }
    found
}

#[test]
fn book_code_terminates_alias_declarations() {
    let root = repo_root();
    let mut unterminated = Vec::new();
    for page in book_pages(&root) {
        let text = std::fs::read_to_string(&page).expect("book page should be readable");
        for (line_number, line) in fol_block_lines(&text) {
            let trimmed = line.trim();
            // A declaration names a type: `ali Failure: err[str];`. A bare `ali`
            // followed by a comment is the keyword vocabulary table, not code.
            let is_alias = (trimmed.starts_with("ali ") || trimmed.starts_with("ali["))
                && trimmed
                    .split("//")
                    .next()
                    .is_some_and(|code| code.contains(':'));
            if is_alias && !trimmed.ends_with(';') && !trimmed.ends_with('{') {
                unterminated.push(format!(
                    "{}:{line_number}  {trimmed}",
                    page_label(&root, &page)
                ));
            }
        }
    }
    assert!(
        unterminated.is_empty(),
        "an `ali` declaration without its `;` does not parse:\n{}",
        unterminated.join("\n")
    );
}

/// Every routine `std` exports has to appear in the library chapter, or it ships
/// with no way for a reader to discover it. The chapter lists them as signatures,
/// so the name alone is what this looks for.
#[test]
fn the_library_chapter_lists_every_exported_std_routine() {
    let root = repo_root();

    let mut documented = BTreeSet::new();
    let chapter = root.join("book/src/625_library");
    for entry in std::fs::read_dir(&chapter)
        .expect("the library chapter should be readable")
        .flatten()
    {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim_start();
            for kind in ["fun[exp] ", "pro[exp] "] {
                if let Some(rest) = line.strip_prefix(kind) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        documented.insert(name);
                    }
                }
            }
        }
    }

    let mut missing = Vec::new();
    let std_root = root.join("lang/library/std");
    for module in std::fs::read_dir(&std_root)
        .expect("the bundled std library should be readable")
        .flatten()
        .filter(|entry| entry.path().is_dir())
    {
        for source in std::fs::read_dir(module.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if source.path().extension().is_none_or(|ext| ext != "fol") {
                continue;
            }
            let text = std::fs::read_to_string(source.path()).unwrap_or_default();
            for line in text.lines() {
                for kind in ["fun[exp] ", "pro[exp] "] {
                    if let Some(rest) = line.strip_prefix(kind) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() && !documented.contains(&name) {
                            missing.push(format!(
                                "std::{}::{name}",
                                module.file_name().to_string_lossy()
                            ));
                        }
                    }
                }
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these std routines are exported but never listed in book/src/625_library:\n  {}",
        missing.join("\n  ")
    );
}

// There is deliberately no test that every `pkg = {...}` target is quoted:
// `600_modules/100_import.md` shows an unquoted one on purpose, under the line
// "Old unquoted targets are invalid and should fail in the parser". A lint for
// it would report the book's own counter-example as a defect.
