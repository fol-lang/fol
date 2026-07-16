use super::capabilities::{canonical_graph_construction_capabilities, BuildEvaluationBoundary};
use super::error::{
    evaluation_api_error, evaluation_error, evaluation_invalid_input, BuildEvaluationError,
    BuildEvaluationErrorKind,
};
use super::types::{
    BuildEvaluationOperationKind, BuildEvaluationRequest, BuildEvaluationResult,
    BuildEvaluationRunArgKind,
};
use crate::api::BuildApi;
use crate::option::{
    BuildOptimizeMode, BuildOptionDeclaration, BuildOptionDeclarationSet, BuildTargetTriple,
    ResolvedBuildOptionSet, StandardOptimizeDeclaration, StandardTargetDeclaration,
    UserOptionDeclaration,
};
use std::collections::BTreeMap;

fn parse_dependency_module_identity(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("dep::")?;
    let (alias, rest) = rest.split_once("::module::")?;
    Some((alias, rest))
}

fn parse_dependency_output_identity(name: &str) -> Option<(&str, &str, &str)> {
    let rest = name.strip_prefix("dep::")?;
    for kind in ["generated", "path"] {
        let marker = format!("::{kind}::");
        if let Some((alias, query_name)) = rest.split_once(marker.as_str()) {
            return Some((alias, kind, query_name));
        }
    }
    None
}

pub fn evaluate_build_plan(
    request: &BuildEvaluationRequest,
) -> Result<BuildEvaluationResult, BuildEvaluationError> {
    let mut step_names = BTreeMap::new();
    let mut artifact_names = BTreeMap::new();
    let mut module_names: BTreeMap<String, crate::graph::BuildModuleId> = BTreeMap::new();
    let mut generated_names: BTreeMap<String, crate::graph::BuildGeneratedFileId> = BTreeMap::new();
    let mut dependency_requests = Vec::new();
    let mut option_declarations = BuildOptionDeclarationSet::new();
    let raw_option_overrides = request.inputs.options.clone();
    let mut resolved_options = ResolvedBuildOptionSet::new();
    let mut graph = crate::graph::BuildGraph::new();
    let mut api = BuildApi::with_install_prefix(&mut graph, request.inputs.install_prefix.clone());

    for operation in &request.operations {
        match &operation.kind {
            BuildEvaluationOperationKind::StandardTarget(operation_request) => {
                option_declarations.add(BuildOptionDeclaration::StandardTarget(
                    StandardTargetDeclaration {
                        name: operation_request.name.clone(),
                        default: operation_request
                            .default
                            .as_deref()
                            .and_then(BuildTargetTriple::parse),
                    },
                ));
                api.standard_target(operation_request.clone());
            }
            BuildEvaluationOperationKind::StandardOptimize(operation_request) => {
                option_declarations.add(BuildOptionDeclaration::StandardOptimize(
                    StandardOptimizeDeclaration {
                        name: operation_request.name.clone(),
                        default: operation_request
                            .default
                            .as_deref()
                            .and_then(BuildOptimizeMode::parse),
                    },
                ));
                api.standard_optimize(operation_request.clone());
            }
            BuildEvaluationOperationKind::Option(operation_request) => {
                option_declarations.add(BuildOptionDeclaration::User(UserOptionDeclaration {
                    name: operation_request.name.clone(),
                    kind: operation_request.kind,
                    default: operation_request.default.clone(),
                    help: None,
                }));
                api.option(operation_request.clone());
            }
            BuildEvaluationOperationKind::AddExe(operation_request) => {
                let handle = api
                    .add_exe(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                artifact_names.insert(operation_request.name.clone(), handle);
            }
            BuildEvaluationOperationKind::AddStaticLib(operation_request) => {
                let handle = api
                    .add_static_lib(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                artifact_names.insert(operation_request.name.clone(), handle);
            }
            BuildEvaluationOperationKind::AddSharedLib(operation_request) => {
                let handle = api
                    .add_shared_lib(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                artifact_names.insert(operation_request.name.clone(), handle);
            }
            BuildEvaluationOperationKind::AddTest(operation_request) => {
                let handle = api
                    .add_test(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                artifact_names.insert(operation_request.name.clone(), handle);
            }
            BuildEvaluationOperationKind::Step(operation_request) => {
                let depends_on = operation_request
                    .depends_on
                    .iter()
                    .map(|name| {
                        step_names.get(name).copied().ok_or_else(|| {
                            evaluation_invalid_input(
                                format!("unknown step dependency '{name}'"),
                                operation.origin.clone(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let handle = api
                    .step(crate::StepRequest {
                        name: operation_request.name.clone(),
                        description: operation_request.description.clone(),
                        depends_on,
                    })
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                step_names.insert(operation_request.name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::AddRun(operation_request) => {
                let artifact = artifact_names
                    .get(&operation_request.artifact)
                    .cloned()
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown run artifact '{}'", operation_request.artifact),
                            operation.origin.clone(),
                        )
                    })?;
                let depends_on = operation_request
                    .depends_on
                    .iter()
                    .map(|name| {
                        step_names.get(name).copied().ok_or_else(|| {
                            evaluation_invalid_input(
                                format!("unknown step dependency '{name}'"),
                                operation.origin.clone(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let handle = api
                    .add_run(crate::RunRequest {
                        name: operation_request.name.clone(),
                        artifact,
                        depends_on,
                    })
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                step_names.insert(operation_request.name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::InstallArtifact(operation_request) => {
                let artifact = artifact_names
                    .get(&operation_request.artifact)
                    .cloned()
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown install artifact '{}'", operation_request.artifact),
                            operation.origin.clone(),
                        )
                    })?;
                let depends_on = operation_request
                    .depends_on
                    .iter()
                    .map(|name| {
                        step_names.get(name).copied().ok_or_else(|| {
                            evaluation_invalid_input(
                                format!("unknown step dependency '{name}'"),
                                operation.origin.clone(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let handle = api
                    .install(crate::InstallArtifactRequest {
                        name: operation_request.name.clone(),
                        artifact,
                        depends_on,
                    })
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                step_names.insert(operation_request.name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::InstallFile(operation_request) => {
                let handle = api
                    .install_file(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                step_names.insert(operation_request.name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::InstallGeneratedFile {
                name,
                generated_name,
            } => {
                let handle = if let Some(generated_id) =
                    generated_names.get(generated_name).copied()
                {
                    api.install_generated_file(name.clone(), generated_id)
                        .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?
                } else if let Some((alias, _kind, query_name)) =
                    parse_dependency_output_identity(generated_name)
                {
                    api.install_file(crate::InstallFileRequest {
                        name: name.clone(),
                        path: format!("$dep/{alias}/{query_name}"),
                        depends_on: Vec::new(),
                    })
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?
                } else {
                    return Err(evaluation_invalid_input(
                        format!("unknown generated file '{generated_name}' in graph.install_file"),
                        operation.origin.clone(),
                    ));
                };
                step_names.insert(name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::InstallGeneratedDir {
                name,
                generated_name,
            } => {
                let handle = if let Some(generated_id) =
                    generated_names.get(generated_name).copied()
                {
                    api.install_generated_dir(name.clone(), generated_id)
                        .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?
                } else if let Some((alias, _kind, query_name)) =
                    parse_dependency_output_identity(generated_name)
                {
                    api.install_dir(crate::InstallDirRequest {
                        name: name.clone(),
                        path: format!("$dep/{alias}/{query_name}"),
                        depends_on: Vec::new(),
                    })
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?
                } else {
                    return Err(evaluation_invalid_input(
                        format!("unknown generated dir '{generated_name}' in graph.install_dir"),
                        operation.origin.clone(),
                    ));
                };
                step_names.insert(name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::InstallDir(operation_request) => {
                let handle = api
                    .install_dir(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                step_names.insert(operation_request.name.clone(), handle.step_id);
            }
            BuildEvaluationOperationKind::WriteFile(operation_request) => {
                let handle = api
                    .write_file(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                let generated_file_id = handle.generated_file_id().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!(
                            "output '{}' from graph.write_file must resolve to a local generated file",
                            operation_request.name
                        ),
                        operation.origin.clone(),
                    )
                })?;
                generated_names.insert(operation_request.name.clone(), generated_file_id);
            }
            BuildEvaluationOperationKind::CopyFile(operation_request) => {
                let handle = api
                    .copy_file(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                let generated_file_id = handle.generated_file_id().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!(
                            "output '{}' from graph.copy_file must resolve to a local generated file",
                            operation_request.name
                        ),
                        operation.origin.clone(),
                    )
                })?;
                generated_names.insert(operation_request.name.clone(), generated_file_id);
            }
            BuildEvaluationOperationKind::SystemTool(operation_request) => {
                let handles = api
                    .add_system_tool(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                for (output, handle) in operation_request.outputs.iter().zip(handles) {
                    generated_names.insert(output.clone(), handle.generated_file_id);
                }
            }
            BuildEvaluationOperationKind::SystemToolDir(operation_request) => {
                let output = operation_request.outputs.first().ok_or_else(|| {
                    evaluation_invalid_input(
                        "graph.add_system_tool_dir requires one output directory".to_string(),
                        operation.origin.clone(),
                    )
                })?;
                let handle = api
                    .add_system_tool_dir(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                generated_names.insert(output.clone(), handle.generated_file_id);
            }
            BuildEvaluationOperationKind::Codegen(operation_request) => {
                let handle = api
                    .add_codegen(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                generated_names.insert(operation_request.output.clone(), handle.generated_file_id);
            }
            BuildEvaluationOperationKind::CodegenDir(operation_request) => {
                let handle = api
                    .add_codegen_dir(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                generated_names.insert(operation_request.output.clone(), handle.generated_file_id);
            }
            BuildEvaluationOperationKind::Dependency(operation_request) => {
                dependency_requests.push(operation_request.clone());
                api.dependency(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
            }
            BuildEvaluationOperationKind::AddModule(operation_request) => {
                let handle = api
                    .add_module(operation_request.clone())
                    .map_err(|error| evaluation_api_error(error, operation.origin.clone()))?;
                module_names.insert(handle.name.clone(), handle.module_id);
            }
            BuildEvaluationOperationKind::ArtifactLink { artifact, linked } => {
                let artifact_id = artifact_names
                    .get(artifact)
                    .map(|h: &crate::api::BuildArtifactHandle| h.artifact_id)
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown artifact '{artifact}' in artifact.link"),
                            operation.origin.clone(),
                        )
                    })?;
                let linked_id = artifact_names
                    .get(linked)
                    .map(|h: &crate::api::BuildArtifactHandle| h.artifact_id)
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown artifact '{linked}' in artifact.link"),
                            operation.origin.clone(),
                        )
                    })?;
                api.artifact_link(artifact_id, linked_id);
            }
            BuildEvaluationOperationKind::ArtifactLinkSystemLibrary { artifact, request } => {
                let artifact_id = artifact_names
                    .get(artifact)
                    .map(|h: &crate::api::BuildArtifactHandle| h.artifact_id)
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown artifact '{artifact}' in artifact.link"),
                            operation.origin.clone(),
                        )
                    })?;
                api.artifact_link_system_library(artifact_id, request.clone());
            }
            BuildEvaluationOperationKind::ArtifactImport {
                artifact,
                module_name,
            } => {
                let artifact_id = artifact_names
                    .get(artifact)
                    .map(|h: &crate::api::BuildArtifactHandle| h.artifact_id)
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown artifact '{artifact}' in artifact.import"),
                            operation.origin.clone(),
                        )
                    })?;
                let module_id = if let Some(module_id) = module_names.get(module_name).copied() {
                    module_id
                } else if let Some((alias, query_name)) =
                    parse_dependency_module_identity(module_name)
                {
                    let synthetic_name = format!("dep:{alias}:{query_name}");
                    let module_id = api
                        .graph_mut()
                        .add_module(crate::graph::BuildModuleKind::Imported, synthetic_name);
                    module_names.insert(module_name.clone(), module_id);
                    module_id
                } else {
                    return Err(evaluation_invalid_input(
                        format!("unknown module '{module_name}' in artifact.import"),
                        operation.origin.clone(),
                    ));
                };
                api.artifact_import(artifact_id, module_id);
            }
            BuildEvaluationOperationKind::ArtifactAddGenerated {
                artifact,
                generated_name,
            } => {
                let artifact_id = artifact_names
                    .get(artifact)
                    .map(|h: &crate::api::BuildArtifactHandle| h.artifact_id)
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown artifact '{artifact}' in artifact.add_generated"),
                            operation.origin.clone(),
                        )
                    })?;
                let gen_id = generated_names.get(generated_name).copied().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!("unknown generated file '{generated_name}' in artifact.add_generated"),
                        operation.origin.clone(),
                    )
                })?;
                api.artifact_add_generated(artifact_id, gen_id);
            }
            BuildEvaluationOperationKind::RunAddArg {
                run_name,
                kind,
                value,
            } => {
                let step_id = step_names.get(run_name).copied().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!("unknown run step '{run_name}' in run.add_arg"),
                        operation.origin.clone(),
                    )
                })?;
                let arg = match kind {
                    BuildEvaluationRunArgKind::Literal => {
                        crate::graph::BuildRunArg::Literal(value.clone())
                    }
                    BuildEvaluationRunArgKind::GeneratedFile => {
                        if let Some(gen_id) = generated_names.get(value).copied() {
                            crate::graph::BuildRunArg::GeneratedFile(gen_id)
                        } else if let Some((alias, _kind, query_name)) =
                            parse_dependency_output_identity(value)
                        {
                            crate::graph::BuildRunArg::Path(format!("$dep/{alias}/{query_name}"))
                        } else {
                            return Err(evaluation_invalid_input(
                                format!("unknown generated file '{value}' in run.add_file_arg"),
                                operation.origin.clone(),
                            ));
                        }
                    }
                    BuildEvaluationRunArgKind::Path => {
                        crate::graph::BuildRunArg::Path(value.clone())
                    }
                };
                api.run_add_arg(step_id, arg);
            }
            BuildEvaluationOperationKind::RunCapture {
                run_name,
                output_name,
            } => {
                let step_id = step_names.get(run_name).copied().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!("unknown run step '{run_name}' in run.capture_stdout"),
                        operation.origin.clone(),
                    )
                })?;
                let handle = api.run_capture_stdout(step_id, output_name.clone());
                let generated_file_id = handle.generated_file_id().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!(
                            "output '{}' from run.capture_stdout must resolve to a local generated file",
                            output_name
                        ),
                        operation.origin.clone(),
                    )
                })?;
                generated_names.insert(output_name.clone(), generated_file_id);
            }
            BuildEvaluationOperationKind::RunSetEnv {
                run_name,
                key,
                value,
            } => {
                let step_id = step_names.get(run_name).copied().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!("unknown run step '{run_name}' in run.set_env"),
                        operation.origin.clone(),
                    )
                })?;
                api.run_set_env(step_id, key.clone(), value.clone());
            }
            BuildEvaluationOperationKind::StepAttach {
                step_name,
                generated_name,
            } => {
                let step_id = step_names.get(step_name).copied().ok_or_else(|| {
                    evaluation_invalid_input(
                        format!("unknown step '{step_name}' in step.attach"),
                        operation.origin.clone(),
                    )
                })?;
                let gen_id = generated_names
                    .get(generated_name)
                    .copied()
                    .ok_or_else(|| {
                        evaluation_invalid_input(
                            format!("unknown generated file '{generated_name}' in step.attach"),
                            operation.origin.clone(),
                        )
                    })?;
                api.step_attach(step_id, gen_id);
            }
            BuildEvaluationOperationKind::Unsupported { label } => {
                return Err(evaluation_error(
                    BuildEvaluationErrorKind::Unsupported,
                    format!("unsupported build operation: {label}"),
                    operation.origin.clone(),
                ));
            }
        }
    }

    if let Some(target) = &request.inputs.target {
        resolved_options.insert("target", target.render());
    }
    if let Some(optimize) = request.inputs.optimize {
        resolved_options.insert("optimize", optimize.as_str());
    }
    for declaration in option_declarations.declarations() {
        if resolved_options.get(declaration.name()).is_none() {
            if let Some(default) = declaration.default_raw_value() {
                resolved_options.insert(declaration.name(), default);
            }
        }
    }
    for (name, raw_value) in &raw_option_overrides {
        let Some(declaration) = option_declarations.find(name) else {
            resolved_options.insert(name.clone(), raw_value.clone());
            continue;
        };
        let Some(coerced) = declaration.coerce_raw_value(raw_value) else {
            return Err(evaluation_invalid_input(
                format!("build option '{name}' cannot coerce value '{raw_value}'"),
                None,
            ));
        };
        resolved_options.insert(name.clone(), coerced);
    }

    if let Some(validation_error) = graph.validate().into_iter().next() {
        return Err(evaluation_error(
            BuildEvaluationErrorKind::ValidationFailed,
            validation_error.message,
            None,
        ));
    }

    Ok(BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        canonical_graph_construction_capabilities(),
        request.package_root.clone(),
        option_declarations,
        resolved_options,
        dependency_requests,
        graph,
    ))
}
