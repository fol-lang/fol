//! M7.4: opaque C handles, proven by running the program.
//!
//! A `struct widget` that is declared and never defined is C's opaque handle.
//! The C type system says only "an address"; who created it, who may borrow
//! it, and who releases it are facts C cannot state. They come from the
//! annotation overlay, and what FOL adds on top is a proof: the handle is
//! released on every path, exactly once, and never read afterwards.
//!
//! Each test builds the real provider, runs the real PARC/LINC/GERC pipeline
//! through `fol tool bind c`, and then either runs the program or reads the
//! diagnostic that refused it. Asserting on generated text instead would prove
//! nothing about whether the address actually reaches C.
//!
//! Run by `make test-v4-c-handle`.

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

/// The interop pipeline needs a real C compiler and a probe directory, and
/// section 4.13 forbids discovering either. Without them the lane is skipped
/// unless `FOL_H7_REQUIRED` says it must run.
fn interop_environment() -> Option<(String, PathBuf)> {
    let compiler = std::env::var_os("FOL_INTEROP_GCC")
        .and_then(|value| value.to_str().map(str::to_string))
        .or_else(|| {
            ["gcc", "cc"].into_iter().find_map(|candidate| {
                Command::new(candidate)
                    .arg("--version")
                    .output()
                    .is_ok_and(|out| out.status.success())
                    .then(|| which(candidate))
                    .flatten()
            })
        })?;
    let temp = std::env::var_os("FOL_INTEROP_TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let temp = temp.canonicalize().ok()?;
    Some((compiler, temp))
}

fn which(program: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|path| !path.is_empty())
}

fn require_or_skip() -> Option<(String, PathBuf)> {
    match interop_environment() {
        Some(environment) => Some(environment),
        None if std::env::var_os("FOL_H7_REQUIRED").is_some() => {
            panic!("FOL_H7_REQUIRED is set but no C toolchain or probe directory is available")
        }
        None => None,
    }
}

fn folc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_folc"))
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("staging directory");
    for entry in std::fs::read_dir(from).expect("readable example") {
        let entry = entry.expect("directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            if entry.file_name() == ".fol" {
                continue;
            }
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy example file");
        }
    }
}

/// Copy an example into a scratch directory and build its C provider.
///
/// Copied rather than used in place so the repository keeps no `.fol` tree and
/// no built archive, both of which a source-only guard enforces.
fn stage(fixture: &Path, example: &str, compiler: &str) -> PathBuf {
    let root = fixture.join(example);
    copy_tree(&repo_root().join("examples").join(example), &root);

    let native = root.join("native");
    let object = native.join("widget.o");
    let status = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("widget.c"))
        .status()
        .expect("compile the C provider");
    assert!(status.success(), "the C provider should compile");
    let status = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libwidget.a"))
        .arg(&object)
        .status()
        .expect("archive the C provider");
    assert!(status.success(), "the C provider should archive");
    root
}

fn run_folc(root: &Path, compiler: &str, temp: &Path, args: &[&str]) -> (bool, String) {
    let mut command = Command::new(folc());
    command
        .current_dir(root)
        .env("FOL_INTEROP_GCC", compiler)
        .env("FOL_INTEROP_TEMP", temp)
        .args(args);
    if args.first() == Some(&"code") {
        command.arg("--package-store-root").arg(store_root());
    }
    let output = command.output().expect("folc should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    (output.status.success(), text)
}

/// Regenerate the checked-in manifest so the test proves the pipeline, not the
/// file. A stale checked-in manifest would otherwise let the whole slice pass
/// without the C front end running at all.
fn bind(root: &Path, compiler: &str, temp: &Path) -> (bool, String) {
    run_folc(
        root,
        compiler,
        temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "widget",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/widget.h",
            "--provider",
            "native/libwidget.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/widget.toml",
            "--out",
            "interop/widget.folabi.json",
        ],
    )
}

/// The whole slice: an address crosses to C, C reads through it, and FOL
/// releases it exactly once.
///
/// `widget_size` doubling the seed is the proof the pointer is real: 42 can
/// only come from C dereferencing the address FOL handed it.
#[test]
fn a_c_handle_is_created_borrowed_and_released_exactly_once() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the opaque handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_handle");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_opaque_handle", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding the opaque header should succeed:\n{output}");
    assert!(
        output.contains("bound 3 routine(s)"),
        "the producer, the borrower, and the destroy:\n{output}"
    );

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "the borrowed handle should have reached C and come back doubled:\n{output}"
    );
}

/// The manifest records the domain, its destroy, and each routine's role.
#[test]
fn the_manifest_records_the_domain_and_every_role() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the opaque handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_handle_manifest");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_opaque_handle", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let manifest = std::fs::read_to_string(root.join("interop/widget.folabi.json"))
        .expect("the manifest should be written");
    for expected in [
        r#""handle":{"domain":"Widget","role":"produces"}"#,
        r#""handle":{"domain":"Widget","role":"borrows"}"#,
        r#""handle":{"domain":"Widget","role":"consumes"}"#,
        r#""kind":"opaque-handle","name":"Widget""#,
    ] {
        assert!(
            manifest.contains(expected),
            "the manifest should record {expected}:\n{manifest}"
        );
    }
}

/// Each way of getting a handle wrong is refused, with its own reason.
///
/// These are the four cases C cannot catch at all: it would compile every one
/// of them and fail, or corrupt memory, at run time.
#[test]
fn every_handle_misuse_is_refused_with_its_reason() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the opaque handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_handle_negatives");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let cases = [
        ("fail_v4_c_handle_leak", "abandon the linear resource 'w'"),
        ("fail_v4_c_handle_double_free", "use of moved"),
        (
            "fail_v4_c_handle_use_after_free",
            "cannot borrow from an owner whose value was already moved",
        ),
        (
            "fail_v4_c_handle_duplicate",
            "is a linear resource and cannot be duplicated",
        ),
    ];

    for (example, expected) in cases {
        let root = stage(fixture.path(), example, &compiler);
        let (ok, output) = bind(&root, &compiler, &temp);
        assert!(ok, "{example}: binding should succeed:\n{output}");

        let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "check"]);
        assert!(!ok, "{example} should be refused:\n{output}");
        assert!(
            output.contains(expected),
            "{example} should say '{expected}':\n{output}"
        );
    }
}

/// A producer that returns NULL is refused at the boundary, not adopted.
///
/// C's only way to say "no handle" is a null return. Adopting it would make a
/// FOL value that owes a release on nothing, and the release would be called
/// on NULL later -- safe for `free`, undefined for a provider's own destroy.
/// `FolHandle::is_null` existed and had no caller; this is the caller.
#[test]
fn a_producer_returning_null_is_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the opaque handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_handle_null");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "fail_v4_c_handle_null", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    // It builds: this is a runtime contract, not a compile-time one, because
    // whether the provider allocates is not knowable until it runs.
    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(!ok, "a null handle must not be adopted:\n{output}");
    assert!(
        output.contains("'widget_new' is declared to produce an opaque handle but returned NULL"),
        "the fault should name the producer:\n{output}"
    );
    assert!(
        output.contains("declare a status convention"),
        "the message should say what to do instead:\n{output}"
    );
    assert!(
        output.contains("SIGABRT") || output.contains("signal: 6"),
        "refusing means aborting, not returning:\n{output}"
    );
}

/// One release on every path, across control flow the straight-line case omits.
///
/// Each shape here is somewhere a release can be missed: a branch that returns
/// early with the handle already live, two handles live at once, and a loop
/// that acquires and releases once per iteration. The program running matters
/// as much as it compiling -- a proof about paths nothing takes proves nothing,
/// and the printed 7 says all three ran and agreed.
#[test]
fn a_handle_is_released_once_on_every_path() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_handle_paths");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_handle_lifecycle", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "every lifecycle shape should run and agree:\n{output}"
    );
}

/// Dropping the release on one branch is refused, so the proof is not vacuous.
///
/// Without this, a checker that proved nothing would pass the test above
/// exactly as a working one does.
#[test]
fn a_release_missed_on_one_branch_is_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the handle slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_handle_missed");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_handle_lifecycle", &compiler);

    // The early-return branch loses its release; the fall-through keeps one,
    // so only the branching path is wrong.
    let source = root.join("src/main.fol");
    let text = std::fs::read_to_string(&source).expect("main.fol readable");
    std::fs::write(
        &source,
        text.replace(
            "        wid::widget_free([mov]w);\n        return size;",
            "        return size;",
        ),
    )
    .expect("main.fol writable");

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should still succeed:\n{output}");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(!ok, "an abandoned handle should be refused:\n{output}");
    assert!(
        output.contains("abandon the linear resource 'w'"),
        "the refusal should name the resource and the path:\n{output}"
    );
}
