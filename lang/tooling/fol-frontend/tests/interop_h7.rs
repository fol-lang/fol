use fol_frontend::{
    execute_workspace_build_route, load_frontend_workspace, require_discovered_root,
    FrontendArtifactKind, FrontendConfig, FrontendProfile, FrontendWorkspaceBuildRequest,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fol-h7-smoke-{}-{}",
            std::process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create H7 scratch root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn build_fol_c_import_runs_the_locked_typed_pipeline() {
    let required = std::env::var_os("FOL_H7_REQUIRED").is_some();
    let Some(gcc) = std::env::var_os("FOL_H7_GCC") else {
        if required {
            panic!("make test-interop requires an explicit FOL_H7_GCC");
        }
        eprintln!("H7 system smoke skipped; run `make test-interop` to require it");
        return;
    };
    let gcc = canonical_tool(gcc, "GCC");
    let scratch = Scratch::new();
    let package = scratch.path().join("package");
    let probe_parent = scratch.path().join("probes");
    fs::create_dir_all(package.join("src")).expect("create fixture source directory");
    fs::create_dir_all(package.join("native")).expect("create fixture native directory");
    fs::create_dir_all(&probe_parent).expect("create explicit LINC temporary parent");

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("interop-fixtures")
        .join("h7");
    copy_fixture(&fixture, &package, "build.fol");
    copy_fixture(&fixture, &package, "src/main.fol");
    copy_fixture(&fixture, &package, "native/provider.h");
    copy_fixture(&fixture, &package, "native/provider.c");

    // Select a per-test-process value that cannot be baked into the fixed H7
    // anchor source while retaining 42 as the fixture's standalone default.
    let expected_value = 1_000_000 + (std::process::id() % 1_000_000) as i32;
    let provider = package.join("native/provider.o");
    let output = Command::new(&gcc)
        .env_clear()
        .current_dir(&package)
        .args(["-std=gnu17", "-m64", "-fPIC", "-c"])
        .arg(format!("-DFOL_H7_VALUE={expected_value}"))
        .arg("native/provider.c")
        .arg("-o")
        .arg(&provider)
        .output()
        .expect("launch explicit H7 GCC provider compilation");
    assert!(
        output.status.success(),
        "GCC provider compilation failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        provider.is_file(),
        "GCC did not produce the selected object"
    );

    let build_root = scratch.path().join("build");
    let config = FrontendConfig {
        working_directory: package.clone(),
        build_root_override: Some(build_root.clone()),
        cache_root_override: Some(scratch.path().join("cache")),
        git_cache_root_override: Some(scratch.path().join("git-cache")),
        install_prefix_override: Some(scratch.path().join("install")),
        build_target_override: Some(fol_interop::CERTIFIED_INTEROP_TARGET.to_owned()),
        build_optimize_override: Some("debug".to_owned()),
        interop_compiler_override: Some(gcc),
        interop_temporary_parent_override: Some(probe_parent),
        keep_build_dir: true,
        ..FrontendConfig::default()
    };
    let discovered = require_discovered_root(&package).expect("discover copied H7 package");
    let workspace = load_frontend_workspace(&discovered, &config).expect("load H7 package");
    let result = execute_workspace_build_route(
        &workspace,
        &config,
        &FrontendWorkspaceBuildRequest {
            requested_step: "build".to_owned(),
            profile: FrontendProfile::Debug,
            run_args: Vec::new(),
        },
    )
    .expect("build the authoritative build.fol H7 route");

    assert_eq!(result.command, "build");
    let binaries = result
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    assert_eq!(binaries.len(), 1);
    let binary = binaries[0]
        .path
        .as_ref()
        .expect("H7 build result must retain the linked binary path");

    // The fixed H7 anchor must contain the real GERC-projected raw-symbol call.
    // This exact source assertion prevents a constant-return/no-call anchor from
    // satisfying the runtime value check below merely by returning `42` itself.
    let anchor = find_single_h7_anchor_source(&build_root);
    let anchor_source = fs::read_to_string(&anchor).expect("read materialized H7 anchor");
    assert_eq!(
        anchor_source,
        concat!(
            "#![no_std]\n",
            "#[inline(never)]\n",
            "pub fn fol_h7_read_provider() -> i32 {\n",
            "unsafe { fol_h7_raw::fol_h7_value() as i32 }\n",
            "}\n",
        )
    );
    assert!(
        !anchor_source.contains(&expected_value.to_string()),
        "H7 anchor injected the test's expected provider value"
    );

    let output = Command::new(binary)
        .current_dir(&package)
        .output()
        .expect("execute the linked H7 binary directly");
    assert!(
        output.status.success(),
        "linked H7 binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        format!("{expected_value}\n").into_bytes(),
        "H7 did not observe the C value"
    );
    let evidence = result
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == FrontendArtifactKind::InteropEvidence)
        .expect("build result must report exact sibling evidence");
    for required_identity in [
        "target=x86_64-unknown-linux-gnu",
        "parc=ba603cdccc9375473eca0c42e5462cf90b6da249",
        "linc=37c8fb16171114b39e2283ff4b9e351fa2d5975b",
        "gerc=423b14aec40f509de64152ec1fcc74a9371154f1",
        "source=psource2_",
        "evidence=lanalysis2_",
        "generation=gprojection1_",
        "provider=lartifact1_",
    ] {
        assert!(
            evidence.label.contains(required_identity),
            "interop evidence omitted {required_identity}: {}",
            evidence.label
        );
    }
}

#[test]
fn c_import_pipeline_fails_closed_at_each_owner() {
    let Some(gcc) = explicit_gcc() else {
        return;
    };
    let cases = [
        (
            "partial-source",
            "_BitInt(17) fol_h7_value(_BitInt(17) value);\n",
            "int unrelated_provider(void) { return 1; }\n",
            "PARC source",
        ),
        (
            "unresolved-provider",
            "int fol_h7_value(void);\n",
            "int unrelated_provider(void) { return 1; }\n",
            "LINC certification failed",
        ),
        (
            "generation-rejection",
            "extern _Thread_local int fol_h7_tls;\nint fol_h7_value(void);\n",
            "_Thread_local int fol_h7_tls = 42;\nint fol_h7_value(void) { return fol_h7_tls; }\n",
            "GERC generation failed",
        ),
    ];

    for (label, header, provider, expected_error) in cases {
        let (result, generated_files, generated_output_exists) =
            run_failing_case(&gcc, label, header, provider);
        let error = result.expect_err("negative H7 case must fail closed");
        assert!(
            error.message().contains(expected_error),
            "{label} failed at the wrong boundary: {error}"
        );
        assert_eq!(
            generated_files, 0,
            "{label} wrote generated/backend files before rejection"
        );
        assert!(
            !generated_output_exists,
            "{label} created interop-generated before rejection"
        );
    }
}

fn explicit_gcc() -> Option<PathBuf> {
    let required = std::env::var_os("FOL_H7_REQUIRED").is_some();
    match std::env::var_os("FOL_H7_GCC") {
        Some(gcc) => Some(canonical_tool(gcc, "GCC")),
        None if required => panic!("make test-interop requires an explicit FOL_H7_GCC"),
        None => {
            eprintln!("H7 system smoke skipped; run `make test-interop` to require it");
            None
        }
    }
}

fn run_failing_case(
    gcc: &Path,
    label: &str,
    header: &str,
    provider_source: &str,
) -> (
    fol_frontend::FrontendResult<fol_frontend::FrontendCommandResult>,
    usize,
    bool,
) {
    let scratch = Scratch::new();
    let package = scratch.path().join("package");
    let probe_parent = scratch.path().join("probes");
    fs::create_dir_all(package.join("src")).expect("create negative fixture source directory");
    fs::create_dir_all(package.join("native")).expect("create negative fixture native directory");
    fs::create_dir_all(&probe_parent).expect("create negative LINC temporary parent");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("interop-fixtures")
        .join("h7");
    copy_fixture(&fixture, &package, "build.fol");
    copy_fixture(&fixture, &package, "src/main.fol");
    fs::write(package.join("native/provider.h"), header).expect("write negative header");
    fs::write(package.join("native/provider.c"), provider_source).expect("write negative provider");

    let provider = package.join("native/provider.o");
    let output = Command::new(gcc)
        .env_clear()
        .current_dir(&package)
        .args(["-std=gnu17", "-m64", "-fPIC", "-c"])
        .arg("native/provider.c")
        .arg("-o")
        .arg(&provider)
        .output()
        .unwrap_or_else(|error| panic!("launch GCC for {label}: {error}"));
    assert!(
        output.status.success(),
        "GCC setup for {label} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let build_root = scratch.path().join("build");
    let config = FrontendConfig {
        working_directory: package.clone(),
        build_root_override: Some(build_root.clone()),
        build_target_override: Some(fol_interop::CERTIFIED_INTEROP_TARGET.to_owned()),
        build_optimize_override: Some("debug".to_owned()),
        interop_compiler_override: Some(gcc.to_owned()),
        interop_temporary_parent_override: Some(probe_parent),
        keep_build_dir: true,
        ..FrontendConfig::default()
    };
    let discovered = require_discovered_root(&package).expect("discover negative H7 package");
    let workspace = load_frontend_workspace(&discovered, &config).expect("load negative package");
    let result = execute_workspace_build_route(
        &workspace,
        &config,
        &FrontendWorkspaceBuildRequest {
            requested_step: "run".to_owned(),
            profile: FrontendProfile::Debug,
            run_args: Vec::new(),
        },
    );
    let generated_files = count_regular_files(&build_root);
    let generated_output_exists = build_root.join("debug/interop-generated").exists();
    (result, generated_files, generated_output_exists)
}

fn count_regular_files(root: &Path) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    entries
        .map(|entry| entry.expect("read generated entry").path())
        .map(|path| {
            if path.is_dir() {
                count_regular_files(&path)
            } else if path.is_file() {
                1
            } else {
                0
            }
        })
        .sum()
}

fn find_single_h7_anchor_source(root: &Path) -> PathBuf {
    fn visit(root: &Path, anchors: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(root).unwrap_or_else(|error| {
            panic!("read H7 output directory '{}': {error}", root.display())
        });
        for entry in entries {
            let path = entry.expect("read H7 output entry").path();
            if path.is_dir() {
                visit(&path, anchors);
            } else if path.is_file()
                && path.file_name().is_some_and(|name| name == "lib.rs")
                && path
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "anchor")
            {
                anchors.push(path);
            }
        }
    }

    let mut anchors = Vec::new();
    visit(root, &mut anchors);
    assert_eq!(
        anchors.len(),
        1,
        "H7 build must materialize exactly one anchor source: {anchors:?}"
    );
    anchors.pop().expect("one anchor source checked above")
}

fn canonical_tool(path: OsString, role: &str) -> PathBuf {
    let supplied = PathBuf::from(path);
    let canonical = fs::canonicalize(&supplied).unwrap_or_else(|error| {
        panic!(
            "could not canonicalize {role} '{}': {error}",
            supplied.display()
        )
    });
    assert!(canonical.is_file(), "{role} is not a regular file");
    canonical
}

fn copy_fixture(fixture: &Path, package: &Path, relative: &str) {
    let destination = package.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("create fixture parent");
    }
    fs::copy(fixture.join(relative), &destination)
        .unwrap_or_else(|error| panic!("copy H7 fixture '{relative}': {error}"));
}
