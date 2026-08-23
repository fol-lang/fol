//! The dependency prohibition from section 4.1.
//!
//! `fol-abi` is the dependency-foundation crate: both the compiler and the
//! interop stack consume its schema, so anything it depends on becomes a
//! dependency of both. A single convenience import here would quietly couple
//! the two, which is why this is a test rather than a comment.

use std::path::PathBuf;

/// Crates `fol-abi` may never depend on, directly or transitively.
const FORBIDDEN: &[&str] = &[
    "fol-parser",
    "fol-package",
    "fol-resolver",
    "fol-typecheck",
    "fol-lower",
    "fol-backend",
    "fol-build",
    "fol-frontend",
    "fol-editor",
    "follang-parc",
    "follang-linc",
    "follang-gerc",
];

fn manifest() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(path).expect("fol-abi's manifest should be readable")
}

#[test]
fn fol_abi_depends_only_on_fol_types() {
    let text = manifest();
    let dependencies = text
        .split("[dependencies]")
        .nth(1)
        .expect("fol-abi should declare a dependencies section");
    // Stop at the next section header, so dev-dependencies are not counted.
    let dependencies = dependencies.split("\n[").next().unwrap_or(dependencies);

    let declared: Vec<&str> = dependencies
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .collect();

    assert_eq!(
        declared,
        vec!["fol-types"],
        "fol-abi may depend only on fol-types (section 4.1)"
    );
}

/// The prohibition holds transitively: `fol-types` must not drag any of them in.
#[test]
fn the_prohibition_holds_through_fol_types() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fol-types/Cargo.toml")
        .canonicalize()
        .expect("fol-types should be a sibling");
    let text = std::fs::read_to_string(path).expect("fol-types' manifest should be readable");

    for forbidden in FORBIDDEN {
        assert!(
            !text.contains(forbidden),
            "fol-types gained a dependency on {forbidden}, which reaches fol-abi transitively"
        );
    }
}

/// The crate is registered in the workspace, or `cargo test` at the root would
/// skip its suite entirely.
#[test]
fn fol_abi_is_a_workspace_default_member() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Cargo.toml")
        .canonicalize()
        .expect("the workspace root should exist");
    let text = std::fs::read_to_string(root).expect("the workspace manifest should be readable");
    assert!(
        text.contains("\"lang/compiler/fol-abi\""),
        "fol-abi is not a workspace member"
    );
    // Section 9's primary-files note: registration only, no version change.
    assert!(
        text.contains("version = \"0.2.6\""),
        "the workspace version changed; M4 registers fol-abi without touching it"
    );
}
