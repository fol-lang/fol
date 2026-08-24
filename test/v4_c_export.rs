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

/// Build any checked-in example by name, unchanged.
///
/// `build_slice` exists to rewrite the scalar example's artifact kind; this
/// one takes a different example as-is, which is what a second slice needs.
fn build_named_slice(fixture: &fol_testkit::TempFixture, example: &str) -> PathBuf {
    let source = repo_root().join("examples").join(example);
    let root = fixture.path().join(example);
    copy_dir(&source, &root);

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

/// The frontend reports each ABI output under its own role.
///
/// Lumping them under `installed` would leave a consumer of the JSON unable to
/// find the header without guessing at paths.
#[test]
fn the_frontend_reports_every_abi_role() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_roles");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let source = repo_root().join("examples/v4_c_export_scalar");
    let root = fixture.path().join("slice");
    copy_dir(&source, &root);

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

    for role in [
        "c-header",
        "abi-manifest",
        "symbol-allowlist",
        "static-library",
    ] {
        assert!(
            text.contains(role),
            "the JSON output does not list the {role} role:\n{text}"
        );
    }
}

/// A std-free library exports C symbols without declaring bundled `std`.
///
/// The scalar slice touches no hosted capability, so requiring a `standard`
/// dependency for it would make every C export drag in the standard library.
#[test]
fn a_std_free_library_exports_without_bundled_std() {
    let Some(cc) = c_compiler() else {
        eprintln!("skipping: no C compiler on PATH");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_stdfree");
    std::fs::create_dir_all(fixture.path()).expect("fixture root");
    let root = fixture.path().join("stdfree");
    std::fs::create_dir_all(root.join("src")).expect("package tree");

    // No `build.add_dep` for `std` at all.
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "stdfree", version = "0.1.0", kind = "lib",
        description = "a scalar export with no bundled std", license = "MIT",
    });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var lib = graph.add_static_lib({
        name = "stdfree", root = "src/lib.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    lib.set_abi_version({ major = 1, minor = 0 });
    lib.add_abi_export({ routine = "triple", symbol = "fol_stdfree_triple" });
    graph.install(lib);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/lib.fol"),
        "fun[exp] triple(value: int): int = {\n    return value * 3;\n};\n",
    )
    .expect("lib.fol should be writable");

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
        "a std-free library should build:\n{text}"
    );

    let prefix = root.join(".fol/install");
    let library = prefix.join("lib/libstdfree.a");
    assert!(library.is_file(), "the library did not install:\n{text}");

    let consumer = fixture.path().join("consumer.c");
    std::fs::write(
        &consumer,
        "#include \"stdfree.h\"\n\
         #include <stdio.h>\n\
         int main(void) {\n\
         \x20   int64_t out = 0;\n\
         \x20   if (fol_stdfree_triple(14, &out) != FOL_STATUS_OK) { return 1; }\n\
         \x20   printf(\"%lld\\n\", (long long)out);\n\
         \x20   return out == 42 ? 0 : 2;\n\
         }\n",
    )
    .expect("the consumer should be writable");

    let binary = fixture.path().join("stdfree_consumer");
    let link = Command::new(&cc)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(prefix.join("include"))
        .arg(&consumer)
        .arg(&library)
        .args(["-lpthread", "-ldl", "-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("the C compiler should run");
    assert!(
        link.status.success(),
        "the std-free consumer failed to link:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let run = Command::new(&binary)
        .output()
        .expect("the consumer should run");
    assert!(
        run.status.success(),
        "the std-free consumer failed at runtime"
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "42");
}

/// M7.1: a POD record crosses as a canonical C struct.
///
/// The header's `_Static_assert`s are the layout half of the gate: FOL
/// computes size, alignment, and every offset from the System V rules, and
/// this consumer fails to compile if the C compiler recomputes any of them
/// differently. The calls are the behaviour half -- a struct passed by value
/// arrives intact, and one returned comes back the same way.
#[test]
fn a_record_crosses_as_a_c_struct_whose_layout_c_agrees_with() {
    let Some(compiler) = c_compiler() else {
        eprintln!("skipping: no C compiler for the record slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_record");
    let prefix = build_named_slice(&fixture, "v4_c_record");

    let consumer = repo_root().join("examples/v4_c_record/consumer.c");
    let binary = fixture.path().join("record_consumer");
    let library = prefix.join("lib/libv4_c_record.a");
    let output = Command::new(&compiler)
        .arg("-std=c11")
        .arg("-I")
        .arg(prefix.join("include"))
        .arg("-o")
        .arg(&binary)
        .arg(&consumer)
        .arg(&library)
        .args(["-lm", "-lpthread", "-ldl"])
        .output()
        .expect("the C consumer should compile");
    let text = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "the C compiler must agree with FOL's computed layout:\n{text}"
    );

    let run = Command::new(&binary).output().expect("run the C consumer");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "every record check should pass:\n{stdout}"
    );
    assert!(
        stdout.contains("all record checks passed"),
        "got:\n{stdout}"
    );
}

/// M7.3: text is lent into a call, and every bad view is refused.
///
/// The safety argument for the inbound direction is that FOL's `str` owns its
/// bytes, so the callee copies during the call and never retains the caller's
/// pointer. That makes the lifetime rule hold by construction rather than by
/// documentation -- but only if the view itself is checked first, which is
/// what the consumer's null, length, and UTF-8 cases exercise. The sanitized
/// run in `v4_c_sanitize.rs` is the other half: it frees every lent buffer
/// immediately, so a retained pointer would surface as a use-after-free.
#[test]
fn borrowed_text_crosses_inbound_and_bad_views_are_refused() {
    let Some(compiler) = c_compiler() else {
        eprintln!("skipping: no C compiler for the string view slice");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_string_view");
    let prefix = build_named_slice(&fixture, "v4_c_string_view");

    let header = std::fs::read_to_string(prefix.join("include/v4_c_string_view.h"))
        .expect("the header should be installed");
    assert!(
        header.contains(
            "typedef struct {\n    const uint8_t *ptr;\n    size_t len;\n} fol_str_view_t;"
        ),
        "the view is a pointer and a length, not a NUL-terminated string:\n{header}"
    );
    assert!(
        header.contains("fol_str_view_t arg0"),
        "a `str` parameter crosses as a view:\n{header}"
    );

    let consumer = repo_root().join("examples/v4_c_string_view/consumer.c");
    let binary = fixture.path().join("string_view_consumer");
    let library = prefix.join("lib/libv4_c_string_view.a");
    let output = Command::new(&compiler)
        .arg("-std=c11")
        .arg("-I")
        .arg(prefix.join("include"))
        .arg("-o")
        .arg(&binary)
        .arg(&consumer)
        .arg(&library)
        .args(["-lm", "-lpthread", "-ldl"])
        .output()
        .expect("the C consumer should compile");
    assert!(
        output.status.success(),
        "the string view consumer failed to build:\n{}",
        strip_ansi(&String::from_utf8_lossy(&output.stderr))
    );

    let run = Command::new(&binary).output().expect("run the C consumer");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "every string view check should pass:\n{stdout}"
    );
    assert!(
        stdout.contains("all string view checks passed"),
        "got:\n{stdout}"
    );
}

/// M7.2: an entry is refused at the boundary, with the reason.
///
/// FOL has no syntax for an explicit ABI discriminant, and the tag it uses
/// internally is positional — inserting a variant would renumber every later
/// one, which is a silent ABI break. Section 12.2's gate is that a
/// declaration reorder must not renumber tags, and the only honest way to hold
/// that today is to refuse the export and say why.
#[test]
fn an_entry_without_explicit_tags_is_refused_with_its_reason() {
    let fixture = fol_testkit::TempFixture::new("fol_v4_c_entry_reject");
    let source = repo_root().join("examples/fail_v4_c_entry_error");
    let root = fixture.path().join("entry");
    copy_dir(&source, &root);

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
        !output.status.success(),
        "the entry export must fail:\n{text}"
    );
    assert!(
        text.contains("has no explicit discriminants"),
        "the diagnostic should name the missing tags:\n{text}"
    );
    assert!(
        text.contains("Severity"),
        "the diagnostic should name the entry:\n{text}"
    );
    assert!(
        text.contains("renumber"),
        "the diagnostic should say what would break:\n{text}"
    );
}
