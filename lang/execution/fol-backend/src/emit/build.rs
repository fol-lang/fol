use crate::{
    BackendArtifact, BackendAuxiliaryRustCrate, BackendAuxiliaryRustPlan, BackendBuildPaths,
    BackendBuildProfile, BackendConfig, BackendError, BackendErrorKind, BackendMachineTarget,
    BackendMode, BackendResult, BackendSession,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
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
    Ok(PathBuf::from(
        prepare_backend_build_paths(output_root)?.build_root,
    ))
}

fn apply_rustc_profile_args(command: &mut Command, profile: BackendBuildProfile) {
    match profile {
        BackendBuildProfile::Debug => {}
        BackendBuildProfile::Release => {
            command.arg("-C").arg("opt-level=3");
        }
    }
}

fn rustc_extern_assignment(crate_name: &str, rlib_path: &Path) -> OsString {
    let mut assignment = OsString::from(crate_name);
    assignment.push("=");
    assignment.push(rlib_path.as_os_str());
    assignment
}

fn rustc_search_path_assignment(kind: &str, path: &Path) -> OsString {
    let mut assignment = OsString::from(kind);
    assignment.push("=");
    assignment.push(path.as_os_str());
    assignment
}

pub(crate) fn configure_auxiliary_crate_rustc_command(
    auxiliary_crate: &BackendAuxiliaryRustCrate,
    auxiliary_build_dir: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    dependency_rlibs: &BTreeMap<String, PathBuf>,
) -> BackendResult<Command> {
    let mut command = Command::new("rustc");
    command
        .arg("--crate-name")
        .arg(auxiliary_crate.crate_name())
        .arg("--crate-type")
        .arg("rlib")
        .arg("--edition=2021")
        .arg("--target")
        .arg(machine_target.rust_target_triple())
        .arg(auxiliary_crate.source_root())
        .arg("--out-dir")
        .arg(auxiliary_build_dir);

    for dependency in auxiliary_crate.dependencies() {
        let dependency_rlib = dependency_rlibs.get(dependency).ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "auxiliary crate '{}' dependency '{}' has no compiled rlib",
                    auxiliary_crate.crate_name(),
                    dependency
                ),
            )
        })?;
        command
            .arg("--extern")
            .arg(rustc_extern_assignment(dependency, dependency_rlib));
    }
    if !auxiliary_crate.dependencies().is_empty() {
        command.arg("-L").arg(rustc_search_path_assignment(
            "dependency",
            auxiliary_build_dir,
        ));
    }
    apply_rustc_profile_args(&mut command, profile);
    Ok(command)
}

pub(crate) fn configure_runtime_rustc_command(
    runtime_source: &Path,
    runtime_build_dir: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<Command> {
    let mut command = Command::new("rustc");
    command
        .arg("--crate-name")
        .arg("fol_runtime")
        .arg("--crate-type")
        .arg("rlib")
        .arg("--edition=2021");
    command
        .arg("--target")
        .arg(machine_target.rust_target_triple());
    command
        .arg(runtime_source)
        .arg("--out-dir")
        .arg(runtime_build_dir);
    apply_rustc_profile_args(&mut command, profile);
    Ok(command)
}

#[cfg(test)]
pub(crate) fn configure_generated_crate_rustc_command(
    crate_root: &Path,
    main_rs: &Path,
    runtime_rlib: &Path,
    binary_path: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<Command> {
    configure_generated_crate_rustc_command_with_args(
        crate_root,
        main_rs,
        runtime_rlib,
        binary_path,
        machine_target,
        profile,
        &[],
    )
}

pub(crate) fn configure_generated_crate_rustc_command_with_args(
    crate_root: &Path,
    main_rs: &Path,
    runtime_rlib: &Path,
    binary_path: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    additional_rustc_args: &[OsString],
) -> BackendResult<Command> {
    configure_generated_crate_rustc_command_impl(
        crate_root,
        main_rs,
        runtime_rlib,
        binary_path,
        machine_target,
        profile,
        None,
        additional_rustc_args,
    )
}

// Rustc command construction keeps each path/policy input explicit so callers
// cannot accidentally conflate host, runtime, and auxiliary artifacts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn configure_generated_crate_rustc_command_with_auxiliary(
    crate_root: &Path,
    main_rs: &Path,
    runtime_rlib: &Path,
    binary_path: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    entry_crate_name: &str,
    entry_rlib: &Path,
    auxiliary_build_dir: &Path,
    additional_rustc_args: &[OsString],
) -> BackendResult<Command> {
    configure_generated_crate_rustc_command_impl(
        crate_root,
        main_rs,
        runtime_rlib,
        binary_path,
        machine_target,
        profile,
        Some((entry_crate_name, entry_rlib, auxiliary_build_dir)),
        additional_rustc_args,
    )
}

#[allow(clippy::too_many_arguments)]
fn configure_generated_crate_rustc_command_impl(
    crate_root: &Path,
    main_rs: &Path,
    runtime_rlib: &Path,
    binary_path: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    auxiliary_entry: Option<(&str, &Path, &Path)>,
    additional_rustc_args: &[OsString],
) -> BackendResult<Command> {
    let mut command = Command::new("rustc");
    command
        .current_dir(crate_root)
        .arg("--crate-name")
        .arg(rustc_crate_name_for_generated_crate(crate_root)?)
        .arg("--edition=2021");
    command
        .arg("--target")
        .arg(machine_target.rust_target_triple());
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
    if let Some((entry_crate_name, entry_rlib, auxiliary_build_dir)) = auxiliary_entry {
        // Keep `--extern` and its assignment as two exact argv items. The
        // opaque caller argv remains an untouched suffix after backend-owned
        // dependency arguments.
        command
            .arg("--extern")
            .arg(rustc_extern_assignment(entry_crate_name, entry_rlib))
            .arg("-L")
            .arg(rustc_search_path_assignment(
                "dependency",
                auxiliary_build_dir,
            ));
    }
    command.args(additional_rustc_args);
    Ok(command)
}

fn runtime_rlib_path(runtime_build_dir: &Path) -> PathBuf {
    runtime_build_dir.join("libfol_runtime.rlib")
}

fn auxiliary_rlib_path(auxiliary_build_dir: &Path, crate_name: &str) -> PathBuf {
    auxiliary_build_dir.join(format!("lib{crate_name}.rlib"))
}

fn auxiliary_build_dir_for_generated_crate(
    crate_root: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> PathBuf {
    crate_root
        .join("target")
        .join(machine_target.rust_target_directory_name())
        .join(profile.as_str())
        .join("fol-auxiliary")
}

#[derive(Debug)]
struct BuiltAuxiliaryRustPlan {
    build_dir: PathBuf,
    entry_crate_name: String,
    entry_rlib: PathBuf,
}

fn build_auxiliary_rust_plan(
    crate_root: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    plan: &BackendAuxiliaryRustPlan,
) -> BackendResult<BuiltAuxiliaryRustPlan> {
    let build_dir = auxiliary_build_dir_for_generated_crate(crate_root, machine_target, profile);
    fs::create_dir_all(&build_dir).map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to create auxiliary Rust build dir '{}': {error}",
                build_dir.display()
            ),
        )
    })?;

    let mut compiled_rlibs = BTreeMap::new();
    for auxiliary_crate in plan.crates() {
        let mut command = configure_auxiliary_crate_rustc_command(
            auxiliary_crate,
            &build_dir,
            machine_target,
            profile,
            &compiled_rlibs,
        )?;
        let output = command.output().map_err(|error| {
            BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "failed to launch rustc for auxiliary crate '{}' from '{}': {error}",
                    auxiliary_crate.crate_name(),
                    auxiliary_crate.source_root().display()
                ),
            )
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "rustc failed for auxiliary crate '{}' from '{}'\nstdout:\n{}\nstderr:\n{}",
                    auxiliary_crate.crate_name(),
                    auxiliary_crate.source_root().display(),
                    stdout.trim(),
                    stderr.trim()
                ),
            ));
        }
        let rlib = auxiliary_rlib_path(&build_dir, auxiliary_crate.crate_name());
        if !rlib.is_file() {
            return Err(BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "rustc succeeded but auxiliary crate '{}' artifact '{}' is missing",
                    auxiliary_crate.crate_name(),
                    rlib.display()
                ),
            ));
        }
        compiled_rlibs.insert(auxiliary_crate.crate_name().to_string(), rlib);
    }

    let entry_crate_name = plan.entry_call().crate_name().to_string();
    let entry_rlib = compiled_rlibs
        .get(&entry_crate_name)
        .cloned()
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("auxiliary main entry crate '{entry_crate_name}' has no compiled rlib"),
            )
        })?;
    Ok(BuiltAuxiliaryRustPlan {
        build_dir,
        entry_crate_name,
        entry_rlib,
    })
}

fn runtime_build_dir_for_generated_crate(
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    crate_root: &Path,
) -> BackendResult<PathBuf> {
    let crate_dir_name = package_name_for_generated_crate(crate_root)?;
    Ok(
        super::runtime::backend_runtime_build_dir(paths, machine_target, profile)?
            .join(crate::sanitize_backend_ident(crate_dir_name)),
    )
}

fn package_name_for_generated_crate(crate_root: &Path) -> BackendResult<&str> {
    crate_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
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
    Ok(crate::sanitize_backend_ident(
        package_name_for_generated_crate(crate_root)?,
    ))
}

fn built_binary_output_path(
    crate_root: &Path,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let package_name = package_name_for_generated_crate(crate_root)?;
    let target_dir = machine_target.rust_target_directory_name();
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
    )?;
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
    build_generated_crate_with_rustc_args(crate_root, paths, machine_target, profile, &[])
}

/// Build a generated FOL binary while appending opaque Rust compiler arguments
/// as exact process arguments.
///
/// Each [`OsString`] is passed to `rustc` as one argv item. This function never
/// shell-splits, joins, normalizes, or deduplicates the supplied arguments.
/// Validation belongs to the typed producer; the backend transports the
/// caller's checked argv unchanged.
pub fn build_generated_crate_with_rustc_args(
    crate_root: &Path,
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    additional_rustc_args: &[OsString],
) -> BackendResult<PathBuf> {
    build_generated_crate_with_rustc_impl(
        crate_root,
        paths,
        machine_target,
        profile,
        None,
        additional_rustc_args,
    )
}

/// Compile an ordered auxiliary Rust plan and link its final safe entry crate
/// into the generated FOL binary.
pub fn build_generated_crate_with_auxiliary_plan(
    crate_root: &Path,
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    plan: &BackendAuxiliaryRustPlan,
) -> BackendResult<PathBuf> {
    build_generated_crate_with_rustc_impl(
        crate_root,
        paths,
        machine_target,
        profile,
        Some(plan),
        plan.final_rustc_argv(),
    )
}

fn build_generated_crate_with_rustc_impl(
    crate_root: &Path,
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
    auxiliary_plan: Option<&BackendAuxiliaryRustPlan>,
    additional_rustc_args: &[OsString],
) -> BackendResult<PathBuf> {
    // Complete all plan, source, dependency-order, target, and profile checks
    // before creating outputs or launching the runtime rustc command.
    if let Some(plan) = auxiliary_plan {
        plan.validate_for_build(machine_target, profile)?;
    }
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
    )?;
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
    let built_auxiliary = auxiliary_plan
        .map(|plan| build_auxiliary_rust_plan(crate_root, machine_target, profile, plan))
        .transpose()?;
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
    let mut command = match &built_auxiliary {
        Some(auxiliary) => configure_generated_crate_rustc_command_with_auxiliary(
            crate_root,
            &main_rs,
            &runtime_rlib,
            &binary_path,
            machine_target,
            profile,
            &auxiliary.entry_crate_name,
            &auxiliary.entry_rlib,
            &auxiliary.build_dir,
            additional_rustc_args,
        )?,
        None => configure_generated_crate_rustc_command_with_args(
            crate_root,
            &main_rs,
            &runtime_rlib,
            &binary_path,
            machine_target,
            profile,
            additional_rustc_args,
        )?,
    };
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
    if let Some(plan) = &config.auxiliary_rust_plan {
        if !matches!(config.mode, BackendMode::BuildArtifact) {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "auxiliary Rust plans require backend build-artifact mode",
            ));
        }
        plan.validate_for_build(&config.machine_target, config.build_profile)?;
    }
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
        BackendMode::BuildArtifact => match &config.auxiliary_rust_plan {
            Some(plan) => build_generated_crate_with_auxiliary_plan(
                &crate_root,
                &paths,
                &config.machine_target,
                config.build_profile,
                plan,
            )?,
            None => build_generated_crate_with_rustc(
                &crate_root,
                &paths,
                &config.machine_target,
                config.build_profile,
            )?,
        },
    };
    let final_binary_dir = PathBuf::from(&paths.bin_root);
    let target_dir = config.machine_target.rust_target_directory_name();
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
    #[cfg(windows)]
    if final_binary.exists() {
        fs::remove_file(&final_binary).map_err(|error| {
            BackendError::new(
                BackendErrorKind::BuildFailure,
                format!(
                    "failed to replace existing binary '{}': {error}",
                    final_binary.display()
                ),
            )
        })?;
    }
    // rustc has exited and closed the built executable. Move that inode into
    // its public output path instead of copying it in this multi-threaded
    // parent process: a concurrent fork can briefly inherit a copy writer and
    // make immediate execution fail with Linux ETXTBSY.
    fs::rename(&built_binary, &final_binary).map_err(|error| {
        BackendError::new(
            BackendErrorKind::BuildFailure,
            format!(
                "failed to publish built binary '{}' as '{}': {error}",
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
