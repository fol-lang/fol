//! FOL against a real third-party library.
//!
//! `plan/V4_GAPS.md`'s completion rule: the C ABI is done when a program
//! written against a real library's real header compiles, runs, and agrees
//! with the rest of the world. Every other lane in this repository tests a
//! header FOL wrote for itself.
//!
//! zlib is the first. Its header is 96,000 lines FOL did not choose, its
//! archive is the system's own, and CRC-32 and Adler-32 of `"hello"` are
//! published constants — so agreeing with them is agreeing with everyone
//! else's zlib rather than with FOL's idea of itself.
//!
//! Run by `make test-v4-c-zlib`.

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

fn which(program: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_string_lossy().into_owned())
}

/// The system's own zlib archive, found the way a consumer would.
///
/// Located rather than vendored: a copy checked in beside the header would
/// prove FOL can read a file it shipped, which is the thing this lane exists
/// not to do.
fn system_libz() -> Option<PathBuf> {
    let probe = Command::new("cc")
        .args(["-print-file-name=libz.a"])
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8_lossy(&probe.stdout).trim());
    if path.is_file() {
        return Some(path);
    }
    // Nix keeps the static archive in its own output.
    let listing = Command::new("sh")
        .args([
            "-c",
            "ls -d /nix/store/*zlib*static*/lib/libz.a 2>/dev/null | head -1",
        ])
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8_lossy(&listing.stdout).trim());
    path.is_file().then_some(path)
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

fn require_or_skip() -> Option<(String, PathBuf, PathBuf)> {
    let compiler = std::env::var_os("FOL_INTEROP_GCC")
        .and_then(|value| value.to_str().map(str::to_string))
        .or_else(|| which("gcc"))
        .or_else(|| which("cc"));
    let temp = std::env::var_os("FOL_INTEROP_TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .canonicalize()
        .ok();
    match (compiler, temp, system_libz()) {
        (Some(compiler), Some(temp), Some(libz)) => Some((compiler, temp, libz)),
        _ if std::env::var_os("FOL_H7_REQUIRED").is_some() => {
            panic!("FOL_H7_REQUIRED is set but no C toolchain or system zlib is available")
        }
        _ => None,
    }
}

fn run_folc(root: &Path, compiler: &str, temp: &Path, args: &[&str]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_folc"));
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

fn stage(fixture: &Path, libz: &Path) -> PathBuf {
    let root = fixture.join("v4_c_zlib");
    copy_tree(&repo_root().join("examples/v4_c_zlib"), &root);
    std::fs::copy(libz, root.join("native/libz.a")).expect("the system libz should copy");
    root
}

/// FOL computes zlib's checksums and agrees with the published values.
#[test]
fn fol_calls_real_zlib_and_agrees_with_it() {
    let Some((compiler, temp, libz)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain or no system zlib");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_zlib");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), &libz);

    let (ok, output) = run_folc(
        &root,
        &compiler,
        &temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "z",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/zlib.h",
            "--provider",
            "native/libz.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/z.toml",
            "--define",
            "Z_SOLO",
            "--out",
            "interop/z.folabi.json",
        ],
    );
    assert!(ok, "binding real zlib should succeed:\n{output}");
    assert!(
        output.contains("bound 2 routine(s)"),
        "both checksums should bind:\n{output}"
    );

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "FOL should agree with zlib on both checksums:\n{output}"
    );
}

/// Without the define, the failure names the header line rather than a count.
///
/// PARC refuses `#if (UINT_MAX == 0xffffffffUL)` in `zconf.h`, and the failure
/// cascades hundreds of lines to declarations that then misparse. Before this
/// lane, all of that arrived as *"selected source closure has 1 blocker(s)"* —
/// a count, against a 96,000-line header.
#[test]
fn a_blocked_header_names_the_lines_that_blocked_it() {
    let Some((compiler, temp, libz)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain or no system zlib");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_zlib_blocked");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), &libz);

    let (ok, output) = run_folc(
        &root,
        &compiler,
        &temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "z",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/zlib.h",
            "--provider",
            "native/libz.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/z.toml",
            "--out",
            "interop/z.folabi.json",
        ],
    );
    assert!(!ok, "zlib without Z_SOLO should be refused:\n{output}");
    assert!(
        output.contains("zconf.h:") && output.contains("PARC-E2113"),
        "the refusal should name the line and the code:\n{output}"
    );
    assert!(
        output.contains("unsigned integer semantics"),
        "the refusal should say what PARC would not evaluate:\n{output}"
    );
}
