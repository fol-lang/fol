use super::error::{BuildEvaluationError, BuildEvaluationErrorKind};
use super::plan::evaluate_build_plan;
use super::types::{
    BuildEvaluationRequest, BuildEvaluationResult, EvaluatedBuildProgram, EvaluatedBuildSource,
};
use crate::executor::{BuildBodyExecutor, ExecutionOutput};
use crate::runtime::{
    BuildExecutionRepresentation, BuildRuntimeArtifact, BuildRuntimeArtifactKind,
    BuildRuntimeDependency, BuildRuntimeProgram, BuildRuntimeStepBinding,
    BuildRuntimeStepBindingKind,
};
use std::path::Path;

pub fn evaluate_build_source(
    request: &BuildEvaluationRequest,
    build_path: &Path,
    _source: &str,
) -> Result<Option<EvaluatedBuildSource>, BuildEvaluationError> {
    let Some((executor, body)) = BuildBodyExecutor::from_file(build_path)? else {
        return Ok(None);
    };
    let mut resolved_inputs = std::collections::BTreeMap::new();
    if let Some(target) = &request.inputs.target {
        resolved_inputs.insert("target".to_string(), target.render());
    }
    if let Some(optimize) = request.inputs.optimize {
        resolved_inputs.insert("optimize".to_string(), optimize.as_str().to_string());
    }
    let executor = executor.with_resolved_inputs(resolved_inputs);
    let executor = executor.with_install_prefix(request.inputs.install_prefix.clone());
    let exec_output = executor.execute(&body)?;
    if exec_output.operations.is_empty() {
        return Ok(None);
    }
    let result = evaluate_build_plan(&BuildEvaluationRequest {
        package_root: request.package_root.clone(),
        inputs: request.inputs.clone(),
        operations: exec_output.operations.clone(),
    })?;
    let evaluated = build_evaluated_program(&exec_output, &result)?;
    Ok(Some(EvaluatedBuildSource { evaluated, result }))
}

pub(super) fn build_evaluated_program(
    exec_output: &ExecutionOutput,
    result: &BuildEvaluationResult,
) -> Result<EvaluatedBuildProgram, BuildEvaluationError> {
    let mut step_bindings = exec_output
        .run_steps
        .iter()
        .map(|(step_name, artifact_name)| {
            let kind = if step_name == "run" {
                BuildRuntimeStepBindingKind::DefaultRun
            } else {
                BuildRuntimeStepBindingKind::NamedRun
            };
            BuildRuntimeStepBinding::new(step_name.clone(), kind, Some(artifact_name.clone()))
        })
        .collect::<Vec<_>>();
    if exec_output.executable_artifacts.len() == 1
        && !step_bindings
            .iter()
            .any(|binding| binding.step_name == "build")
    {
        step_bindings.push(BuildRuntimeStepBinding::new(
            "build",
            BuildRuntimeStepBindingKind::DefaultBuild,
            Some(exec_output.executable_artifacts[0].name.clone()),
        ));
    }
    if exec_output.test_artifacts.len() == 1
        && !step_bindings
            .iter()
            .any(|binding| binding.step_name == "test")
    {
        step_bindings.push(BuildRuntimeStepBinding::new(
            "test",
            BuildRuntimeStepBindingKind::DefaultTest,
            Some(exec_output.test_artifacts[0].name.clone()),
        ));
    }
    let artifacts = exec_output
        .executable_artifacts
        .iter()
        .map(|artifact| {
            BuildRuntimeArtifact::new(
                artifact.name.clone(),
                BuildRuntimeArtifactKind::Executable,
                artifact.root_module.resolve(&result.resolved_options),
            )
            .with_fol_model(artifact.fol_model)
            .with_target_config(
                artifact
                    .target
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
                artifact
                    .optimize
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
            )
        })
        .chain(exec_output.static_library_artifacts.iter().map(|artifact| {
            BuildRuntimeArtifact::new(
                artifact.name.clone(),
                BuildRuntimeArtifactKind::StaticLibrary,
                artifact.root_module.resolve(&result.resolved_options),
            )
            .with_fol_model(artifact.fol_model)
            .with_target_config(
                artifact
                    .target
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
                artifact
                    .optimize
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
            )
        }))
        .chain(exec_output.shared_library_artifacts.iter().map(|artifact| {
            BuildRuntimeArtifact::new(
                artifact.name.clone(),
                BuildRuntimeArtifactKind::SharedLibrary,
                artifact.root_module.resolve(&result.resolved_options),
            )
            .with_fol_model(artifact.fol_model)
            .with_target_config(
                artifact
                    .target
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
                artifact
                    .optimize
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
            )
        }))
        .chain(exec_output.test_artifacts.iter().map(|artifact| {
            BuildRuntimeArtifact::new(
                artifact.name.clone(),
                BuildRuntimeArtifactKind::Test,
                artifact.root_module.resolve(&result.resolved_options),
            )
            .with_fol_model(artifact.fol_model)
            .with_target_config(
                artifact
                    .target
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
                artifact
                    .optimize
                    .as_ref()
                    .map(|v| v.resolve(&result.resolved_options)),
            )
        }))
        .collect::<Vec<_>>();
    let dependencies = result
        .dependency_requests
        .iter()
        .map(|request| {
            let args = request
                .args
                .iter()
                .map(|(name, value)| {
                    value
                        .resolve(&result.resolved_options)
                        .map(|resolved| (name.clone(), resolved))
                        .ok_or_else(|| {
                            let detail = match value {
                                crate::DependencyArgValue::OptionRef(option_name) => {
                                    match result.option_declarations.find(option_name) {
                                        Some(crate::BuildOptionDeclaration::StandardTarget(_)) => {
                                            format!(
                                                "dependency '{}' requires resolved target option '{}' for arg '{}'",
                                                request.alias, option_name, name
                                            )
                                        }
                                        Some(crate::BuildOptionDeclaration::StandardOptimize(
                                            _,
                                        )) => {
                                            format!(
                                                "dependency '{}' requires resolved optimize option '{}' for arg '{}'",
                                                request.alias, option_name, name
                                            )
                                        }
                                        Some(crate::BuildOptionDeclaration::User(decl)) => {
                                            let option_kind = match decl.kind {
                                                crate::BuildOptionKind::Target => "target",
                                                crate::BuildOptionKind::Optimize => "optimize",
                                                crate::BuildOptionKind::Bool => "bool",
                                                crate::BuildOptionKind::Int => "int",
                                                crate::BuildOptionKind::String => "str",
                                                crate::BuildOptionKind::Enum => "enum",
                                                crate::BuildOptionKind::Path => "path",
                                            };
                                            format!(
                                                "dependency '{}' requires resolved {} option '{}' for arg '{}'",
                                                request.alias,
                                                option_kind,
                                                option_name,
                                                name
                                            )
                                        }
                                        None => format!(
                                            "dependency '{}' requires a resolved option '{}' for arg '{}'",
                                            request.alias, option_name, name
                                        ),
                                    }
                                }
                                _ => format!(
                                    "dependency '{}' requires a resolved option for arg '{}'",
                                    request.alias, name
                                ),
                            };
                            BuildEvaluationError::new(
                                BuildEvaluationErrorKind::InvalidInput,
                                detail,
                            )
                        })
                })
                .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
            Ok(BuildRuntimeDependency {
                alias: request.alias.clone(),
                source_kind: request.source_kind.clone(),
                package: request.package.clone(),
                args,
                evaluation_mode: request.evaluation_mode,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EvaluatedBuildProgram {
        program: BuildRuntimeProgram::new(BuildExecutionRepresentation::RestrictedRuntimeIr),
        artifacts,
        generated_files: exec_output.generated_files.clone(),
        dependencies,
        dependency_exports: exec_output.dependency_exports.clone(),
        dependency_queries: exec_output.dependency_queries.clone(),
        step_bindings,
        result: result.clone(),
    })
}
