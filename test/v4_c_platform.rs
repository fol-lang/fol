//! M9's platform lane: what a shared library says about itself, and how a
//! consumer finds it at runtime.
//!
//! A static archive has no identity of its own -- it is dissolved into whoever
//! links it. A shared library does, and getting that identity wrong is not
//! visible in any test that only checks the program's output: the consumer
//! runs fine on the machine that built it and fails everywhere else.
//!
//! The specific failure this lane exists for: with no `SONAME`, GNU ld records
//! whatever spelling the consumer linked through. Link by absolute path -- as
//! any build system resolving a full path does -- and the consumer's `NEEDED`
//! entry becomes a build-machine path that will not exist on the target.
//!
//! It also holds M9's rule that runtime lookup must not be bought with a
//! hidden default rpath, and the concurrent-build/cache-isolation check.

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
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        }
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

/// The first tool in `names` that runs, if any.
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

/// Build the scalar export example as a shared library and return its prefix.
fn build_shared(fixture: &Path, name: &str) -> PathBuf {
    let root = fixture.join(name);
    copy_dir(&repo_root().join("examples/v4_c_export_scalar"), &root);

    let build = root.join("build.fol");
    let text = std::fs::read_to_string(&build).expect("build.fol should be readable");
    std::fs::write(&build, text.replace("add_static_lib", "add_shared_lib"))
        .expect("build.fol should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["code", "build", "--package-store-root"])
        .arg(store_root())
        .current_dir(&root)
        .output()
        .expect("the build should run");
    assert!(
        output.status.success(),
        "the shared slice failed to build:\n{}",
        strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    );

    let prefix = root.join(".fol/install");
    let library = prefix.join("lib/libv4_c_export_scalar.so");
    assert!(
        library.is_file(),
        "the shared library did not install at {}",
        library.display()
    );
    prefix
}

/// `readelf -d` on one file, as text.
fn dynamic_section(readelf: &str, file: &Path) -> String {
    let output = Command::new(readelf)
        .arg("-d")
        .arg(file)
        .output()
        .expect("readelf should run");
    assert!(
        output.status.success(),
        "readelf failed on {}",
        file.display()
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn skip(reason: &str) {
    eprintln!("SKIP: {reason}");
}

/// An installed shared library names itself.
#[test]
fn the_installed_shared_library_records_its_own_soname() {
    let Some(readelf) = tool(&["readelf", "llvm-readelf"]) else {
        skip("no readelf; cannot inspect the dynamic section");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-platform-soname");
    let prefix = build_shared(fixture.path(), "slice");
    let library = prefix.join("lib/libv4_c_export_scalar.so");

    let dynamic = dynamic_section(&readelf, &library);
    assert!(
        dynamic.contains("SONAME"),
        "the installed shared library carries no SONAME:\n{dynamic}"
    );
    // The SONAME must be the *installed* file name. The build-tree file is
    // named after the build slice, so a SONAME copied from there would be
    // wrong in exactly the way that is hard to notice.
    assert!(
        dynamic.contains("[libv4_c_export_scalar.so]"),
        "the SONAME is not the installed file name:\n{dynamic}"
    );
}

/// The reason the SONAME is there: a consumer that links through a full path
/// must still record the library's name, not that path.
#[test]
fn a_consumer_linked_by_absolute_path_records_the_soname() {
    let (Some(cc), Some(readelf)) = (c_compiler(), tool(&["readelf", "llvm-readelf"])) else {
        skip("no C compiler or no readelf; cannot link a consumer");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-platform-consumer");
    let prefix = build_shared(fixture.path(), "slice");
    let library = prefix.join("lib/libv4_c_export_scalar.so");

    let source = fixture.path().join("consumer.c");
    std::fs::write(
        &source,
        b"#include <stdio.h>\n#include \"v4_c_export_scalar.h\"\n\
          int main(void) { int32_t out; fol_slice_add_i32(2, 3, &out); \
          printf(\"%d\\n\", out); return 0; }\n",
    )
    .expect("the consumer should be writable");

    let program = fixture.path().join("consumer");
    let compile = Command::new(&cc)
        .arg("-std=c11")
        .arg("-I")
        .arg(prefix.join("include"))
        .arg(&source)
        // Deliberately the full path rather than -L/-l.
        .arg(&library)
        .arg("-o")
        .arg(&program)
        .output()
        .expect("the compiler should run");
    assert!(
        compile.status.success(),
        "the consumer failed to link:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let dynamic = dynamic_section(&readelf, &program);
    let needed: Vec<&str> = dynamic
        .lines()
        .filter(|line| line.contains("(NEEDED)") && line.contains("v4_c_export_scalar"))
        .collect();
    assert_eq!(
        needed.len(),
        1,
        "expected exactly one NEEDED entry for the library:\n{dynamic}"
    );
    assert!(
        needed[0].contains("[libv4_c_export_scalar.so]"),
        "the consumer recorded a path instead of the SONAME: {}",
        needed[0].trim()
    );
    assert!(
        !needed[0].contains('/'),
        "the consumer baked a build-machine path into its NEEDED entry: {}",
        needed[0].trim()
    );

    // And it resolves through the ordinary mechanism, with no rpath helping.
    let run = Command::new(&program)
        .env("LD_LIBRARY_PATH", prefix.join("lib"))
        .output()
        .expect("the consumer should run");
    assert!(
        run.status.success(),
        "the consumer did not run:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "5");
}

/// The dynamic export set of a shared build is exactly the allowlist.
///
/// `the_built_symbol_set_matches_the_allowlist_exactly` proves this for the
/// static archive. A shared library is the one that actually exports at
/// runtime, and its export set is decided by a different mechanism, so it
/// needs its own evidence.
#[test]
fn the_shared_library_exports_exactly_the_allowlist() {
    let Some(nm) = tool(&["nm", "llvm-nm"]) else {
        skip("no nm; cannot read the export table");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("v4-platform-exports");
    let prefix = build_shared(fixture.path(), "slice");

    let mut declared: Vec<String> =
        std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.symbols"))
            .expect("the allowlist should install")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
    declared.sort();
    assert!(!declared.is_empty(), "the allowlist is empty");

    let output = Command::new(&nm)
        .args(["-D", "--defined-only"])
        .arg(prefix.join("lib/libv4_c_export_scalar.so"))
        .output()
        .expect("nm should run");
    assert!(output.status.success(), "nm failed on the shared library");

    let mut found: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(2))
        .filter(|name| name.starts_with("fol_slice_"))
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();

    assert_eq!(
        found, declared,
        "the shared library's dynamic exports differ from the declared allowlist"
    );
}

/// Two builds running at once in separate trees must not share cache state.
///
/// FOL caches by content fingerprint, so a cache keyed too coarsely would let
/// one build serve the other's artifact. Both trees are built from the same
/// source at the same time and each must produce its own complete prefix.
#[test]
fn concurrent_builds_do_not_share_cache_state() {
    let fixture = fol_testkit::TempFixture::new("v4-platform-concurrent");
    let root = fixture.path().to_path_buf();

    let handles: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|name| {
            let root = root.clone();
            std::thread::spawn(move || build_shared(&root, name))
        })
        .collect();

    let prefixes: Vec<PathBuf> = handles
        .into_iter()
        .map(|handle| handle.join().expect("a concurrent build panicked"))
        .collect();

    for prefix in &prefixes {
        for relative in [
            "lib/libv4_c_export_scalar.so",
            "include/v4_c_export_scalar.h",
            "share/fol/abi/v4_c_export_scalar.folabi.json",
            "share/fol/abi/v4_c_export_scalar.symbols",
        ] {
            assert!(
                prefix.join(relative).is_file(),
                "{} is missing from {}",
                relative,
                prefix.display()
            );
        }
    }

    // Same source, so the declared surface must agree; a cache that leaked
    // between the two would more likely show up as a missing file above, but
    // a disagreeing manifest is the other way it could go wrong.
    let manifest = |prefix: &PathBuf| {
        std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.folabi.json"))
            .expect("the manifest should install")
    };
    assert_eq!(
        manifest(&prefixes[0]),
        manifest(&prefixes[1]),
        "concurrent builds of the same source disagree on the ABI manifest"
    );
}
