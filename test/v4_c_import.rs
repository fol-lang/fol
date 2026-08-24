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
/// The provider is the one input whose *contents* nothing else re-checks.
///
/// A changed header or overlay changes the projected interface, so a stale
/// manifest shows up as a type error sooner or later. A swapped archive need
/// not: it can carry the same symbol names with different code behind them,
/// and the manifest would still name the same path and still verify against
/// itself. Its digest is what closes that.
#[test]
fn swapping_the_provider_after_binding_is_refused() {
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
    assert!(ok, "the initial bind should succeed:\n{text}");
    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the package should build before the swap:\n{text}");

    // Same declarations, different code behind them: the archive that gets
    // linked is not the one that was measured.
    let source = root.join("native/c_math.c");
    let original = std::fs::read_to_string(&source).expect("readable provider source");
    std::fs::write(&source, original.replace("+ 1", "+ 2")).expect("writable provider source");
    build_provider(&root, &compiler);

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(!ok, "a swapped provider must be refused:\n{text}");
    assert!(
        text.contains("provider") && text.contains("was bound"),
        "the refusal should name the provider; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A manifest's own fingerprints prove only that nobody edited the manifest.
/// Whether it still describes the header is a different question, and it used
/// to go unasked: editing the header left the checked-in interface in force,
/// so callers compiled against a surface the header no longer had.
#[test]
fn editing_the_header_after_binding_is_refused_at_compile_time() {
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
    assert!(ok, "the initial bind should succeed:\n{text}");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(
        ok,
        "the package should build before the header changes:\n{text}"
    );

    // Any change at all: the check is on the bytes, not on whether the change
    // happens to alter the projected interface.
    let header = root.join("native/c_math.h");
    let original = std::fs::read_to_string(&header).expect("readable header");
    std::fs::write(&header, format!("{original}\n/* edited after binding */\n"))
        .expect("writable header");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(
        !ok,
        "building against a header edited after the bind must be refused:\n{text}"
    );
    assert!(
        text.contains("has changed since C import 'c_math' was bound"),
        "the refusal should say what went stale; got:\n{text}"
    );
    assert!(
        text.contains("bind c"),
        "the refusal should name the command that fixes it; got:\n{text}"
    );

    // Re-binding makes it current again, so the refusal is a staleness check
    // and not a permanent wedge.
    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "re-binding should succeed:\n{text}");
    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(
        ok,
        "the package should build again after re-binding:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The annotation overlay is the other half of the contract, so a change to it
/// goes stale the same way.
#[test]
fn editing_the_annotation_overlay_after_binding_is_refused() {
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
    assert!(ok, "the initial bind should succeed:\n{text}");

    let overlay = root.join("interop/c_math.toml");
    let original = std::fs::read_to_string(&overlay).expect("readable overlay");
    std::fs::write(&overlay, format!("{original}\n# edited after binding\n"))
        .expect("writable overlay");

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(!ok, "an edited overlay must be refused:\n{text}");
    assert!(
        text.contains("annotation overlay") && text.contains("was bound"),
        "the refusal should name the overlay; got:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Section 4.13 lets unsupported declarations stay in an imported header: only
/// the overlay's chosen symbols have to be translatable. That guarantee held
/// for `tool bind c` and was lost on the compile path, which scanned the whole
/// header and rejected it outright -- so a header bound cleanly and then
/// failed to build, which is the worst place to find out.
///
/// A variadic the overlay never names is the cheapest unsupported declaration
/// to add, and it is added to the staged copy so the checked-in example keeps
/// showing the ordinary case.
#[test]
fn an_unselected_unsupported_declaration_does_not_break_the_build() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    let header = root.join("native/c_math.h");
    let text = std::fs::read_to_string(&header).expect("the header should be readable");
    let guard = text
        .rfind("#endif")
        .expect("the header should have an include guard");
    let mut widened = text.clone();
    widened.insert_str(guard, "int c_math_log(const char *fmt, ...);\n");
    std::fs::write(&header, &widened).expect("the header should be writable");

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(
        ok,
        "binding must ignore a declaration the overlay does not name:\n{text}"
    );

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(
        ok,
        "the build must select the same closure the bind did, not rescan the \
         whole header:\n{text}"
    );

    let binary = root.join(".fol/install/bin/v4_c_import_scalar");
    let output = Command::new(&binary).output().expect("run the program");
    assert!(
        output.status.success(),
        "the program should still run:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("add_one(41) = 42"));

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

/// A C enum crosses as the integer the target measured it at.
///
/// Deliberately not as a FOL entry: a C enum is an integer with named
/// constants, and projecting it as a tagged union would invent a discriminant
/// contract C never made -- the same trap M12 exists to avoid on the export
/// side.
#[test]
fn a_c_enum_crosses_at_its_measured_width() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    std::fs::write(
        root.join("native/c_math.h"),
        b"#ifndef C_MATH_H\n#define C_MATH_H\n\
          enum shade { SHADE_DIM = 3, SHADE_BRIGHT = 9 };\n\
          int c_math_level(enum shade s);\n#endif\n",
    )
    .expect("header writable");
    std::fs::write(
        root.join("native/c_math.c"),
        b"#include \"c_math.h\"\nint c_math_level(enum shade s) { return (int)s * 2; }\n",
    )
    .expect("provider writable");
    std::fs::write(
        root.join("interop/c_math.toml"),
        b"version = 1\n[routine.c_math_level]\nfol_name = \"level\"\nerror = \"infallible\"\n",
    )
    .expect("overlay writable");
    std::fs::write(
        root.join("src/main.fol"),
        b"use std: pkg = {\"std\"};\nuse cm: pkg = {\"c_math\"};\n\n\
          fun[] main(): int = {\n\
          \x20   var bright: u32 = 9;\n\
          \x20   var got: int[32] = cm::level(bright);\n\
          \x20   var expected: int[32] = 18;\n\
          \x20   var marker: int = 0;\n\
          \x20   when(.eq(got, expected)) {\n\
          \x20       case(true) { marker = 7; }\n\
          \x20       * { marker = 1; }\n\
          \x20   };\n\
          \x20   var shown: int = std::io::echo_int(marker);\n\
          \x20   return 0;\n};\n",
    )
    .expect("source writable");
    build_provider(&root, &compiler);

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "an enum parameter should bind:\n{text}");

    let manifest = std::fs::read_to_string(root.join("interop/c_math.folabi.json"))
        .expect("manifest readable");
    assert!(
        manifest.contains("\"scalar\":\"u32\""),
        "the enum should record its measured storage:\n{manifest}"
    );

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the importing package should build:\n{text}");

    let binary = root.join(".fol/install/bin/v4_c_import_scalar");
    let output = Command::new(&binary).output().expect("run the program");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "the program should run:\n{stdout}");
    // 9 doubled is 18: the value can only be right if the enum crossed intact.
    assert!(stdout.contains('7'), "the enum call went wrong:\n{stdout}");

    let _ = std::fs::remove_dir_all(&root);
}

/// A C struct crosses inbound as an ordinary FOL value.
///
/// The struct FOL emits for it is FOL's own -- FOL layout, FOL derives -- and
/// the provider's struct is rebuilt from the fields inside the adapter, which
/// is the same discipline the export wrapper follows in the other direction:
/// field by field, never transmuted.
///
/// 6 * 7 can only be 42 if the struct FOL built reached C intact, and reading
/// `p.x` afterwards can only give 6 if a C struct passes by value the way C
/// says it does.
#[test]
fn a_c_record_crosses_inbound_and_survives_the_call() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    std::fs::write(
        root.join("native/c_math.h"),
        b"#ifndef C_MATH_H\n#define C_MATH_H\n\
          struct point { int x; int y; };\n\
          int c_math_area(struct point p);\n#endif\n",
    )
    .expect("header writable");
    std::fs::write(
        root.join("native/c_math.c"),
        b"#include \"c_math.h\"\nint c_math_area(struct point p) { return p.x * p.y; }\n",
    )
    .expect("provider writable");
    std::fs::write(
        root.join("interop/c_math.toml"),
        b"version = 1\n[routine.c_math_area]\nfol_name = \"area\"\nerror = \"infallible\"\n",
    )
    .expect("overlay writable");
    std::fs::write(
        root.join("src/main.fol"),
        b"use std: pkg = {\"std\"};\nuse cm: pkg = {\"c_math\"};\n\n\
          fun[] main(): int = {\n\
          \x20   var p: cm::point = { x = 6, y = 7 };\n\
          \x20   var area: int[32] = cm::area(p);\n\
          \x20   var back: int[32] = p.x;\n\
          \x20   var expected: int[32] = 42;\n\
          \x20   var six: int[32] = 6;\n\
          \x20   var marker: int = 0;\n\
          \x20   when(.eq(area, expected)) {\n\
          \x20       case(true) {\n\
          \x20           when(.eq(back, six)) {\n\
          \x20               case(true) { marker = 7; }\n\
          \x20               * { marker = 2; }\n\
          \x20           };\n\
          \x20       }\n\
          \x20       * { marker = 1; }\n\
          \x20   };\n\
          \x20   var shown: int = std::io::echo_int(marker);\n\
          \x20   return 0;\n};\n",
    )
    .expect("source writable");
    build_provider(&root, &compiler);

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "a record parameter should bind:\n{text}");

    let manifest = std::fs::read_to_string(root.join("interop/c_math.folabi.json"))
        .expect("manifest readable");
    assert!(
        manifest.contains("\"kind\":\"record\"") && manifest.contains("\"name\":\"point\""),
        "the manifest should record the struct and its fields:\n{manifest}"
    );

    let (ok, text) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the importing package should build:\n{text}");

    let binary = root.join(".fol/install/bin/v4_c_import_scalar");
    let output = Command::new(&binary).output().expect("run the program");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "the program should run:\n{stdout}");
    // 2 would mean the struct crossed but did not survive the call; 1 would
    // mean it never crossed correctly at all.
    assert!(
        stdout.contains('7'),
        "the record did not cross intact:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A struct that refers to itself is refused before it can recurse.
///
/// `struct node { struct node *next; }` is an ordinary header shape and a cycle
/// in the type graph: the pointer's target resolves back to the record being
/// projected. Without the guard the projection recursed until the stack ended,
/// which is what this asserts is no longer possible.
#[test]
fn a_self_referential_record_is_refused_rather_than_recursing() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the interop lane");
        return;
    };
    let root = stage_example();
    std::fs::write(
        root.join("native/c_math.h"),
        b"#ifndef C_MATH_H\n#define C_MATH_H\n\
          struct node { int value; struct node *next; };\n\
          int c_math_chain(struct node n);\n#endif\n",
    )
    .expect("header writable");
    std::fs::write(
        root.join("native/c_math.c"),
        b"#include \"c_math.h\"\nint c_math_chain(struct node n) { return n.value; }\n",
    )
    .expect("provider writable");
    std::fs::write(
        root.join("interop/c_math.toml"),
        b"version = 1\n[routine.c_math_chain]\nfol_name = \"chain\"\nerror = \"infallible\"\n",
    )
    .expect("overlay writable");
    build_provider(&root, &compiler);

    let (ok, text) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(!ok, "a self-referential record must be refused:\n{text}");
    assert!(
        text.contains("refers to itself"),
        "the refusal should name the cycle:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The compiler that measured the layouts is recorded by content, not by path.
///
/// Every offset, width, and signedness in the interface came out of that
/// binary. Replace it at the same path -- a different major version, a
/// different vendor -- and the path still matches while the measurements it
/// would produce may not.
///
/// It is recorded and folded into cache identity, and deliberately *not* a
/// staleness gate: the header, provider, and overlay are package-relative and
/// checked in, so every machine reads the same bytes, while the compiler is an
/// absolute path into whatever toolchain this machine has. Gating on it would
/// refuse a manifest bound on one machine and built on another.
#[test]
fn the_compiler_is_recorded_by_content() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the import slice");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    let (ok, output) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "binding should succeed:\n{output}");

    let manifest = std::fs::read_to_string(root.join("interop/c_math.folabi.json"))
        .expect("the manifest should be written");
    let digest = manifest
        .split("\"compiler_digest\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the compiler digest should be recorded");
    assert!(
        digest.len() >= 16 && digest.chars().all(|c| c.is_ascii_hexdigit()),
        "the digest should be a content hash, got {digest:?}"
    );
    // The path alone was what this replaced, so both are present and differ.
    assert!(
        manifest.contains(&compiler) && !manifest.contains(&format!("\"{compiler}\",\"{digest}\"")),
        "the path and the digest are separate facts:\n{manifest}"
    );
}

/// A declaration can live in a header the entry includes, and editing that one
/// used to be invisible.
///
/// Only the entry header's digest was checked, so a change to an included file
/// left every recorded digest matching. The build re-ran the C pipeline, picked
/// up the new signature in the raw bindings, and then failed as a *Rust type
/// error in generated code* -- the right build outcome for entirely the wrong
/// reason, and with nothing telling the author to re-bind.
#[test]
fn an_edited_include_is_reported_as_staleness() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the import slice");
        return;
    };
    let root = stage_example();
    build_provider(&root, &compiler);

    // Move one declaration into a second header the entry includes.
    let header = root.join("native/c_math.h");
    let text = std::fs::read_to_string(&header).expect("c_math.h readable");
    let Some(moved) = text
        .lines()
        .find(|line| line.trim_start().starts_with("int32_t") && line.contains('('))
    else {
        eprintln!("skipping: the fixture header has no declaration to move");
        return;
    };
    let moved = moved.to_string();
    std::fs::write(
        root.join("native/shapes.h"),
        format!(
            "#ifndef FOL_SHAPES_H\n#define FOL_SHAPES_H\n#include <stdint.h>\n#include <stddef.h>\n{moved}\n#endif\n"
        ),
    )
    .expect("shapes.h writable");
    std::fs::write(&header, text.replace(&moved, "#include \"shapes.h\""))
        .expect("c_math.h writable");

    let (ok, output) = bind(
        &root,
        &compiler,
        &temp,
        "interop/c_math.toml",
        "interop/c_math.folabi.json",
    );
    assert!(ok, "binding a split header should succeed:\n{output}");
    let manifest = std::fs::read_to_string(root.join("interop/c_math.folabi.json"))
        .expect("the manifest should be written");
    assert!(
        manifest.contains("native/shapes.h"),
        "the included header should be recorded:\n{manifest}"
    );

    // Touch only the include. Every previously recorded digest still matches.
    let shapes = root.join("native/shapes.h");
    let text = std::fs::read_to_string(&shapes).expect("shapes.h readable");
    std::fs::write(&shapes, format!("{text}\n/* edited */\n")).expect("shapes.h rewritable");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(!ok, "an edited include should be refused:\n{output}");
    assert!(
        output.contains("included header") && output.contains("native/shapes.h"),
        "the changed include should be named:\n{output}"
    );
}
