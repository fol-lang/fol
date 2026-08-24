//! M7.4's linear resource discipline, proven by building and running FOL.
//!
//! The capability exists because a C resource's release can fail and the
//! failure means something. `fin` cannot carry that failure anywhere -- a
//! scope-exit finalizer has no caller waiting on a result -- so a `lin` value
//! is never finalized implicitly. It must be consumed exactly once, explicitly,
//! on every path.
//!
//! Every test here runs the real compiler over a checked-in example rather than
//! asserting on an internal structure, because the claim being made is about
//! what a FOL program is allowed to say.
//!
//! Run by `make test-v4-linear`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn store_root() -> PathBuf {
    repo_root().join("lang/library")
}

fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("destination should be creatable");
    for entry in std::fs::read_dir(from).expect("source should be readable") {
        let entry = entry.expect("entry should be readable");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            if entry.file_name() == ".fol" {
                continue;
            }
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("file should copy");
        }
    }
}

/// Run one subcommand against a copy of a checked-in example.
fn run_example(fixture: &Path, example: &str, subcommand: &str) -> (bool, String) {
    let root = fixture.join(example);
    copy_dir(&repo_root().join("examples").join(example), &root);

    let output = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["code", subcommand, "--package-store-root"])
        .arg(store_root())
        .current_dir(&root)
        .output()
        .expect("the compiler should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    (output.status.success(), text)
}

/// Both consumers work, and the release failure is readable.
///
/// The example releases one handle with a consuming call whose error lands in
/// an ordinary recoverable result, and a second with `[fin]`, whose finalizer
/// prints 0 -- what "the failure was thrown away" looks like from outside. That
/// the two are spelled differently is the point: the unsafe choice is in the
/// source and greppable.
#[test]
fn a_linear_handle_is_released_by_a_call_or_by_an_explicit_discard() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_linear_ok");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let (ok, output) = run_example(fixture.path(), "v4_linear_handle", "run");
    assert!(ok, "the linear example should build and run:\n{output}");
    let printed: Vec<&str> = output
        .lines()
        .filter(|line| matches!(line.trim(), "0" | "1" | "3"))
        .collect();
    assert_eq!(
        printed,
        vec!["3", "0", "1"],
        "the close result, then the discarded finalizer, then the final marker:\n{output}"
    );
}

/// A scope cannot end still holding one.
#[test]
fn an_unreleased_linear_resource_is_refused() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_linear_leak");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let (ok, output) = run_example(fixture.path(), "fail_v4_linear_leak", "check");
    assert!(!ok, "a leaked linear resource must be refused:\n{output}");
    assert!(
        output.contains("abandon the linear resource 'handle'"),
        "the diagnostic must name the resource:\n{output}"
    );
}

/// `report` while holding one is refused, and the reason is stated.
///
/// This is resolution 4 of the decision record, chosen because it refuses the
/// case it cannot represent honestly rather than silently picking which of two
/// errors to lose. The diagnostic has to say that, or the rule reads as
/// arbitrary.
#[test]
fn reporting_while_holding_a_linear_resource_is_refused_with_its_reason() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_linear_report");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let (ok, output) = run_example(fixture.path(), "fail_v4_linear_report", "check");
    assert!(!ok, "reporting while holding must be refused:\n{output}");
    assert!(
        output.contains("cannot report while holding the linear resource 'handle'"),
        "the diagnostic must name the resource:\n{output}"
    );
    assert!(
        output.contains("only one error can be returned"),
        "the diagnostic must say why, not just that:\n{output}"
    );
    assert!(
        output.contains("'handle' is acquired here and still held"),
        "the diagnostic must point at the acquisition too, or the rule reads as arbitrary:\n{output}"
    );
}

/// Branches must agree on whether the resource is still held.
#[test]
fn a_release_on_only_one_branch_is_refused() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_linear_branch");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let (ok, output) = run_example(fixture.path(), "fail_v4_linear_branch", "check");
    assert!(!ok, "a one-sided release must be refused:\n{output}");
    assert!(
        output.contains("released on some branches"),
        "the diagnostic must be about the disagreement:\n{output}"
    );
}
