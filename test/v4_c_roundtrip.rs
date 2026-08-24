//! M9's round trip: a FOL library imported back through the ordinary C path.
//!
//! Every other test in the V4 set proves one direction. This one closes the
//! loop, and the point is what it is *not* allowed to use: there is no
//! FOL-to-FOL shortcut, no repository-relative path, and no privileged
//! knowledge that the provider happens to have been written in FOL. The
//! consumer package sees an installed header, an installed archive, and an
//! annotation overlay -- exactly what it would see from a third-party C
//! library -- and reaches the routines through `add_c_import`.
//!
//! The example cannot carry a checked-in manifest, because its provider is
//! built rather than committed: `native/` is populated here from the *installed
//! prefix* of the library example, and the bind runs against those files.
//!
//! Run by `make test-v4-c-roundtrip`.

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
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.display().to_string())
}

/// The interop lane needs an explicit compiler and an explicit temp root.
///
/// Without them the lane skips, which is right on a machine that has no C
/// toolchain and wrong on the certified one: `FOL_H7_REQUIRED` is how the
/// certified lane says a skip is a failure.
fn require_or_skip() -> Option<(String, PathBuf)> {
    let environment = (|| {
        let compiler = std::env::var("FOL_INTEROP_GCC")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| which("gcc"))
            .or_else(|| which("cc"))?;
        let temp = std::env::var("FOL_INTEROP_TEMP")
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Some((compiler, temp))
    })();
    match environment {
        Some(environment) => Some(environment),
        None if std::env::var_os("FOL_H7_REQUIRED").is_some() => {
            panic!("FOL_H7_REQUIRED is set but no C toolchain is available")
        }
        None => None,
    }
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

fn folc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_folc"))
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

/// Build `v4_c_export_scalar` and return its install prefix.
fn build_library(fixture: &Path, compiler: &str, temp: &Path) -> PathBuf {
    let root = fixture.join("library");
    copy_tree(&repo_root().join("examples/v4_c_export_scalar"), &root);
    let (ok, text) = run_folc(&root, compiler, temp, &["code", "build"]);
    assert!(ok, "the exporting library should build:\n{text}");

    let prefix = root.join(".fol/install");
    for relative in [
        "include/v4_c_export_scalar.h",
        "lib/libv4_c_export_scalar.a",
    ] {
        assert!(
            prefix.join(relative).is_file(),
            "{relative} did not install"
        );
    }
    prefix
}

/// Stage the consumer with the *installed* artifacts as its native inputs.
fn stage_consumer(fixture: &Path, prefix: &Path) -> PathBuf {
    let root = fixture.join("consumer");
    copy_tree(&repo_root().join("examples/v4_c_roundtrip_fol"), &root);
    std::fs::create_dir_all(root.join("native")).expect("native directory");
    for relative in [
        "include/v4_c_export_scalar.h",
        "lib/libv4_c_export_scalar.a",
    ] {
        let name = Path::new(relative)
            .file_name()
            .expect("the installed path has a file name");
        std::fs::copy(prefix.join(relative), root.join("native").join(name))
            .expect("copy the installed artifact");
    }
    root
}

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
            "folslice",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/v4_c_export_scalar.h",
            "--provider",
            "native/libv4_c_export_scalar.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/folslice.toml",
            "--out",
            "interop/folslice.folabi.json",
        ],
    )
}

/// The whole loop, proven by running it.
///
/// Both printed lines require the call to have crossed FOL -> C ABI -> FOL and
/// come back with the right value at the right width, which is the only way to
/// show the two sides agree about more than symbol names.
#[test]
fn a_fol_library_is_imported_back_through_the_ordinary_c_path() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the round-trip lane");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_roundtrip");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let prefix = build_library(fixture.path(), &compiler, &temp);
    let consumer = stage_consumer(fixture.path(), &prefix);

    let (ok, text) = bind(&consumer, &compiler, &temp);
    assert!(
        ok,
        "FOL must be able to bind its own generated header:\n{text}"
    );

    let (ok, text) = run_folc(&consumer, &compiler, &temp, &["code", "build"]);
    assert!(ok, "the importing package should build:\n{text}");

    let binary = consumer.join(".fol/install/bin/v4_c_roundtrip_fol");
    let output = Command::new(&binary).output().expect("run the program");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "the program should run:\n{stdout}");
    assert!(
        stdout.contains("add_i32(2, 3) = 5"),
        "a 32-bit call must cross and come back:\n{stdout}"
    );
    assert!(
        stdout.contains("add_i64(10, 20) = 30"),
        "a 64-bit call must cross and come back:\n{stdout}"
    );
}

/// The header FOL generates is full of typedefs -- `fol_status_t` on every
/// routine, and `int32_t` on most -- and typedefs used to be refused wholesale
/// as named aggregates. That made FOL's own C surface unimportable by FOL, and
/// it made every `stdint.h`-typed third-party header unimportable too.
#[test]
fn a_typedef_resolves_to_the_type_it_names() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the round-trip lane");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_roundtrip_typedef");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let prefix = build_library(fixture.path(), &compiler, &temp);
    let consumer = stage_consumer(fixture.path(), &prefix);
    let (ok, text) = bind(&consumer, &compiler, &temp);
    assert!(ok, "the bind should succeed:\n{text}");

    // `fol_status_t` is `int32_t` is `int`, so the manifest must record the
    // measured 32-bit integer and not a named type of any kind.
    let manifest =
        std::fs::read_to_string(consumer.join("interop/folslice.folabi.json")).expect("manifest");
    assert!(
        !manifest.contains("fol_status_t") && !manifest.contains("int32_t"),
        "the manifest kept a C spelling instead of the measured type:\n{manifest}"
    );
    assert!(
        manifest.contains("\"scalar\":\"i32\"") && manifest.contains("\"scalar\":\"i64\""),
        "the manifest should record the measured integers:\n{manifest}"
    );
}

/// The installed C surface, measured independently, must describe the same
/// thing the export manifest claims.
///
/// The export manifest is what FOL *says* it built. The import manifest is
/// what PARC, LINC, and GERC *measured* from the installed header and archive,
/// with no access to FOL's own record. Comparing them is the only check that
/// can catch the export manifest describing a library that was not built that
/// way -- every other test reads one side or the other.
///
/// The two are related by the wrapper convention rather than being identical:
/// an exported `add_i32(int32, int32) -> int32` is a C
/// `fol_slice_add_i32(int32_t, int32_t, int32_t *out)` returning a status. So
/// the exported parameters must be the imported *in* parameters, and the
/// exported result must be what the imported out-parameter points at.
#[test]
fn the_measured_surface_matches_the_declared_export_surface() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the round-trip lane");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_roundtrip_correlate");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");

    let prefix = build_library(fixture.path(), &compiler, &temp);
    let consumer = stage_consumer(fixture.path(), &prefix);
    let (ok, text) = bind(&consumer, &compiler, &temp);
    assert!(ok, "the bind should succeed:\n{text}");

    let export: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(prefix.join("share/fol/abi/v4_c_export_scalar.folabi.json"))
            .expect("the export manifest should install"),
    )
    .expect("the export manifest should parse");
    let import: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(consumer.join("interop/folslice.folabi.json"))
            .expect("the import manifest should be written"),
    )
    .expect("the import manifest should parse");

    let export = &export["interface"];
    let import = &import["interface"];
    assert_eq!(
        export["target"], import["target"],
        "the two sides disagree about the target"
    );

    let scalar = |types: &serde_json::Value, id: u64| -> String {
        types
            .as_array()
            .expect("types is an array")
            .iter()
            .find(|entry| entry["id"] == id)
            .map(|entry| {
                entry["scalar"]
                    .as_str()
                    .unwrap_or_else(|| entry["kind"].as_str().unwrap_or("?"))
                    .to_string()
            })
            .expect("every referenced type is in the table")
    };
    let pointee = |types: &serde_json::Value, id: u64| -> u64 {
        types
            .as_array()
            .expect("types is an array")
            .iter()
            .find(|entry| entry["id"] == id)
            .and_then(|entry| entry["target"].as_u64())
            .expect("an out parameter is a pointer")
    };

    // `bol` and `chr` are FOL's validated scalars: the header carries them as
    // `uint8_t` and `uint32_t` with the valid range stated in a comment, and
    // the import measures the representation because that is all C has. This
    // mapping is the ABI's representation choice, so it is pinned here rather
    // than papered over.
    fn representation(declared: &str) -> &str {
        match declared {
            "bol" => "u8",
            "chr" => "u32",
            other => other,
        }
    }

    let exported: std::collections::HashMap<String, &serde_json::Value> = export["routines"]
        .as_array()
        .expect("routines is an array")
        .iter()
        .map(|routine| {
            (
                routine["symbol"].as_str().expect("a symbol").to_string(),
                routine,
            )
        })
        .collect();

    let imported = import["routines"].as_array().expect("routines is an array");
    assert!(
        imported.len() >= 9,
        "the round trip should correlate the whole bound surface, not a token one"
    );

    for routine in imported {
        let symbol = routine["symbol"].as_str().expect("a symbol");
        let declared = exported
            .get(symbol)
            .unwrap_or_else(|| panic!("{symbol} was measured but is not an exported symbol"));

        assert_eq!(
            declared["convention"], routine["convention"],
            "{symbol}: calling convention disagrees"
        );

        let parameters = routine["parameters"].as_array().expect("parameters");
        let inputs: Vec<String> = parameters
            .iter()
            .filter(|parameter| parameter["direction"] == "in")
            .map(|parameter| {
                scalar(
                    &import["types"],
                    parameter["type"].as_u64().expect("a type id"),
                )
            })
            .collect();
        let expected: Vec<String> = declared["parameters"]
            .as_array()
            .expect("parameters")
            .iter()
            .map(|parameter| {
                let declared = scalar(
                    &export["types"],
                    parameter["type"].as_u64().expect("a type id"),
                );
                representation(&declared).to_string()
            })
            .collect();
        assert_eq!(inputs, expected, "{symbol}: parameter types disagree");

        // The exported result travels through the out parameter.
        let out = parameters
            .iter()
            .find(|parameter| parameter["direction"] == "out")
            .unwrap_or_else(|| panic!("{symbol}: no out parameter was measured"));
        let measured = scalar(
            &import["types"],
            pointee(&import["types"], out["type"].as_u64().expect("a type id")),
        );
        let result = declared["result"].as_u64().expect("a result type id");
        let expected = representation(&scalar(&export["types"], result)).to_string();
        assert_eq!(
            measured, expected,
            "{symbol}: the out parameter does not carry the exported result type"
        );
    }
}
