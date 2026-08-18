//! The parser is generated from `tree-sitter/src/grammar.json` at build time
//! rather than committed. The generated `parser.c` is tens of megabytes and a
//! fresh blob on every grammar edit, so only the grammar itself is tracked.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Must match `REQUIRED_TREE_SITTER_VERSION` in `src/commands.rs`; a different
/// CLI series emits a different parser ABI.
const REQUIRED_TREE_SITTER_VERSION: &str = "0.26";

const GRAMMAR: &str = "tree-sitter/src/grammar.json";
const CONFIG: &str = "tree-sitter.json";

fn fail(message: &str) -> ! {
    println!("cargo:warning={message}");
    panic!("{message}");
}

fn cli_version() -> Option<String> {
    let output = Command::new("tree-sitter").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
}

fn stage(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| fail(&format!("cannot create {}: {error}", parent.display())));
    }
    std::fs::copy(from, to)
        .unwrap_or_else(|error| fail(&format!("cannot stage {}: {error}", from.display())));
}

fn generate(root: &Path) {
    match cli_version() {
        Some(version) if version.starts_with(REQUIRED_TREE_SITTER_VERSION) => {}
        Some(version) => fail(&format!(
            "tree-sitter CLI {version} cannot generate this parser; \
             {REQUIRED_TREE_SITTER_VERSION}.x is required — \
             cargo install tree-sitter-cli --version 0.26.8 --locked"
        )),
        None => fail(
            "the `tree-sitter` CLI is required to build fol-editor; \
             install it with: cargo install tree-sitter-cli --version 0.26.8 --locked",
        ),
    }

    let status = Command::new("tree-sitter")
        .arg("generate")
        .arg("src/grammar.json")
        .current_dir(root)
        .status()
        .unwrap_or_else(|error| fail(&format!("failed to run `tree-sitter generate`: {error}")));

    if !status.success() {
        fail(&format!(
            "`tree-sitter generate` failed with status {status}"
        ));
    }
}

fn main() {
    let out = PathBuf::from(
        std::env::var_os("OUT_DIR").unwrap_or_else(|| fail("cargo did not provide OUT_DIR")),
    );
    let staged_src = out.join("src");

    // The CLI reads the grammar and its ABI config from the directory it runs in.
    stage(Path::new(GRAMMAR), &staged_src.join("grammar.json"));
    stage(&Path::new("tree-sitter").join(CONFIG), &out.join(CONFIG));
    generate(&out);

    cc::Build::new()
        .include(&staged_src)
        .file(staged_src.join("parser.c"))
        .warnings(false)
        .compile("tree-sitter-fol");

    println!("cargo:rerun-if-changed={GRAMMAR}");
    println!("cargo:rerun-if-changed=tree-sitter/{CONFIG}");
}
