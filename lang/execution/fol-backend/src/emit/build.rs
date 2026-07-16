use crate::{
    BackendArtifact, BackendBuildPaths, BackendBuildProfile, BackendConfig, BackendError,
    BackendErrorKind, BackendMachineTarget, BackendMode, BackendResult, BackendSession,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use super::skeleton::emit_generated_crate_skeleton_for_config;

pub fn backend_build_paths(output_root: &Path) -> BackendBuildPaths {
    BackendBuildPaths {
        output_root: output_root.display().to_string(),
        build_root: output_root.join("fol-backend").display().to_string(),
        bin_root: output_root.join("bin").display().to_string(),
        runtime_root: output_root
            .join("fol-backend")
            .join("runtime")
            .display()
            .to_string(),
    }
}

pub fn prepare_backend_build_paths(output_root: &Path) -> BackendResult<BackendBuildPaths> {
    let paths = backend_build_paths(output_root);
    for dir in [&paths.build_root, &paths.bin_root, &paths.runtime_root] {
        fs::create_dir_all(dir).map_err(|error| {
            BackendError::new(
                BackendErrorKind::EmissionFailure,
                format!("failed to create backend output dir '{}': {error}", dir),
            )
        })?;
    }
    Ok(paths)
}

pub fn write_generated_crate(
    output_root: &Path,
    artifact: &BackendArtifact,
) -> BackendResult<PathBuf> {
    let BackendArtifact::RustSourceCrate { root, files } = artifact else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "write_generated_crate expects a RustSourceCrate artifact",
        ));
    };

    let crate_root = output_root.join(root);
    if crate_root.exists() {
        fs::remove_dir_all(&crate_root).map_err(|error| {
            BackendError::new(
                BackendErrorKind::EmissionFailure,
                format!(
                    "failed to clean generated crate root '{}': {error}",
                    crate_root.display()
                ),
            )
        })?;
    }
    fs::create_dir_all(&crate_root).map_err(|error| {
        BackendError::new(
            BackendErrorKind::EmissionFailure,
            format!(
                "failed to create generated crate root '{}': {error}",
                crate_root.display()
            ),
        )
    })?;

    for file in files {
        let path = crate_root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                BackendError::new(
                    BackendErrorKind::EmissionFailure,
                    format!(
                        "failed to create generated module dir '{}': {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        fs::write(&path, &file.contents).map_err(|error| {
            BackendError::new(
                BackendErrorKind::EmissionFailure,
                format!(
                    "failed to write generated file '{}': {error}",
                    path.display()
                ),
            )
        })?;
    }

    Ok(crate_root)
}

pub fn prepare_generated_build_dir(output_root: &Path) -> BackendResult<PathBuf> {
    Ok(PathBuf::from(prepare_backend_build_paths(output_root)?.build_root))
}

fn apply_rustc_profile_args(command: &mut Command, profile: BackendBuildProfile) {
    match profile {
        BackendBuildProfile::Debug => {}
        BackendBuildProfile::Release => {
            command.arg("-C").arg("opt-level=3");
        }
    }
}

pub(crate) fn configure_runtime_rustc_command(
    runtime_source: &Path,
    runtime_build_dir: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> Command {
    let mut command = Command::new("rustc");
    command
        .arg("--crate-name")
        .arg("fol_runtime")
        .arg("--crate-type")
        .arg("rlib")
        .arg("--edition=2021");
    if let Some(target_triple) = machine_target.rust_target_triple() {
        command.arg("--target").arg(target_triple);
    }
    command
        .arg(runtime_source)
        .arg("--out-dir")
        .arg(runtime_build_dir);
    apply_rustc_profile_args(&mut command, profile);
    command
}

pub(crate) fn configure_generated_crate_rustc_command(
    crate_root: &Path,
    main_rs: &Path,
    runtime_rlib: &Path,
    binary_path: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<Command> {
    let mut command = Command::new("rustc");
    command
        .current_dir(crate_root)
        .arg("--crate-name")
        .arg(rustc_crate_name_for_generated_crate(crate_root)?)
        .arg("--edition=2021");
    if let Some(target_triple) = machine_target.rust_target_triple() {
        command.arg("--target").arg(target_triple);
    }
    command
        .arg(main_rs)
        .arg("--extern")
        .arg(format!("fol_runtime={}", runtime_rlib.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            runtime_rlib
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .display()
        ))
        .arg("-o")
        .arg(binary_path);
    apply_rustc_profile_args(&mut command, profile);
    Ok(command)
}

fn runtime_rlib_path(runtime_build_dir: &Path) -> PathBuf {
    runtime_build_dir.join("libfol_runtime.rlib")
}

fn runtime_build_dir_for_generated_crate(
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    crate_root: &Path,
) -> BackendResult<PathBuf> {
    let crate_dir_name = package_name_for_generated_crate(crate_root)?;
    Ok(super::runtime::backend_runtime_build_dir(paths, machine_target, profile)
        .join(crate::sanitize_backend_ident(crate_dir_name)))
}

fn package_name_for_generated_crate(crate_root: &Path) -> BackendResult<&str> {
    crate_root.file_name().and_then(|value| value.to_str()).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "generated crate root '{}' does not have a valid package name",
                crate_root.display()
            ),
        )
    })
}

fn rustc_crate_name_for_generated_crate(crate_root: &Path) -> BackendResult<String> {
    Ok(crate::sanitize_backend_ident(package_name_for_generated_crate(
        crate_root,
    )?))
}

fn built_binary_output_path(
    crate_root: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let package_name = package_name_for_generated_crate(crate_root)?;
    let target_dir = machine_target
        .rust_target_triple()
        .unwrap_or_else(|| machine_target.display_name().to_string());
    Ok(crate_root
        .join("target")
        .join(target_dir)
        .join(profile.as_str())
        .join(package_name))
}

fn wait_for_emitted_path(path: &Path) -> bool {
    for _ in 0..20 {
        if path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

pub fn build_runtime_rlib_with_rustc(
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let runtime_source = super::runtime::backend_runtime_source_entry();
    let runtime_build_dir =
        super::runtime::prepare_backend_runtime_build_dir(paths, machine_target, profile)?;
    let mut command = configure_runtime_rustc_command(
        &runtime_source,
        &runtime_build_dir,
        machine_target,
        profile,
    );
    let output = command.output().map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to launch rustc for runtime '{}': {error}",
                runtime_source.display()
            ),
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc failed for runtime '{}'\nstdout:\n{}\nstderr:\n{}",
                runtime_source.display(),
                stdout.trim(),
                stderr.trim()
            ),
        ));
    }
    let rlib_path = runtime_rlib_path(&runtime_build_dir);
    if !rlib_path.exists() {
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc succeeded but runtime artifact '{}' is missing",
                rlib_path.display()
            ),
        ));
    }
    Ok(rlib_path)
}

pub fn build_generated_crate_with_rustc(
    crate_root: &Path,
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let runtime_build_dir =
        runtime_build_dir_for_generated_crate(paths, machine_target, profile, crate_root)?;
    fs::create_dir_all(&runtime_build_dir).map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to create generated runtime dir '{}': {error}",
                runtime_build_dir.display()
            ),
        )
    })?;
    let runtime_source = super::runtime::backend_runtime_source_entry();
    let runtime_rlib = runtime_rlib_path(&runtime_build_dir);
    let mut runtime_command = configure_runtime_rustc_command(
        &runtime_source,
        &runtime_build_dir,
        machine_target,
        profile,
    );
    let runtime_output = runtime_command.output().map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to launch rustc for runtime '{}': {error}",
                runtime_source.display()
            ),
        )
    })?;
    if !runtime_output.status.success() {
        let stderr = String::from_utf8_lossy(&runtime_output.stderr);
        let stdout = String::from_utf8_lossy(&runtime_output.stdout);
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc failed for runtime '{}'\nstdout:\n{}\nstderr:\n{}",
                runtime_source.display(),
                stdout.trim(),
                stderr.trim()
            ),
        ));
    }
    if !runtime_rlib.exists() {
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc succeeded but runtime artifact '{}' is missing",
                runtime_rlib.display()
            ),
        ));
    }
    let main_rs = crate_root.join("src").join("main.rs");
    let binary_path = built_binary_output_path(crate_root, machine_target, profile)?;
    if let Some(parent) = binary_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "failed to create generated binary dir '{}': {error}",
                    parent.display()
                ),
            )
        })?;
    }
    let mut command = configure_generated_crate_rustc_command(
        crate_root,
        &main_rs,
        &runtime_rlib,
        &binary_path,
        machine_target,
        profile,
    )?;
    let output = command.output().map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to launch rustc for generated crate '{}': {error}",
                main_rs.display()
            ),
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc failed for generated crate '{}'\nstdout:\n{}\nstderr:\n{}",
                main_rs.display(),
                stdout.trim(),
                stderr.trim()
            ),
        ));
    }
    if !wait_for_emitted_path(&binary_path) {
        return Err(BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "rustc succeeded but generated binary '{}' is missing",
                binary_path.display()
            ),
        ));
    }
    Ok(binary_path)
}

pub fn emit_backend_artifact(
    session: &BackendSession,
    config: &BackendConfig,
    output_root: &Path,
) -> BackendResult<BackendArtifact> {
    let paths = prepare_backend_build_paths(output_root)?;
    let build_root = PathBuf::from(&paths.build_root);
    let source_artifact = emit_generated_crate_skeleton_for_config(session, config)?;
    let crate_root = write_generated_crate(&build_root, &source_artifact)?;

    if matches!(config.mode, BackendMode::EmitSource) {
        let BackendArtifact::RustSourceCrate { files, .. } = source_artifact else {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "generated crate skeleton produced an unexpected artifact type",
            ));
        };
        return Ok(BackendArtifact::RustSourceCrate {
            root: crate_root.display().to_string(),
            files,
        });
    }

    let built_binary = match config.mode {
        BackendMode::EmitSource => unreachable!("emit source handled above"),
        BackendMode::BuildArtifact => {
            build_generated_crate_with_rustc(
                &crate_root,
                &paths,
                &config.machine_target,
                config.build_profile,
            )?
        }
    };
    let final_binary_dir = PathBuf::from(&paths.bin_root);
    let target_dir = config
        .machine_target
        .rust_target_triple()
        .unwrap_or_else(|| config.machine_target.display_name().to_string());
    let final_binary_dir = final_binary_dir.join(target_dir);
    fs::create_dir_all(&final_binary_dir).map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to create target-scoped binary dir '{}': {error}",
                final_binary_dir.display()
            ),
        )
    })?;
    let final_binary = final_binary_dir.join(built_binary.file_name().ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "built binary '{}' does not have a file name",
                built_binary.display()
            ),
        )
    })?);
    fs::copy(&built_binary, &final_binary).map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to copy built binary '{}' to '{}': {error}",
                built_binary.display(),
                final_binary.display()
            ),
        )
    })?;

    if !config.keep_build_dir {
        fs::remove_dir_all(&crate_root).map_err(|error| {
            BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "failed to remove generated crate dir '{}': {error}",
                    crate_root.display()
                ),
            )
        })?;
    }

    Ok(BackendArtifact::CompiledBinary {
        crate_root: crate_root.display().to_string(),
        binary_path: final_binary.display().to_string(),
    })
}

pub fn summarize_emitted_artifact(artifact: &BackendArtifact) -> String {
    match artifact {
        BackendArtifact::RustSourceCrate { root, files } => format!(
            "generated Rust crate root={root} files={}",
            files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        BackendArtifact::CompiledBinary {
            crate_root,
            binary_path,
        } => format!("compiled backend artifact crate_root={crate_root} binary={binary_path}"),
    }
}
