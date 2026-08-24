//! M7.5: synchronous C callbacks into FOL closures.
//!
//! A C function pointer is not a Rust closure -- a closure carries an
//! environment and a function pointer has nowhere to put one. The bridge is a
//! generated trampoline: a monomorphic `extern "C"` shim that recovers the
//! closure from the opaque context FOL passed alongside it.
//!
//! What is worth testing is therefore not that the code compiles but that C
//! *actually invokes the closure*, and that the two things the trampoline is
//! responsible for -- validating the context and containing a fault -- happen.
//! Each test here runs the real pipeline and the real binary.
//!
//! Run by `make test-v4-c-callback`.

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

/// Copy an example into scratch and build its C provider.
fn stage(fixture: &Path, example: &str, compiler: &str) -> PathBuf {
    let root = fixture.join(example);
    copy_tree(&repo_root().join("examples").join(example), &root);

    let native = root.join("native");
    let object = native.join("tally.o");
    let status = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("tally.c"))
        .status()
        .expect("compile the C provider");
    assert!(status.success(), "{example}: the C provider should compile");
    let status = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libtally.a"))
        .arg(&object)
        .status()
        .expect("archive the C provider");
    assert!(status.success(), "{example}: the C provider should archive");
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

/// Regenerate the manifest so the test proves the pipeline, not the file.
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
            "tally",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/tally.h",
            "--provider",
            "native/libtally.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/tally.toml",
            "--out",
            "interop/tally.folabi.json",
        ],
    )
}

/// C invokes the FOL closure, four times, with the value it was handed back.
///
/// The printed 7 stands for `total == 10`, and 10 is 1+2+3+4 accumulated. No
/// arrangement of a *missing* callback produces that: it can only come from C
/// having called into FOL once per step and received each partial sum.
#[test]
fn c_invokes_a_fol_closure_during_the_call() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the callback slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_callback");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_callback", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding the callback header should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "the accumulated total should be 10, which only C calling back produces:\n{output}"
    );
}

/// The manifest records the pairing and the callback's own signature.
///
/// Both matter: the pairing is the fact C cannot state, and the signature is
/// what the FOL closure is checked against. A manifest carrying the pairing but
/// not the signature would type-check a closure against nothing.
#[test]
fn the_manifest_records_the_pairing_and_the_callback_signature() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the callback slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_cb_manifest");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_callback", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let manifest = std::fs::read_to_string(root.join("interop/tally.folabi.json"))
        .expect("the manifest should be written");
    assert!(
        manifest.contains(r#""callback":{"context":"context","parameter":"step"}"#),
        "the pairing is the fact C cannot state:\n{manifest}"
    );
    assert!(
        manifest.contains(r#""kind":"callback","parameters":[0,0]"#),
        "the callback's own parameter types belong in the manifest:\n{manifest}"
    );
    // The context is a declared C parameter but never a FOL one: FOL fills it
    // with the address of the closure it is lending.
    assert!(
        manifest.contains(r#"{"direction":"out","name":"context""#),
        "the context stays a declared parameter:\n{manifest}"
    );
}

/// A fault inside a callback ends the process instead of unwinding into C.
///
/// Unwinding out of an `extern "C"` function is undefined behaviour, and a
/// callback has no status channel to report through, so there is no value that
/// would be a true answer. The trampoline catches the fault, names the symbol,
/// and aborts. The alternative -- returning a default and letting the provider
/// continue on it -- is the silent wrong answer this boundary exists to prevent.
#[test]
fn a_faulting_callback_is_contained_rather_than_unwound() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the callback slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_cb_panic");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "fail_v4_c_callback_panic", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        !ok,
        "a faulting callback must not report success:\n{output}"
    );
    assert!(
        output.contains("a FOL callback passed to 'tally_range' panicked"),
        "the runtime should name the symbol that called back:\n{output}"
    );
    assert!(
        output.contains("unwinding into C is undefined"),
        "the message should say why the process ends:\n{output}"
    );
    // SIGABRT, not an ordinary exit: the process was ended deliberately.
    assert!(
        output.contains("SIGABRT") || output.contains("signal: 6"),
        "containment means aborting, not returning:\n{output}"
    );
}

/// A provider whose callback takes the context last is refused at bind time.
///
/// C permits it and real APIs do it, but FOL cannot tell a trailing context
/// from any other trailing pointer. Guessing wrong hands the provider an
/// address that is not the closure, so the shape is refused instead -- and the
/// diagnostic says which shape is imported, so a reader knows what to change.
#[test]
fn a_context_in_any_other_position_is_refused_with_the_canonical_shape() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the callback slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_cb_shape");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "fail_v4_c_callback_shape", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(!ok, "a trailing context must be refused:\n{output}");
    assert!(
        output.contains("whose first parameter is not the context it is handed"),
        "the diagnostic must name the problem:\n{output}"
    );
    assert!(
        output.contains("f(void *context, ...)"),
        "the diagnostic must name the shape that is imported:\n{output}"
    );
}
