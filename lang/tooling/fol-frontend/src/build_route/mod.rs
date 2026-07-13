use crate::{FrontendError, FrontendErrorKind, FrontendProfile, FrontendResult, FrontendWorkspace};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendBuildWorkflowMode {
    Modern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendBuildStep {
    Build,
    Run,
    Test,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendMemberBuildRoute {
    pub member_root: PathBuf,
    pub package_name: String,
    pub mode: FrontendBuildWorkflowMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendWorkspaceBuildRoute {
    pub requested_step: String,
    pub members: Vec<FrontendMemberBuildRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendWorkspaceBuildRequest {
    pub requested_step: String,
    pub profile: FrontendProfile,
    pub run_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrontendMemberExecutionPlan {
    steps: Vec<FrontendMemberPlannedStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontendMemberPlannedStep {
    name: String,
    description: Option<String>,
    default_kind: Option<fol_package::BuildDefaultStepKind>,
    execution: Option<FrontendStepExecutionKind>,
    selection: Option<crate::compile::FrontendArtifactExecutionSelection>,
    ambiguous_selection: bool,
    available_models: Vec<fol_backend::BackendFolModel>,
    has_bundled_std: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontendStepExecutionKind {
    Build,
    Run,
    Test,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedWorkspaceStepExecution {
    execution: FrontendStepExecutionKind,
    selections: Vec<crate::compile::FrontendArtifactExecutionSelection>,
    available_models: Vec<fol_backend::BackendFolModel>,
    all_members_have_bundled_std: bool,
}

impl FrontendBuildWorkflowMode {
    pub fn from_package_build_mode(mode: fol_package::PackageBuildMode) -> Option<Self> {
        match mode {
            fol_package::PackageBuildMode::ModernOnly => Some(FrontendBuildWorkflowMode::Modern),
            _ => None,
        }
    }
}

impl FrontendBuildStep {
    pub fn as_str(self) -> &'static str {
        match self {
            FrontendBuildStep::Build => "build",
            FrontendBuildStep::Run => "run",
            FrontendBuildStep::Test => "test",
            FrontendBuildStep::Check => "check",
        }
    }

    pub fn default_for_code_subcommand(command: &crate::CodeSubcommand) -> Self {
        match command {
            crate::CodeSubcommand::Build(_) => FrontendBuildStep::Build,
            crate::CodeSubcommand::Run(_) => FrontendBuildStep::Run,
            crate::CodeSubcommand::Test(_) => FrontendBuildStep::Test,
            crate::CodeSubcommand::Check(_) => FrontendBuildStep::Check,
            crate::CodeSubcommand::Emit(_) => FrontendBuildStep::Build,
            // `code explain` is dispatched directly and never enters workspace
            // build-step routing, so it has no build step of its own.
            crate::CodeSubcommand::Explain(_) => unreachable!(
                "code explain is dispatched directly and never routes through build steps"
            ),
        }
    }
}

pub fn requested_workspace_step(
    command: &crate::CodeSubcommand,
    override_step: Option<&str>,
) -> String {
    override_step.map(str::to_string).unwrap_or_else(|| {
        FrontendBuildStep::default_for_code_subcommand(command)
            .as_str()
            .to_string()
    })
}

pub fn plan_workspace_build_route(
    workspace: &FrontendWorkspace,
    requested_step: impl Into<String>,
) -> FrontendResult<FrontendWorkspaceBuildRoute> {
    let members = workspace
        .members
        .iter()
        .map(|member| {
            let build_path = member.root.join("build.fol");
            let metadata = fol_package::parse_package_metadata_from_build(&build_path)?;
            let mode = load_member_build_mode(&build_path)?;
            Ok(FrontendMemberBuildRoute {
                member_root: member.root.clone(),
                package_name: metadata.name,
                mode,
            })
        })
        .collect::<FrontendResult<Vec<_>>>()?;

    Ok(FrontendWorkspaceBuildRoute {
        requested_step: requested_step.into(),
        members,
    })
}

fn load_member_build_mode(
    build_path: &std::path::Path,
) -> FrontendResult<FrontendBuildWorkflowMode> {
    let mode = fol_package::parse_package_build_mode(build_path)?;
    FrontendBuildWorkflowMode::from_package_build_mode(mode).ok_or_else(|| {
        FrontendError::new(
            FrontendErrorKind::Internal,
            format!(
                "workspace member build '{}' has an unmappable build mode",
                build_path.display()
            ),
        )
    })
}

pub fn execute_workspace_build_route(
    workspace: &FrontendWorkspace,
    config: &crate::FrontendConfig,
    request: &FrontendWorkspaceBuildRequest,
) -> FrontendResult<crate::FrontendCommandResult> {
    let route = plan_workspace_build_route(workspace, request.requested_step.clone())?;
    let member_plans = route
        .members
        .iter()
        .map(|member| plan_member_execution(workspace, member, config))
        .collect::<FrontendResult<Vec<_>>>()?;
    let requested_step = request.requested_step.as_str();
    if !member_plans
        .iter()
        .any(|plan| plan.steps.iter().any(|step| step.name == requested_step))
    {
        return Err(unknown_workspace_build_step_error(
            requested_step,
            &route,
            &member_plans,
        ));
    }

    let resolved = resolve_requested_step_execution(requested_step, &member_plans)?;

    match resolved.execution {
        FrontendStepExecutionKind::Build => {
            if resolved.selections.is_empty() {
                crate::build_workspace_for_profile_with_config(workspace, config, request.profile)
            } else {
                crate::compile::build_selected_artifacts_for_profile_with_config(
                    workspace,
                    config,
                    request.profile,
                    &resolved.selections,
                )
            }
        }
        FrontendStepExecutionKind::Check => crate::check_workspace_with_config(workspace, config),
        FrontendStepExecutionKind::Run => match resolved.selections.as_slice() {
            [] => {
                ensure_std_workspace_route_models(
                    "run",
                    &resolved.available_models,
                    resolved.all_members_have_bundled_std,
                )?;
                crate::run_workspace_with_args_and_config(workspace, config, &request.run_args)
            }
            [selection] => {
                ensure_std_workspace_step_selection("run", requested_step, selection)?;
                crate::compile::run_selected_artifact_with_args_and_config(
                    workspace,
                    config,
                    request.profile,
                    selection,
                    &request.run_args,
                )
            }
            selections => Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!(
                    "workspace build execution step '{}' resolved to {} runnable artifacts",
                    requested_step,
                    selections.len()
                ),
            )),
        },
        FrontendStepExecutionKind::Test => {
            if resolved.selections.is_empty() {
                ensure_std_workspace_route_models(
                    "test",
                    &resolved.available_models,
                    resolved.all_members_have_bundled_std,
                )?;
                crate::test_workspace_with_config(workspace, config)
            } else {
                ensure_std_workspace_step_selections("test", requested_step, &resolved.selections)?;
                crate::compile::test_selected_artifacts_with_config(
                    workspace,
                    config,
                    request.profile,
                    &resolved.selections,
                )
            }
        }
    }
}

pub(crate) fn plan_member_execution(
    workspace: &FrontendWorkspace,
    member: &FrontendMemberBuildRoute,
    config: &crate::FrontendConfig,
) -> FrontendResult<FrontendMemberExecutionPlan> {
    plan_member_execution_from_build_source(workspace, member, config)
}

fn plan_member_execution_from_build_source(
    workspace: &FrontendWorkspace,
    member: &FrontendMemberBuildRoute,
    config: &crate::FrontendConfig,
) -> FrontendResult<FrontendMemberExecutionPlan> {
    let build_path = member.member_root.join("build.fol");
    let source = std::fs::read_to_string(&build_path).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "failed to read build file '{}': {error}",
                build_path.display()
            ),
        )
    })?;
    let inputs = crate::compile::build_evaluation_inputs(&member.member_root, config)?;
    let evaluated = fol_package::evaluate_build_source(
        &fol_package::BuildEvaluationRequest {
            package_root: member.member_root.display().to_string(),
            inputs,
            operations: Vec::new(),
        },
        &build_path,
        &source,
    )
    .map_err(|error| FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string()))?;
    let Some(evaluated) = evaluated else {
        return Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            format!(
                "workspace member '{}' must declare a semantic `pro[] build(): non` entry",
                member.package_name
            ),
        ));
    };
    validate_dependency_queries_for_member(workspace, config, member, &evaluated)?;

    plan_member_execution_from_graph(member, &evaluated.result.graph, &evaluated.evaluated, true)
}

fn validate_dependency_queries_for_member(
    workspace: &FrontendWorkspace,
    config: &crate::FrontendConfig,
    member: &FrontendMemberBuildRoute,
    evaluated: &fol_package::build_eval::EvaluatedBuildSource,
) -> FrontendResult<()> {
    if evaluated.evaluated.dependency_queries.is_empty() {
        return Ok(());
    }

    let metadata =
        fol_package::parse_package_metadata_from_build(&member.member_root.join("build.fol"))
            .map_err(FrontendError::from)?;
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
        let dependency_root = dependency_root_for_query(
            &member.member_root,
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

fn dependency_root_for_query(
    member_root: &Path,
    package_store_root: &Path,
    std_root: Option<&Path>,
    metadata_dependency: Option<&fol_package::PackageDependencyDecl>,
    evaluated_dependency: Option<&fol_package::DependencyRequest>,
) -> FrontendResult<PathBuf> {
    if let Some(dependency) = metadata_dependency {
        return Ok(match dependency.source_kind {
            fol_package::PackageDependencySourceKind::Local => member_root.join(&dependency.target),
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
        let local_root = member_root.join(&dependency.package);
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

pub(crate) fn plan_member_execution_from_graph(
    member: &FrontendMemberBuildRoute,
    graph: &fol_package::BuildGraph,
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    synthesize_defaults: bool,
) -> FrontendResult<FrontendMemberExecutionPlan> {
    let has_bundled_std = crate::compile::evaluated_program_declares_bundled_std(evaluated);
    let mut steps = fol_package::project_graph_steps(graph)
        .into_iter()
        .map(|step| {
            let selection = selection_for_step(member, evaluated, &step.name, step.default_kind);
            // A step whose artifact binding already resolved (explicit
            // graph binding or exact artifact name) is never ambiguous;
            // the multi-artifact heuristic only applies to unresolved
            // default selections.
            let ambiguous_selection = selection.is_none()
                && step_has_ambiguous_default_selection(evaluated, step.default_kind);
            FrontendMemberPlannedStep {
                selection,
                ambiguous_selection,
                available_models: models_for_step(evaluated, &step.name, step.default_kind),
                has_bundled_std,
                description: step.description,
                default_kind: step.default_kind,
                name: step.name,
                execution: step.default_kind.and_then(step_execution_kind_from_default),
            }
        })
        .collect::<Vec<_>>();
    if synthesize_defaults {
        for step in synthesized_default_steps(member, evaluated, has_bundled_std) {
            if !steps.iter().any(|existing| existing.name == step.name) {
                steps.push(step);
            }
        }
    }
    if !steps.iter().any(|step| step.name == "check") {
        steps.push(FrontendMemberPlannedStep {
            name: "check".to_string(),
            description: Some("Typecheck the workspace graph".to_string()),
            default_kind: Some(fol_package::BuildDefaultStepKind::Check),
            execution: Some(FrontendStepExecutionKind::Check),
            selection: None,
            ambiguous_selection: false,
            available_models: Vec::new(),
            has_bundled_std,
        });
    }
    if steps.is_empty() {
        return Err(FrontendError::new(
            FrontendErrorKind::Internal,
            format!(
                "workspace member '{}' produced an empty graph-driven step plan",
                member.package_name
            ),
        ));
    }
    steps.sort_by(|left, right| left.name.cmp(&right.name));
    steps.dedup_by(|left, right| {
        left.name == right.name
            && left.execution == right.execution
            && left.selection == right.selection
    });
    Ok(FrontendMemberExecutionPlan { steps })
}

fn selection_for_step(
    member: &FrontendMemberBuildRoute,
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    step_name: &str,
    default_kind: Option<fol_package::BuildDefaultStepKind>,
) -> Option<crate::compile::FrontendArtifactExecutionSelection> {
    if let Some(binding) = evaluated
        .step_bindings
        .iter()
        .find(|binding| binding.step_name == step_name)
    {
        if let Some(artifact_name) = &binding.artifact_name {
            return evaluated
                .artifacts
                .iter()
                .find(|artifact| artifact.name == *artifact_name)
                .map(|artifact| artifact_selection(member, evaluated, artifact));
        }
    }
    if let Some(artifact) = evaluated
        .artifacts
        .iter()
        .find(|artifact| artifact.name == step_name)
    {
        return Some(artifact_selection(member, evaluated, artifact));
    }
    match default_kind {
        Some(fol_package::BuildDefaultStepKind::Build)
        | Some(fol_package::BuildDefaultStepKind::Run) => single_selection(
            member,
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
        ),
        Some(fol_package::BuildDefaultStepKind::Test) => single_selection(
            member,
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
        ),
        _ => None,
    }
}

fn synthesized_default_steps(
    member: &FrontendMemberBuildRoute,
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    has_bundled_std: bool,
) -> Vec<FrontendMemberPlannedStep> {
    let mut steps = Vec::new();
    let executable_count = artifact_count(
        evaluated,
        fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
    );
    if executable_count > 0 {
        let selection = single_selection(
            member,
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
        );
        steps.push(FrontendMemberPlannedStep {
            name: "build".to_string(),
            description: Some("Build default executable artifacts".to_string()),
            default_kind: Some(fol_package::BuildDefaultStepKind::Build),
            execution: Some(FrontendStepExecutionKind::Build),
            selection: selection.clone(),
            ambiguous_selection: executable_count > 1,
            available_models: artifact_models(
                evaluated,
                fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
            ),
            has_bundled_std,
        });
        steps.push(FrontendMemberPlannedStep {
            name: "run".to_string(),
            description: Some("Run the default executable artifact".to_string()),
            default_kind: Some(fol_package::BuildDefaultStepKind::Run),
            execution: Some(FrontendStepExecutionKind::Run),
            selection,
            ambiguous_selection: executable_count > 1,
            available_models: artifact_models(
                evaluated,
                fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
            ),
            has_bundled_std,
        });
    }
    let test_count = artifact_count(
        evaluated,
        fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
    );
    if test_count > 0 {
        let selection = single_selection(
            member,
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
        );
        steps.push(FrontendMemberPlannedStep {
            name: "test".to_string(),
            description: Some("Run the default test artifact".to_string()),
            default_kind: Some(fol_package::BuildDefaultStepKind::Test),
            execution: Some(FrontendStepExecutionKind::Test),
            selection,
            ambiguous_selection: test_count > 1,
            available_models: artifact_models(
                evaluated,
                fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
            ),
            has_bundled_std,
        });
    }
    steps
}

fn step_has_ambiguous_default_selection(
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    default_kind: Option<fol_package::BuildDefaultStepKind>,
) -> bool {
    match default_kind {
        Some(fol_package::BuildDefaultStepKind::Build)
        | Some(fol_package::BuildDefaultStepKind::Run) => {
            artifact_count(
                evaluated,
                fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
            ) > 1
        }
        Some(fol_package::BuildDefaultStepKind::Test) => {
            artifact_count(
                evaluated,
                fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
            ) > 1
        }
        _ => false,
    }
}

fn artifact_count(
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    kind: fol_package::build_runtime::BuildRuntimeArtifactKind,
) -> usize {
    evaluated
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == kind)
        .count()
}

fn artifact_models(
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    kind: fol_package::build_runtime::BuildRuntimeArtifactKind,
) -> Vec<fol_backend::BackendFolModel> {
    let mut models = evaluated
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == kind)
        .map(|artifact| backend_fol_model(artifact.fol_model))
        .collect::<Vec<_>>();
    models.sort_by_key(|model| match model {
        fol_backend::BackendFolModel::Core => 0,
        fol_backend::BackendFolModel::Memo => 1,
        fol_backend::BackendFolModel::Std => 1,
    });
    models.dedup();
    models
}

fn models_for_step(
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    step_name: &str,
    default_kind: Option<fol_package::BuildDefaultStepKind>,
) -> Vec<fol_backend::BackendFolModel> {
    if let Some(binding) = evaluated
        .step_bindings
        .iter()
        .find(|binding| binding.step_name == step_name)
    {
        if let Some(artifact_name) = &binding.artifact_name {
            return evaluated
                .artifacts
                .iter()
                .find(|artifact| artifact.name == *artifact_name)
                .map(|artifact| vec![backend_fol_model(artifact.fol_model)])
                .unwrap_or_default();
        }
    }
    if let Some(artifact) = evaluated
        .artifacts
        .iter()
        .find(|artifact| artifact.name == step_name)
    {
        return vec![backend_fol_model(artifact.fol_model)];
    }
    match default_kind {
        Some(fol_package::BuildDefaultStepKind::Build)
        | Some(fol_package::BuildDefaultStepKind::Run) => artifact_models(
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Executable,
        ),
        Some(fol_package::BuildDefaultStepKind::Test) => artifact_models(
            evaluated,
            fol_package::build_runtime::BuildRuntimeArtifactKind::Test,
        ),
        _ => Vec::new(),
    }
}

fn single_selection(
    member: &FrontendMemberBuildRoute,
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    kind: fol_package::build_runtime::BuildRuntimeArtifactKind,
) -> Option<crate::compile::FrontendArtifactExecutionSelection> {
    let artifacts = evaluated
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == kind)
        .collect::<Vec<_>>();
    (artifacts.len() == 1).then(|| artifact_selection(member, evaluated, artifacts[0]))
}

fn artifact_selection(
    member: &FrontendMemberBuildRoute,
    evaluated: &fol_package::build_eval::EvaluatedBuildProgram,
    artifact: &fol_package::build_runtime::BuildRuntimeArtifact,
) -> crate::compile::FrontendArtifactExecutionSelection {
    let has_bundled_std = crate::compile::evaluated_program_declares_bundled_std(evaluated);
    let capability_model = match backend_fol_model(artifact.fol_model) {
        fol_backend::BackendFolModel::Std => fol_backend::BackendFolModel::Memo,
        other => other,
    };
    crate::compile::FrontendArtifactExecutionSelection {
        package_root: member.member_root.clone(),
        label: artifact.name.clone(),
        root_module: Some(artifact.root_module.clone()),
        artifact_capabilities: evaluated
            .artifacts
            .iter()
            .map(|candidate| crate::compile::DeclaredArtifactCapability {
                root_module: candidate.root_module.clone(),
                model: match backend_fol_model(candidate.fol_model) {
                    fol_backend::BackendFolModel::Std => fol_backend::BackendFolModel::Memo,
                    other => other,
                },
            })
            .collect(),
        capability_model,
        fol_model: crate::compile::effective_runtime_model_for_package_with_bundled_std(
            &member.member_root,
            capability_model,
            has_bundled_std,
        ),
        has_bundled_std,
    }
}

fn backend_fol_model(
    model: fol_package::build_artifact::BuildArtifactFolModel,
) -> fol_backend::BackendFolModel {
    match model {
        fol_package::build_artifact::BuildArtifactFolModel::Core => {
            fol_backend::BackendFolModel::Core
        }
        fol_package::build_artifact::BuildArtifactFolModel::Memo => {
            fol_backend::BackendFolModel::Memo
        }
    }
}

fn step_execution_kind_from_default(
    kind: fol_package::BuildDefaultStepKind,
) -> Option<FrontendStepExecutionKind> {
    match kind {
        fol_package::BuildDefaultStepKind::Build => Some(FrontendStepExecutionKind::Build),
        fol_package::BuildDefaultStepKind::Run => Some(FrontendStepExecutionKind::Run),
        fol_package::BuildDefaultStepKind::Test => Some(FrontendStepExecutionKind::Test),
        fol_package::BuildDefaultStepKind::Check => Some(FrontendStepExecutionKind::Check),
        fol_package::BuildDefaultStepKind::Install => None,
    }
}

fn resolve_requested_step_execution(
    requested_step: &str,
    member_plans: &[FrontendMemberExecutionPlan],
) -> FrontendResult<ResolvedWorkspaceStepExecution> {
    let mut resolved = None;
    let mut selections = Vec::new();
    let mut available_models = Vec::new();
    let mut all_members_have_bundled_std = true;
    let mut saw_untargeted = false;
    for step in member_plans
        .iter()
        .flat_map(|plan| plan.steps.iter())
        .filter(|step| step.name == requested_step)
    {
        let Some(execution_kind) = step.execution else {
            continue;
        };
        if step.ambiguous_selection {
            let resolved = if step.available_models.is_empty() {
                "unknown".to_string()
            } else {
                step.available_models
                    .iter()
                    .map(|model| model.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!(
                    "workspace build execution step '{}' matches multiple artifacts and requires an explicit named step; resolved model(s): {}",
                    requested_step, resolved
                ),
            ));
        }
        match &step.selection {
            Some(selection) => {
                if saw_untargeted {
                    return Err(FrontendError::new(
                        FrontendErrorKind::InvalidInput,
                        format!(
                            "workspace build execution step '{}' mixes targeted and untargeted members",
                            requested_step
                        ),
                    ));
                }
                if !selections.contains(selection) {
                    selections.push(selection.clone());
                }
            }
            None => {
                if !selections.is_empty() {
                    return Err(FrontendError::new(
                        FrontendErrorKind::InvalidInput,
                        format!(
                            "workspace build execution step '{}' mixes targeted and untargeted members",
                            requested_step
                        ),
                    ));
                }
                saw_untargeted = true;
                all_members_have_bundled_std &= step.has_bundled_std;
                for model in &step.available_models {
                    if !available_models.contains(model) {
                        available_models.push(*model);
                    }
                }
            }
        }
        match resolved {
            None => resolved = Some(execution_kind),
            Some(current) if current == execution_kind => {}
            Some(current) => {
                return Err(FrontendError::new(
                    FrontendErrorKind::InvalidInput,
                    format!(
                        "workspace build execution step '{}' resolves to incompatible execution kinds: {:?} and {:?}",
                        requested_step, current, execution_kind
                    ),
                ))
            }
        }
    }

    resolved
        .map(|execution| ResolvedWorkspaceStepExecution {
            execution,
            selections,
            available_models,
            all_members_have_bundled_std,
        })
        .ok_or_else(|| {
            FrontendError::new(
                FrontendErrorKind::InvalidInput,
                format!("workspace build execution does not support step '{requested_step}'"),
            )
        })
}

fn ensure_std_workspace_route_models(
    command: &str,
    models: &[fol_backend::BackendFolModel],
    all_members_have_bundled_std: bool,
) -> FrontendResult<()> {
    if all_members_have_bundled_std {
        return Ok(());
    }

    let resolved_models = if models.is_empty() {
        "unknown".to_string()
    } else {
        models
            .iter()
            .map(|model| model.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!(
            "{command} command requires the bundled internal 'standard' dependency for hosted execution; resolved capability model(s): {resolved_models}"
        ),
    )
    .with_note(
        "declare build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" }) in build.fol",
    ))
}

fn ensure_std_workspace_step_selection(
    command: &str,
    step_name: &str,
    selection: &crate::compile::FrontendArtifactExecutionSelection,
) -> FrontendResult<()> {
    if selection.has_bundled_std {
        return Ok(());
    }

    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!(
            "workspace build step '{}' resolves artifact '{}' with capability model '{}', but {command} requires the bundled internal 'standard' dependency for hosted execution",
            step_name,
            selection.label,
            selection.capability_model.as_str(),
        ),
    )
    .with_note(
        "declare build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" }) in build.fol",
    ))
}

fn ensure_std_workspace_step_selections(
    command: &str,
    step_name: &str,
    selections: &[crate::compile::FrontendArtifactExecutionSelection],
) -> FrontendResult<()> {
    for selection in selections {
        ensure_std_workspace_step_selection(command, step_name, selection)?;
    }
    Ok(())
}

fn unknown_workspace_build_step_error(
    requested_step: &str,
    route: &FrontendWorkspaceBuildRoute,
    member_plans: &[FrontendMemberExecutionPlan],
) -> FrontendError {
    let members = route
        .members
        .iter()
        .map(|member| member.package_name.as_str())
        .collect::<Vec<_>>();
    let known_steps = render_known_workspace_steps(member_plans);
    FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!(
            "workspace build execution does not define step '{requested_step}' for workspace members: {}. known steps: {}",
            members.join(", "),
            known_steps
        ),
    )
}

fn render_known_workspace_steps(member_plans: &[FrontendMemberExecutionPlan]) -> String {
    let mut rendered = member_plans
        .iter()
        .flat_map(|plan| plan.steps.iter())
        .map(render_step_catalog_entry)
        .collect::<Vec<_>>();
    rendered.sort();
    rendered.dedup();
    rendered.join(", ")
}

fn render_step_catalog_entry(step: &FrontendMemberPlannedStep) -> String {
    let mut rendered = step.name.clone();
    if let Some(default_kind) = step.default_kind {
        rendered.push_str(&format!(" [default:{}]", default_kind.as_str()));
    }
    if let Some(selection) = step.selection.as_ref() {
        rendered.push_str(&format!(" [artifact:{}]", selection.label));
    }
    if !step.available_models.is_empty() {
        let models = step
            .available_models
            .iter()
            .map(|model| model.as_str())
            .collect::<Vec<_>>()
            .join(",");
        rendered.push_str(&format!(" [models:{models}]"));
    }
    if let Some(description) = step.description.as_deref() {
        rendered.push_str(&format!(" - {description}"));
    }
    rendered
}
