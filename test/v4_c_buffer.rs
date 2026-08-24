//! M13: a buffer crossing as one FOL value rather than two C parameters.
//!
//! C carries a buffer as an address and a count with nothing joining them, so
//! the pairing is declared in the overlay. What is worth testing is not that
//! the code compiles but that the length is *derived* from the FOL value: a
//! wrong or hardcoded length reads the wrong number of elements, and the sum
//! says so.
//!
//! Direction is tested the same way -- by whether C's write is visible on the
//! FOL side afterwards.
//!
//! Run by `make test-v4-c-buffer`.

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
    let object = native.join("digest.o");
    let status = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("digest.c"))
        .status()
        .expect("compile the C provider");
    assert!(status.success(), "{example}: the C provider should compile");
    let status = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libdigest.a"))
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
            "digest",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/digest.h",
            "--provider",
            "native/libdigest.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/digest.toml",
            "--out",
            "interop/digest.folabi.json",
        ],
    )
}

/// The length reaches C from the FOL value, and C reads exactly that many.
///
/// The printed 7 stands for two facts at once: `digest_sum` returned 10, which
/// is 1+2+3+4 and no other prefix or suffix of that buffer; and after
/// `digest_fill` wrote 5 into three elements, summing them back gave 15. A
/// length that was hardcoded, off by one, or read from uninitialised memory
/// fails one of the two.
#[test]
fn a_buffer_crosses_as_one_value_in_both_directions() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_buffer");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding the buffer header should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "both directions should agree on the length:\n{output}"
    );
}

/// Changing the buffer's length changes what C reads.
///
/// This is the control the previous test needs. Without it, a length C ignored
/// entirely would still produce 10 whenever the first four elements were
/// 1,2,3,4 -- so the sum has to move when the buffer does.
#[test]
fn the_length_follows_the_value_rather_than_the_declaration() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_len");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let source = root.join("src/main.fol");
    let text = std::fs::read_to_string(&source).expect("main.fol readable");
    // One more element, and a sum that only a five-element read produces.
    let text = text
        .replace("vec[u8] = {1, 2, 3, 4};", "vec[u8] = {1, 2, 3, 4, 90};")
        .replace("is(10) {", "is(100) {");
    std::fs::write(&source, text).expect("main.fol writable");

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "a longer buffer should change what C sums:\n{output}"
    );
}

/// The manifest records the pairing and the slice's element.
///
/// Both matter. The pairing is the fact C cannot state; the element is what
/// makes the count mean anything, and a slice serialized without it would read
/// back as a buffer of nothing.
#[test]
fn the_manifest_records_the_pairing_and_the_element() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_manifest");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let manifest = std::fs::read_to_string(root.join("interop/digest.folabi.json"))
        .expect("the manifest should be written");
    assert!(
        manifest.contains(r#""buffer":{"length":"count","parameter":"bytes"}"#),
        "the pairing is the fact C cannot state:\n{manifest}"
    );
    // Two slices of the same element, told apart by whether C may write.
    assert!(
        manifest.contains(r#""kind":"borrowed-slice","mutability":"const""#),
        "a read-only buffer keeps its constness:\n{manifest}"
    );
    assert!(
        manifest.contains(r#""kind":"borrowed-slice","mutability":"mutable""#),
        "a written buffer is a distinct type:\n{manifest}"
    );
    // Declared, not inferred: `digest_fill` says `writes`, and the direction
    // in the manifest is that declaration rather than a reading of constness.
    assert!(
        manifest.contains(r#"{"direction":"out","name":"bytes""#),
        "the declared direction should reach the manifest:\n{manifest}"
    );
}

/// Every way of pairing a buffer incoherently, refused by name.
///
/// These are all facts C cannot check for itself: it has no opinion about
/// whether two of its parameters belong together, so an overlay that pairs the
/// wrong ones has to be caught here or not at all.
#[test]
fn incoherent_buffer_pairings_are_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_reject");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    for (name, overlay, expected) in [
        (
            "unknown",
            "buffer = \"nope\"\nbuffer_length = \"count\"",
            "names 'nope' as its buffer address, which is not one of its parameters",
        ),
        (
            "self_paired",
            "buffer = \"bytes\"\nbuffer_length = \"bytes\"",
            "names 'bytes' as both its buffer and its length",
        ),
        (
            "half",
            "buffer = \"bytes\"",
            "is missing required key 'buffer_length'",
        ),
        (
            "writes_const",
            "buffer = \"bytes\"\nbuffer_length = \"count\"\nwrites = [\"bytes\"]",
            "C declares its pointee const, so the provider cannot write through it",
        ),
    ] {
        let root = stage(fixture.path().join(name).as_path(), "v4_c_buffer", &compiler);
        std::fs::write(
            root.join("interop/digest.toml"),
            format!("version = 1\n\n[routine.digest_sum]\nerror = \"infallible\"\n{overlay}\n"),
        )
        .expect("overlay writable");

        let (ok, output) = bind(&root, &compiler, &temp);
        assert!(!ok, "{name} should be refused:\n{output}");
        assert!(
            output.contains(expected),
            "{name} should be refused by name:\n{output}"
        );
    }
}
