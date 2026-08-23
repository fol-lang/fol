//! M3 native product gate: FOL library artifacts a C program can actually link.
//!
//! These build real archives and shared objects and hand them to a C compiler.
//! A unit test can assert that the backend *intended* to pass `--crate-type
//! staticlib`; only linking proves the file it produced is one.
//!
//! Run by `make test-native`.

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

/// A C compiler, or `None` when the suite is running outside `nix develop`.
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

/// Write a package declaring one library artifact of `kind`.
fn library_package(root: &Path, name: &str, kind: &str) {
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        format!(
            r#"pro[] build(): non = {{
    var build = .build();
    build.meta({{
        name = "{name}", version = "0.1.0", kind = "lib",
        description = "native product gate", license = "MIT",
    }});
    build.add_dep({{ alias = "std", source = "internal", target = "standard" }});
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var lib = graph.{kind}({{
        name = "{name}", root = "src/lib.fol", fol_model = "memo",
        target = target, optimize = optimize,
    }});
    graph.install(lib);
    return;
}};
"#
        ),
    )
    .expect("build.fol should be writable");
    // No `main`: a library artifact must not be searched for an entry routine.
    std::fs::write(
        root.join("src/lib.fol"),
        "fun[exp] triple(x: int): int = {\n    return x * 3;\n};\n",
    )
    .expect("lib.fol should be writable");
}

fn build_library(fixture: &Path, name: &str, kind: &str) -> String {
    let root = fixture.join(name);
    library_package(&root, name, kind);
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
    assert!(output.status.success(), "{kind} build failed:\n{text}");
    text
}

/// Every installed file under an install prefix.
fn installed_files(root: &Path, name: &str) -> Vec<String> {
    let prefix = root.join(name).join(".fol/install");
    let mut found = Vec::new();
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else if let Ok(relative) = path.strip_prefix(base) {
                out.push(relative.display().to_string());
            }
        }
    }
    walk(&prefix, &prefix, &mut found);
    found.sort();
    found
}

/// A static library builds, installs under the frozen layout, and a C program
/// links against it and runs.
///
/// This is M3's STOP condition.
#[test]
fn a_c_program_links_and_runs_against_a_fol_static_library() {
    let Some(cc) = c_compiler() else {
        eprintln!("skipping: no C compiler on PATH");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_native_static");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    build_library(fixture.path(), "statlib", "add_static_lib");

    // Section 4.16: a static archive installs as `lib/lib<artifact>.a` on this
    // target. The name is a target convention, not the crate name.
    assert_eq!(
        installed_files(fixture.path(), "statlib"),
        vec!["lib/libstatlib.a".to_string()],
        "the static library did not install under the frozen layout"
    );

    let archive = fixture.path().join("statlib/.fol/install/lib/libstatlib.a");
    let program = fixture.path().join("consumer.c");
    std::fs::write(
        &program,
        "#include <stdio.h>\nint main(void) { printf(\"ok\\n\"); return 0; }\n",
    )
    .expect("the C program should be writable");
    let binary = fixture.path().join("consumer");

    let link = Command::new(&cc)
        .args(["-std=c17", "-Wall", "-Wextra"])
        .arg(&program)
        .arg(&archive)
        .args(["-lpthread", "-ldl", "-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("the C compiler should run");
    assert!(
        link.status.success(),
        "a C program could not link the FOL static library:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&binary)
        .output()
        .expect("the program should run");
    assert!(run.status.success(), "the linked program did not run");
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ok");
}

/// The same for a shared library.
#[test]
fn a_c_program_links_and_runs_against_a_fol_shared_library() {
    let Some(cc) = c_compiler() else {
        eprintln!("skipping: no C compiler on PATH");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_native_shared");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    build_library(fixture.path(), "shlib", "add_shared_lib");

    assert_eq!(
        installed_files(fixture.path(), "shlib"),
        vec!["lib/libshlib.so".to_string()],
        "the shared library did not install under the frozen layout"
    );

    let shared = fixture.path().join("shlib/.fol/install/lib/libshlib.so");
    let program = fixture.path().join("consumer.c");
    std::fs::write(
        &program,
        "#include <stdio.h>\nint main(void) { printf(\"ok\\n\"); return 0; }\n",
    )
    .expect("the C program should be writable");
    let binary = fixture.path().join("consumer");

    let link = Command::new(&cc)
        .args(["-std=c17", "-Wall", "-Wextra"])
        .arg(&program)
        .arg(&shared)
        .arg(format!(
            "-Wl,-rpath,{}",
            shared.parent().expect("the library has a parent").display()
        ))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("the C compiler should run");
    assert!(
        link.status.success(),
        "a C program could not link the FOL shared library:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&binary)
        .output()
        .expect("the program should run");
    assert!(run.status.success(), "the linked program did not run");
}

/// A library artifact is never searched for `main`.
///
/// Before M3 this failed with "does not expose a runnable entry", which turned
/// a correct library into a build error.
#[test]
fn a_library_artifact_does_not_look_for_an_entry_routine() {
    let fixture = fol_testkit::TempFixture::new("fol_native_no_main");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    let text = build_library(fixture.path(), "nomain", "add_static_lib");
    assert!(
        !text.contains("runnable entry"),
        "the build looked for an entry routine in a library:\n{text}"
    );
    assert!(
        text.contains("[static-library]"),
        "the pre-build summary should name the product kind:\n{text}"
    );
}

/// A static and a shared library built from identical source do not collide.
#[test]
fn library_kinds_built_from_one_source_do_not_collide() {
    let fixture = fol_testkit::TempFixture::new("fol_native_kinds");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    build_library(fixture.path(), "bothstatic", "add_static_lib");
    build_library(fixture.path(), "bothshared", "add_shared_lib");

    assert_eq!(
        installed_files(fixture.path(), "bothstatic"),
        vec!["lib/libbothstatic.a".to_string()]
    );
    assert_eq!(
        installed_files(fixture.path(), "bothshared"),
        vec!["lib/libbothshared.so".to_string()]
    );
}

/// The produced files are the formats they claim to be, for the right target.
///
/// Linking proves an archive is usable; this proves it is an archive rather
/// than, say, an executable that happened to satisfy the linker.
#[test]
fn produced_files_have_the_expected_format_and_target() {
    let fixture = fol_testkit::TempFixture::new("fol_native_format");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    build_library(fixture.path(), "fmtstatic", "add_static_lib");
    build_library(fixture.path(), "fmtshared", "add_shared_lib");

    let archive = fixture
        .path()
        .join("fmtstatic/.fol/install/lib/libfmtstatic.a");
    let shared = fixture
        .path()
        .join("fmtshared/.fol/install/lib/libfmtshared.so");

    // An `ar` archive begins with this exact magic.
    let archive_bytes = std::fs::read(&archive).expect("the archive should be readable");
    assert!(
        archive_bytes.starts_with(b"!<arch>\n"),
        "the static library is not an ar archive"
    );

    // ELF magic, 64-bit, little-endian, and the x86-64 machine field.
    let shared_bytes = std::fs::read(&shared).expect("the shared library should be readable");
    assert_eq!(&shared_bytes[0..4], b"\x7fELF", "not an ELF file");
    assert_eq!(shared_bytes[4], 2, "not 64-bit");
    assert_eq!(shared_bytes[5], 1, "not little-endian");
    // e_type 3 is a shared object; an executable would be 2.
    assert_eq!(
        u16::from_le_bytes([shared_bytes[16], shared_bytes[17]]),
        3,
        "the shared library is not an ELF shared object"
    );
    // e_machine 0x3e is x86-64.
    assert_eq!(
        u16::from_le_bytes([shared_bytes[18], shared_bytes[19]]),
        0x3e,
        "the shared library is not built for x86-64"
    );
}

/// The frontend reports a library under its own role, not as a binary.
///
/// Everything used to be `binary`, so a static library and an executable were
/// indistinguishable to any consumer of the summary or its JSON.
#[test]
fn frontend_json_lists_the_library_role() {
    let fixture = fol_testkit::TempFixture::new("fol_native_json");
    std::fs::create_dir_all(fixture.path()).expect("fixture root should be creatable");

    let root = fixture.path().join("jsonlib");
    library_package(&root, "jsonlib", "add_static_lib");

    let output = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["--output", "json", "code", "build", "--package-store-root"])
        .arg(store_root())
        .current_dir(&root)
        .output()
        .expect("the build should run");
    let text = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "the JSON build failed:\n{}",
        strip_ansi(&String::from_utf8_lossy(&output.stderr))
    );

    assert!(
        text.contains("static-library"),
        "the JSON output does not name the static-library role:\n{text}"
    );
    assert!(
        !text.contains("\"kind\":\"binary\"") && !text.contains("\"kind\": \"binary\""),
        "a library was reported as a binary:\n{text}"
    );
}
