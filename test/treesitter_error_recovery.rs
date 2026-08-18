//! Editors spend most of their time on half-typed code, so how the grammar
//! behaves on *broken* input matters as much as how it behaves on valid input.
//! The failure mode that hurts is a single stray token collapsing the file:
//! the outline empties, folding dies, and symbol search goes blank for
//! declarations that are still perfectly well formed.
//!
//! This applies one local typo to every real source file and asserts the rest
//! of the file survives it.

use std::path::{Path, PathBuf};

/// Measured over every tracked source: a missing terminator and a half-written
/// binding cost nothing anywhere, and only an unclosed delimiter costs anything
/// at all — ten files lose one declaration and one loses two. An unclosed `(`
/// genuinely puts the remainder of the file inside the call, so some loss there
/// is honest. Past this, recovery has regressed rather than merely changed shape.
const MAX_DECLARATIONS_LOST: usize = 2;

const DECLARATIONS: &[&str] = &[
    "fun_decl", "pro_decl", "log_decl", "typ_decl", "ali_decl", "def_decl", "seg_decl", "std_decl",
    "use_decl", "var_decl", "con_decl",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Real, valid FOL — mutating anything already broken proves nothing.
fn sources(root: &Path) -> Vec<PathBuf> {
    let listing = std::process::Command::new("git")
        .args(["ls-files", "-z", "--", "*.fol"])
        .current_dir(root)
        .output()
        .expect("git should list the tracked sources");

    let mut found: Vec<PathBuf> = String::from_utf8_lossy(&listing.stdout)
        .split('\0')
        .filter(|path| !path.is_empty())
        .filter(|path| {
            ["examples/", "lang/library/", "test/apps/showcases/"]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .filter(|path| !path.contains("/fail_") && !path.contains("/.fol/"))
        .map(|path| root.join(path))
        .collect();
    found.sort();
    found
}

/// Drop the first statement terminator, drop the first closing paren, and leave
/// a binding half-written — the three shapes a keystroke actually produces.
fn mutations(source: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();

    if let Some(line) = source.lines().position(|line| line.ends_with(';')) {
        let mutated: Vec<String> = source
            .lines()
            .enumerate()
            .map(|(index, text)| {
                if index == line {
                    text.trim_end_matches(';').to_string()
                } else {
                    text.to_string()
                }
            })
            .collect();
        out.push(("missing terminator", mutated.join("\n")));
    }

    if let Some(index) = source.find(')') {
        let mut mutated = source.to_string();
        mutated.remove(index);
        out.push(("unclosed call", mutated));
    }

    let lines: Vec<&str> = source.lines().collect();
    if lines.len() > 4 {
        let mut mutated = lines.clone();
        mutated.insert(lines.len() / 2, "    var partial = ");
        out.push(("half-written binding", mutated.join("\n")));
    }

    out
}

/// Count the declarations that are direct children of `source_file`, reading
/// the s-expression the parse summary reports.
fn top_level_declarations(tree: &str) -> usize {
    let mut depth = 0usize;
    let mut found = 0usize;
    let mut quoted = false;
    let bytes = tree.as_bytes();

    for (index, byte) in bytes.iter().enumerate() {
        // Recovery renders an inserted token as `(MISSING ")")`, so parens
        // inside quotes are text, not structure.
        if *byte == b'"' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match byte {
            b'(' => {
                depth += 1;
                if depth == 2 {
                    let name: String = tree[index + 1..]
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_alphanumeric() || *character == '_'
                        })
                        .collect();
                    if DECLARATIONS.contains(&name.as_str()) {
                        found += 1;
                    }
                }
            }
            b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    found
}

/// Each probe gets its own file: reusing one path across parses lets a stale
/// result stand in for the source actually under test.
fn parse_tree(directory: &Path, source: &str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROBE: AtomicUsize = AtomicUsize::new(0);

    let path = directory.join(format!(
        "probe_{}.fol",
        PROBE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).expect("the probe source should be writable");
    let summary = fol_editor::editor_parse_file(&path).expect("the probe should parse");
    summary
        .details
        .iter()
        .find_map(|detail| detail.strip_prefix("syntax_tree=").map(str::to_string))
        .expect("the parse summary should report a syntax tree")
}

#[test]
fn a_single_typo_does_not_collapse_the_rest_of_the_file() {
    let root = repo_root();
    let scratch = crate::fixture::TempFixture::new("fol_treesitter_error_recovery");
    std::fs::create_dir_all(&scratch).expect("the scratch directory should exist");

    let mut checked = 0usize;
    let mut collapsed = Vec::new();

    for source_path in sources(&root) {
        let Ok(source) = std::fs::read_to_string(&source_path) else {
            continue;
        };
        let intact = top_level_declarations(&parse_tree(&scratch, &source));
        if intact == 0 {
            continue;
        }

        for (label, mutated) in mutations(&source) {
            let recovered = top_level_declarations(&parse_tree(&scratch, &mutated));
            checked += 1;
            if intact.saturating_sub(recovered) > MAX_DECLARATIONS_LOST {
                collapsed.push(format!(
                    "{}: {label} dropped {} of {intact} declarations",
                    source_path
                        .strip_prefix(&root)
                        .unwrap_or(&source_path)
                        .display(),
                    intact - recovered
                ));
            }
        }
    }

    assert!(checked > 100, "the probe corpus should be substantial");
    assert!(
        collapsed.is_empty(),
        "one typo should not erase the surrounding declarations:\n  {}",
        collapsed.join("\n  ")
    );
}
