//! M5: the scalar C export vertical slice, proven by a real C consumer.
//!
//! Every test here builds a FOL library, installs it, and compiles a C program
//! against the **installed** prefix using the **generated** header. M5's STOP
//! rules out anything less: no Rust or FOL internals, no skipped linking, no
//! bypassed installation, and no assertions on generated text alone.
//!
//! Run by `make test-v4-c`.

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

fn c_compiler() -> Option<String> {
    if let Some(clang) = std::env::var_os("FOL_H7_CLANG") {
        return clang.to_str().map(str::to_string);
    }
    for candidate in ["clang", "cc", "gcc"] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
        {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Copy the checked-in slice example into a scratch directory and build it.
///
/// Copied rather than built in place so the repository keeps no `.fol` tree,
/// which a source-only guard already enforces.
fn build_slice(fixture: &Path, kind: &str) -> PathBuf {
    let source = repo_root().join("examples/v4_c_export_scalar");
    let root = fixture.join("slice");
    copy_dir(&source, &root);

    if kind != "add_static_lib" {
        let build = root.join("build.fol");
        let text = std::fs::read_to_string(&build).expect("build.fol should be readable");
        std::fs::write(&build, text.replace("add_static_lib", kind))
            .expect("build.fol should be writable");
    }

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
        "the slice failed to build:\n{text}"
    );
    root.join(".fol/install")
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

/// Compile and run the checked-in C consumer against an installed prefix.
fn run_consumer(cc: &str, prefix: &Path, library: &Path, shared: bool) -> String {
    let fixture = library
        .parent()
        .and_then(Path::parent)
        .expect("the library has a prefix");
    let binary = fixture.join("consumer_binary");
    let consumer = repo_root().join("examples/v4_c_export_scalar/consumer.c");

    let mut command = Command::new(cc);
    command
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(prefix.join("include"))
        .arg(&consumer)
        .arg(library);
    if shared {
        command.arg(format!(
            "-Wl,-rpath,{}",
            library.parent().expect("a parent").display()
        ));
    } else {
        command.args(["-lpthread", "-ldl", "-lm"]);
    }
    let link = command
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("the C compiler should run");
    assert!(
        link.status.success(),
        "the C consumer failed to link:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&binary)
        .output()
        .expect("the consumer should run");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    assert!(
        run.status.success(),
        "the C consumer failed at runtime (exit {:?}):\n{stdout}",
        run.status.code()
    );
    stdout
}

/// C calls every scalar export through a static library.
///
/// The consumer itself asserts the boolean and Unicode rejections, the
/// no-value status, and the null-out-pointer rule; reaching its final line
/// means all of them held.
#[test]
fn c_calls_every_scalar_export_through_a_static_library() {
    let Some(cc) = c_compiler() else {
        eprintln!("skipping: no C compiler on PATH");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_static");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let prefix = build_slice(fixture.path(), "add_static_lib");
    let library = prefix.join("lib/libv4_c_export_scalar.a");
    assert!(library.is_file(), "the static library did not install");

    let stdout = run_consumer(&cc, &prefix, &library, false);
    assert!(stdout.contains("all scalar exports ok"), "got: {stdout}");
}

/// The same API through a shared library.
#[test]
fn c_calls_the_same_api_through_a_shared_library() {
    let Some(cc) = c_compiler() else {
        eprintln!("skipping: no C compiler on PATH");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_shared");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let prefix = build_slice(fixture.path(), "add_shared_lib");
    let library = prefix.join("lib/libv4_c_export_scalar.so");
    assert!(library.is_file(), "the shared library did not install");

    let stdout = run_consumer(&cc, &prefix, &library, true);
    assert!(stdout.contains("all scalar exports ok"), "got: {stdout}");
}

/// Symbol inspection finds all and only the allowlisted exports.
#[test]
fn the_built_symbol_set_matches_the_allowlist_exactly() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_symbols");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let prefix = build_slice(fixture.path(), "add_static_lib");

    let allowlist =
        std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.symbols"))
            .expect("the allowlist should install");
    let mut declared: Vec<&str> = allowlist.lines().filter(|l| !l.is_empty()).collect();
    declared.sort_unstable();
    assert!(!declared.is_empty(), "the allowlist is empty");

    let Ok(output) = Command::new("nm")
        .args(["--defined-only", "--extern-only"])
        .arg(prefix.join("lib/libv4_c_export_scalar.a"))
        .output()
    else {
        eprintln!("skipping symbol inspection: no nm on PATH");
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut found: Vec<&str> = text
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter(|symbol| symbol.starts_with("fol_slice_"))
        .collect();
    found.sort_unstable();
    found.dedup();

    assert_eq!(
        found, declared,
        "the built symbol set does not match the allowlist"
    );
}

/// Two clean builds produce the same header, manifest, and interface
/// fingerprint.
#[test]
fn two_clean_builds_agree_on_header_manifest_and_fingerprint() {
    let read = |label: &str| {
        let fixture = fol_testkit::TempFixture::new(label);
        std::fs::create_dir_all(fixture.path()).expect("fixture root");
        let prefix = build_slice(fixture.path(), "add_static_lib");
        let header = std::fs::read_to_string(prefix.join("include/v4_c_export_scalar.h"))
            .expect("the header should install");
        let manifest =
            std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.folabi.json"))
                .expect("the manifest should install");
        (header, manifest)
    };

    let (first_header, first_manifest) = read("fol_v4_c_repro_a");
    let (second_header, second_manifest) = read("fol_v4_c_repro_b");

    assert_eq!(
        first_header, second_header,
        "the header is not reproducible"
    );
    assert_eq!(
        first_manifest, second_manifest,
        "the manifest is not reproducible"
    );
    assert!(
        first_manifest.contains("interface_fingerprint"),
        "the manifest carries no interface fingerprint"
    );
}

/// The header and the manifest describe the same surface.
///
/// They are rendered from one `ResolvedAbiSurface`; this is what would catch a
/// future change that generated them from two.
#[test]
fn the_header_and_manifest_describe_the_same_symbols() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_agreement");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let prefix = build_slice(fixture.path(), "add_static_lib");

    let header = std::fs::read_to_string(prefix.join("include/v4_c_export_scalar.h"))
        .expect("the header should install");
    let manifest =
        std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.folabi.json"))
            .expect("the manifest should install");
    let allowlist =
        std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.symbols"))
            .expect("the allowlist should install");

    for symbol in allowlist.lines().filter(|l| !l.is_empty()) {
        assert!(
            header.contains(symbol),
            "{symbol} is allowlisted and absent from the header"
        );
        assert!(
            manifest.contains(symbol),
            "{symbol} is allowlisted and absent from the manifest"
        );
    }
}
