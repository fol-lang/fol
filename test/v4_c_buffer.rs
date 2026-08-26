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

/// Rebuild a staged provider after editing its C source.
fn rebuild_provider(root: &Path, compiler: &str) {
    let native = root.join("native");
    let object = native.join("digest.o");
    let status = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("digest.c"))
        .status()
        .expect("recompile the C provider");
    assert!(status.success(), "the edited provider should compile");
    let status = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libdigest.a"))
        .arg(&object)
        .status()
        .expect("rearchive the C provider");
    assert!(status.success(), "the edited provider should archive");
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
        let root = stage(
            fixture.path().join(name).as_path(),
            "v4_c_buffer",
            &compiler,
        );
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

/// A provider-allocated buffer is validated, copied, and released in one call.
///
/// The printed 7 now carries a fourth fact: `digest_live()` returned 0, so the
/// domain's release ran. FOL cannot see C's heap, so the provider is asked --
/// and the answer is the only way to tell a working program from a leaking
/// one. Removing the release turns this into a 4.
#[test]
fn an_owned_buffer_is_copied_and_released_inside_the_call() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_owned");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "the owned buffer should copy and release cleanly:\n{output}"
    );
}

/// The leak check can fail, so passing it means something.
///
/// Without this, a `digest_live()` that always returned 0 -- or a FOL side
/// that never compared it -- would look identical to a release that ran.
#[test]
fn a_release_that_does_not_run_is_visible() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_leak");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    // The provider stops reporting the free. Nothing on FOL's side changes.
    let source = root.join("native/digest.c");
    let text = std::fs::read_to_string(&source).expect("digest.c readable");
    std::fs::write(&source, text.replace("    live -= 1;\n", "")).expect("digest.c writable");
    rebuild_provider(&root, &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "4"),
        "an outstanding allocation should be reported as 4:\n{output}"
    );
}

/// A provider that contradicts its own report is refused, not read.
///
/// Both fixtures describe memory that does not exist: one reports more
/// elements than it allocated, the other reports a length for a buffer it did
/// not return. Copying on either would read whatever happened to be there.
#[test]
fn a_provider_contradicting_its_own_report_is_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_lies");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    for (example, expected) in [
        (
            "fail_v4_c_buffer_capacity",
            "reported a longer buffer than the capacity it allocated",
        ),
        (
            "fail_v4_c_buffer_null",
            "returned NULL but reported a nonzero length",
        ),
    ] {
        let root = stage(fixture.path(), example, &compiler);
        let (ok, output) = bind(&root, &compiler, &temp);
        assert!(ok, "{example} should bind:\n{output}");

        let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
        assert!(!ok, "{example} should not run to completion:\n{output}");
        assert!(
            output.contains(expected),
            "{example} should name what the provider contradicted:\n{output}"
        );
    }
}

/// Every way of pairing an owned buffer domain incoherently, refused by name.
///
/// The same four an opaque handle domain gets, because the fact is the same:
/// memory a provider allocated is memory only that provider can free, and an
/// overlay that gets the pairing wrong compiles into a program that frees
/// nothing or frees it twice.
#[test]
fn incoherent_buffer_domains_are_refused() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_buffer_domain");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    const DOMAIN: &str = "version = 1\n\n[buffer.Bytes]\ndestroy = \"digest_release\"\n\n";
    const PRODUCER: &str = "[routine.digest_take]\nerror = \"infallible\"\n\
        buffer_domain = \"Bytes\"\nbuffer_role = \"produces\"\nbuffer_length = \"out_len\"\n\n";
    const CONSUMER: &str = "[routine.digest_release]\nerror = \"infallible\"\n\
        buffer_domain = \"Bytes\"\nbuffer_role = \"consumes\"\nbuffer_length = \"count\"\n";

    for (name, overlay, expected) in [
        (
            "destroy_unselected",
            format!("{DOMAIN}{PRODUCER}"),
            "names destroy 'digest_release', which the overlay does not select",
        ),
        (
            "domain_undeclared",
            format!("version = 1\n\n{PRODUCER}"),
            "which no [buffer.Bytes] table declares",
        ),
        (
            "no_producer",
            format!("{DOMAIN}{CONSUMER}"),
            "has 0 producers; exactly one is needed",
        ),
        (
            "owned_and_borrowed",
            format!(
                "{DOMAIN}{CONSUMER}\n[routine.digest_take]\nerror = \"infallible\"\n\
                 buffer_domain = \"Bytes\"\nbuffer_role = \"produces\"\n\
                 buffer_length = \"out_len\"\nbuffer = \"start\"\n"
            ),
            "declares both a borrowed buffer and an owned one",
        ),
    ] {
        let root = stage(
            fixture.path().join(name).as_path(),
            "v4_c_buffer",
            &compiler,
        );
        std::fs::write(root.join("interop/digest.toml"), overlay).expect("overlay writable");

        let (ok, output) = bind(&root, &compiler, &temp);
        assert!(!ok, "{name} should be refused:\n{output}");
        assert!(
            output.contains(expected),
            "{name} should be refused by name:\n{output}"
        );
    }
}

/// A link failure reports the missing symbol, not the linker's transcript.
///
/// This is the one M14 exists for. Before the summary, an undefined symbol
/// produced 57,462 lines -- 8,398 of them rustc naming warnings about the
/// generated crate's own mangled identifiers -- with the one useful line
/// somewhere in the middle. The fact a reader needs is which symbol is
/// referenced and defined nowhere.
#[test]
fn a_link_failure_names_the_symbol_rather_than_dumping_the_linker() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_link_summary");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    // Referenced by the provider, defined by nothing it links against. The
    // symbols the header declares are all present, so binding succeeds and
    // the failure lands at the link, which is the path under test.
    let source = root.join("native/digest.c");
    let text = std::fs::read_to_string(&source).expect("digest.c readable");
    let text = text
        .replace(
            "#include <stdlib.h>",
            "#include <stdlib.h>\nextern uint32_t fol_absent_helper(uint32_t);",
        )
        .replace(
            "        total += (uint32_t)bytes[index];",
            "        total += fol_absent_helper((uint32_t)bytes[index]);",
        );
    std::fs::write(&source, text).expect("digest.c writable");
    rebuild_provider(&root, &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should still succeed:\n{output}");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(!ok, "the link should fail:\n{output}");
    assert!(
        output.contains("fol_absent_helper"),
        "the missing symbol should be named:\n{output}"
    );
    assert!(
        output.contains("referenced but defined by no linked provider"),
        "the failure should say what kind it is:\n{output}"
    );
    // The transcript is kept, but beside the diagnostic rather than as it.
    assert!(
        output.contains("the full output is at"),
        "the full linker output should still be reachable:\n{output}"
    );
    assert!(
        !output.contains("--eh-frame-hdr"),
        "the linker command line is not the error:\n{output}"
    );
    assert!(
        output.lines().count() < 40,
        "a link failure should not be a transcript; got {} lines:\n{output}",
        output.lines().count()
    );
}

/// The generated crate's own lints never reach the user.
///
/// Its identifiers are mangled and some of its routines are unreachable, both
/// by construction. Warning about either tells whoever wrote the FOL nothing
/// they can act on, and burying a real failure under thousands of them is how
/// this was found.
#[test]
fn the_generated_crate_does_not_warn_at_the_user() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_no_lints");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(ok, "binding should succeed:\n{output}");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the example should build:\n{output}");
    for noise in ["should have an upper camel case name", "is never used"] {
        assert!(
            !output.contains(noise),
            "the generated crate should not warn about {noise:?}:\n{output}"
        );
    }
}

/// A rejection about a declaration names where it was written.
///
/// LINC reports against its own declaration ids -- `pdecl1_<hash>` -- which
/// say nothing to whoever wrote the header. The symbol is in the message, and
/// the scanned package knows where each declaration is, so the id becomes a
/// place.
#[test]
fn a_missing_provider_symbol_names_its_header_line() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_decl_origin");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    // Declared in the header, defined in no provider.
    let header = root.join("native/digest.h");
    let text = std::fs::read_to_string(&header).expect("digest.h readable");
    std::fs::write(
        &header,
        text.replace("#endif", "uint32_t digest_absent(uint8_t seed);\n\n#endif"),
    )
    .expect("digest.h writable");
    let overlay = root.join("interop/digest.toml");
    let text = std::fs::read_to_string(&overlay).expect("overlay readable");
    std::fs::write(
        &overlay,
        format!("{text}\n[routine.digest_absent]\nerror = \"infallible\"\n"),
    )
    .expect("overlay writable");

    let (ok, output) = bind(&root, &compiler, &temp);
    assert!(
        !ok,
        "a symbol no provider defines should be refused:\n{output}"
    );
    assert!(
        output.contains("missing exact provider symbol"),
        "the reason should survive:\n{output}"
    );
    assert!(
        output.contains("digest.h:23"),
        "the declaration's own line should be named:\n{output}"
    );
    assert!(
        !output.contains("pdecl"),
        "an internal declaration id is not a place:\n{output}"
    );
}

/// Every provider form that can fail reports which failure it was.
///
/// These are the shapes an author actually hits, and they are easy to confuse:
/// a file that is not there, a file that is not what it claims, and a file that
/// is fine but whose dependencies cannot be resolved. Each says so.
#[test]
fn each_provider_failure_says_which_one_it_is() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the buffer slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_provider_forms");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = stage(fixture.path(), "v4_c_buffer", &compiler);

    let bind_with = |provider: &str, kind: &str| -> (bool, String) {
        run_folc(
            &root,
            &compiler,
            &temp,
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
                provider,
                "--provider-kind",
                kind,
                "--annotations",
                "interop/digest.toml",
                "--out",
                "interop/digest.folabi.json",
            ],
        )
    };

    // Not there at all: the path the author wrote, and why it could not be read.
    let (ok, output) = bind_with("native/absent.a", "static");
    assert!(!ok, "an absent provider should be refused:\n{output}");
    assert!(
        output.contains("native/absent.a") && output.contains("No such file"),
        "an absent provider should name the path:\n{output}"
    );

    // There, but not an archive. The bytes decide, not the extension.
    std::fs::write(root.join("native/text.a"), "this is not an archive\n")
        .expect("the decoy should be writable");
    let (ok, output) = bind_with("native/text.a", "static");
    assert!(!ok, "a non-archive should be refused:\n{output}");
    assert!(
        output.contains("truncated or corrupt") || output.contains("file magic"),
        "a non-archive should say what was wrong with it:\n{output}"
    );

    // A shared provider carries its own dependencies, which exact-path
    // resolution cannot search for. The reported name is one of *those*, so
    // without the note it reads as though the author's file were missing.
    let shared = Command::new(&compiler)
        .args(["-shared", "-fPIC", "-o"])
        .arg(root.join("native/libdigest.so"))
        .arg(root.join("native/digest.c"))
        .status();
    if shared.is_ok_and(|status| status.success()) {
        let (ok, output) = bind_with("native/libdigest.so", "shared");
        assert!(!ok, "a shared provider cannot certify yet:\n{output}");
        assert!(
            output.contains("is a dependency of the provider you supplied"),
            "the real cause should be named, not just the missing name:\n{output}"
        );
    }

    // A good provider stays good: none of the above is a false positive.
    let (ok, output) = bind_with("native/libdigest.a", "static");
    assert!(ok, "the real provider should still bind:\n{output}");
}

/// FOL text reaches a C routine as a NUL-terminated string.
///
/// This was the largest gap the shape corpus found: `const char *` bound,
/// wrote a manifest, and could not be mounted, so no imported routine could
/// take a string. `sqlite3_open(const char *filename, ...)` could not cross.
///
/// The printed 7 carries two facts. C walks to the NUL to count the bytes, so
/// 5 requires the terminator the adapter added; and 104 is `h`, which pins the
/// start. Together they say the whole string arrived.
#[test]
fn fol_text_crosses_as_a_nul_terminated_string() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the string slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_string_arg");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = fixture.path().join("v4_c_string_arg");
    copy_tree(&repo_root().join("examples/v4_c_string_arg"), &root);

    let native = root.join("native");
    assert!(
        Command::new(&compiler)
            .arg("-c")
            .arg("-o")
            .arg(native.join("text.o"))
            .arg(native.join("text.c"))
            .status()
            .expect("the C provider should compile")
            .success(),
        "the provider should compile"
    );
    assert!(
        Command::new("ar")
            .arg("rcs")
            .arg(native.join("libtext.a"))
            .arg(native.join("text.o"))
            .status()
            .expect("ar should run")
            .success(),
        "the provider should archive"
    );

    let (ok, output) = run_folc(
        &root,
        &compiler,
        &temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "text",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/text.h",
            "--provider",
            "native/libtext.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/text.toml",
            "--out",
            "interop/text.folabi.json",
        ],
    );
    assert!(ok, "binding a declared string should succeed:\n{output}");

    let (_, output) = run_folc(&root, &compiler, &temp, &["code", "run"]);
    assert!(
        output.lines().any(|line| line.trim() == "7"),
        "the string should arrive intact and terminated:\n{output}"
    );
}

/// An undeclared `char *` stays a pointer, and a pointer stays unusable.
///
/// The declaration is the whole mechanism: nothing measured says a `char *` is
/// text, so inferring it would be the guess every other pairing in the overlay
/// exists to avoid. Removing `string = [...]` must break the program.
#[test]
fn an_undeclared_char_pointer_is_not_text() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the string slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_string_undeclared");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = fixture.path().join("v4_c_string_arg");
    copy_tree(&repo_root().join("examples/v4_c_string_arg"), &root);

    let native = root.join("native");
    let _ = Command::new(&compiler)
        .arg("-c")
        .arg("-o")
        .arg(native.join("text.o"))
        .arg(native.join("text.c"))
        .status();
    let _ = Command::new("ar")
        .arg("rcs")
        .arg(native.join("libtext.a"))
        .arg(native.join("text.o"))
        .status();

    let overlay = root.join("interop/text.toml");
    let text = std::fs::read_to_string(&overlay).expect("overlay readable");
    std::fs::write(&overlay, text.replace("string = [\"s\"]\n", "")).expect("overlay writable");

    let (ok, output) = run_folc(
        &root,
        &compiler,
        &temp,
        &[
            "tool",
            "bind",
            "c",
            "--alias",
            "text",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/text.h",
            "--provider",
            "native/libtext.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/text.toml",
            "--out",
            "interop/text.folabi.json",
        ],
    );
    assert!(ok, "an undeclared pointer still binds:\n{output}");

    let (ok, output) = run_folc(&root, &compiler, &temp, &["code", "build"]);
    assert!(
        !ok,
        "an undeclared char pointer should not mount:\n{output}"
    );
    assert!(
        output.contains("uses a pointer type"),
        "the refusal should name the pointer:\n{output}"
    );
}
