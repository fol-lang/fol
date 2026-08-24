//! M9's release lane: an archive a stranger can use.
//!
//! Everything else in the V4 set proves the toolchain works *here*. An
//! installed prefix is enough for that, because a path into the build tree
//! still resolves. A release archive is the first artifact that has to work
//! somewhere else, so the tests below extract it into a clean directory and
//! build against nothing but what came out of the tarball.
//!
//! Two things get proven that no earlier lane can. The archive must not carry
//! backend source -- FOL compiles through generated Rust, and a `Cargo.toml`
//! or a `.rs` facade in a published archive would present an implementation
//! detail as an interface, which Section 14's STOP names as a reason V4 cannot
//! close. And its checksums must be verifiable by the tool a consumer already
//! has, which is why they are SHA-256 and why the test shells out to
//! `sha256sum -c` rather than re-hashing with the same code that wrote them.
//!
//! Run by `make test-v4-c-release`.

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

fn tool(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|name| (*name).to_string())
}

fn c_compiler() -> Option<String> {
    std::env::var("FOL_INTEROP_GCC")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| tool(&["cc", "gcc", "clang"]))
}

/// A missing tool is a skip, or a failure on the certified lane.
fn skip(reason: &str) {
    assert!(
        std::env::var_os("FOL_H7_REQUIRED").is_none(),
        "FOL_H7_REQUIRED is set but this lane cannot run: {reason}"
    );
    eprintln!("SKIP: {reason}");
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

fn folc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_folc"))
}

/// Build the scalar export example and return its install prefix.
fn build_library(fixture: &Path, kind: &str) -> PathBuf {
    let root = fixture.join(format!("library-{kind}"));
    copy_dir(&repo_root().join("examples/v4_c_export_scalar"), &root);
    if kind != "add_static_lib" {
        let build = root.join("build.fol");
        let text = std::fs::read_to_string(&build).expect("build.fol readable");
        std::fs::write(&build, text.replace("add_static_lib", kind)).expect("build.fol writable");
    }

    let output = Command::new(folc())
        .args(["code", "build", "--package-store-root"])
        .arg(store_root())
        .current_dir(&root)
        .output()
        .expect("the build should run");
    assert!(
        output.status.success(),
        "the library failed to build:\n{}",
        strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    );
    root.join(".fol/install")
}

/// Pack a prefix and return the archive path.
fn package(fixture: &Path, prefix: &Path, name: &str) -> PathBuf {
    let archive = fixture.join(name);
    let output = Command::new(folc())
        .args(["tool", "abi", "package", "--prefix"])
        .arg(prefix)
        .arg("--out")
        .arg(&archive)
        .arg("--license")
        .arg(repo_root().join("LICENSE.md"))
        .output()
        .expect("the package command should run");
    assert!(
        output.status.success(),
        "packaging failed:\n{}",
        strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    );
    assert!(archive.is_file(), "no archive was written");
    archive
}

/// Extract into a fresh directory and return the archive's single root.
fn extract(fixture: &Path, archive: &Path, into: &str) -> PathBuf {
    let destination = fixture.join(into);
    std::fs::create_dir_all(&destination).expect("extraction directory");
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&destination)
        .output()
        .expect("tar should run");
    assert!(
        output.status.success(),
        "extraction failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut roots: Vec<PathBuf> = std::fs::read_dir(&destination)
        .expect("readable extraction")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(roots.len(), 1, "an archive should have one root directory");
    roots.remove(0)
}

fn every_file(root: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).expect("readable directory") {
        let entry = entry.expect("entry");
        if entry.path().is_dir() {
            every_file(&entry.path(), into);
        } else {
            into.push(entry.path());
        }
    }
}

/// A release archive carries the interface and nothing behind it.
#[test]
fn a_release_archive_publishes_no_backend_source() {
    let fixture = fol_testkit::TempFixture::new("v4-release-contents");
    let prefix = build_library(fixture.path(), "add_static_lib");
    let archive = package(fixture.path(), &prefix, "static.tar.gz");
    let root = extract(fixture.path(), &archive, "extract-static");

    let mut files = Vec::new();
    every_file(&root, &mut files);
    assert!(!files.is_empty(), "the archive is empty");

    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let extension = path.extension().unwrap_or_default().to_string_lossy();
        assert!(
            extension != "rs",
            "the archive publishes generated Rust: {}",
            path.display()
        );
        assert!(
            name != "Cargo.toml" && name != "Cargo.lock",
            "the archive publishes a Cargo manifest: {}",
            path.display()
        );
    }

    // And it carries what a consumer needs to act on it.
    for required in [
        "include/v4_c_export_scalar.h",
        "lib/libv4_c_export_scalar.a",
        "share/fol/abi/v4_c_export_scalar.folabi.json",
        "share/fol/abi/v4_c_export_scalar.symbols",
        "CHECKSUMS.sha256",
        "PROVENANCE",
        "SBOM",
        "LICENSE",
    ] {
        assert!(
            root.join(required).is_file(),
            "the archive is missing {required}"
        );
    }
}

/// The checksums are verifiable by the tool a consumer already has.
///
/// Deliberately not re-hashed with FOL's own code: that would only prove the
/// writer agrees with itself. `sha256sum -c` is an independent implementation
/// and an independent reading of the file format.
#[test]
fn the_published_checksums_verify_with_the_system_tool() {
    let Some(sha256sum) = tool(&["sha256sum"]) else {
        skip("no sha256sum; cannot verify checksums independently");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-release-checksums");
    let prefix = build_library(fixture.path(), "add_static_lib");
    let archive = package(fixture.path(), &prefix, "static.tar.gz");
    let root = extract(fixture.path(), &archive, "extract-checksums");

    let output = Command::new(&sha256sum)
        .arg("-c")
        .arg("CHECKSUMS.sha256")
        .current_dir(&root)
        .output()
        .expect("sha256sum should run");
    assert!(
        output.status.success(),
        "the published checksums did not verify:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // A checksum file that verifies because it lists nothing would pass the
    // check above, so the count is asserted too.
    let listed = std::fs::read_to_string(root.join("CHECKSUMS.sha256"))
        .expect("checksums readable")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert_eq!(listed, 4, "expected one line per published file");
}

/// A C consumer builds and runs against nothing but the extracted archive.
///
/// The compile is given exactly one include path and one library, both inside
/// the extraction directory. Nothing reaches back into the repository or the
/// build tree, which is the whole claim a release archive makes.
#[test]
fn a_c_consumer_builds_from_the_extracted_static_archive() {
    let Some(cc) = c_compiler() else {
        skip("no C compiler; cannot build a consumer");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-release-static");
    let prefix = build_library(fixture.path(), "add_static_lib");
    let archive = package(fixture.path(), &prefix, "static.tar.gz");
    let root = extract(fixture.path(), &archive, "extract-consumer");

    let consumer = fixture.path().join("consumer.c");
    std::fs::copy(
        repo_root().join("examples/v4_c_export_scalar/consumer.c"),
        &consumer,
    )
    .expect("the consumer should copy");

    let binary = fixture.path().join("from_archive");
    let compile = Command::new(&cc)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(root.join("include"))
        .arg(&consumer)
        .arg(root.join("lib/libv4_c_export_scalar.a"))
        .args(["-lpthread", "-ldl", "-lm"])
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("the compiler should run");
    assert!(
        compile.status.success(),
        "the consumer failed to build from the archive:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&binary).output().expect("the consumer runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "the consumer failed:\n{stdout}");
    assert!(
        stdout.contains("all scalar exports ok"),
        "the consumer did not reach its final line:\n{stdout}"
    );
}

/// The same, through the shared form, resolved by SONAME.
#[test]
fn a_c_consumer_builds_from_the_extracted_shared_archive() {
    let Some(cc) = c_compiler() else {
        skip("no C compiler; cannot build a consumer");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-release-shared");
    let prefix = build_library(fixture.path(), "add_shared_lib");
    let archive = package(fixture.path(), &prefix, "shared.tar.gz");
    let root = extract(fixture.path(), &archive, "extract-shared");

    let consumer = fixture.path().join("consumer.c");
    std::fs::copy(
        repo_root().join("examples/v4_c_export_scalar/consumer.c"),
        &consumer,
    )
    .expect("the consumer should copy");

    let binary = fixture.path().join("from_shared_archive");
    let compile = Command::new(&cc)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(root.join("include"))
        .arg(&consumer)
        .arg("-L")
        .arg(root.join("lib"))
        .arg("-lv4_c_export_scalar")
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("the compiler should run");
    assert!(
        compile.status.success(),
        "the consumer failed to build from the shared archive:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&binary)
        .env("LD_LIBRARY_PATH", root.join("lib"))
        .output()
        .expect("the consumer runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "the consumer failed:\n{stdout}");
    assert!(
        stdout.contains("all scalar exports ok"),
        "the consumer did not reach its final line:\n{stdout}"
    );
}

/// Packaging a prefix that carries backend source is refused, not filtered.
///
/// Filtering would make the command succeed on a prefix that should never have
/// existed, and the next person would find out when something else shipped.
#[test]
fn a_prefix_carrying_backend_source_is_refused() {
    let fixture = fol_testkit::TempFixture::new("v4-release-refusal");
    let prefix = build_library(fixture.path(), "add_static_lib");
    std::fs::write(prefix.join("lib/facade.rs"), b"pub fn leaked() {}\n")
        .expect("the intruding file should write");

    let output = Command::new(folc())
        .args(["tool", "abi", "package", "--prefix"])
        .arg(&prefix)
        .arg("--out")
        .arg(fixture.path().join("refused.tar.gz"))
        .output()
        .expect("the package command should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    assert!(
        !output.status.success(),
        "packaging should have failed:\n{text}"
    );
    assert!(
        text.contains("backend source"),
        "the refusal should say what is wrong:\n{text}"
    );
    assert!(
        text.contains("facade.rs"),
        "the refusal should name the file:\n{text}"
    );
    assert!(
        !fixture.path().join("refused.tar.gz").exists(),
        "a refused package must not leave an archive behind"
    );
}
