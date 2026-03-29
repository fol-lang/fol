#[cfg(test)]
mod tests {
    use crate::emit::{
        backend_build_paths, build_generated_crate_with_rustc, build_runtime_rlib_with_rustc,
        emit_backend_artifact, emit_cargo_toml, emit_generated_crate_skeleton,
        emit_generated_crate_skeleton_for_config, emit_main_rs, emit_main_rs_for_config,
        emit_namespace_module_shells, emit_namespace_module_shells_for_config,
        emit_package_module_shells, prepare_backend_runtime_build_dir, prepare_backend_build_paths,
        prepare_generated_build_dir, summarize_emitted_artifact, write_generated_crate,
        backend_runtime_build_dir, backend_runtime_manifest_path,
        backend_runtime_manifest_path_with_override, backend_runtime_source_entry,
        backend_runtime_source_entry_with_override, backend_runtime_source_root,
        backend_runtime_source_root_with_override,
    };
    use super::super::build::{
        configure_generated_crate_rustc_command, configure_runtime_rustc_command,
    };
    use crate::{
        testing::{
            lowered_workspace_from_entry_path, lowered_workspace_from_entry_path_with_config,
            sample_lowered_workspace,
        },
        BackendArtifact, BackendBuildProfile, BackendConfig, BackendFolModel,
        BackendMachineTarget, BackendMode, BackendSession,
    };
    use fol_package::PackageConfig;
    use fol_resolver::ResolverConfig;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("fol_backend_{label}_{unique}"))
    }

    fn write_fixture(root: &Path, source: &str) -> PathBuf {
        fs::create_dir_all(root).expect("backend fixture root");
        let fixture = root.join("main.fol");
        fs::write(&fixture, source).expect("backend fixture source");
        fixture
    }

    fn build_and_run_fixture(source: &str) -> std::process::Output {
        let fixture_root = temp_root("exec");
        let fixture = write_fixture(&fixture_root, source);
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);
        let artifact = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                ..BackendConfig::default()
            },
            &fixture_root,
        )
        .expect("backend artifact");
        let BackendArtifact::CompiledBinary { binary_path, .. } = artifact else {
            panic!("expected compiled binary artifact");
        };
        let output = Command::new(&binary_path)
            .output()
            .expect("run emitted binary");
        let _ = fs::remove_dir_all(&fixture_root);
        output
    }

    fn cargo_build_generated_crate_for_profile(
        crate_root: &Path,
        profile: BackendBuildProfile,
    ) -> PathBuf {
        let manifest_path = crate_root.join("Cargo.toml");
        let mut command = Command::new("cargo");
        command.arg("build").arg("--manifest-path").arg(&manifest_path);
        if matches!(profile, BackendBuildProfile::Release) {
            command.arg("--release");
        }
        let output: Output = command.output().expect("cargo build");
        assert!(
            output.status.success(),
            "cargo build failed for '{}'\nstdout:\n{}\nstderr:\n{}",
            manifest_path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let package_name = crate_root
            .file_name()
            .and_then(|value| value.to_str())
            .expect("package name");
        let binary = crate_root
            .join("target")
            .join(profile.as_str())
            .join(package_name);
        assert!(binary.exists(), "cargo binary missing at '{}'", binary.display());
        binary
    }

    fn build_and_run_fixture_with_cargo(source: &str) -> std::process::Output {
        let fixture_root = temp_root("exec_cargo");
        let fixture = write_fixture(&fixture_root, source);
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);
        let artifact = emit_generated_crate_skeleton(&session).expect("generated crate");
        let paths = prepare_backend_build_paths(&fixture_root).expect("prepare paths");
        let crate_root =
            write_generated_crate(Path::new(&paths.build_root), &artifact).expect("write crate");
        let binary =
            cargo_build_generated_crate_for_profile(&crate_root, BackendBuildProfile::Release);
        let output = Command::new(&binary)
            .output()
            .expect("run cargo emitted binary");
        let _ = fs::remove_dir_all(&fixture_root);
        output
    }

    fn build_and_run_fixture_with_rustc(source: &str) -> std::process::Output {
        let fixture_root = temp_root("exec_rustc");
        let fixture = write_fixture(&fixture_root, source);
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);
        let artifact = emit_generated_crate_skeleton(&session).expect("generated crate");
        let paths = prepare_backend_build_paths(&fixture_root).expect("prepare paths");
        let crate_root =
            write_generated_crate(Path::new(&paths.build_root), &artifact).expect("write crate");
        let binary = build_generated_crate_with_rustc(
            &crate_root,
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        )
        .expect("rustc build");
        let output = Command::new(&binary)
            .output()
            .expect("run rustc emitted binary");
        let _ = fs::remove_dir_all(&fixture_root);
        output
    }

    fn build_and_run_workspace(
        entry_path: &Path,
        package_config: PackageConfig,
        resolver_config: ResolverConfig,
    ) -> std::process::Output {
        let lowered = lowered_workspace_from_entry_path_with_config(
            entry_path,
            package_config,
            resolver_config,
        );
        let session = BackendSession::new(lowered);
        let output_root = temp_root("workspace_exec");
        let artifact = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                ..BackendConfig::default()
            },
            &output_root,
        )
        .expect("backend artifact");
        let BackendArtifact::CompiledBinary { binary_path, .. } = artifact else {
            panic!("expected compiled binary artifact");
        };
        let output = Command::new(&binary_path)
            .output()
            .expect("run emitted binary");
        let _ = fs::remove_dir_all(&output_root);
        output
    }

    #[test]
    fn cargo_toml_emission_keeps_runtime_dependency_and_generated_crate_identity() {
        let session = BackendSession::new(sample_lowered_workspace());

        let emitted = emit_cargo_toml(&session);

        assert_eq!(emitted.path, "Cargo.toml");
        assert_eq!(emitted.module_name, "cargo");
        assert!(emitted.contents.contains("[package]"));
        assert!(emitted.contents.contains("edition = \"2021\""));
        assert!(emitted.contents.contains(&format!(
            "name = \"{}\"",
            session.workspace_identity().crate_dir_name
        )));
        assert!(emitted.contents.contains("[dependencies]"));
        assert!(emitted.contents.contains("fol-runtime = { path = "));
        assert!(emitted.contents.contains("/fol-runtime"));
    }

    #[test]
    fn main_rs_emission_keeps_std_model_import_and_entry_metadata() {
        let session = BackendSession::new(sample_lowered_workspace());

        let emitted = emit_main_rs(&session).expect("main.rs");

        assert_eq!(emitted.path, "src/main.rs");
        assert_eq!(emitted.module_name, "main");
        assert!(emitted.contents.contains("use fol_runtime::std as rt;"));
        assert!(emitted.contents.contains("use fol_runtime::std as rt_model;"));
        assert!(emitted.contents.contains("mod packages;"));
        assert!(emitted.contents.contains("let _entry_package = \"app\";"));
        assert!(emitted.contents.contains("let _entry_name = \"main\";"));
    }

    #[test]
    fn main_rs_emission_uses_runtime_tier_specific_model_imports() {
        let session = BackendSession::new(sample_lowered_workspace());

        let core_emitted = emit_main_rs_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("core main");
        let mem_emitted = emit_main_rs_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Memo,
                ..BackendConfig::default()
            },
        )
        .expect("memo main");

        assert!(core_emitted
            .contents
            .contains("use fol_runtime::core as rt_model;"));
        assert!(core_emitted
            .contents
            .contains("use fol_runtime::core as rt;"));
        assert!(mem_emitted
            .contents
            .contains("use fol_runtime::memo as rt_model;"));
        assert!(mem_emitted
            .contents
            .contains("use fol_runtime::memo as rt;"));
        assert!(core_emitted
            .contents
            .contains("let _runtime_tier = rt_model::tier_name();"));
        assert!(!core_emitted.contents.contains("use fol_runtime::memo"));
        assert!(!core_emitted.contents.contains("use fol_runtime::std"));
        assert!(!mem_emitted.contents.contains("use fol_runtime::core"));
        assert!(!mem_emitted.contents.contains("use fol_runtime::std"));

        let std_emitted = emit_main_rs_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Std,
                ..BackendConfig::default()
            },
        )
        .expect("std main");
        assert!(std_emitted.contents.contains("use fol_runtime::std as rt_model;"));
        assert!(std_emitted.contents.contains("use fol_runtime::std as rt;"));
        assert!(!std_emitted.contents.contains("use fol_runtime::core"));
        assert!(!std_emitted.contents.contains("use fol_runtime::memo"));
    }

    #[test]
    fn main_rs_emission_parses_bool_entry_params_from_cli_args() {
        let fixture_root = temp_root("bool_entry_signature");
        let fixture = write_fixture(
            &fixture_root,
            "fun[] main(flag: bol): bol = {\n    return .not(flag);\n};\n",
        );
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);

        let emitted = emit_main_rs(&session).expect("bool entry should emit");

        assert!(emitted.contents.contains("__fol_parse_bool"));
        assert!(emitted
            .contents
            .contains("__fol_cli_arg(0).and_then(|raw| __fol_parse_bool(&raw)).unwrap_or_default()"));
        assert!(emitted.contents.contains("let _ = packages::"));
        let _ = fs::remove_dir_all(&fixture_root);
    }

    #[test]
    fn main_rs_emission_rejects_receiver_entry_routines() {
        let fixture_root = temp_root("bad_entry_receiver");
        let fixture = write_fixture(
            &fixture_root,
            "typ App: rec = {\n    value: int;\n};\nfun (App)main(): int = {\n    return 0;\n};\n",
        );
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);

        let error = emit_main_rs(&session).expect_err("receiver entry should fail");

        assert!(error
            .message()
            .contains("entry routine 'main' must be a plain free routine"));
        let _ = fs::remove_dir_all(&fixture_root);
    }

    #[test]
    fn package_module_emission_keeps_package_and_namespace_module_tree() {
        let session = BackendSession::new(sample_lowered_workspace());

        let emitted = emit_package_module_shells(&session);

        assert_eq!(emitted.len(), 3);
        assert_eq!(emitted[0].path, "src/packages/mod.rs");
        assert!(emitted[0].contents.contains("pub mod pkg__entry__app;"));
        assert!(emitted[0].contents.contains("pub mod pkg__local__shared;"));
        assert_eq!(emitted[1].path, "src/packages/pkg__entry__app/mod.rs");
        assert!(emitted[1].contents.contains("pub mod root;"));
        assert!(emitted[1].contents.contains("pub mod math;"));
        assert_eq!(emitted[2].path, "src/packages/pkg__local__shared/mod.rs");
        assert!(emitted[2].contents.contains("pub mod root;"));
        assert!(emitted[2].contents.contains("pub mod util;"));
    }

    #[test]
    fn namespace_module_shell_emission_keeps_runtime_imports_and_namespace_markers() {
        let session = BackendSession::new(sample_lowered_workspace());

        let emitted = emit_namespace_module_shells(&session).expect("namespace shells");

        assert_eq!(emitted.len(), 4);
        assert_eq!(emitted[0].path, "src/packages/pkg__entry__app/root.rs");
        assert!(emitted[0]
            .contents
            .contains("use fol_runtime::std as rt;"));
        assert!(emitted[0]
            .contents
            .contains("use fol_runtime::std as rt_model;"));
        assert!(emitted[0]
            .contents
            .contains("NAMESPACE_NAME: &str = \"app\""));
        assert!(emitted[0]
            .contents
            .contains("SOURCE_UNIT_IDS: &[usize] = &[0]"));
        assert_eq!(emitted[1].path, "src/packages/pkg__entry__app/math.rs");
        assert!(emitted[1]
            .contents
            .contains("NAMESPACE_NAME: &str = \"app::math\""));
        assert_eq!(emitted[3].path, "src/packages/pkg__local__shared/util.rs");
        assert!(emitted[3]
            .contents
            .contains("NAMESPACE_NAME: &str = \"shared::util\""));
    }

    #[test]
    fn namespace_module_shell_emission_uses_runtime_tier_specific_model_imports() {
        let session = BackendSession::new(sample_lowered_workspace());

        let emitted = emit_namespace_module_shells_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("core namespace shells");

        assert!(emitted[0]
            .contents
            .contains("use fol_runtime::core as rt_model;"));
        assert!(emitted[0].contents.contains("use fol_runtime::core as rt;"));
        assert!(!emitted[0].contents.contains("use fol_runtime::memo"));
        assert!(!emitted[0].contents.contains("use fol_runtime::std"));
        assert!(emitted[0].contents.contains("rt_model::tier_name()"));
    }

    #[test]
    fn generated_crate_skeleton_keeps_core_artifacts_off_alloc_and_std_imports() {
        let session = BackendSession::new(sample_lowered_workspace());

        let artifact = emit_generated_crate_skeleton_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("core artifact");

        let BackendArtifact::RustSourceCrate { files, .. } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        for file in files {
            if !file.path.ends_with(".rs") {
                continue;
            }
            assert!(
                !file.contents.contains("use fol_runtime::memo"),
                "core artifact should not import memo runtime paths in {}:\n{}",
                file.path,
                file.contents
            );
            assert!(
                !file.contents.contains("use fol_runtime::std"),
                "core artifact should not import std runtime paths in {}:\n{}",
                file.path,
                file.contents
            );
        }
    }

    #[test]
    fn generated_crate_skeleton_keeps_alloc_artifacts_off_std_imports() {
        let session = BackendSession::new(sample_lowered_workspace());

        let artifact = emit_generated_crate_skeleton_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Memo,
                ..BackendConfig::default()
            },
        )
        .expect("memo artifact");

        let BackendArtifact::RustSourceCrate { files, .. } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        for file in files {
            if !file.path.ends_with(".rs") {
                continue;
            }
            assert!(
                !file.contents.contains("use fol_runtime::std"),
                "memo artifact should not import std runtime paths in {}:\n{}",
                file.path,
                file.contents
            );
        }
    }

    #[test]
    fn generated_crate_skeleton_snapshot_stays_stable_for_foundation_backend_shape() {
        let session = BackendSession::new(sample_lowered_workspace());

        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");

        let BackendArtifact::RustSourceCrate { root, files } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        let snapshot = files
            .iter()
            .map(|file| format!("== {} ==\n{}", file.path, file.contents))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(root.starts_with("fol-build-app-"));
        assert_eq!(files.len(), 9);
        assert!(snapshot.contains("== Cargo.toml =="));
        assert!(snapshot.contains("== src/main.rs =="));
        assert!(snapshot.contains("== src/packages/mod.rs =="));
        assert!(snapshot.contains("== src/packages/pkg__entry__app/mod.rs =="));
        assert!(snapshot.contains("== src/packages/pkg__local__shared/root.rs =="));
        assert!(snapshot.contains("use fol_runtime::std as rt;"));
        assert!(snapshot.contains("use fol_runtime::std as rt_model;"));
        assert!(snapshot.contains("pub mod pkg__entry__app;"));
        assert!(snapshot.contains("NAMESPACE_NAME: &str = \"shared::util\""));
    }

    #[test]
    fn generated_crate_skeleton_uses_runtime_tier_specific_model_modules() {
        let session = BackendSession::new(sample_lowered_workspace());

        let artifact = emit_generated_crate_skeleton_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Memo,
                ..BackendConfig::default()
            },
        )
        .expect("artifact");

        let BackendArtifact::RustSourceCrate { files, .. } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        let main_rs = files
            .iter()
            .find(|file| file.path == "src/main.rs")
            .expect("main rs");
        let root_namespace = files
            .iter()
            .find(|file| file.path == "src/packages/pkg__entry__app/root.rs")
            .expect("root namespace");

        assert!(main_rs
            .contents
            .contains("use fol_runtime::memo as rt;"));
        assert!(main_rs
            .contents
            .contains("use fol_runtime::memo as rt_model;"));
        assert!(root_namespace
            .contents
            .contains("use fol_runtime::memo as rt_model;"));
    }

    #[test]
    fn core_generated_crate_keeps_heap_and_hosted_helpers_out_of_emitted_tree() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton_for_config(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("core artifact");

        let BackendArtifact::RustSourceCrate { files, .. } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        let snapshot = files
            .iter()
            .map(|file| file.contents.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!snapshot.contains("use fol_runtime::memo"));
        assert!(!snapshot.contains("use fol_runtime::std"));
        assert!(!snapshot.contains("rt::echo("));
        assert!(!snapshot.contains("FolSeq"));
        assert!(!snapshot.contains("FolVec"));
    }

    #[test]
    fn model_specific_len_rendering_keeps_runtime_imports_pure() {
        let core_root = temp_root("core_len_emit");
        let core_fixture = write_fixture(
            &core_root,
            concat!(
                "fun[] main(): int = {\n",
                "    var values: arr[int, 3] = {1, 2, 3};\n",
                "    return .len(values);\n",
                "};\n",
            ),
        );
        let core_session = BackendSession::new(lowered_workspace_from_entry_path(&core_fixture));
        let core_artifact = emit_generated_crate_skeleton_for_config(
            &core_session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("core artifact");
        let BackendArtifact::RustSourceCrate { files: core_files, .. } = core_artifact else {
            panic!("expected RustSourceCrate artifact");
        };
        let core_snapshot = core_files
            .iter()
            .map(|file| file.contents.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(core_snapshot.contains("use fol_runtime::core as rt;"));
        assert!(core_snapshot.contains("rt::len("));
        assert!(!core_snapshot.contains("use fol_runtime::memo"));
        assert!(!core_snapshot.contains("use fol_runtime::std"));

        let mem_root = temp_root("mem_len_emit");
        let mem_fixture = write_fixture(
            &mem_root,
            concat!(
                "fun[] main(): int = {\n",
                "    var values: seq[int] = {1, 2, 3};\n",
                "    return .len(values);\n",
                "};\n",
            ),
        );
        let mem_session = BackendSession::new(lowered_workspace_from_entry_path(&mem_fixture));
        let mem_artifact = emit_generated_crate_skeleton_for_config(
            &mem_session,
            &BackendConfig {
                fol_model: BackendFolModel::Memo,
                ..BackendConfig::default()
            },
        )
        .expect("memo artifact");
        let BackendArtifact::RustSourceCrate { files: mem_files, .. } = mem_artifact else {
            panic!("expected RustSourceCrate artifact");
        };
        let mem_snapshot = mem_files
            .iter()
            .map(|file| file.contents.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(mem_snapshot.contains("use fol_runtime::memo as rt;"));
        assert!(mem_snapshot.contains("rt::len("));
        assert!(!mem_snapshot.contains("use fol_runtime::std"));

        let _ = fs::remove_dir_all(&core_root);
        let _ = fs::remove_dir_all(&mem_root);
    }

    #[test]
    fn generated_crate_writer_materializes_files_under_backend_build_root() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("write");
        let build_root = prepare_generated_build_dir(&temp_root).expect("build root");

        let crate_root = write_generated_crate(&build_root, &artifact).expect("write crate");

        assert!(crate_root.ends_with(session.workspace_identity().crate_dir_name.as_str()));
        assert!(crate_root.join("Cargo.toml").exists());
        assert!(crate_root.join("src/main.rs").exists());
        assert!(crate_root.join("src/packages/mod.rs").exists());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn prepare_generated_build_dir_creates_the_expected_backend_root() {
        let temp_root = temp_root("build_root");

        let build_root = prepare_generated_build_dir(&temp_root).expect("prepare build root");

        assert!(build_root.ends_with("fol-backend"));
        assert!(build_root.exists());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn backend_build_paths_keep_backend_roots_stable() {
        let temp_root = temp_root("paths");

        let paths = backend_build_paths(&temp_root);

        assert_eq!(paths.output_root, temp_root.display().to_string());
        assert!(paths.build_root.ends_with("fol-backend"));
        assert!(paths.bin_root.ends_with("bin"));
        assert!(paths.runtime_root.ends_with("fol-backend/runtime"));
    }

    #[test]
    fn prepare_backend_build_paths_materializes_backend_directories() {
        let temp_root = temp_root("prepared_paths");

        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");

        assert!(Path::new(&paths.build_root).exists());
        assert!(Path::new(&paths.bin_root).exists());
        assert!(Path::new(&paths.runtime_root).exists());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn runtime_source_helpers_default_to_workspace_runtime_layout() {
        let runtime_root = backend_runtime_source_root();
        let manifest_path = backend_runtime_manifest_path();
        let source_entry = backend_runtime_source_entry();

        assert!(runtime_root.ends_with("lang/execution/fol-runtime"));
        assert_eq!(manifest_path, runtime_root.join("Cargo.toml"));
        assert_eq!(source_entry, runtime_root.join("src/lib.rs"));
    }

    #[test]
    fn runtime_source_helpers_honor_explicit_runtime_override_without_mutating_env() {
        let temp_root = temp_root("runtime_override");
        let runtime_root = backend_runtime_source_root_with_override(Some(&temp_root));
        let manifest_path = backend_runtime_manifest_path_with_override(Some(&temp_root));
        let source_entry = backend_runtime_source_entry_with_override(Some(&temp_root));

        assert_eq!(runtime_root, temp_root);
        assert_eq!(manifest_path, temp_root.join("Cargo.toml"));
        assert_eq!(source_entry, temp_root.join("src/lib.rs"));
    }

    #[test]
    fn runtime_build_dir_helpers_keep_profile_scoped_runtime_outputs() {
        let temp_root = temp_root("runtime_dirs");
        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");

        let debug_dir =
            backend_runtime_build_dir(&paths, &BackendMachineTarget::Host, BackendBuildProfile::Debug);
        let release_dir = backend_runtime_build_dir(
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        );
        let prepared_release = prepare_backend_runtime_build_dir(
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        )
        .expect("prepare runtime dir");

        assert!(debug_dir.ends_with("fol-backend/runtime/host/debug"));
        assert!(release_dir.ends_with("fol-backend/runtime/host/release"));
        assert_eq!(prepared_release, release_dir);
        assert!(prepared_release.exists());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn runtime_rustc_command_uses_target_for_cross_builds() {
        let runtime_source = PathBuf::from("/tmp/runtime/src/lib.rs");
        let runtime_build_dir = PathBuf::from("/tmp/runtime/out");
        let command = configure_runtime_rustc_command(
            &runtime_source,
            &runtime_build_dir,
            &BackendMachineTarget::Triple("aarch64-macos-gnu".to_string()),
            BackendBuildProfile::Release,
        );
        let args = command
            .get_args()
            .map(|arg: &std::ffi::OsStr| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.windows(2).any(|pair| {
            pair == ["--target", "aarch64-apple-darwin"]
        }));
    }

    #[test]
    fn runtime_rustc_command_skips_target_for_host_builds() {
        let runtime_source = PathBuf::from("/tmp/runtime/src/lib.rs");
        let runtime_build_dir = PathBuf::from("/tmp/runtime/out");
        let command = configure_runtime_rustc_command(
            &runtime_source,
            &runtime_build_dir,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Debug,
        );
        let args = command
            .get_args()
            .map(|arg: &std::ffi::OsStr| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!args.iter().any(|arg| arg == "--target"));
    }

    #[test]
    fn generated_crate_rustc_command_uses_target_for_cross_builds() {
        let crate_root = PathBuf::from("/tmp/generated/demo");
        let main_rs = crate_root.join("src/main.rs");
        let runtime_rlib = PathBuf::from("/tmp/runtime/libfol_runtime.rlib");
        let binary_path = crate_root.join("target/app");
        let command = configure_generated_crate_rustc_command(
            &crate_root,
            &main_rs,
            &runtime_rlib,
            &binary_path,
            &BackendMachineTarget::Triple("x86_64-linux-gnu".to_string()),
            BackendBuildProfile::Release,
        )
        .expect("generated rustc command");
        let args = command
            .get_args()
            .map(|arg: &std::ffi::OsStr| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.windows(2).any(|pair| {
            pair == ["--target", "x86_64-unknown-linux-gnu"]
        }));
    }

    #[test]
    fn generated_crate_rustc_command_skips_target_for_host_builds() {
        let crate_root = PathBuf::from("/tmp/generated/demo");
        let main_rs = crate_root.join("src/main.rs");
        let runtime_rlib = PathBuf::from("/tmp/runtime/libfol_runtime.rlib");
        let binary_path = crate_root.join("target/app");
        let command = configure_generated_crate_rustc_command(
            &crate_root,
            &main_rs,
            &runtime_rlib,
            &binary_path,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Debug,
        )
        .expect("generated rustc command");
        let args = command
            .get_args()
            .map(|arg: &std::ffi::OsStr| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(!args.iter().any(|arg| arg == "--target"));
    }

    #[test]
    fn rustc_runtime_builder_produces_release_runtime_rlib() {
        let temp_root = temp_root("runtime_rlib_release");
        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");

        let rlib = build_runtime_rlib_with_rustc(
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        )
        .expect("build runtime rlib");

        assert!(rlib.exists());
        assert!(rlib.ends_with("libfol_runtime.rlib"));
        assert!(rlib
            .to_string_lossy()
            .contains("/fol-backend/runtime/host/release/"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn rustc_generated_crate_builder_produces_runnable_release_binary() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("rustc_generated_release");
        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");
        let crate_root =
            write_generated_crate(Path::new(&paths.build_root), &artifact).expect("write crate");

        let binary = build_generated_crate_with_rustc(
            &crate_root,
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        )
        .expect("rustc build");
        let output = Command::new(&binary).output().expect("run rustc binary");

        assert!(binary.exists());
        assert!(binary.to_string_lossy().contains("/target/host/release/"));
        assert!(output.status.code().is_some());

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn rustc_generated_crate_builder_sanitizes_hyphenated_crate_dir_names() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("rustc_hyphenated_release");
        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");
        let crate_root =
            write_generated_crate(Path::new(&paths.build_root), &artifact).expect("write crate");
        let renamed_root = crate_root
            .parent()
            .expect("crate parent")
            .join("demo-with-hyphen");
        fs::rename(&crate_root, &renamed_root).expect("rename crate root");

        let binary = build_generated_crate_with_rustc(
            &renamed_root,
            &paths,
            &BackendMachineTarget::Host,
            BackendBuildProfile::Release,
        )
        .expect("rustc build");

        assert!(binary.exists());
        assert!(binary.ends_with("demo-with-hyphen"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn rustc_and_cargo_drivers_keep_scalar_program_behavior_in_sync() {
        let source = "fun[] main(): int = {\n    return 7;\n};\n";

        let cargo_output = build_and_run_fixture_with_cargo(source);
        let rustc_output = build_and_run_fixture_with_rustc(source);

        assert_eq!(cargo_output.status.code(), rustc_output.status.code());
        assert_eq!(cargo_output.stdout, rustc_output.stdout);
        assert_eq!(cargo_output.stderr, rustc_output.stderr);
    }

    #[test]
    fn rustc_and_cargo_drivers_keep_recoverable_entry_behavior_in_sync() {
        let source = "fun[] main(): int / str = {\n    report \"broken\";\n    return 0;\n};\n";

        let cargo_output = build_and_run_fixture_with_cargo(source);
        let rustc_output = build_and_run_fixture_with_rustc(source);

        assert_eq!(cargo_output.status.code(), rustc_output.status.code());
        assert_eq!(cargo_output.stdout, rustc_output.stdout);
        assert_eq!(cargo_output.stderr, rustc_output.stderr);
    }

    #[test]
    fn rustc_and_cargo_drivers_keep_runtime_helper_behavior_in_sync() {
        let source = concat!(
            "fun[] main(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .echo(.len(values));\n",
            "};\n",
        );

        let cargo_output = build_and_run_fixture_with_cargo(source);
        let rustc_output = build_and_run_fixture_with_rustc(source);

        assert_eq!(cargo_output.status.code(), rustc_output.status.code());
        assert_eq!(cargo_output.stdout, rustc_output.stdout);
        assert_eq!(cargo_output.stderr, rustc_output.stderr);
    }

    #[test]
    fn rustc_backend_build_mode_runs_runtime_helper_programs() {
        let output = build_and_run_fixture(concat!(
            "fun[] main(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .echo(.len(values));\n",
            "};\n",
        ));

        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("3"));
    }

    #[test]
    fn emitted_generated_crate_remains_cargo_buildable_in_release_mode() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("cargo_build");
        let build_root = prepare_generated_build_dir(&temp_root).expect("build root");
        let crate_root = write_generated_crate(&build_root, &artifact).expect("write crate");

        let binary =
            cargo_build_generated_crate_for_profile(&crate_root, BackendBuildProfile::Release);

        assert!(binary.exists());
        assert!(binary.ends_with(session.workspace_identity().crate_dir_name.as_str()));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn emitted_generated_crate_remains_cargo_buildable_in_debug_mode() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("cargo_debug_driver");
        let build_root = prepare_generated_build_dir(&temp_root).expect("build root");
        let crate_root = write_generated_crate(&build_root, &artifact).expect("write crate");

        let binary =
            cargo_build_generated_crate_for_profile(&crate_root, BackendBuildProfile::Debug);
        assert!(binary.exists());
        assert!(binary.to_string_lossy().contains("/target/debug/"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn target_scoped_runtime_and_binary_outputs_use_resolved_machine_target_dirs() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");
        let temp_root = temp_root("cross_target_layout");
        let paths = prepare_backend_build_paths(&temp_root).expect("prepare paths");
        let crate_root =
            write_generated_crate(Path::new(&paths.build_root), &artifact).expect("write crate");
        let machine_target =
            BackendMachineTarget::Triple("aarch64-macos-gnu".to_string());

        let runtime_dir =
            backend_runtime_build_dir(&paths, &machine_target, BackendBuildProfile::Release);
        let binary = build_generated_crate_with_rustc(
            &crate_root,
            &paths,
            &machine_target,
            BackendBuildProfile::Release,
        )
        .expect("rustc build");
        let emitted = emit_backend_artifact(
            &session,
            &BackendConfig {
                machine_target,
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                ..BackendConfig::default()
            },
            &temp_root,
        )
        .expect("build artifact");

        assert!(runtime_dir.ends_with("fol-backend/runtime/aarch64-apple-darwin/release"));
        assert!(binary
            .to_string_lossy()
            .contains("/target/aarch64-apple-darwin/release/"));
        let BackendArtifact::CompiledBinary { binary_path, .. } = emitted else {
            panic!("expected compiled binary artifact");
        };
        assert!(binary_path.contains("/bin/aarch64-apple-darwin/"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn emit_backend_artifact_honors_emit_source_and_build_artifact_modes() {
        let session = BackendSession::new(sample_lowered_workspace());
        let temp_root = temp_root("modes");

        let emitted = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::EmitSource,
                ..BackendConfig::default()
            },
            &temp_root,
        )
        .expect("emit source");
        let built = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                ..BackendConfig::default()
            },
            &temp_root,
        )
        .expect("build artifact");

        assert!(matches!(emitted, BackendArtifact::RustSourceCrate { .. }));
        assert!(matches!(built, BackendArtifact::CompiledBinary { .. }));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn emit_backend_artifact_respects_keep_build_dir_and_summary_output() {
        let session = BackendSession::new(sample_lowered_workspace());
        let temp_root = temp_root("keep");
        let artifact = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                ..BackendConfig::default()
            },
            &temp_root,
        )
        .expect("build artifact");

        let summary = summarize_emitted_artifact(&artifact);
        let BackendArtifact::CompiledBinary {
            crate_root,
            binary_path,
        } = &artifact
        else {
            panic!("expected compiled artifact");
        };

        assert!(Path::new(crate_root).exists());
        assert!(Path::new(binary_path).exists());
        assert!(summary.contains("compiled backend artifact"));
        assert!(summary.contains("binary="));

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn full_generated_crate_snapshot_stays_stable_after_backend_materialization() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");

        let summary = summarize_emitted_artifact(&artifact);

        assert!(summary.contains("generated Rust crate root="));
        assert!(summary.contains("Cargo.toml"));
        assert!(summary.contains("src/main.rs"));
        assert!(summary.contains("src/packages/pkg__entry__app/root.rs"));
    }

    #[test]
    fn package_module_emission_adds_nested_mod_files_for_deep_namespaces() {
        let fixture_root = temp_root("deep_namespace_layout");
        let app_root = fixture_root.join("app");
        fs::create_dir_all(app_root.join("api/tools/math")).expect("nested namespace root");
        fs::write(
            app_root.join("main.fol"),
            "fun[] main(): int = {\n    return api::tools::math::leaf();\n};\n",
        )
        .expect("app source");
        fs::write(
            app_root.join("api/tools/math/leaf.fol"),
            "fun[] leaf(): int = {\n    return 7;\n};\n",
        )
        .expect("nested source");

        let lowered = lowered_workspace_from_entry_path(&app_root);
        let session = BackendSession::new(lowered);
        let emitted = emit_package_module_shells(&session);

        assert!(emitted
            .iter()
            .any(|file| file.path.ends_with("pkg__entry__app/api/mod.rs")
                && file.contents.contains("pub mod tools;")));
        assert!(emitted.iter().any(
            |file| file.path.ends_with("pkg__entry__app/api/tools/mod.rs")
                && file.contents.contains("pub mod math;")
        ));

        let _ = fs::remove_dir_all(&fixture_root);
    }

    #[test]
    fn rustc_backend_build_mode_runs_multi_package_workspace_programs() {
        let fixture_root = temp_root("workspace_runtime");
        let app_root = fixture_root.join("app");
        let shared_root = fixture_root.join("shared");
        let std_root = fixture_root.join("std");
        let pkg_root = fixture_root.join("pkg");
        let pkg_math_root = pkg_root.join("math");

        fs::create_dir_all(&app_root).expect("app root");
        fs::create_dir_all(&shared_root).expect("shared root");
        fs::create_dir_all(std_root.join("fmt")).expect("std root");
        fs::create_dir_all(&pkg_math_root).expect("pkg root");
        fs::write(
            app_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "};\n",
            ),
        )
        .expect("app build");
        fs::write(
            app_root.join("main.fol"),
            concat!(
                "use shared: loc = {\"../shared\"};\n",
                "use std: pkg = {\"std\"};\n",
                "use math: pkg = {\"math\"};\n",
                "fun[] main(): int = {\n",
                "    return loc_answer + std::fmt::answer() + math::src::pkg_answer;\n",
                "};\n",
            ),
        )
        .expect("app source");
        fs::write(
            shared_root.join("lib.fol"),
            "var[exp] loc_answer: int = 2;\n",
        )
        .expect("shared");
        fs::write(
            std_root.join("fmt").join("lib.fol"),
            "fun[exp] answer(): int = {\n    return 3;\n};\n",
        )
        .expect("std");
        fs::write(
            pkg_math_root.join("build.fol"),
            "name: math\nversion: 0.1.0\n",
        )
        .expect("pkg manifest");
        fs::write(
            pkg_math_root.join("build.fol"),
            "pro[] build(): non = {\n    return;\n};\n",
        )
        .expect("pkg build");
        fs::create_dir_all(pkg_math_root.join("src")).expect("pkg src");
        fs::write(
            pkg_math_root.join("src").join("lib.fol"),
            "var[exp] pkg_answer: int = 4;\n",
        )
        .expect("pkg source");

        let output = build_and_run_workspace(
            &app_root,
            PackageConfig {
                std_root: Some(std_root.display().to_string()),
                package_store_root: Some(pkg_root.display().to_string()),
                package_cache_root: None,
                package_git_cache_root: None,
            },
            ResolverConfig {
                std_root: Some(std_root.display().to_string()),
                package_store_root: Some(pkg_root.display().to_string()),
            },
        );

        assert!(output.status.success());

        let _ = fs::remove_dir_all(&fixture_root);
    }

    #[test]
    fn generated_crate_artifact_file_order_stays_deterministic() {
        let session = BackendSession::new(sample_lowered_workspace());
        let artifact = emit_generated_crate_skeleton(&session).expect("artifact");

        let BackendArtifact::RustSourceCrate { files, .. } = artifact else {
            panic!("expected RustSourceCrate artifact");
        };

        let mut sorted_paths = files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let original_paths = sorted_paths.clone();
        sorted_paths.sort();

        assert_eq!(original_paths, sorted_paths);
    }

    #[test]
    fn executable_backend_runs_scalar_entry_routines_successfully() {
        let output = build_and_run_fixture("fun[] main(): int = {\n    return 7;\n};\n");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    }

    #[test]
    fn executable_backend_handles_recoverable_entry_failure_through_process_outcome() {
        let output =
            build_and_run_fixture("fun[] main(): int / str = {\n    report \"broken\";\n    return 0;\n};\n");

        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("broken"));
    }

    #[test]
    fn executable_backend_handles_explicit_recoverable_report_between_zero_arg_routines() {
        let output = build_and_run_fixture(concat!(
            "fun[] load(): int / str = {\n",
            "    report \"bad-input\";\n",
            "    return 0;\n",
            "};\n",
            "fun[] main(): int / str = {\n",
            "    return load() || report \"bad-input\";\n",
            "    return 0;\n",
            "};\n",
        ));

        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("bad-input"));
    }

    #[test]
    fn executable_backend_runs_container_length_programs() {
        let output = build_and_run_fixture(concat!(
            "fun[] main(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .echo(.len(values));\n",
            "};\n",
        ));

        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("3"));
    }

    #[test]
    fn executable_backend_runs_echo_programs() {
        let output = build_and_run_fixture("fun[] main(): int = {\n    return .echo(0);\n};\n");

        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("0"));
    }

    #[test]
    fn executable_backend_std_model_main_runs_hosted_entry_after_runtime_move() {
        let fixture_root = temp_root("std_hosted_entry");
        let fixture = write_fixture(
            &fixture_root,
            concat!(
                "fun[] main(): int = {\n",
                "    .echo(\"std-hosted\");\n",
                "    return 0;\n",
                "};\n",
            ),
        );
        let lowered = lowered_workspace_from_entry_path(&fixture);
        let session = BackendSession::new(lowered);
        let artifact = emit_backend_artifact(
            &session,
            &BackendConfig {
                mode: BackendMode::BuildArtifact,
                keep_build_dir: true,
                fol_model: BackendFolModel::Std,
                ..BackendConfig::default()
            },
            &fixture_root,
        )
        .expect("backend std artifact");
        let BackendArtifact::CompiledBinary {
            binary_path,
            emitted_crate_root,
        } = artifact
        else {
            panic!("expected compiled binary artifact");
        };
        let main_rs =
            fs::read_to_string(emitted_crate_root.join("src/main.rs")).expect("generated main.rs");
        let output = Command::new(&binary_path)
            .output()
            .expect("run emitted std binary");
        let _ = fs::remove_dir_all(&fixture_root);

        assert!(main_rs.contains("use fol_runtime::std as rt;"));
        assert!(main_rs.contains("use fol_runtime::std as rt_model;"));
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("std-hosted"));
    }

    #[test]
    fn executable_backend_runs_check_programs() {
        let output = build_and_run_fixture(concat!(
            "fun[] load(): int / str = {\n",
            "    report \"broken\";\n",
            "    return 0;\n",
            "};\n",
            "fun[] main(): int = {\n",
            "    when(check(load())) {\n",
            "        case(true) { return 1; }\n",
            "        * { return 0; }\n",
            "    }\n",
            "};\n",
        ));

        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn executable_backend_runs_pipe_or_fallback_programs() {
        let output = build_and_run_fixture(concat!(
            "fun[] load(): int / str = {\n",
            "    report \"broken\";\n",
            "    return 0;\n",
            "};\n",
            "fun[] main(): int = {\n",
            "    return load() || 9;\n",
            "};\n",
        ));

        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn executable_backend_runs_across_loc_std_and_pkg_package_graphs() {
        let fixture_root = temp_root("workspace_graphs");
        let app_root = fixture_root.join("app");
        let shared_root = fixture_root.join("shared");
        let std_root = fixture_root.join("std");
        let pkg_root = fixture_root.join("pkg");
        let pkg_math_root = pkg_root.join("math");

        fs::create_dir_all(&app_root).expect("app root");
        fs::create_dir_all(&shared_root).expect("shared root");
        fs::create_dir_all(std_root.join("fmt")).expect("std root");
        fs::create_dir_all(&pkg_math_root).expect("pkg root");
        fs::write(
            app_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "};\n",
            ),
        )
        .expect("app build");

        fs::write(
            app_root.join("main.fol"),
            concat!(
                "use shared: loc = {\"../shared\"};\n",
                "use std: pkg = {\"std\"};\n",
                "use math: pkg = {\"math\"};\n",
                "fun[] main(): int = {\n",
                "    return loc_answer + std::fmt::answer() + math::src::pkg_answer;\n",
                "};\n",
            ),
        )
        .expect("app source");
        fs::write(
            shared_root.join("lib.fol"),
            "var[exp] loc_answer: int = 2;\n",
        )
        .expect("shared");
        fs::write(
            std_root.join("fmt").join("lib.fol"),
            "fun[exp] answer(): int = {\n    return 3;\n};\n",
        )
        .expect("std");
        fs::write(
            pkg_math_root.join("build.fol"),
            "name: math\nversion: 0.1.0\n",
        )
        .expect("pkg manifest");
        fs::write(
            pkg_math_root.join("build.fol"),
            "pro[] build(): non = {\n    return;\n};\n",
        )
        .expect("pkg build");
        fs::create_dir_all(pkg_math_root.join("src")).expect("pkg src");
        fs::write(
            pkg_math_root.join("src").join("lib.fol"),
            "var[exp] pkg_answer: int = 4;\n",
        )
        .expect("pkg source");

        let output = build_and_run_workspace(
            &app_root,
            PackageConfig {
                std_root: Some(std_root.display().to_string()),
                package_store_root: Some(pkg_root.display().to_string()),
                package_cache_root: None,
                package_git_cache_root: None,
            },
            ResolverConfig {
                std_root: Some(std_root.display().to_string()),
                package_store_root: Some(pkg_root.display().to_string()),
            },
        );

        let _ = fs::remove_dir_all(&fixture_root);

        assert!(output.status.success());
    }

    #[test]
    fn backend_emit_accepts_quoted_loc_imports_through_the_full_workspace_chain() {
        let fixture_root = temp_root("quoted_loc_workspace");
        let app_root = fixture_root.join("app");
        let shared_root = fixture_root.join("shared");

        fs::create_dir_all(&app_root).expect("app root");
        fs::create_dir_all(&shared_root).expect("shared root");
        fs::write(
            app_root.join("main.fol"),
            concat!(
                "use shared: loc = {\"../shared\"};\n",
                "fun[] main(): int = {\n",
                "    return shared::answer;\n",
                "};\n",
            ),
        )
        .expect("app source");
        fs::write(
            shared_root.join("lib.fol"),
            "var[exp] answer: int = 7;\n",
        )
        .expect("shared source");

        let output = build_and_run_workspace(
            &app_root.join("main.fol"),
            PackageConfig::default(),
            ResolverConfig::default(),
        );

        let _ = fs::remove_dir_all(&fixture_root);

        assert!(
            output.status.success(),
            "Quoted loc imports should survive parse, resolve, lower, typecheck, and backend emit"
        );
    }

    #[test]
    fn backend_emit_accepts_quoted_pkg_imports_through_the_full_workspace_chain() {
        let fixture_root = temp_root("quoted_pkg_workspace");
        let app_root = fixture_root.join("app");
        let pkg_root = fixture_root.join("pkg");
        let json_root = pkg_root.join("json");

        fs::create_dir_all(&app_root).expect("app root");
        fs::create_dir_all(json_root.join("src")).expect("pkg root");
        fs::write(
            app_root.join("main.fol"),
            concat!(
                "use json: pkg = {\"json\"};\n",
                "fun[] main(): int = {\n",
                "    return json::src::answer;\n",
                "};\n",
            ),
        )
        .expect("app source");
        fs::write(
            json_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"json\", version = \"0.1.0\" });\n",
                "};\n",
            ),
        )
        .expect("pkg build");
        fs::write(
            json_root.join("src/lib.fol"),
            "var[exp] answer: int = 9;\n",
        )
        .expect("pkg source");

        let output = build_and_run_workspace(
            &app_root.join("main.fol"),
            PackageConfig {
                package_store_root: Some(pkg_root.display().to_string()),
                ..PackageConfig::default()
            },
            ResolverConfig {
                package_store_root: Some(pkg_root.display().to_string()),
                ..ResolverConfig::default()
            },
        );

        let _ = fs::remove_dir_all(&fixture_root);

        assert!(
            output.status.success(),
            "Quoted pkg imports should survive parse, resolve, lower, typecheck, and backend emit"
        );
    }
}
