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
