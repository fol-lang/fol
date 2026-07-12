use super::super::{
    evaluate_build_plan, BuildEnvironmentSelectionPolicy, BuildEvaluationErrorKind,
    BuildEvaluationInputs, BuildEvaluationInstallArtifactRequest, BuildEvaluationOperation,
    BuildEvaluationOperationKind, BuildEvaluationRequest, BuildEvaluationRunRequest,
    BuildEvaluationStepRequest,
};
use crate::api::{
    CopyFileRequest, DependencyRequest, ExecutableRequest, InstallDirRequest,
    StandardOptimizeRequest, StandardTargetRequest, UserOptionRequest, WriteFileRequest,
};
use crate::codegen::{CodegenKind, CodegenRequest, SystemToolRequest};
use crate::option::{BuildOptimizeMode, BuildOptionDeclaration, BuildTargetTriple};
use fol_parser::ast::SyntaxOrigin;
use std::collections::BTreeMap;

#[test]
fn build_evaluation_operations_keep_origin_and_payload_shape() {
    let operation = BuildEvaluationOperation {
        origin: Some(SyntaxOrigin {
            file: Some("build.fol".to_string()),
            line: 2,
            column: 1,
            length: 3,
        }),
        kind: BuildEvaluationOperationKind::AddExe(ExecutableRequest {
            name: "app".to_string(),
            root_module: "src/app.fol".to_string(),
        }),
    };

    assert_eq!(operation.origin.as_ref().map(|origin| origin.line), Some(2));
    match operation.kind {
        BuildEvaluationOperationKind::AddExe(request) => {
            assert_eq!(request.name, "app");
            assert_eq!(request.root_module, "src/app.fol");
        }
        other => panic!("unexpected operation kind: {other:?}"),
    }
}

#[test]
fn build_evaluator_replays_standard_and_user_option_operations() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardTarget(StandardTargetRequest::new(
                    "target",
                )),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardOptimize(StandardOptimizeRequest::new(
                    "optimize",
                )),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::Option(UserOptionRequest::bool("strip", false)),
            },
        ],
    };

    let result = evaluate_build_plan(&request).expect("option replay should succeed");

    assert_eq!(result.graph.options().len(), 3);
    assert_eq!(result.package_root, "/pkg");
}

#[test]
fn build_evaluator_replays_graph_building_operations_into_a_validated_graph() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::AddExe(ExecutableRequest {
                    name: "app".to_string(),
                    root_module: "src/app.fol".to_string(),
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::Step(BuildEvaluationStepRequest {
                    name: "build".to_string(),
                    description: Some("Compile the app".to_string()),
                    depends_on: Vec::new(),
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::AddRun(BuildEvaluationRunRequest {
                    name: "run".to_string(),
                    artifact: "app".to_string(),
                    depends_on: vec!["build".to_string()],
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::InstallArtifact(
                    BuildEvaluationInstallArtifactRequest {
                        name: "install-app".to_string(),
                        artifact: "app".to_string(),
                        depends_on: vec!["build".to_string()],
                    },
                ),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::InstallDir(InstallDirRequest {
                    name: "install-assets".to_string(),
                    path: "share/assets".to_string(),
                    depends_on: Vec::new(),
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::Dependency(DependencyRequest {
                    alias: "logtiny".to_string(),
                    source_kind: crate::api::DependencySourceKind::PackageStore,
                    package: "org/logtiny".to_string(),
                    args: std::collections::BTreeMap::new(),
                    evaluation_mode: None,
                    git_version: None,
                    git_hash: None,
                    surface: None,
                }),
            },
        ],
    };

    let result = evaluate_build_plan(&request).expect("graph replay should succeed");

    assert_eq!(result.graph.artifacts().len(), 1);
    assert_eq!(result.graph.steps().len(), 4);
    assert_eq!(result.graph.installs().len(), 2);
    assert_eq!(result.graph.modules().len(), 2);
    let install_app = result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "install-app")
        .expect("install-app step should exist");
    let build = result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "build")
        .expect("build step should exist");
    assert_eq!(
        result
            .graph
            .step_dependencies_for(install_app.id)
            .collect::<Vec<_>>(),
        vec![build.id]
    );
}

#[test]
fn build_evaluator_rejects_unsupported_operations_with_source_origins() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![BuildEvaluationOperation {
            origin: Some(SyntaxOrigin {
                file: Some("build.fol".to_string()),
                line: 8,
                column: 1,
                length: 4,
            }),
            kind: BuildEvaluationOperationKind::Unsupported {
                label: "clock_now".to_string(),
            },
        }],
    };

    let error = evaluate_build_plan(&request).expect_err("unsupported operations should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::Unsupported);
    assert_eq!(
        error.origin().and_then(|origin| origin.file.as_deref()),
        Some("build.fol")
    );
    assert_eq!(error.origin().map(|origin| origin.line), Some(8));
}

#[test]
fn build_evaluator_surfaces_graph_validation_failures_as_evaluation_errors() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![BuildEvaluationOperation {
            origin: None,
            kind: BuildEvaluationOperationKind::InstallDir(InstallDirRequest {
                name: "install-assets".to_string(),
                path: String::new(),
                depends_on: Vec::new(),
            }),
        }],
    };

    let error = evaluate_build_plan(&request).expect_err("invalid install dirs should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::ValidationFailed);
    assert!(error
        .message()
        .contains("directory target must not be empty"));
}

#[test]
fn build_evaluator_replays_option_declarations_and_input_overrides() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: "/tmp/pkg".to_string(),
            install_prefix: String::new(),
            target: None,
            optimize: None,
            options: BTreeMap::from([
                ("target".to_string(), "aarch64-macos-gnu".to_string()),
                ("optimize".to_string(), "release-fast".to_string()),
            ]),
            environment_policy: BuildEnvironmentSelectionPolicy::default(),
            environment: BTreeMap::new(),
        },
        operations: vec![
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardTarget(
                    StandardTargetRequest::new("target").with_default("x86_64-linux-gnu"),
                ),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardOptimize(
                    StandardOptimizeRequest::new("optimize").with_default("debug"),
                ),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::Option(UserOptionRequest::bool("strip", false)),
            },
        ],
    };

    let result = evaluate_build_plan(&request).expect("option replay should succeed");

    assert_eq!(result.option_declarations.declarations().len(), 3);
    assert!(matches!(
        &result.option_declarations.declarations()[0],
        BuildOptionDeclaration::StandardTarget(declaration)
        if declaration.default == BuildTargetTriple::parse("x86_64-linux-gnu")
    ));
    assert_eq!(
        result.resolved_options.get("target"),
        Some("aarch64-macos-gnu")
    );
    assert_eq!(
        result.resolved_options.get("optimize"),
        Some("release-fast")
    );
}

#[test]
fn build_evaluator_uses_typed_target_and_optimize_inputs_without_duplicate_option_overrides() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: "/tmp/pkg".to_string(),
            target: BuildTargetTriple::parse("x86_64-linux-gnu"),
            optimize: BuildOptimizeMode::parse("release-safe"),
            ..BuildEvaluationInputs::default()
        },
        operations: vec![
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardTarget(StandardTargetRequest::new(
                    "target",
                )),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::StandardOptimize(StandardOptimizeRequest::new(
                    "optimize",
                )),
            },
        ],
    };

    let result = evaluate_build_plan(&request)
        .expect("typed target/optimize inputs should seed resolved options");

    assert_eq!(
        result.resolved_options.get("target"),
        Some("x86_64-linux-gnu")
    );
    assert_eq!(
        result.resolved_options.get("optimize"),
        Some("release-safe")
    );
}

#[test]
fn build_evaluator_replays_generated_file_and_codegen_operations() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::WriteFile(WriteFileRequest {
                    name: "version".to_string(),
                    path: "gen/version.fol".to_string(),
                    contents: "generated".to_string(),
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::CopyFile(CopyFileRequest {
                    name: "config".to_string(),
                    source_path: "assets/config.json".to_string(),
                    destination_path: "gen/config.json".to_string(),
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::SystemTool(SystemToolRequest {
                    tool: "schema-gen".to_string(),
                    args: vec!["api.yaml".to_string()],
                    file_args: vec!["schema/api.yaml".to_string()],
                    env: std::collections::BTreeMap::new(),
                    outputs: vec!["gen/api.fol".to_string()],
                }),
            },
            BuildEvaluationOperation {
                origin: None,
                kind: BuildEvaluationOperationKind::Codegen(CodegenRequest {
                    kind: CodegenKind::Schema,
                    input: "api.yaml".to_string(),
                    output: "gen/api_bindings.fol".to_string(),
                }),
            },
        ],
    };

    let result = evaluate_build_plan(&request).expect("generated-file replay should succeed");

    assert_eq!(result.graph.generated_files().len(), 4);
}
