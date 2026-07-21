use super::super::{
    evaluate_build_plan, BuildEvaluationErrorKind, BuildEvaluationInputs, BuildEvaluationOperation,
    BuildEvaluationOperationKind, BuildEvaluationRequest,
};
use crate::api::{
    BuildCImportRequest, SourceFileHandle, StandardOptimizeRequest, StandardTargetRequest,
    UserOptionRequest,
};

#[test]
fn build_plan_seeds_declared_option_defaults_into_resolved_values() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
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
                kind: BuildEvaluationOperationKind::Option(UserOptionRequest::int("jobs", 8)),
            },
        ],
    };

    let result = evaluate_build_plan(&request).expect("declared defaults should seed");

    assert_eq!(
        result.resolved_options.get("target"),
        Some("x86_64-linux-gnu")
    );
    assert_eq!(result.resolved_options.get("optimize"), Some("debug"));
    assert_eq!(result.resolved_options.get("jobs"), Some("8"));
}

#[test]
fn build_plan_rejects_raw_overrides_that_do_not_match_declared_option_kinds() {
    let mut inputs = BuildEvaluationInputs::default();
    inputs
        .options
        .insert("jobs".to_string(), "fast".to_string());
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs,
        operations: vec![BuildEvaluationOperation {
            origin: None,
            kind: BuildEvaluationOperationKind::Option(UserOptionRequest::int("jobs", 8)),
        }],
    };

    let error = evaluate_build_plan(&request)
        .expect_err("invalid raw overrides should fail against declared kinds");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert!(error.message().contains("jobs"));
    assert!(error.message().contains("fast"));
}

#[test]
fn build_plan_rejects_c_imports_for_unknown_artifact_names() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![BuildEvaluationOperation {
            origin: None,
            kind: BuildEvaluationOperationKind::ArtifactAddCImport {
                artifact: "missing".to_string(),
                request: BuildCImportRequest {
                    header: SourceFileHandle {
                        relative_path: "native/widget.h".to_string(),
                    },
                    provider: SourceFileHandle {
                        relative_path: "native/widget.o".to_string(),
                    },
                    provider_kind: crate::graph::BuildCImportProviderKind::Object,
                },
            },
        }],
    };

    let error = evaluate_build_plan(&request)
        .expect_err("C imports must not attach to an inferred artifact");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "unknown artifact 'missing' in artifact.add_c_import"
    );
}
