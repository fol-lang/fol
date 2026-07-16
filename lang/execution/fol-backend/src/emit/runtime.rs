use crate::{
    BackendBuildPaths, BackendBuildProfile, BackendError, BackendErrorKind, BackendMachineTarget,
    BackendResult,
};
use std::fs;
use std::path::{Path, PathBuf};

pub fn backend_runtime_source_root_with_override(override_path: Option<&Path>) -> PathBuf {
    override_path.map(Path::to_path_buf).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("fol-runtime")
    })
}

pub fn backend_runtime_source_root() -> PathBuf {
    backend_runtime_source_root_with_override(
        std::env::var_os("FOL_BACKEND_RUNTIME_PATH")
            .as_deref()
            .map(Path::new),
    )
}

pub fn backend_runtime_source_entry_with_override(override_path: Option<&Path>) -> PathBuf {
    backend_runtime_source_root_with_override(override_path)
        .join("src")
        .join("lib.rs")
}

pub fn backend_runtime_source_entry() -> PathBuf {
    backend_runtime_source_entry_with_override(
        std::env::var_os("FOL_BACKEND_RUNTIME_PATH")
            .as_deref()
            .map(Path::new),
    )
}

pub fn backend_runtime_manifest_path_with_override(override_path: Option<&Path>) -> PathBuf {
    backend_runtime_source_root_with_override(override_path).join("Cargo.toml")
}

pub fn backend_runtime_manifest_path() -> PathBuf {
    backend_runtime_manifest_path_with_override(
        std::env::var_os("FOL_BACKEND_RUNTIME_PATH")
            .as_deref()
            .map(Path::new),
    )
}

pub fn backend_runtime_build_dir(
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let target_dir = machine_target.rust_target_directory_name();
    Ok(PathBuf::from(&paths.runtime_root)
        .join(target_dir)
        .join(profile.as_str()))
}

pub fn prepare_backend_runtime_build_dir(
    paths: &BackendBuildPaths,
    machine_target: &BackendMachineTarget,
    profile: BackendBuildProfile,
) -> BackendResult<PathBuf> {
    let runtime_dir = backend_runtime_build_dir(paths, machine_target, profile)?;
    fs::create_dir_all(&runtime_dir).map_err(|error| {
        BackendError::new(
            BackendErrorKind::EmissionFailure,
            format!(
                "failed to create backend runtime build dir '{}': {error}",
                runtime_dir.display()
            ),
        )
    })?;
    Ok(runtime_dir)
}
