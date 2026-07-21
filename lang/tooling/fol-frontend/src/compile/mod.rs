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
    pub artifact_capabilities: Vec<DeclaredArtifactCapability>,
    pub capability_model: fol_backend::BackendFolModel,
    pub fol_model: fol_backend::BackendFolModel,
    pub has_bundled_std: bool,
    pub target: fol_types::ResolvedTarget,
    pub optimize: fol_package::BuildOptimizeMode,
    pub c_imports: Vec<fol_package::BuildCImportAttachment>,
    pub graph_binding: Option<FrontendArtifactGraphBinding>,
}

impl FrontendArtifactExecutionSelection {
    pub(crate) fn backend_build_profile(&self) -> fol_backend::BackendBuildProfile {
        backend_build_profile_for_optimize(self.optimize)
    }

    fn output_profile(&self) -> FrontendProfile {
        match self.backend_build_profile() {
            fol_backend::BackendBuildProfile::Debug => FrontendProfile::Debug,
            fol_backend::BackendBuildProfile::Release => FrontendProfile::Release,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrontendArtifactGraphBinding {
    pub graph: fol_package::BuildGraph,
    pub artifact_id: fol_package::BuildArtifactId,
}

pub(crate) fn effective_runtime_model_for_package_with_bundled_std(
    root: &std::path::Path,
    fol_model: fol_backend::BackendFolModel,
    has_bundled_std: bool,
) -> fol_backend::BackendFolModel {
    // The bundled std package IS the hosted substrate; its own units are
    // the layer that legally reaches hosted intrinsics like `.echo`.
    if is_bundled_std_root(root) {
        return fol_backend::BackendFolModel::Std;
    }
    match fol_model {
        fol_backend::BackendFolModel::Memo if has_bundled_std => fol_backend::BackendFolModel::Std,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeclaredArtifactCapability {
    pub root_module: String,
    pub model: fol_backend::BackendFolModel,
}

pub(crate) fn build_evaluation_inputs(
    root: &Path,
    config: &FrontendConfig,
) -> FrontendResult<BuildEvaluationInputs> {
    let mut inputs = BuildEvaluationInputs {
        working_directory: root.display().to_string(),
        install_prefix: config
            .install_prefix_override
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| root.join(".fol/install").display().to_string()),
        ..BuildEvaluationInputs::default()
    };
    if let Some(target) = &config.build_target_override {
        let target = target.trim();
        inputs.target = Some(fol_types::ResolvedTarget::resolve(target).map_err(|error| {
            FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
        })?);
    }
    if let Some(optimize) = &config.build_optimize_override {
        inputs.optimize = Some(parse_build_optimize(optimize)?);
    }
    for override_value in &config.build_option_overrides {
        let Some((key, value)) = override_value.split_once('=') else {
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!("invalid build option '{override_value}'; expected name=value"),
            ));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!("invalid build option '{override_value}'; option name must not be empty"),
            ));
        }
        inputs.options.insert(key.to_string(), value.to_string());
    }
    Ok(inputs)
}

fn evaluate_package_build(
    root: &Path,
    config: &FrontendConfig,
) -> FrontendResult<Option<fol_build::EvaluatedBuildSource>> {
    let build_path = root.join("build.fol");
    let source = match fs::read_to_string(&build_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(FrontendError::new(
                FrontendErrorKind::CommandFailed,
                format!(
                    "failed to read build file '{}': {error}",
                    build_path.display()
                ),
            ));
        }
    };
    evaluate_build_source(
        &BuildEvaluationRequest {
            package_root: root.display().to_string(),
            inputs: build_evaluation_inputs(root, config)?,
            operations: Vec::new(),
        },
        &build_path,
        &source,
    )
    .map_err(|error| FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvaluatedPackageCapabilityContract {
    artifacts: Vec<DeclaredArtifactCapability>,
    has_bundled_std: bool,
}

pub(crate) fn evaluated_program_declares_bundled_std(
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
) -> bool {
    evaluated.dependencies.iter().any(|dependency| {
        dependency.source_kind == fol_build::DependencySourceKind::Internal
            && dependency.package == "standard"
    })
}

fn evaluated_package_capability_contract(
    root: &Path,
    config: &FrontendConfig,
) -> FrontendResult<EvaluatedPackageCapabilityContract> {
    let Some(evaluated) = evaluate_package_build(root, config)? else {
        return Ok(EvaluatedPackageCapabilityContract {
            artifacts: Vec::new(),
            has_bundled_std: false,
        });
    };

    let has_bundled_std = evaluated_program_declares_bundled_std(&evaluated.evaluated);
    let artifacts = evaluated
        .evaluated
        .artifacts
        .iter()
        .map(|artifact| DeclaredArtifactCapability {
            root_module: artifact.root_module.clone(),
            model: match artifact.fol_model {
                fol_package::build_artifact::BuildArtifactFolModel::Core => {
                    fol_backend::BackendFolModel::Core
                }
                fol_package::build_artifact::BuildArtifactFolModel::Memo => {
                    fol_backend::BackendFolModel::Memo
                }
            },
        })
        .collect();
    Ok(EvaluatedPackageCapabilityContract {
        artifacts,
        has_bundled_std,
    })
}

#[cfg(test)]
fn declared_artifact_capabilities_for_package_with_config(
    root: &Path,
    config: &FrontendConfig,
) -> FrontendResult<Vec<DeclaredArtifactCapability>> {
    Ok(evaluated_package_capability_contract(root, config)?.artifacts)
}

#[cfg(test)]
fn declared_artifact_capabilities_for_package(
    root: &std::path::Path,
) -> Vec<DeclaredArtifactCapability> {
    declared_artifact_capabilities_for_package_with_config(root, &FrontendConfig::default())
        .unwrap_or_default()
}

fn distinct_declared_capability_models(
    capabilities: &[DeclaredArtifactCapability],
) -> Vec<fol_backend::BackendFolModel> {
    let mut models = capabilities
        .iter()
        .map(|capability| capability.model)
        .collect::<Vec<_>>();
    if models.is_empty() {
        models.push(fol_backend::BackendFolModel::Memo);
    }
    models.sort_by_key(|model| match model {
        fol_backend::BackendFolModel::Core => 0,
        fol_backend::BackendFolModel::Memo => 1,
        fol_backend::BackendFolModel::Std => 2,
    });
    models.dedup();
    models
}

fn public_capability_model(model: fol_backend::BackendFolModel) -> fol_backend::BackendFolModel {
    match model {
        fol_backend::BackendFolModel::Std => fol_backend::BackendFolModel::Memo,
        other => other,
    }
}

fn canonical_artifact_root(
    package_root: &Path,
    artifact_label: &str,
    root_module: &str,
) -> FrontendResult<std::path::PathBuf> {
    if root_module.trim().is_empty() {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!("artifact '{artifact_label}' declares an empty root"),
        ));
    }

    let canonical_package = package_root.canonicalize().map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "failed to resolve package root '{}': {error}",
                package_root.display()
            ),
        )
    })?;
    let candidate = package_root.join(root_module);
    let canonical_candidate = candidate.canonicalize().map_err(|error| {
        let message = if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "artifact '{artifact_label}' declares root '{root_module}' but no such source file exists in package '{}'",
                package_root.display()
            )
        } else {
            format!(
                "artifact '{artifact_label}' declares root '{root_module}' but it cannot be resolved in package '{}': {error}",
                package_root.display()
            )
        };
        FrontendError::new(FrontendErrorKind::InvalidInput, message)
    })?;
    if !canonical_candidate.is_file() {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "artifact '{artifact_label}' declares root '{root_module}' but it is not a source file in package '{}'",
                package_root.display()
            ),
        ));
    }
    if !canonical_candidate.starts_with(&canonical_package) {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "artifact '{artifact_label}' root '{root_module}' escapes package '{}'",
                package_root.display()
            ),
        )
        .with_note("artifact roots must remain inside their declaring package"));
    }
    Ok(canonical_candidate)
}

fn artifact_source_scope(
    package_root: &Path,
    artifact_label: &str,
    root_module: &str,
) -> FrontendResult<std::path::PathBuf> {
    canonical_artifact_root(package_root, artifact_label, root_module)?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!("artifact '{artifact_label}' root '{root_module}' has no source directory"),
            )
        })
}

fn artifact_scopes_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn isolated_artifact_source_scope(
    package_root: &Path,
    root_module: &str,
    model: fol_backend::BackendFolModel,
    capabilities: &[DeclaredArtifactCapability],
) -> FrontendResult<(std::path::PathBuf, std::path::PathBuf)> {
    let selected_root = canonical_artifact_root(package_root, root_module, root_module)?;
    let selected_scope = selected_root
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!("artifact root '{root_module}' has no source directory"),
            )
        })?;
    let selected_model = public_capability_model(model);

    for capability in capabilities {
        if capability.model == selected_model {
            continue;
        }
        let other_scope = artifact_source_scope(
            package_root,
            &capability.root_module,
            &capability.root_module,
        )?;
        if artifact_scopes_overlap(&selected_scope, &other_scope) {
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!(
                    "artifact root '{root_module}' cannot be compiled as {} because its source directory '{}' overlaps {} artifact root '{}'",
                    selected_model.as_str(),
                    selected_scope.display(),
                    capability.model.as_str(),
                    capability.root_module
                ),
            )
            .with_note(
                "artifacts with different fol_model values must use disjoint source directories",
            ));
        }
    }

    Ok((selected_scope, selected_root))
}

#[cfg(test)]
fn declared_capability_model_for_package(root: &std::path::Path) -> fol_backend::BackendFolModel {
    let models =
        distinct_declared_capability_models(&declared_artifact_capabilities_for_package(root));
    if models == [fol_backend::BackendFolModel::Core] {
        fol_backend::BackendFolModel::Core
    } else {
        fol_backend::BackendFolModel::Memo
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackageWideRuntimeContract {
    capability_model: fol_backend::BackendFolModel,
    fol_model: fol_backend::BackendFolModel,
    has_bundled_std: bool,
}

fn package_wide_runtime_contract_for_package(
    root: &std::path::Path,
    command: &str,
    config: &FrontendConfig,
) -> FrontendResult<PackageWideRuntimeContract> {
    let contract = evaluated_package_capability_contract(root, config)?;
    let models = distinct_declared_capability_models(&contract.artifacts);
    let [model] = models.as_slice() else {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "{command} cannot use one capability model for mixed-artifact package '{}'; declared models: {}",
                root.display(),
                models
                    .iter()
                    .map(|model| model.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with_note(
            "package-wide compilation cannot collapse a core artifact into memo; use an artifact-targeted build step or split the artifacts into package members",
        ));
    };
    Ok(PackageWideRuntimeContract {
        capability_model: *model,
        fol_model: effective_runtime_model_for_package_with_bundled_std(
            root,
            *model,
            contract.has_bundled_std,
        ),
        has_bundled_std: contract.has_bundled_std,
    })
}

pub(crate) fn runtime_model_for_direct_input(
    input: &std::path::Path,
    config: &FrontendConfig,
) -> FrontendResult<fol_backend::BackendFolModel> {
    let search_root = if input.is_dir() {
        input
    } else {
        input.parent().unwrap_or(input)
    };
    let Some(package_root) = search_root
        .ancestors()
        .find(|candidate| candidate.join("build.fol").is_file())
    else {
        // Standalone direct compilation has no build graph from which to
        // derive a capability contract, so retain its explicit hosted mode.
        return Ok(fol_backend::BackendFolModel::Std);
    };

    let contract = evaluated_package_capability_contract(package_root, config)?;
    let capabilities = &contract.artifacts;
    let input_path = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());
    let exact_matching = capabilities
        .iter()
        .filter(|capability| {
            let artifact_path = package_root.join(&capability.root_module);
            let artifact_path = artifact_path.canonicalize().unwrap_or(artifact_path);
            if input.is_dir() {
                artifact_path.starts_with(&input_path)
            } else {
                artifact_path == input_path
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    let scope_matching = if input.is_file() && exact_matching.is_empty() {
        capabilities
            .iter()
            .filter(|capability| {
                package_root
                    .join(&capability.root_module)
                    .canonicalize()
                    .ok()
                    .and_then(|root| root.parent().map(Path::to_path_buf))
                    .map(|scope| input_path.starts_with(scope))
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let relevant = if !exact_matching.is_empty() {
        exact_matching.as_slice()
    } else if !scope_matching.is_empty() {
        scope_matching.as_slice()
    } else {
        capabilities.as_slice()
    };
    let models = distinct_declared_capability_models(relevant);
    let [model] = models.as_slice() else {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "direct input '{}' is ambiguous across mixed artifact capability models: {}",
                input.display(),
                models
                    .iter()
                    .map(|model| model.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with_note("check or emit an exact artifact root, or use its named build step"));
    };
    Ok(effective_runtime_model_for_package_with_bundled_std(
        package_root,
        *model,
        contract.has_bundled_std,
    ))
}

/// Verify each declared artifact's `root` points at a real source file, so
/// `check` surfaces a missing entry instead of reporting a false clean and
/// deferring the failure to `build`.
fn validate_declared_artifact_roots(
    root: &std::path::Path,
    config: &FrontendConfig,
) -> FrontendResult<()> {
    let evaluated = evaluate_package_build(root, config)?;
    let Some(evaluated) = evaluated else {
        return Ok(());
    };

    for artifact in &evaluated.evaluated.artifacts {
        canonical_artifact_root(root, &artifact.name, &artifact.root_module)?;
    }
    Ok(())
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
        .filter(|artifact| {
            !matches!(
                artifact.kind,
                FrontendArtifactKind::BuildRoot | FrontendArtifactKind::InteropEvidence
            )
        })
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
        // A declared entry root that names no file is a build-blocking error
        // that check should surface, not defer.
        validate_declared_artifact_roots(&member.root, config)?;
        // Check under the member's declared capability model so core/memo
        // legality surfaces at check time, not first at build time.
        let contract = package_wide_runtime_contract_for_package(&member.root, "check", config)?;
        compile_member_workspace_for_model(workspace, config, &member.root, contract.fol_model)?;
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
    let optimize = optimize_for_profile(config, profile)?;
    let selections = workspace
        .members
        .iter()
        .map(|member| {
            let contract =
                package_wide_runtime_contract_for_package(&member.root, "build", config)?;
            Ok(FrontendArtifactExecutionSelection {
                package_root: member.root.clone(),
                label: member
                    .root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("package")
                    .to_string(),
                root_module: None,
                artifact_capabilities: Vec::new(),
                capability_model: contract.capability_model,
                fol_model: contract.fol_model,
                has_bundled_std: contract.has_bundled_std,
                target: config.backend_machine_target().map_err(|error| {
                    FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
                })?,
                optimize,
                c_imports: Vec::new(),
                graph_binding: None,
            })
        })
        .collect::<FrontendResult<Vec<_>>>()?;
    build_selected_artifacts_for_profile_with_config(workspace, config, profile, &selections)
}

fn parse_build_optimize(raw: &str) -> FrontendResult<fol_package::BuildOptimizeMode> {
    let optimize = raw.trim();
    fol_package::BuildOptimizeMode::parse(optimize).ok_or_else(|| {
        FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "invalid build optimize mode '{optimize}'; expected debug, release-safe, release-fast, or release-small"
            ),
        )
    })
}

fn optimize_for_profile(
    config: &FrontendConfig,
    profile: FrontendProfile,
) -> FrontendResult<fol_package::BuildOptimizeMode> {
    match config.build_optimize_override.as_deref() {
        Some(optimize) => parse_build_optimize(optimize),
        None => Ok(match profile {
            FrontendProfile::Debug => fol_package::BuildOptimizeMode::Debug,
            FrontendProfile::Release => fol_package::BuildOptimizeMode::ReleaseSafe,
        }),
    }
}

pub(crate) fn build_selected_artifacts_for_profile_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
    selections: &[FrontendArtifactExecutionSelection],
) -> FrontendResult<FrontendCommandResult> {
    let profile = effective_output_profile(profile, selections)?;
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
            &selection.artifact_capabilities,
        )?;
        if lowered.entry_candidates().is_empty() {
            continue;
        }
        let prepared_interop =
            crate::interop::prepare_h7_interop_for_selection(selection, config, &output_root)?;
        let interop_report = prepared_interop.as_ref().map(|prepared| prepared.report);
        let mut selected_backend_config = backend_config(
            config,
            selection.fol_model,
            selection.target.clone(),
            selection.optimize,
        );
        selected_backend_config.auxiliary_rust_plan =
            prepared_interop.map(|prepared| prepared.backend_plan);
        let backend_session = fol_backend::BackendSession::new(lowered);
        let artifact = fol_backend::emit_backend_artifact(
            &backend_session,
            &selected_backend_config,
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
            Some(std::path::PathBuf::from(binary_path.clone())),
        ));
        // Materialize `graph.install(...)` for this artifact: copy the built
        // binary to its projected destination (`<install_prefix>/bin/<name>`).
        // Before this, the install step was projection-only — the summary
        // advertised an install prefix nothing was ever copied into.
        if let Some(binding) = &selection.graph_binding {
            for install in binding.graph.installs() {
                let Some(fol_package::BuildInstallTarget::Artifact(artifact_id)) = &install.target
                else {
                    continue;
                };
                if *artifact_id != binding.artifact_id {
                    continue;
                }
                let destination = std::path::PathBuf::from(&install.projected_destination);
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        FrontendError::new(
                            FrontendErrorKind::CommandFailed,
                            format!(
                                "install step '{}' could not create '{}': {error}",
                                install.name,
                                parent.display()
                            ),
                        )
                    })?;
                }
                std::fs::copy(&binary_path, &destination).map_err(|error| {
                    FrontendError::new(
                        FrontendErrorKind::CommandFailed,
                        format!(
                            "install step '{}' could not copy the binary to '{}': {error}",
                            install.name,
                            destination.display()
                        ),
                    )
                })?;
                result.artifacts.push(FrontendArtifactSummary::new(
                    FrontendArtifactKind::Installed,
                    install.name.clone(),
                    Some(destination),
                ));
            }
        }
        if let Some(report) = interop_report {
            result.artifacts.push(FrontendArtifactSummary::new(
                FrontendArtifactKind::InteropEvidence,
                report.summary(),
                None,
            ));
        }
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
    let runtime_contracts = workspace
        .members
        .iter()
        .map(|member| package_wide_runtime_contract_for_package(&member.root, "run", config))
        .collect::<FrontendResult<Vec<_>>>()?;
    let built = build_workspace_with_config(workspace, config)?;
    let binaries = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    let interop_evidence = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::InteropEvidence)
        .cloned()
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
            "ran {} ({}, bundled_std={}/{})",
            binary.display(),
            summarize_capability_modes(
                runtime_contracts
                    .iter()
                    .map(|contract| contract.capability_model)
            ),
            runtime_contracts
                .iter()
                .filter(|contract| contract.has_bundled_std)
                .count(),
            runtime_contracts.len()
        ),
    );
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::Binary,
        "binary",
        Some(binary),
    ));
    result.artifacts.extend(interop_evidence);
    Ok(result)
}

pub(crate) fn run_selected_artifact_with_args_and_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    profile: FrontendProfile,
    selection: &FrontendArtifactExecutionSelection,
    args: &[String],
) -> FrontendResult<FrontendCommandResult> {
    ensure_resolved_target_runs_on_host(&selection.target, "run")?;
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
    let interop_evidence = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::InteropEvidence)
        .cloned()
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
            if selection.has_bundled_std {
                "1/1"
            } else {
                "0/1"
            }
        ),
    );
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::Binary,
        selection.label.clone(),
        Some(binary),
    ));
    result.artifacts.extend(interop_evidence);
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
    for selection in selections {
        ensure_resolved_target_runs_on_host(&selection.target, "test")?;
    }
    let built =
        build_selected_artifacts_for_profile_with_config(workspace, config, profile, selections)?;
    let binaries = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .collect::<Vec<_>>();
    let interop_evidence = built
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == FrontendArtifactKind::InteropEvidence)
        .cloned()
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
            summarize_capability_modes(
                selections
                    .iter()
                    .map(|selection| selection.capability_model)
            ),
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
    result.artifacts.extend(interop_evidence);
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
        let contract =
            package_wide_runtime_contract_for_package(&member.root, "emit rust", config)?;
        let fol_model = contract.fol_model;
        let lowered =
            compile_member_workspace_for_model(workspace, config, &member.root, fol_model)?;
        let backend_session = fol_backend::BackendSession::new(lowered);
        let artifact = fol_backend::emit_backend_artifact(
            &backend_session,
            &fol_backend::BackendConfig {
                mode: fol_backend::BackendMode::EmitSource,
                keep_build_dir: true,
                ..backend_config(
                    config,
                    fol_model,
                    config.backend_machine_target().map_err(|error| {
                        FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
                    })?,
                    fol_package::BuildOptimizeMode::ReleaseSafe,
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
        let contract =
            package_wide_runtime_contract_for_package(&member.root, "emit lowered", config)?;
        let fol_model = contract.fol_model;
        let lowered =
            compile_member_workspace_for_model(workspace, config, &member.root, fol_model)?;
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
    let contract = package_wide_runtime_contract_for_package(package_root, "compile", config)?;
    compile_member_workspace_for_model(workspace, config, package_root, contract.fol_model)
}

fn compile_member_workspace_for_model(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
    fol_model: fol_backend::BackendFolModel,
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    compile_member_source_scope_for_model(workspace, config, package_root, package_root, fol_model)
}

fn compile_member_source_scope_for_model(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    package_root: &Path,
    source_scope: &Path,
    fol_model: fol_backend::BackendFolModel,
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    validate_build_dependency_queries(workspace, config, package_root)?;
    let display_name = package_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package");
    let syntax = fol_package::parse_directory_package_syntax(
        source_scope,
        display_name,
        fol_package::PackageSourceKind::Package,
    )
    .map_err(FrontendError::from)?;
    let canonical_package_root = package_root.canonicalize().map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "failed to resolve package root '{}': {error}",
                package_root.display()
            ),
        )
    })?;
    // Restricting the parsed source set to an artifact directory must not
    // turn that directory into a different formal package. Package identity
    // remains anchored at the declaring package root while syntax contains
    // only the selected artifact's recursively loaded source units.
    let prepared = fol_package::PreparedPackage::new(
        fol_package::PackageIdentity {
            source_kind: fol_package::PackageSourceKind::Entry,
            canonical_root: canonical_package_root.display().to_string(),
            display_name: display_name.to_string(),
        },
        syntax,
    );

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
    let Some(evaluated) = evaluate_package_build(package_root, config)? else {
        return Ok(());
    };
    if evaluated.evaluated.dependency_queries.is_empty() {
        return Ok(());
    }

    let metadata =
        fol_package::parse_package_metadata_from_build(&build_path).map_err(FrontendError::from)?;
    let local_store = workspace.root.root.join(".fol").join("pkg");
    let package_store_root = config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone())
        .or_else(|| local_store.is_dir().then_some(local_store.clone()))
        .or_else(fol_package::available_bundled_store_root)
        .unwrap_or(local_store);
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
            fol_package::PackageDependencySourceKind::Internal => std_root
                .map(Path::to_path_buf)
                .unwrap_or_else(|| package_store_root.join(&dependency.alias)),
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
    artifact_capabilities: &[DeclaredArtifactCapability],
) -> FrontendResult<fol_lower::LoweredWorkspace> {
    let Some(root_module) = root_module else {
        return compile_member_workspace_for_model(workspace, config, package_root, fol_model);
    };
    let (source_scope, canonical_root) = isolated_artifact_source_scope(
        package_root,
        root_module,
        fol_model,
        artifact_capabilities,
    )?;
    let lowered = compile_member_source_scope_for_model(
        workspace,
        config,
        package_root,
        &source_scope,
        fol_model,
    )?;

    let matching_candidates = lowered
        .entry_candidates()
        .iter()
        .filter(|candidate| {
            entry_candidate_matches_root_module(&lowered, candidate, &canonical_root)
        })
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
    canonical_root: &Path,
) -> bool {
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
    Path::new(&source_unit.path)
        .canonicalize()
        .map(|source| source == canonical_root)
        .unwrap_or(false)
}

fn resolver_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> fol_resolver::ResolverConfig {
    // A fetched local store wins; otherwise fall back to the store bundled
    // with the toolchain so `use std: pkg = {"std"}` works out of the box.
    let local_store = workspace.root.root.join(".fol/pkg");
    let package_store_root = config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone())
        .or_else(|| local_store.is_dir().then_some(local_store.clone()))
        .or_else(fol_package::available_bundled_store_root)
        .unwrap_or(local_store);

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

pub(crate) fn typecheck_capability_model(
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
    fol_model: fol_backend::BackendFolModel,
    target: fol_types::ResolvedTarget,
    optimize: fol_package::BuildOptimizeMode,
) -> fol_backend::BackendConfig {
    fol_backend::BackendConfig {
        fol_model,
        machine_target: target,
        build_profile: backend_build_profile_for_optimize(optimize),
        keep_build_dir: config.keep_build_dir,
        ..fol_backend::BackendConfig::default()
    }
}

pub(crate) fn backend_build_profile_for_optimize(
    optimize: fol_package::BuildOptimizeMode,
) -> fol_backend::BackendBuildProfile {
    match optimize {
        fol_package::BuildOptimizeMode::Debug => fol_backend::BackendBuildProfile::Debug,
        fol_package::BuildOptimizeMode::ReleaseSafe
        | fol_package::BuildOptimizeMode::ReleaseFast
        | fol_package::BuildOptimizeMode::ReleaseSmall => fol_backend::BackendBuildProfile::Release,
    }
}

fn effective_output_profile(
    requested: FrontendProfile,
    selections: &[FrontendArtifactExecutionSelection],
) -> FrontendResult<FrontendProfile> {
    let Some(first) = selections.first() else {
        return Ok(requested);
    };
    // Evaluated artifact optimization is authoritative after graph planning,
    // so an explicit optimize override cannot leave rustc output under the
    // opposite frontend profile directory.
    let effective = first.output_profile();
    if selections
        .iter()
        .any(|selection| selection.output_profile() != effective)
    {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            "selected artifacts mix debug and release build profiles and cannot share one output identity",
        ));
    }
    Ok(effective)
}

fn ensure_host_runnable_target(config: &FrontendConfig, command: &str) -> FrontendResult<()> {
    let target = config
        .backend_machine_target()
        .map_err(|error| FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string()))?;
    ensure_resolved_target_runs_on_host(&target, command)
}

fn ensure_resolved_target_runs_on_host(
    target: &fol_types::ResolvedTarget,
    command: &str,
) -> FrontendResult<()> {
    if target
        .runs_on_host()
        .map_err(|error| FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string()))?
    {
        return Ok(());
    }
    let selected = target.as_str();
    let host = FrontendConfig::host_rust_target_triple().unwrap_or("unknown-host");
    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!("{command} command cannot execute target '{selected}' on host '{host}'"),
    ))
}

fn test_workspace_selected_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    selected_package: Option<&str>,
) -> FrontendResult<FrontendCommandResult> {
    ensure_host_runnable_target(config, "test")?;
    let selected_members = selected_workspace_members(workspace, selected_package)?;
    let runtime_contracts = selected_members
        .iter()
        .map(|member| package_wide_runtime_contract_for_package(&member.root, "test", config))
        .collect::<FrontendResult<Vec<_>>>()?;
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
        "tested {tested_count} workspace package(s) ({}, bundled_std={}/{})",
        summarize_capability_modes(
            runtime_contracts
                .iter()
                .map(|contract| contract.capability_model)
        ),
        runtime_contracts
            .iter()
            .filter(|contract| contract.has_bundled_std)
            .count(),
        runtime_contracts.len()
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
