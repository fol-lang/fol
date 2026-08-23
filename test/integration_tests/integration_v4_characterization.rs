//! Characterization tests for V4 milestone M0.
//!
//! These pin down behaviour that is currently lossy or untruthful, so that the
//! milestone which fixes each one has to change a test rather than quietly
//! change an outcome. A characterization test is not an endorsement: every
//! assertion below records what the compiler does today, and each names the
//! milestone that is expected to replace it.

use super::*;

fn strip_ansi(value: &str) -> String {
    let mut stripped = String::with_capacity(value.len());
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
        stripped.push(ch);
    }
    stripped
}

fn store_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lang/library")
}

/// A package whose graph declares one static library and no executable.
fn library_only_package(fixture: &crate::fixture::TempFixture) -> PathBuf {
    let root = fixture.path().join("libonly");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "libonly", version = "0.1.0", kind = "lib",
        description = "library-only graph, no executable",
        license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var lib = graph.add_static_lib({
        name = "libonly", root = "src/lib.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    graph.install(lib);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/lib.fol"),
        "fun[] add(a: int, b: int): int = {\n    return a + b;\n};\n",
    )
    .expect("lib.fol should be writable");
    root
}

/// M3 positive regression: a library-only graph builds a real library.
///
/// This began as an M0 characterization of a graph with no runnable route at
/// all, became an M1 characterization of a precise not-yet-supported error, and
/// is now the thing itself: the artifact is produced and installed under the
/// frozen layout. `test/native_products.rs` proves a C program can link it.
#[test]
fn library_only_graph_builds_and_installs_a_real_library() {
    let fixture = unique_temp_root("v4_library_only_build");
    let package = library_only_package(&fixture);

    let output = run_fol_in_dir(
        &package,
        &[
            "code",
            "build",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    assert!(
        output.status.success(),
        "a library-only graph should build; got:\n{text}"
    );
    assert!(
        text.contains("[static-library]"),
        "the pre-build summary should name the product kind:\n{text}"
    );
    assert!(
        package.join(".fol/install/lib/liblibonly.a").is_file(),
        "the archive did not install under the frozen layout:\n{text}"
    );
}

/// M1 positive regression: the library-only diagnostic must not advertise a
/// step the CLI cannot run.
///
/// M0 characterized the old message listing "known steps: check, install" while
/// `fol code` has no `install` subcommand -- the one route a library-only
/// package was offered could not be taken. The precise diagnostic no longer
/// lists steps, and this fails if a step catalog comes back with `install` in
/// it while the subcommand still does not exist.
#[test]
fn the_library_only_diagnostic_advertises_no_unreachable_step() {
    let fixture = unique_temp_root("v4_library_only_install");
    let package = library_only_package(&fixture);
    let store = store_root();
    let store = store.to_str().expect("store root should be utf-8");

    let advertised = run_fol_in_dir(&package, &["code", "build", "--package-store-root", store]);
    let advertised_text = strip_ansi(&String::from_utf8_lossy(&advertised.stderr));

    let attempted = run_fol_in_dir(
        &package,
        &["code", "install", "--package-store-root", store],
    );
    let attempted_text = strip_ansi(&String::from_utf8_lossy(&attempted.stderr));

    assert!(
        !attempted.status.success(),
        "`fol code install` now runs; drop this guard and test the step instead"
    );
    assert!(
        attempted_text.contains("unknown code subcommand: install"),
        "expected the subcommand to be rejected outright, got:\n{attempted_text}"
    );
    assert!(
        !advertised_text.contains("install"),
        "the build diagnostic offers `install`, which no subcommand can run:\n{advertised_text}"
    );
}

/// `check` succeeds on a library-only package, so the graph itself is sound and
/// only the build route is missing. This is what makes the two tests above a
/// routing defect rather than a compilation one.
#[test]
fn library_only_graph_typechecks_even_though_it_cannot_be_built() {
    let fixture = unique_temp_root("v4_library_only_check");
    let package = library_only_package(&fixture);

    let output = run_fol_in_dir(
        &package,
        &[
            "code",
            "check",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );

    assert!(
        output.status.success(),
        "a library-only graph should typecheck; got:\n{}",
        strip_ansi(&String::from_utf8_lossy(&output.stderr))
    );
}

/// Two artifacts may declare the same public name. Nothing rejects it, and the
/// evaluator's name map keeps only the last one, so `install`/`add_run` by name
/// silently resolve to whichever was declared later while both artifacts remain
/// in the graph under distinct positional IDs. M1's deterministic plan identity
/// has to make this either an error or a distinction that survives.
#[test]
fn two_artifacts_may_share_one_public_name_without_diagnosis() {
    let fixture = unique_temp_root("v4_same_named_artifacts");
    let root = fixture.path().join("duped");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "duped", version = "0.1.0", kind = "exe",
        description = "two artifacts share one public name",
        license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var first = graph.add_exe({
        name = "duped", root = "src/main.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    var second = graph.add_exe({
        name = "duped", root = "src/main.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    graph.install(first);
    graph.add_run(second);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"FIRST\");\n    return;\n};\n",
    )
    .expect("main.fol should be writable");

    let output = run_fol_in_dir(
        &root,
        &[
            "code",
            "run",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    assert!(
        output.status.success(),
        "duplicate artifact names are accepted today; got:\n{text}"
    );
    assert!(
        !text.contains("found 1 error") && !text.contains("conflicts with"),
        "a same-name diagnostic appeared; convert this test at M1:\n{text}"
    );
}

/// Workspace identity is a hash of the rendered lowered source and nothing
/// else, so a debug build and a release build of the same package produce the
/// same crate directory name. Only the output path keeps them apart, which
/// makes profile a naming convention rather than part of identity. M1 folds
/// kind, target, model, and profile into deterministic plan identity.
#[test]
fn workspace_identity_ignores_the_build_profile() {
    let fixture = unique_temp_root("v4_identity_ignores_profile");
    let root = fixture.path().join("profiled");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "profiled", version = "0.1.0", kind = "exe",
        description = "one source, two profiles",
        license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var app = graph.add_exe({
        name = "profiled", root = "src/main.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    graph.install(app);
    graph.add_run(app);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"ONE\");\n    return;\n};\n",
    )
    .expect("main.fol should be writable");

    let store = store_root();
    let store = store.to_str().expect("store root should be utf-8");
    let mut identities = Vec::new();
    for profile in ["--debug", "--release"] {
        let output = run_fol_in_dir(
            &root,
            &["code", "build", profile, "--package-store-root", store],
        );
        let text = strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
        assert!(
            output.status.success(),
            "the {profile} build should succeed; got:\n{text}"
        );
        let name = text
            .split_whitespace()
            .find_map(|token| {
                token
                    .rsplit('/')
                    .next()
                    .filter(|candidate| candidate.starts_with("fol-build-profiled-"))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| {
                panic!("no crate identity appeared in the {profile} output:\n{text}")
            });
        identities.push(name);
    }

    assert_eq!(
        identities[0], identities[1],
        "profile has become part of workspace identity; convert this test at M1"
    );
}

/// The ABI export spelling frozen in section 4.15, now implemented.
///
/// M0 checked the fixture in while the evaluator rejected it. M5 made it real,
/// so the same file now builds and produces a C surface -- which is a stronger
/// guarantee than the rejection was.
#[test]
fn the_frozen_abi_export_spelling_is_implemented() {
    let fixture = unique_temp_root("v4_frozen_abi_export");
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fail_v4_contract_abi_export");
    let root = fixture.path().join("abi_export");
    std::fs::create_dir_all(root.join("src")).expect("package tree");
    for name in ["build.fol"] {
        std::fs::copy(source.join(name), root.join(name)).expect("copy");
    }
    std::fs::copy(source.join("src/lib.fol"), root.join("src/lib.fol")).expect("copy");

    let output = run_fol_in_dir(
        &root,
        &[
            "code",
            "build",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    assert!(
        output.status.success(),
        "the frozen ABI export spelling should now build:\n{text}"
    );
    assert!(
        root.join(".fol/install/include/fail_v4_contract_abi_export.h")
            .is_file(),
        "the export produced no header:\n{text}"
    );
}

/// The frozen `build.fol` spellings from `plan/V4_PLAN.md` section 4.15.
///
/// Each fixture writes a spelling this plan has committed to and asserts what
/// the evaluator does with it today. The point is not the rejection: it is that
/// renaming a method or a field breaks a checked-in file, so a later milestone
/// cannot quietly introduce a second name for something already named.
#[test]
fn frozen_build_api_spellings_keep_their_exact_names() {
    let store = store_root();
    let store = store.to_str().expect("store root should be utf-8");
    let cases = [
        // `set_abi_version` and `add_abi_export` are implemented as of M5, so
        // this fixture now *builds*. It stays in the freeze set because the
        // point was never the rejection -- it is that the spelling is checked
        // in, so renaming the method breaks a file rather than passing quietly.
        (
            "fail_v4_contract_native_file",
            "unsupported build API call",
            "add_native_file",
        ),
        (
            "fail_v4_contract_c_import_fields",
            "unknown field 'target'; add_c_import accepts header, provider, provider_kind",
            "add_c_import",
        ),
    ];

    for (fixture, expected, spelling) in cases {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(fixture);
        let output = run_fol_in_dir(&root, &["code", "check", "--package-store-root", store]);
        let text = strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));

        assert!(
            !output.status.success(),
            "{fixture} should be rejected while {spelling} is unimplemented; got:\n{text}"
        );
        assert!(
            text.contains(expected),
            "{fixture} should report `{expected}`; got:\n{text}"
        );
        assert!(
            text.contains(spelling),
            "{fixture}'s diagnostic should name `{spelling}`, which is what freezes the \
             spelling; got:\n{text}"
        );
    }
}

/// M1 positive regression: an unsupported target is refused before anything is
/// created.
///
/// Letting one through used to mean an output tree, a launched `rustc`, and a
/// raw `E0463` advising `rustup target add` -- advice this project does not
/// follow, since `flake.nix` is the only toolchain source.
#[test]
fn an_experimental_target_is_refused_before_any_output_exists() {
    let fixture = unique_temp_root("v4_experimental_target");
    let root = fixture.path().join("targeted");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "targeted", version = "0.1.0", kind = "exe",
        description = "target selection", license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var app = graph.add_exe({
        name = "targeted", root = "src/main.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    graph.install(app);
    graph.add_run(app);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"ONE\");\n    return;\n};\n",
    )
    .expect("main.fol should be writable");

    let output = run_fol_in_dir(
        &root,
        &[
            "code",
            "build",
            "--target",
            "aarch64-apple-darwin",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));

    assert!(!output.status.success(), "got:\n{text}");
    assert!(
        text.contains("is experimental and is not built by V4"),
        "expected the tier diagnostic, got:\n{text}"
    );
    assert!(
        !text.contains("rustup") && !text.contains("E0463"),
        "rustc's own error leaked through, so the check ran too late:\n{text}"
    );
    assert!(
        !root.join(".fol").exists(),
        "the build left an output tree behind despite refusing the target"
    );
}

/// The other half of the target gate: naming an unbuildable target must not
/// stop the package from typechecking.
///
/// The first version of this check ran during build evaluation, which `check`
/// also uses, so `fol code check` started failing on a package it had no reason
/// to reject. Typechecking is target-independent; only building is not.
#[test]
fn an_experimental_target_still_typechecks() {
    let fixture = unique_temp_root("v4_experimental_target_check");
    let root = fixture.path().join("targeted");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "targeted", version = "0.1.0", kind = "exe",
        description = "an artifact naming an experimental target",
        license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var optimize = graph.standard_optimize();
    var app = graph.add_exe({
        name = "targeted", root = "src/main.fol", fol_model = "memo",
        target = "aarch64-apple-darwin", optimize = optimize,
    });
    graph.install(app);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"ONE\");\n    return;\n};\n",
    )
    .expect("main.fol should be writable");

    let store = store_root();
    let store = store.to_str().expect("store root should be utf-8");

    let checked = run_fol_in_dir(&root, &["code", "check", "--package-store-root", store]);
    assert!(
        checked.status.success(),
        "check should not care that the target is unbuildable; got:\n{}",
        strip_ansi(&String::from_utf8_lossy(&checked.stderr))
    );

    let built = run_fol_in_dir(&root, &["code", "build", "--package-store-root", store]);
    let text = strip_ansi(&String::from_utf8_lossy(&built.stderr));
    assert!(
        !built.status.success(),
        "build should refuse it; got:\n{text}"
    );
    assert!(
        text.contains("is experimental and is not built by V4"),
        "expected the tier diagnostic, got:\n{text}"
    );
}

/// Section 4.4 target precedence, end to end: command-line override, then the
/// artifact's own target, then the resolved host.
///
/// The order was implemented but never asserted through the CLI, so nothing
/// would have caught a change that let the artifact target shadow an explicit
/// `--target`.
#[test]
fn target_precedence_prefers_cli_then_artifact_then_host() {
    let fixture = unique_temp_root("v4_target_precedence");
    let store = store_root();
    let store = store.to_str().expect("store root should be utf-8");
    let host = fol_types::ResolvedTarget::host().expect("host should resolve");
    // A certified target that is not the host, so "which one won" is visible in
    // the output path.
    let cross = fol_types::TARGETS
        .iter()
        .find(|facts| {
            facts.tier == fol_types::TargetTier::Certified && facts.rust_triple != host.as_str()
        })
        .expect("a certified non-host target should exist");

    let build_package = |name: &str, artifact_target: Option<&str>| -> PathBuf {
        let root = fixture.path().join(name);
        std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
        let target_field = match artifact_target {
            Some(triple) => format!("target = \"{triple}\""),
            None => "target = target".to_string(),
        };
        std::fs::write(
            root.join("build.fol"),
            format!(
                r#"pro[] build(): non = {{
    var build = .build();
    build.meta({{
        name = "{name}", version = "0.1.0", kind = "exe",
        description = "target precedence", license = "MIT",
    }});
    build.add_dep({{ alias = "std", source = "internal", target = "standard" }});
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var app = graph.add_exe({{
        name = "{name}", root = "src/main.fol", fol_model = "memo",
        {target_field}, optimize = optimize,
    }});
    graph.install(app);
    return;
}};
"#
            ),
        )
        .expect("build.fol should be writable");
        std::fs::write(
            root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"ONE\");\n    return;\n};\n",
        )
        .expect("main.fol should be writable");
        root
    };

    let built_target = |root: &PathBuf, args: &[&str]| -> String {
        let mut argv = vec!["code", "build"];
        argv.extend_from_slice(args);
        argv.extend_from_slice(&["--package-store-root", store]);
        let output = run_fol_in_dir(root, &argv);
        let text = strip_ansi(&format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
        assert!(
            output.status.success(),
            "build should succeed; got:\n{text}"
        );
        // The binary lands under a directory named for the selected target.
        fol_types::TARGETS
            .iter()
            .find(|facts| text.contains(&format!("/bin/{}/", facts.rust_triple)))
            .map(|facts| facts.rust_triple.to_string())
            .unwrap_or_else(|| panic!("no target directory in the build output:\n{text}"))
    };

    // Host, with no override anywhere.
    let plain = build_package("plain", None);
    assert_eq!(built_target(&plain, &[]), host.as_str());

    // The artifact's own target beats the host.
    let pinned = build_package("pinned", Some(cross.rust_triple));
    assert_eq!(built_target(&pinned, &[]), cross.rust_triple);

    // An explicit CLI override beats the artifact's own target.
    assert_eq!(
        built_target(&pinned, &["--target", host.as_str()]),
        host.as_str(),
        "an explicit --target must outrank the artifact's target"
    );
}

/// M1: the frontend says what it selected before it compiles it.
///
/// The existing summary reports the model and target after the build, and never
/// reports the artifact kind. On a slow build that is too late to notice a
/// wrong selection.
#[test]
fn the_frontend_announces_kind_target_and_model_before_compiling() {
    let fixture = unique_temp_root("v4_pre_build_summary");
    let root = fixture.path().join("announced");
    std::fs::create_dir_all(root.join("src")).expect("package tree should be creatable");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "announced", version = "0.1.0", kind = "exe",
        description = "pre-build summary", license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var target = graph.standard_target();
    var optimize = graph.standard_optimize();
    var app = graph.add_exe({
        name = "announced", root = "src/main.fol", fol_model = "memo",
        target = target, optimize = optimize,
    });
    graph.install(app);
    return;
};
"#,
    )
    .expect("build.fol should be writable");
    std::fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\n\nfun[] main(): non = {\n    std::io::echo_str(\"ONE\");\n    return;\n};\n",
    )
    .expect("main.fol should be writable");

    let host = fol_types::ResolvedTarget::host().expect("host should resolve");
    let output = run_fol_in_dir(
        &root,
        &[
            "code",
            "build",
            "--package-store-root",
            store_root().to_str().expect("store root should be utf-8"),
        ],
    );
    let text = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "build should succeed; got:\n{text}"
    );

    let announcement = text
        .lines()
        .find(|line| line.starts_with("building announced"))
        .unwrap_or_else(|| panic!("no pre-build announcement in:\n{text}"));
    assert!(
        announcement.contains("[executable]"),
        "the announcement should name the artifact kind: {announcement}"
    );
    assert!(
        announcement.contains(host.as_str()),
        "the announcement should name the target: {announcement}"
    );
}
