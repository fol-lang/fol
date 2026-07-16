use super::super::{
    forbidden_capability_error, forbidden_capability_message, BuildEnvironmentSelectionPolicy,
    BuildEvaluationError, BuildEvaluationErrorKind, BuildEvaluationInputEnvelope,
    BuildEvaluationInputs, BuildEvaluationOperation, BuildEvaluationOperationKind,
    BuildEvaluationRequest, ForbiddenBuildTimeOperation,
};
use crate::api::StandardTargetRequest;
use crate::option::{BuildOptimizeMode, BuildTargetTriple};
use fol_diagnostics::{DiagnosticCode, ToDiagnostic};
use fol_parser::ast::SyntaxOrigin;
use std::collections::BTreeMap;

#[test]
fn build_evaluation_error_exposes_origin_as_a_diagnostic_location() {
    let error = BuildEvaluationError::with_origin(
        BuildEvaluationErrorKind::Unsupported,
        "random clock reads are outside the build evaluator subset",
        SyntaxOrigin {
            file: Some("app/build.fol".to_string()),
            line: 4,
            column: 2,
            length: 5,
        },
    );

    let location = error
        .diagnostic_location()
        .expect("evaluation errors with origins should expose locations");

    assert_eq!(location.file.as_deref(), Some("app/build.fol"));
    assert_eq!(location.line, 4);
    assert_eq!(location.column, 2);
    assert_eq!(location.length, Some(5));
}

#[test]
fn build_evaluation_errors_lower_to_stable_diagnostics() {
    let diagnostic = BuildEvaluationError::new(
        BuildEvaluationErrorKind::ValidationFailed,
        "graph validation failed",
    )
    .to_diagnostic();

    assert_eq!(diagnostic.code, DiagnosticCode::new("K1103"));
    assert!(diagnostic.message.contains("graph validation failed"));
}

#[test]
fn build_evaluation_input_determinism_key_is_stable_for_sorted_inputs() {
    let mut options = BTreeMap::new();
    options.insert("optimize".to_string(), "debug".to_string());
    options.insert("target".to_string(), "native".to_string());
    let mut environment = BTreeMap::new();
    environment.insert("CC".to_string(), "clang".to_string());
    environment.insert("AR".to_string(), "llvm-ar".to_string());
    let inputs = BuildEvaluationInputs {
        working_directory: "/work/app".to_string(),
        install_prefix: String::new(),
        target: BuildTargetTriple::parse("x86_64-linux-gnu"),
        optimize: BuildOptimizeMode::parse("debug"),
        options,
        environment_policy: BuildEnvironmentSelectionPolicy::new(["CC", "AR"]),
        environment,
    };

    assert_eq!(
        inputs.determinism_key(),
        "cwd=/work/app;prefix=;target=x86_64-unknown-linux-gnu;optimize=debug;options=[optimize=debug,target=native];declared_env=[AR,CC];env=[AR=llvm-ar,CC=clang]"
    );
}

#[test]
fn build_evaluation_request_determinism_key_includes_root_and_inputs() {
    let mut request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: Vec::new(),
    };
    request
        .inputs
        .options
        .insert("target".to_string(), "native".to_string());
    request.inputs.target = BuildTargetTriple::parse("aarch64-macos-gnu");
    request.inputs.optimize = BuildOptimizeMode::parse("release-fast");

    assert_eq!(
        request.determinism_key(),
        "root=/pkg;cwd=;prefix=;target=aarch64-apple-darwin;optimize=release-fast;options=[target=native];declared_env=[];env=[];ops=0"
    );
}

#[test]
fn build_evaluation_request_determinism_key_counts_operations() {
    let request = BuildEvaluationRequest {
        package_root: "/pkg".to_string(),
        inputs: BuildEvaluationInputs::default(),
        operations: vec![BuildEvaluationOperation {
            origin: None,
            kind: BuildEvaluationOperationKind::StandardTarget(StandardTargetRequest::new(
                "target",
            )),
        }],
    };

    assert_eq!(
        request.determinism_key(),
        "root=/pkg;cwd=;prefix=;target=;optimize=;options=[];declared_env=[];env=[];ops=1"
    );
}

#[test]
fn explicit_input_envelope_filters_ambient_environment_before_keying() {
    let inputs = BuildEvaluationInputs {
        working_directory: "/pkg".to_string(),
        install_prefix: String::new(),
        target: BuildTargetTriple::parse("x86_64-linux-gnu"),
        optimize: BuildOptimizeMode::parse("release-safe"),
        options: BTreeMap::from([("strip".to_string(), "true".to_string())]),
        environment_policy: BuildEnvironmentSelectionPolicy::new(["CC"]),
        environment: BTreeMap::from([
            ("CC".to_string(), "clang".to_string()),
            ("HOME".to_string(), "/tmp/home".to_string()),
        ]),
    };

    let envelope = inputs.explicit_envelope();

    assert_eq!(
        envelope,
        BuildEvaluationInputEnvelope {
            working_directory: "/pkg".to_string(),
            install_prefix: String::new(),
            target: BuildTargetTriple::parse("x86_64-linux-gnu"),
            optimize: BuildOptimizeMode::parse("release-safe"),
            options: BTreeMap::from([("strip".to_string(), "true".to_string())]),
            declared_environment: vec!["CC".to_string()],
            selected_environment: BTreeMap::from([("CC".to_string(), "clang".to_string())]),
        }
    );
    assert_eq!(
        envelope.determinism_key(),
        "cwd=/pkg;prefix=;target=x86_64-unknown-linux-gnu;optimize=release-safe;options=[strip=true];declared_env=[CC];env=[CC=clang]"
    );
}

#[test]
fn forbidden_capability_messages_are_specific_to_the_runtime_surface() {
    assert!(
        forbidden_capability_message(ForbiddenBuildTimeOperation::ArbitraryFilesystemRead)
            .contains("filesystem reads")
    );
    assert!(
        forbidden_capability_message(ForbiddenBuildTimeOperation::AmbientEnvironmentAccess)
            .contains("declared inputs")
    );
}

#[test]
fn forbidden_capability_errors_lower_to_unsupported_diagnostics() {
    let error = forbidden_capability_error(
        ForbiddenBuildTimeOperation::ArbitraryNetworkAccess,
        Some(SyntaxOrigin {
            file: Some("build.fol".to_string()),
            line: 4,
            column: 2,
            length: 5,
        }),
    );
    let diagnostic = error.to_diagnostic();

    assert_eq!(error.kind(), BuildEvaluationErrorKind::Unsupported);
    assert!(error.message().contains("network access"));
    assert_eq!(diagnostic.code, DiagnosticCode::new("K1102"));
    assert_eq!(diagnostic.labels.len(), 1);
}
