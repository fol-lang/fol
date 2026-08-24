//! M6: the scalar C import vertical slice, proven by running the program.
//!
//! Every test here builds the real C provider, runs the real
//! PARC -> LINC -> GERC pipeline through `fol tool bind c`, compiles a FOL
//! program that calls the result, and runs it. M6's STOP rules out anything
//! less: an imported function cannot ship if its provider path, target,
//! effect, calling convention, error mapping, or safety contract is unknown,
//! and the only way to show they are known is to execute the call.
//!
//! Run by `make test-v4-c-import`.

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

/// Copy the checked-in example into a scratch directory.
///
/// Copied rather than used in place so the repository keeps no `.fol` tree and
/// no built provider archive, both of which a source-only guard enforces.
fn stage_example() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fol-v4-c-import-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source = repo_root().join("examples/v4_c_import_scalar");
    copy_tree(&source, &root);
    root
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("staging directory");
    for entry in std::fs::read_dir(from).expect("readable example") {
        let entry = entry.expect("directory entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy example file");
        }
    }
}

/// Build the C provider the example imports.
fn build_provider(root: &Path, compiler: &str) {
    let native = root.join("native");
    let object = native.join("c_math.o");
    let status = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("c_math.c"))
        .status()
        .expect("compile the C provider");
    assert!(status.success(), "the C provider should compile");
    let status = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libc_math.a"))
        .arg(&object)
        .status()
        .expect("archive the C provider");
    assert!(status.success(), "the C provider should archive");
}

/// Run `folc` with the interop environment set.
///
/// `--package-store-root` is a compile-route flag, so it is added only for the
/// commands that accept one: `tool bind c` takes every input explicitly and
/// rejects flags it does not own.
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

fn bind(root: &Path, compiler: &str, temp: &Path, overlay: &str, out: &str) -> (bool, String) {
    run_folc(
        root,
        compiler,
        temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "c_math",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/c_math.h",
            "--provider",
            "native/libc_math.a",
            "--provider-kind",
            "static",
            "--annotations",
            overlay,
            "--out",
            out,
        ],
    )
}

/// The whole slice: bind, build, run, and check what the program printed.
///
/// The three printed lines are the three contracts M6 owes: an infallible
/// call returns its value, a status call's success reaches FOL as a value, and
/// a status call's failure reaches FOL as a failure rather than as whatever
/// the provider left in the out-parameter.
#[test]
fn fol_calls_a_checked_c_scalar_library_and_observes_both_outcomes() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "binding the checked C import should succeed:\n{text}");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the importing package should build:\n{text}");

    let binary = root.join(".fol/install/bin/v4_c_import_scalar");
    let output = Command::new(&binary).output().expect("run the program");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "the program should run: {stdout}");
    assert!(
        stdout.contains("add_one(41) = 42"),
        "an infallible import should return its value; got:\n{stdout}"
    );
    assert!(
        stdout.contains("checked_div(42, 7) = 6"),
        "a status import's success should reach FOL as a value; got:\n{stdout}"
    );
    assert!(
        stdout.contains("checked_div(42, 0) reported failure"),
        "a status import's failure must reach FOL as a failure; got:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The checked manifest is evidence, not decoration: without it the source
/// does not compile, and the diagnostic says what to run.
#[test]
fn compiling_without_a_checked_manifest_names_the_command_that_writes_one() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);
    std::fs::remove_file(root.join("interop/c_math.folabi.json")).expect("remove the manifest");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "check"]);
    assert!(!ok, "a missing manifest must fail the build:\n{text}");
    assert!(
        text.contains("has no checked manifest"),
        "the diagnostic should say the manifest is missing; got:\n{text}"
    );
    assert!(
        text.contains("fol tool bind c"),
        "the diagnostic should name the command that writes one; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A hand-edited manifest is refused. The file is derived from the header, the
/// overlay, and the measurement; an edit is a claim none of those three made.
#[test]
fn a_hand_edited_manifest_is_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    let manifest_path = root.join("interop/c_math.folabi.json");
    let manifest = std::fs::read_to_string(&manifest_path).expect("read the manifest");
    std::fs::write(
        &manifest_path,
        manifest.replace("\"add_one\"", "\"add_two\""),
    )
    .expect("write the edited manifest");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "check"]);
    assert!(!ok, "an edited manifest must fail the build:\n{text}");
    assert!(
        text.contains("fingerprint"),
        "the diagnostic should report the fingerprint mismatch; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Section 4.13 rejects every error convention it cannot check, by name.
#[test]
fn a_guessed_error_convention_is_refused_by_name() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    for convention in ["errno", "sentinel", "unwind", "longjmp"] {
        std::fs::write(
            root.join("interop/guessed.toml"),
            format!("version = 1\n[routine.c_math_add_one]\nerror = \"{convention}\"\n"),
        )
        .expect("write the overlay");

        let (ok, text) = bind(
            &root,
            &compiler,
            &temp,
            "interop/guessed.toml",
            "interop/guessed.folabi.json",
        );
        assert!(!ok, "'{convention}' must be refused:\n{text}");
        assert!(
            text.contains(convention),
            "the diagnostic should name '{convention}'; got:\n{text}"
        );
        assert!(
            text.contains("rejected"),
            "the diagnostic should say the convention is rejected; got:\n{text}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

/// An overlay entry with no declaration behind it is a typo, and saying so at
/// bind time is much better than a missing symbol at link time.
#[test]
fn an_overlay_naming_an_absent_declaration_is_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    std::fs::write(
        root.join("interop/ghost.toml"),
        "version = 1\n[routine.c_math_add_one]\nerror = \"infallible\"\n\
         [routine.c_math_absent]\nerror = \"infallible\"\n",
    )
    .expect("write the overlay");

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/ghost.toml",
        "interop/ghost.folabi.json",
    );
    assert!(!ok, "a phantom selection must be refused:\n{text}");
    assert!(
        text.contains("c_math_absent") && text.contains("do not declare"),
        "the diagnostic should name the absent symbol; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A declaration the overlay does not select stays out of the FOL namespace,
/// and is not callable however it is spelled.
#[test]
fn an_unselected_declaration_does_not_become_callable() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    // Select only `add_one`, then call `checked_div` anyway.
    std::fs::write(
        root.join("interop/c_math.toml"),
        "version = 1\n[routine.c_math_add_one]\nfol_name = \"add_one\"\nerror = \"infallible\"\n",
    )
    .expect("write the narrowed overlay");
    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "a narrowed overlay should still bind:\n{text}");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "check"]);
    assert!(
        !ok,
        "calling an unselected declaration must not resolve:\n{text}"
    );
    assert!(
        text.contains("checked_div"),
        "the diagnostic should name the unresolved routine; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
