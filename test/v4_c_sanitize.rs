//! The ASan/UBSan boundary lane, run by `make test-v4-sanitize`.
//!
//! Section 16 names this target as required once the M7 pointer and resource
//! slices ship, and M7.3's and M7.4's gates are written against it. It exists
//! now because a sanitizer lane added *after* those slices would only ever
//! confirm what was already believed; added before, it can contradict them.
//!
//! What is instrumented is the **C side**: the consumer is compiled with
//! `-fsanitize=address,undefined` and linked against the FOL static library.
//! That catches what the boundary can get wrong — a struct read past its end,
//! a misaligned access, an out-of-range enum, a null dereference — without
//! needing a nightly Rust toolchain, which `-Zsanitizer` would.
//!
//! The lane skips when no sanitizer-capable compiler is present, and says so.
//! It does not silently pass.

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

/// A compiler that can actually build and run a sanitized binary.
///
/// Presence of the compiler is not enough: a toolchain can accept
/// `-fsanitize=address` and then fail to link its runtime. This compiles and
/// runs a trivial program to find out.
fn sanitizing_compiler(scratch: &Path) -> Option<String> {
    for candidate in ["clang", "gcc", "cc"] {
        let source = scratch.join("probe.c");
        if std::fs::write(&source, "int main(void) { return 0; }\n").is_err() {
            continue;
        }
        let binary = scratch.join(format!("probe_{candidate}"));
        let built = Command::new(candidate)
            .args(["-fsanitize=address,undefined", "-o"])
            .arg(&binary)
            .arg(&source)
            .output();
        let Ok(built) = built else { continue };
        if !built.status.success() {
            continue;
        }
        if Command::new(&binary)
            .output()
            .is_ok_and(|run| run.status.success())
        {
            return Some(candidate.to_string());
        }
    }
    None
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

/// Build one checked-in example and return its install prefix.
fn build_example(fixture: &Path, example: &str) -> PathBuf {
    let root = fixture.join(example);
    copy_dir(&repo_root().join("examples").join(example), &root);

    let output = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["code", "build", "--package-store-root"])
        .arg(store_root())
        .current_dir(&root)
        .output()
        .expect("the build should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    assert!(
        output.status.success(),
        "{example} failed to build:\n{text}"
    );

    // A prefix with no library in it would let every assertion below pass by
    // never linking anything. The build is fast enough to look suspicious --
    // FOL caches by content fingerprint across temp directories -- so the
    // artifact is checked rather than inferred from the exit code.
    let prefix = root.join(".fol/install");
    let lib_dir = prefix.join("lib");
    let archive = std::fs::read_dir(&lib_dir)
        .unwrap_or_else(|error| panic!("{example} installed no lib directory: {error}"))
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "a"))
        .unwrap_or_else(|| {
            panic!(
                "{example} installed no static archive in {}",
                lib_dir.display()
            )
        });
    let size = archive.metadata().expect("archive metadata").len();
    assert!(
        size > 1024,
        "{example}'s archive is {size} bytes, which is too small to be a real library"
    );
    prefix
}

/// Compile a C consumer with sanitizers on and run it.
///
/// Returns the sanitizer's own output, which is where a violation is reported;
/// a clean run leaves it empty.
fn run_sanitized(
    cc: &str,
    prefix: &Path,
    consumer: &Path,
    library: &Path,
    binary: &Path,
) -> (bool, String) {
    let build = Command::new(cc)
        .args(["-std=c11", "-g", "-fsanitize=address,undefined", "-I"])
        .arg(prefix.join("include"))
        .arg("-o")
        .arg(binary)
        .arg(consumer)
        .arg(library)
        .args(["-lpthread", "-ldl", "-lm"])
        .output()
        .expect("the sanitized consumer should compile");
    assert!(
        build.status.success(),
        "the sanitized consumer failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(binary)
        // Make UBSan fail the process rather than only printing, so a
        // violation cannot pass as a warning nobody read.
        .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
        .env("ASAN_OPTIONS", "detect_leaks=0")
        .output()
        .expect("the sanitized consumer should run");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    (run.status.success(), text)
}

fn assert_clean(label: &str, ok: bool, output: &str) {
    for marker in [
        "AddressSanitizer",
        "runtime error:",
        "LeakSanitizer",
        "SEGV",
    ] {
        assert!(
            !output.contains(marker),
            "{label}: sanitizer reported '{marker}':\n{output}"
        );
    }
    assert!(ok, "{label}: the sanitized consumer failed:\n{output}");
}

/// Every scalar export survives a sanitized C consumer.
///
/// This is M5's own consumer, which already exercises the boolean and Unicode
/// rejections, the no-value status, and the null-out-pointer rule. Running it
/// instrumented is what turns "it returned the right numbers" into "and it did
/// not read or write anything it should not have".
#[test]
fn the_scalar_export_surface_is_clean_under_sanitizers() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_san_scalar");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let Some(cc) = sanitizing_compiler(fixture.path()) else {
        eprintln!("skipping: no compiler on this host can build a sanitized binary");
        return;
    };

    let prefix = build_example(fixture.path(), "v4_c_export_scalar");
    let (ok, output) = run_sanitized(
        &cc,
        &prefix,
        &repo_root().join("examples/v4_c_export_scalar/consumer.c"),
        &prefix.join("lib/libv4_c_export_scalar.a"),
        &fixture.path().join("scalar_sanitized"),
    );

    assert_clean("scalar exports", ok, &output);
    assert!(
        output.contains("all scalar exports ok"),
        "the consumer should still reach its final line:\n{output}"
    );
}

/// A record crossing by value is clean under sanitizers.
///
/// The header's `_Static_assert`s already prove FOL and C agree on the layout
/// at compile time. This proves the agreement holds at run time too: a struct
/// passed by value and one returned are both read entirely inside their own
/// storage, with no padding byte treated as a field.
#[test]
fn the_record_surface_is_clean_under_sanitizers() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_san_record");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let Some(cc) = sanitizing_compiler(fixture.path()) else {
        eprintln!("skipping: no compiler on this host can build a sanitized binary");
        return;
    };

    let prefix = build_example(fixture.path(), "v4_c_record");
    let (ok, output) = run_sanitized(
        &cc,
        &prefix,
        &repo_root().join("examples/v4_c_record/consumer.c"),
        &prefix.join("lib/libv4_c_record.a"),
        &fixture.path().join("record_sanitized"),
    );

    assert_clean("record surface", ok, &output);
    assert!(
        output.contains("all record checks passed"),
        "the consumer should still reach its final line:\n{output}"
    );
}

/// The lane can fail.
///
/// A sanitizer suite that has never reported anything is indistinguishable
/// from one that is not running, and this project has been burned by exactly
/// that shape of false green before. So the lane compiles a program with a
/// deliberate heap overflow and asserts the sanitizer catches it. If this test
/// starts passing trivially, the two above stop meaning anything.
#[test]
fn the_sanitizer_lane_actually_reports_violations() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_san_control");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let Some(cc) = sanitizing_compiler(fixture.path()) else {
        eprintln!("skipping: no compiler on this host can build a sanitized binary");
        return;
    };

    let source = fixture.path().join("overflow.c");
    std::fs::write(
        &source,
        "#include <stdlib.h>\n\
         int main(void) {\n\
         \x20   int *heap = malloc(4 * sizeof(int));\n\
         \x20   heap[7] = 1;\n\
         \x20   int seen = heap[7];\n\
         \x20   free(heap);\n\
         \x20   return seen == 1 ? 0 : 1;\n\
         }\n",
    )
    .expect("the control source should be writable");

    let binary = fixture.path().join("overflow");
    let build = Command::new(&cc)
        .args(["-fsanitize=address,undefined", "-g", "-o"])
        .arg(&binary)
        .arg(&source)
        .output()
        .expect("the control should compile");
    assert!(build.status.success(), "the control should build");

    let run = Command::new(&binary)
        .env("ASAN_OPTIONS", "detect_leaks=0")
        .output()
        .expect("the control should run");
    let output = String::from_utf8_lossy(&run.stderr);

    assert!(
        !run.status.success(),
        "a heap overflow must fail the process, or this lane cannot fail at all"
    );
    assert!(
        output.contains("AddressSanitizer"),
        "the sanitizer must name the violation; got:\n{output}"
    );
}
