use crate::{
    FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult, FrontendConfig,
    FrontendError, FrontendErrorKind, FrontendProfile, FrontendResult, FrontendWorkspace,
};
use fol_build::{evaluate_build_source, BuildEvaluationInputs, BuildEvaluationRequest};
use std::{fs, path::Path};

/// Write an executed program's captured stdout/stderr through to the
/// frontend's own streams so `run` stays transparent to child output.
fn forward_child_output(stdout: &[u8], stderr: &[u8]) {
    use std::io::Write;
    if !stdout.is_empty() {
        let mut out = std::io::stdout();
        let _ = out.write_all(stdout);
        let _ = out.flush();
    }
    if !stderr.is_empty() {
        let mut err = std::io::stderr();
        let _ = err.write_all(stderr);
        let _ = err.flush();
    }
}

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrontendArtifactExecutionSelection {
    pub package_root: std::path::PathBuf,
    pub label: String,
    pub root_module: Option<String>,
    pub capability_model: fol_backend::BackendFolModel,
    pub fol_model: fol_backend::BackendFolModel,
    pub has_bundled_std: bool,
}

pub(crate) fn package_declares_bundled_std(root: &std::path::Path) -> bool {
    fol_package::parse_package_metadata_from_build(&root.join("build.fol"))
        .map(|metadata| {
            metadata.dependencies.iter().any(|dependency| {
                dependency.source_kind == fol_package::PackageDependencySourceKind::Internal
                    && dependency.target == "standard"
            })
        })
        .unwrap_or(false)
}

pub(crate) fn effective_runtime_model_for_package(
    root: &std::path::Path,
    fol_model: fol_backend::BackendFolModel,
) -> fol_backend::BackendFolModel {
    // The bundled std package IS the hosted substrate; its own units are
    // the layer that legally reaches hosted intrinsics like `.echo`.
    if is_bundled_std_root(root) {
        return fol_backend::BackendFolModel::Std;
    }
    match fol_model {
        fol_backend::BackendFolModel::Memo if package_declares_bundled_std(root) => {
            fol_backend::BackendFolModel::Std
        }
        other => other,
    }
}

fn is_bundled_std_root(root: &std::path::Path) -> bool {
    fol_package::available_bundled_std_root()
        .and_then(|std_root| std_root.canonicalize().ok())
        .zip(root.canonicalize().ok())
        .map(|(std_root, root)| std_root == root)
        .unwrap_or(false)
}

fn declared_capability_model_for_package(root: &std::path::Path) -> fol_backend::BackendFolModel {
    let build_path = root.join("build.fol");
    let Some(source) = fs::read_to_string(&build_path).ok() else {
        return fol_backend::BackendFolModel::Memo;
    };
    let Some(evaluated) = evaluate_build_source(
        &BuildEvaluationRequest {
            package_root: root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        },
        &build_path,
        &source,
    )
    .ok()
    .flatten()
    else {
        return fol_backend::BackendFolModel::Memo;
    };

    if evaluated
        .evaluated
        .artifacts
        .iter()
        .all(|artifact| artifact.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Core)
    {
        fol_backend::BackendFolModel::Core
    } else {
        fol_backend::BackendFolModel::Memo
    }
}

fn summarize_capability_modes<I>(models: I) -> String
where
    I: IntoIterator<Item = fol_backend::BackendFolModel>,
{
    let mut models = models
        .into_iter()
        .map(|model| match model {
            fol_backend::BackendFolModel::Core => fol_backend::BackendFolModel::Core,
            fol_backend::BackendFolModel::Memo | fol_backend::BackendFolModel::Std => {
                fol_backend::BackendFolModel::Memo
            }
        })
        .collect::<Vec<_>>();
    models.sort_by_key(|model| match model {
        fol_backend::BackendFolModel::Core => 0,
        fol_backend::BackendFolModel::Memo => 1,
        fol_backend::BackendFolModel::Std => 2,
    });
    models.dedup();
    let rendered = models
        .into_iter()
        .map(|model| model.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!("capability_mode={rendered}")
}

fn summarize_bundled_std_presence(selections: &[FrontendArtifactExecutionSelection]) -> String {
    let with_std = selections
        .iter()
        .filter(|selection| selection.has_bundled_std)
        .count();
    format!("bundled_std={with_std}/{}", selections.len())
}

fn produced_artifact_count(result: &FrontendCommandResult) -> usize {
    result
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind != FrontendArtifactKind::BuildRoot)
        .count()
}

pub fn check_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    if config.locked_fetch {
        crate::fetch_workspace_with_config(workspace, config)?;
    }
    for member in &workspace.members {
        // Check under the member's declared capability model so core/memo
        // legality surfaces at check time, not first at build time.
        let fol_model = effective_runtime_model_for_package(
            &member.root,
            declared_capability_model_for_package(&member.root),
        );
        compile_member_workspace_for_model(workspace, config, &member.root, fol_model)?;
    }

    let mut result = FrontendCommandResult::new(
        "check",
        format!("checked {} workspace package(s)", workspace.members.len()),
    );
    for member in &workspace.members {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            member
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package"),
            Some(member.root.clone()),
        ));
    }
    Ok(result)
}

pub fn check_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    check_workspace_with_config(workspace, &FrontendConfig::default())
}

pub fn build_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    build_workspace_for_profile_with_config(workspace, config, FrontendProfile::Debug)
}

pub fn build_workspace_for_profile_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
) -> FrontendResult<FrontendCommandResult> {
    let selections = workspace
        .members
        .iter()
        .map(|member| FrontendArtifactExecutionSelection {
            package_root: member.root.clone(),
            label: member
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package")
                .to_string(),
            root_module: None,
            capability_model: declared_capability_model_for_package(&member.root),
            fol_model: effective_runtime_model_for_package(
                &member.root,
                declared_capability_model_for_package(&member.root),
            ),
            has_bundled_std: package_declares_bundled_std(&member.root),
        })
        .collect::<Vec<_>>();
    build_selected_artifacts_for_profile_with_config(workspace, config, profile, &selections)
}

pub(crate) fn build_selected_artifacts_for_profile_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
    selections: &[FrontendArtifactExecutionSelection],
) -> FrontendResult<FrontendCommandResult> {
    if config.locked_fetch {
        crate::fetch_workspace_with_config(workspace, config)?;
    }
    let mut result = FrontendCommandResult::new("build", "built 0 workspace package(s)");
    let output_root = profile_build_root(workspace, profile);
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::BuildRoot,
        format!("{:?}", profile).to_lowercase(),
        Some(output_root.clone()),
    ));

    for selection in selections {
        let lowered = compile_member_workspace_targeted(
            workspace,
            config,
            &selection.package_root,
            selection.root_module.as_deref(),
            selection.fol_model,
        )?;
        if lowered.entry_candidates().is_empty() {
            continue;
        }
        let backend_session = fol_backend::BackendSession::new(lowered);
        let artifact = fol_backend::emit_backend_artifact(
            &backend_session,
            &backend_config(config, profile, selection.fol_model),
            &output_root,
        )
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;
        let fol_backend::BackendArtifact::CompiledBinary {
            crate_root,
            binary_path,
        } = artifact
        else {
            return Err(FrontendError::new(
                FrontendErrorKind::CommandFailed,
                "build command expected a compiled backend artifact",
            ));
        };
        let crate_root = std::path::PathBuf::from(crate_root);
        if crate_root.exists() {
            result.artifacts.push(FrontendArtifactSummary::new(
                FrontendArtifactKind::EmittedRust,
                format!("{}-crate", selection.label),
                Some(crate_root),
            ));
        }
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::Binary,
            selection.label.clone(),
            Some(std::path::PathBuf::from(binary_path)),
        ));
    }

    if result.artifacts.is_empty() {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            "build command did not find any runnable workspace packages",
        ));
    }

    let binary_count = result
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .count();
    let output_count = produced_artifact_count(&result);
    result.summary = format!(
        "built {binary_count} workspace package(s) into {} ({}, {}, install_prefix={}, outputs={output_count})",
        output_root.display(),
        summarize_capability_modes(selections.iter().map(|selection| selection.capability_model)),
        summarize_bundled_std_presence(selections),
        workspace.install_prefix.display()
    );
    Ok(result)
}

pub fn build_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    build_workspace_with_config(workspace, &FrontendConfig::default())
}

pub fn profile_build_root(
    workspace: &FrontendWorkspace,
    profile: FrontendProfile,
) -> std::path::PathBuf {
    workspace.build_root.join(match profile {
        FrontendProfile::Debug => "debug",
        FrontendProfile::Release => "release",
    })
}

pub fn run_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    run_workspace_with_args_and_config(workspace, config, &[])
}

pub fn run_workspace_with_args_and_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    args: &[String],
) -> FrontendResult<FrontendCommandResult> {
    ensure_host_runnable_target(config, "run")?;
    let built = build_workspace_with_config(workspace, config)?;
    let binaries = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    if binaries.len() != 1 {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "run command requires exactly one runnable workspace package, found {}",
                binaries.len()
            ),
        ));
    }

    let binary = binaries[0].path.as_ref().cloned().ok_or_else(|| {
        FrontendError::new(
            FrontendErrorKind::Internal,
            "build result is missing a binary path",
        )
    })?;
    let output = std::process::Command::new(&binary)
        .args(args)
        .output()
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;

    // Forward the executed program's own output; `run` should be
    // transparent to the child's stdout/stderr, not swallow it.
    forward_child_output(&output.stdout, &output.stderr);

    if !output.status.success() {
        return Err(FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "run command failed for '{}': status {}",
                binary.display(),
                output.status
            ),
        ));
    }

    let mut result = FrontendCommandResult::new(
        "run",
        format!(
            "ran {} ({}, bundled_std={})",
            binary.display(),
            summarize_capability_modes(workspace.members.iter().map(|member| declared_capability_model_for_package(&member.root))),
            if workspace
                .members
                .iter()
                .any(|member| package_declares_bundled_std(&member.root))
            {
                "1/1"
            } else {
                "0/1"
            }
        ),
    );
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::Binary,
        "binary",
        Some(binary),
    ));
    Ok(result)
}

pub(crate) fn run_selected_artifact_with_args_and_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
    selection: &FrontendArtifactExecutionSelection,
    args: &[String],
) -> FrontendResult<FrontendCommandResult> {
    ensure_host_runnable_target(config, "run")?;
    ensure_std_execution_selection(selection, "run")?;
    let built = build_selected_artifacts_for_profile_with_config(
        workspace,
        config,
        profile,
        std::slice::from_ref(selection),
    )?;
    let binaries = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    if binaries.len() != 1 {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "run command requires exactly one runnable selected artifact, found {}",
                binaries.len()
            ),
        ));
    }

    let binary = binaries[0].path.as_ref().cloned().ok_or_else(|| {
        FrontendError::new(
            FrontendErrorKind::Internal,
            "build result is missing a binary path",
        )
    })?;
    let output = std::process::Command::new(&binary)
        .args(args)
        .output()
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;

    // Forward the executed program's own output; `run` should be
    // transparent to the child's stdout/stderr, not swallow it.
    forward_child_output(&output.stdout, &output.stderr);

    if !output.status.success() {
        return Err(FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "run command failed for '{}': status {}",
                binary.display(),
                output.status
            ),
        ));
    }

    let mut result = FrontendCommandResult::new(
        "run",
        format!(
            "ran {} ({}, bundled_std={})",
            binary.display(),
            summarize_capability_modes([selection.capability_model]),
            if selection.has_bundled_std { "1/1" } else { "0/1" }
        ),
    );
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::Binary,
        selection.label.clone(),
        Some(binary),
    ));
    Ok(result)
}

pub fn run_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    run_workspace_with_config(workspace, &FrontendConfig::default())
}

pub fn test_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    if config.locked_fetch {
        crate::fetch_workspace_with_config(workspace, config)?;
    }
    test_workspace_selected_with_config(workspace, config, None)
}

pub(crate) fn test_selected_artifacts_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
    selections: &[FrontendArtifactExecutionSelection],
) -> FrontendResult<FrontendCommandResult> {
    ensure_host_runnable_target(config, "test")?;
    ensure_std_execution_selections(selections, "test")?;
    let built =
        build_selected_artifacts_for_profile_with_config(workspace, config, profile, selections)?;
    let binaries = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    if binaries.is_empty() {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            "test command did not find any runnable selected artifacts",
        ));
    }

    for binary in &binaries {
        let path = binary.path.as_ref().ok_or_else(|| {
            FrontendError::new(
                FrontendErrorKind::Internal,
                "build result is missing a binary path",
            )
        })?;
        let status = std::process::Command::new(path).status().map_err(|error| {
            FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string())
        })?;
        if !status.success() {
            return Err(FrontendError::new(
                FrontendErrorKind::CommandFailed,
                format!(
                    "test command failed for '{}': status {status}",
                    path.display()
                ),
            ));
        }
    }

    let mut result = FrontendCommandResult::new(
        "test",
        format!(
            "tested {} workspace artifact(s) ({}, {})",
            binaries.len(),
            summarize_capability_modes(selections.iter().map(|selection| selection.capability_model)),
            summarize_bundled_std_presence(selections)
        ),
    );
    for binary in binaries {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::Binary,
            binary.label.clone(),
            binary.path.clone(),
        ));
    }
    Ok(result)
}

pub fn test_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    test_workspace_with_config(workspace, &FrontendConfig::default())
}

pub fn test_package_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_name: &str,
) -> FrontendResult<FrontendCommandResult> {
    test_workspace_selected_with_config(workspace, config, Some(package_name))
}

pub fn test_package(
    workspace: &FrontendWorkspace,
    package_name: &str,
) -> FrontendResult<FrontendCommandResult> {
    test_package_with_config(workspace, &FrontendConfig::default(), package_name)
}

pub fn emit_rust_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let output_root = workspace.build_root.join("emit").join("rust");
    let mut result = FrontendCommandResult::new("emit rust", "emitted 0 Rust crate(s)");
    let mut emitted = 0usize;
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::BuildRoot,
        "emit-rust-root",
        Some(output_root.clone()),
    ));

    for member in &workspace.members {
        let lowered = compile_member_workspace(workspace, config, &member.root)?;
        let backend_session = fol_backend::BackendSession::new(lowered);
        let artifact = fol_backend::emit_backend_artifact(
            &backend_session,
            &fol_backend::BackendConfig {
                mode: fol_backend::BackendMode::EmitSource,
                keep_build_dir: true,
                ..backend_config(
                    config,
                    FrontendProfile::Release,
                    fol_backend::BackendFolModel::Std,
                )
            },
            &output_root,
        )
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;
        let fol_backend::BackendArtifact::RustSourceCrate { root, .. } = artifact else {
            return Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "emit rust expected a backend source artifact",
            ));
        };
        emitted += 1;
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::EmittedRust,
            member
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package"),
            Some(std::path::PathBuf::from(root)),
        ));
    }

    result.summary = format!(
        "emitted {emitted} Rust crate(s) into {}",
        output_root.display()
    );
    Ok(result)
}

pub fn emit_rust(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    emit_rust_with_config(workspace, &FrontendConfig::default())
}

pub fn emit_lowered_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let output_root = workspace.build_root.join("emit").join("lowered");
    fs::create_dir_all(&output_root).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "failed to create lowered emit root '{}': {error}",
                output_root.display()
            ),
        )
    })?;

    let mut result = FrontendCommandResult::new("emit lowered", "emitted 0 lowered snapshot(s)");
    let mut emitted = 0usize;
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::BuildRoot,
        "emit-lowered-root",
        Some(output_root.clone()),
    ));

    for member in &workspace.members {
        let lowered = compile_member_workspace(workspace, config, &member.root)?;
        let rendered = fol_lower::render_lowered_workspace(&lowered);
        let label = member
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let snapshot_path = output_root.join(format!("{label}.lowered.txt"));
        fs::write(&snapshot_path, rendered).map_err(|error| {
            FrontendError::new(
                FrontendErrorKind::CommandFailed,
                format!(
                    "failed to write lowered snapshot '{}': {error}",
                    snapshot_path.display()
                ),
            )
        })?;
        emitted += 1;
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::LoweredSnapshot,
            label,
            Some(snapshot_path),
        ));
    }

    result.summary = format!(
        "emitted {emitted} lowered snapshot(s) into {}",
        output_root.display()
    );
    Ok(result)
}

pub fn emit_lowered(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    emit_lowered_with_config(workspace, &FrontendConfig::default())
}

pub fn compile_member_workspace(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    compile_member_workspace_for_model(
        workspace,
        config,
        package_root,
        fol_backend::BackendFolModel::Std,
    )
}

fn compile_member_workspace_for_model(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
    fol_model: fol_backend::BackendFolModel,
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    validate_build_dependency_queries(workspace, config, package_root)?;
    let display_name = package_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package");
    let syntax = fol_package::parse_directory_package_syntax(
        package_root,
        display_name,
        fol_package::PackageSourceKind::Package,
    )
    .map_err(FrontendError::from)?;
    let prepared = fol_package::PackageSession::with_config(fol_package::PackageConfig::default())
        .prepare_entry_package(syntax)
        .map_err(FrontendError::from)?;

    let resolved = fol_resolver::resolve_prepared_workspace_with_config(
        prepared,
        resolver_config(workspace, config),
    )
    .map_err(FrontendError::from_errors)?;
    let typed = fol_typecheck::Typechecker::with_config(fol_typecheck::TypecheckConfig {
        capability_model: typecheck_capability_model(fol_model),
    })
    .check_resolved_workspace(resolved)
    .map_err(FrontendError::from_errors)?;
    fol_lower::Lowerer::new()
        .lower_typed_workspace(typed)
        .map_err(FrontendError::from_errors)
}

fn validate_build_dependency_queries(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
) -> FrontendResult<()> {
    let build_path = package_root.join("build.fol");
    let source = match fs::read_to_string(&build_path) {
        Ok(source) => source,
        Err(_) => return Ok(()),
    };
    let evaluated = match fol_package::evaluate_build_source(
        &fol_package::BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: fol_package::BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                install_prefix: package_root.join(".fol/install").display().to_string(),
                ..fol_package::BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        },
        &build_path,
        &source,
    ) {
        Ok(Some(evaluated)) => evaluated,
        Ok(None) => return Ok(()),
        Err(error) => {
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                error.to_string(),
            ));
        }
    };
    if evaluated.evaluated.dependency_queries.is_empty() {
        return Ok(());
    }

    let metadata =
        fol_package::parse_package_metadata_from_build(&build_path).map_err(FrontendError::from)?;
    let package_store_root = config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone())
        .unwrap_or_else(|| workspace.root.root.join(".fol").join("pkg"));
    let std_root = workspace
        .std_root_override
        .clone()
        .or_else(fol_package::available_bundled_std_root);

    for query in &evaluated.evaluated.dependency_queries {
        let metadata_dependency = metadata
            .dependencies
            .iter()
            .find(|dependency| dependency.alias == query.dependency_alias);
        let evaluated_dependency = evaluated
            .result
            .dependency_requests
            .iter()
            .find(|dependency| dependency.alias == query.dependency_alias);
        let dependency_root = resolve_dependency_query_root(
            package_root,
            &package_store_root,
            std_root.as_deref(),
            metadata_dependency,
            evaluated_dependency,
        )?;
        let syntax = fol_package::parse_directory_package_syntax(
            &dependency_root,
            &query.dependency_alias,
            fol_package::PackageSourceKind::Package,
        )
        .map_err(FrontendError::from)?;
        let surface = fol_package::build_dependency::project_dependency_surface(
            &query.dependency_alias,
            &dependency_root,
            &syntax,
        )
        .map_err(FrontendError::from)?;
        let exported = match query.kind {
            fol_package::BuildRuntimeDependencyQueryKind::Module => {
                surface.find_module(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::Artifact => {
                surface.find_artifact(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::Step => {
                surface.find_step(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::File => {
                surface.find_file(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::Dir => {
                surface.find_dir(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::Path => {
                surface.find_path(&query.query_name).is_some()
            }
            fol_package::BuildRuntimeDependencyQueryKind::GeneratedOutput => {
                surface.find_generated_output(&query.query_name).is_some()
            }
        };
        if !exported {
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!(
                    "dependency '{}' does not export {} '{}'",
                    query.dependency_alias,
                    dependency_query_kind_label(query.kind),
                    query.query_name
                ),
            ));
        }
    }

    Ok(())
}

fn resolve_dependency_query_root(
    package_root: &Path,
    package_store_root: &Path,
    std_root: Option<&Path>,
    metadata_dependency: Option<&fol_package::PackageDependencyDecl>,
    evaluated_dependency: Option<&fol_package::DependencyRequest>,
) -> FrontendResult<std::path::PathBuf> {
    if let Some(dependency) = metadata_dependency {
        return Ok(match dependency.source_kind {
            fol_package::PackageDependencySourceKind::Local => {
                package_root.join(&dependency.target)
            }
            fol_package::PackageDependencySourceKind::PackageStore => {
                package_store_root.join(&dependency.target)
            }
            fol_package::PackageDependencySourceKind::Git => {
                package_store_root.join(&dependency.alias)
            }
            fol_package::PackageDependencySourceKind::Internal => {
                std_root
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| package_store_root.join(&dependency.alias))
            }
        });
    }

    if let Some(dependency) = evaluated_dependency {
        let local_root = package_root.join(&dependency.package);
        if local_root.join("build.fol").is_file() {
            return Ok(local_root);
        }
        let package_root = package_store_root.join(&dependency.package);
        if package_root.join("build.fol").is_file() {
            return Ok(package_root);
        }
        let alias_root = package_store_root.join(&dependency.alias);
        if alias_root.join("build.fol").is_file() {
            return Ok(alias_root);
        }
    }

    let alias = metadata_dependency
        .map(|dependency| dependency.alias.as_str())
        .or_else(|| evaluated_dependency.map(|dependency| dependency.alias.as_str()))
        .unwrap_or("<unknown>");
    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!(
            "build dependency query references undeclared dependency alias '{}'",
            alias
        ),
    ))
}

fn dependency_query_kind_label(kind: fol_package::BuildRuntimeDependencyQueryKind) -> &'static str {
    match kind {
        fol_package::BuildRuntimeDependencyQueryKind::Module => "module",
        fol_package::BuildRuntimeDependencyQueryKind::Artifact => "artifact",
        fol_package::BuildRuntimeDependencyQueryKind::Step => "step",
        fol_package::BuildRuntimeDependencyQueryKind::File => "file",
        fol_package::BuildRuntimeDependencyQueryKind::Dir => "dir",
        fol_package::BuildRuntimeDependencyQueryKind::Path => "path",
        fol_package::BuildRuntimeDependencyQueryKind::GeneratedOutput => "generated output",
    }
}

fn compile_member_workspace_targeted(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
    root_module: Option<&str>,
    fol_model: fol_backend::BackendFolModel,
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    let lowered = compile_member_workspace_for_model(workspace, config, package_root, fol_model)?;
    let Some(root_module) = root_module else {
        return Ok(lowered);
    };

    let matching_candidates = lowered
        .entry_candidates()
        .iter()
        .filter(|candidate| entry_candidate_matches_root_module(&lowered, candidate, root_module))
        .cloned()
        .collect::<Vec<_>>();
    if matching_candidates.is_empty() {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "workspace package '{}' does not expose a runnable entry for build root '{}'",
                package_root.display(),
                root_module
            ),
        ));
    }
    Ok(lowered.with_entry_candidates(matching_candidates))
}

fn entry_candidate_matches_root_module(
    lowered: &fol_lower::LoweredWorkspace,
    candidate: &fol_lower::LoweredEntryCandidate,
    root_module: &str,
) -> bool {
    let normalized_root = root_module.replace('\\', "/");
    let Some(package) = lowered.package(&candidate.package_identity) else {
        return false;
    };
    let Some(routine) = package.routine_decls.get(&candidate.routine_id) else {
        return false;
    };
    let Some(source_unit_id) = routine.source_unit_id else {
        return false;
    };
    let Some(source_unit) = package
        .source_units
        .iter()
        .find(|unit| unit.source_unit_id == source_unit_id)
    else {
        return false;
    };
    let normalized_source = source_unit.path.replace('\\', "/");
    normalized_source == normalized_root
        || normalized_source.ends_with(&format!("/{normalized_root}"))
}

fn resolver_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> fol_resolver::ResolverConfig {
    let package_store_root = config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone())
        .unwrap_or_else(|| workspace.root.root.join(".fol/pkg"));

    fol_resolver::ResolverConfig {
        std_root: config
            .std_root_override
            .clone()
            .or_else(|| workspace.std_root_override.clone())
            .or_else(fol_package::available_bundled_std_root)
            .map(|path| path.to_string_lossy().to_string()),
        package_store_root: Some(package_store_root.to_string_lossy().to_string()),
    }
}

fn backend_profile(profile: FrontendProfile) -> fol_backend::BackendBuildProfile {
    match profile {
        FrontendProfile::Debug => fol_backend::BackendBuildProfile::Debug,
        FrontendProfile::Release => fol_backend::BackendBuildProfile::Release,
    }
}

fn typecheck_capability_model(
    fol_model: fol_backend::BackendFolModel,
) -> fol_typecheck::TypecheckCapabilityModel {
    match fol_model {
        fol_backend::BackendFolModel::Core => fol_typecheck::TypecheckCapabilityModel::Core,
        fol_backend::BackendFolModel::Memo => fol_typecheck::TypecheckCapabilityModel::Memo,
        fol_backend::BackendFolModel::Std => fol_typecheck::TypecheckCapabilityModel::Std,
    }
}

fn backend_config(
    config: &FrontendConfig,
    profile: FrontendProfile,
    fol_model: fol_backend::BackendFolModel,
) -> fol_backend::BackendConfig {
    fol_backend::BackendConfig {
        fol_model,
        machine_target: config.backend_machine_target(),
        build_profile: backend_profile(profile),
        keep_build_dir: config.keep_build_dir,
        ..fol_backend::BackendConfig::default()
    }
}

fn ensure_host_runnable_target(config: &FrontendConfig, command: &str) -> FrontendResult<()> {
    if config.machine_target_runs_on_host() {
        return Ok(());
    }
    let machine_target = config.backend_machine_target();
    let selected = machine_target
        .rust_target_triple()
        .unwrap_or_else(|| machine_target.display_name().to_string());
    let host = FrontendConfig::host_rust_target_triple().unwrap_or("unknown-host");
    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!("{command} command cannot execute target '{selected}' on host '{host}'"),
    ))
}

fn ensure_std_execution_selection(
    selection: &FrontendArtifactExecutionSelection,
    command: &str,
) -> FrontendResult<()> {
    let _ = (selection, command);
    Ok(())
}

fn ensure_std_execution_selections(
    selections: &[FrontendArtifactExecutionSelection],
    command: &str,
) -> FrontendResult<()> {
    for selection in selections {
        ensure_std_execution_selection(selection, command)?;
    }
    Ok(())
}

fn test_workspace_selected_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    selected_package: Option<&str>,
) -> FrontendResult<FrontendCommandResult> {
    ensure_host_runnable_target(config, "test")?;
    let selected_members = selected_workspace_members(workspace, selected_package)?;
    let mut result = FrontendCommandResult::new("test", "tested 0 workspace package(s)");
    let mut tested_count = 0usize;

    for member in selected_members {
        let member_workspace = FrontendWorkspace {
            root: workspace.root.clone(),
            members: vec![member.clone()],
            std_root_override: workspace.std_root_override.clone(),
            package_store_root_override: workspace.package_store_root_override.clone(),
            build_root: workspace.build_root.clone(),
            cache_root: workspace.cache_root.clone(),
            git_cache_root: workspace.git_cache_root.clone(),
            install_prefix: workspace.install_prefix.clone(),
        };
        let member_result = run_workspace_with_config(&member_workspace, config)?;
        result.artifacts.extend(member_result.artifacts);
        tested_count += 1;
    }

    result.summary = format!(
        "tested {tested_count} workspace package(s) ({}, bundled_std={})",
        summarize_capability_modes(
            workspace
                .members
                .iter()
                .map(|member| declared_capability_model_for_package(&member.root))
        ),
        if workspace
            .members
            .iter()
            .any(|member| package_declares_bundled_std(&member.root))
        {
            "1/1"
        } else {
            "0/1"
        }
    );
    Ok(result)
}

fn selected_workspace_members(
    workspace: &FrontendWorkspace,
    selected_package: Option<&str>,
) -> FrontendResult<Vec<crate::PackageRoot>> {
    match selected_package {
        Some(selected_package) => workspace
            .members
            .iter()
            .find(|member| {
                member.root.file_name().and_then(|name| name.to_str()) == Some(selected_package)
            })
            .cloned()
            .map(|member| vec![member])
            .ok_or_else(|| {
                FrontendError::new(
                    FrontendErrorKind::InvalidInput,
                    format!("workspace package '{selected_package}' was not found"),
                )
            }),
        None => Ok(workspace.members.clone()),
    }
}
