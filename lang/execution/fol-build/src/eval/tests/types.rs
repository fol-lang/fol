use super::super::{
    canonical_graph_construction_capabilities, AllowedBuildTimeOperation,
    BuildEnvironmentSelectionPolicy, BuildEvaluationBoundary, BuildEvaluationResult,
    BuildRuntimeCapabilityModel, ForbiddenBuildTimeOperation,
};
use crate::api::{DependencyArgValue, DependencyRequest};
use crate::graph::BuildGraph;
use crate::option::{
    BuildOptimizeMode, BuildOptionDeclaration, BuildOptionDeclarationSet, ResolvedBuildOptionSet,
};

#[test]
fn build_evaluation_request_defaults_to_an_empty_package_root() {
    use super::super::BuildEvaluationRequest;
    let request = BuildEvaluationRequest::default();

    assert!(request.package_root.is_empty());
    assert!(request.inputs.working_directory.is_empty());
    assert!(request.inputs.target.is_none());
    assert!(request.inputs.optimize.is_none());
    assert!(request.operations.is_empty());
}

#[test]
fn build_evaluation_result_carries_the_constructed_graph() {
    let graph = BuildGraph::new();
    let result = BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        BuildRuntimeCapabilityModel::new(
            vec![AllowedBuildTimeOperation::GraphMutation],
            Vec::new(),
        ),
        "app",
        crate::BuildOptionDeclarationSet::new(),
        crate::ResolvedBuildOptionSet::new(),
        Vec::new(),
        graph.clone(),
    );

    assert_eq!(result.package_root, "app");
    assert_eq!(result.graph, graph);
}

#[test]
fn build_evaluation_result_keeps_boundary_and_allowed_operation_metadata() {
    let result = BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        BuildRuntimeCapabilityModel::new(
            vec![
                AllowedBuildTimeOperation::GraphMutation,
                AllowedBuildTimeOperation::OptionRead,
            ],
            vec![ForbiddenBuildTimeOperation::ArbitraryNetworkAccess],
        ),
        "pkg",
        crate::BuildOptionDeclarationSet::new(),
        crate::ResolvedBuildOptionSet::new(),
        Vec::new(),
        BuildGraph::new(),
    );

    assert_eq!(
        result.boundary,
        BuildEvaluationBoundary::GraphConstructionSubset
    );
    assert_eq!(
        result.capabilities.allowed_operations,
        vec![
            AllowedBuildTimeOperation::GraphMutation,
            AllowedBuildTimeOperation::OptionRead,
        ]
    );
    assert_eq!(
        result.capabilities.forbidden_operations,
        vec![ForbiddenBuildTimeOperation::ArbitraryNetworkAccess]
    );
}

#[test]
fn forbidden_build_time_operations_cover_phase_four_runtime_gaps() {
    let forbidden = vec![
        ForbiddenBuildTimeOperation::ArbitraryFilesystemRead,
        ForbiddenBuildTimeOperation::ArbitraryFilesystemWrite,
        ForbiddenBuildTimeOperation::ArbitraryNetworkAccess,
        ForbiddenBuildTimeOperation::WallClockAccess,
        ForbiddenBuildTimeOperation::AmbientEnvironmentAccess,
        ForbiddenBuildTimeOperation::UncontrolledProcessExecution,
    ];

    assert_eq!(forbidden.len(), 6);
    assert!(forbidden.contains(&ForbiddenBuildTimeOperation::ArbitraryNetworkAccess));
    assert!(forbidden.contains(&ForbiddenBuildTimeOperation::WallClockAccess));
}

#[test]
fn runtime_capability_models_keep_allowed_and_forbidden_sets_together() {
    let model = BuildRuntimeCapabilityModel::new(
        vec![AllowedBuildTimeOperation::GraphMutation],
        vec![ForbiddenBuildTimeOperation::WallClockAccess],
    );

    assert_eq!(
        model.allowed_operations,
        vec![AllowedBuildTimeOperation::GraphMutation]
    );
    assert_eq!(
        model.forbidden_operations,
        vec![ForbiddenBuildTimeOperation::WallClockAccess]
    );
}

#[test]
fn canonical_graph_construction_capabilities_cover_the_declared_runtime_surface() {
    let capabilities = canonical_graph_construction_capabilities();

    assert!(capabilities
        .allowed_operations
        .contains(&AllowedBuildTimeOperation::GraphMutation));
    assert!(capabilities
        .allowed_operations
        .contains(&AllowedBuildTimeOperation::ControlledFileGeneration));
    assert!(capabilities
        .allowed_operations
        .contains(&AllowedBuildTimeOperation::ControlledProcessExecution));
    assert!(capabilities
        .forbidden_operations
        .contains(&ForbiddenBuildTimeOperation::ArbitraryNetworkAccess));
    assert!(capabilities
        .forbidden_operations
        .contains(&ForbiddenBuildTimeOperation::AmbientEnvironmentAccess));
}

#[test]
fn environment_selection_policy_sorts_and_filters_declared_variables() {
    use std::collections::BTreeMap;
    let policy = BuildEnvironmentSelectionPolicy::new(["CC", "AR", "CC"]);
    let selected = policy.select(
        BTreeMap::from([
            ("CC".to_string(), "clang".to_string()),
            ("AR".to_string(), "llvm-ar".to_string()),
            ("HOME".to_string(), "/tmp/home".to_string()),
        ])
        .iter(),
    );

    assert_eq!(
        policy.declared_names(),
        &vec!["AR".to_string(), "CC".to_string()]
    );
    assert_eq!(
        selected,
        BTreeMap::from([
            ("AR".to_string(), "llvm-ar".to_string()),
            ("CC".to_string(), "clang".to_string()),
        ])
    );
}

#[test]
fn build_evaluation_result_keeps_declared_and_resolved_options() {
    let mut declarations = crate::BuildOptionDeclarationSet::new();
    declarations.add(BuildOptionDeclaration::StandardOptimize(
        crate::StandardOptimizeDeclaration {
            name: "optimize".to_string(),
            default: Some(BuildOptimizeMode::Debug),
        },
    ));
    let mut resolved = crate::ResolvedBuildOptionSet::new();
    resolved.insert("optimize", "release-fast");
    let result = BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        BuildRuntimeCapabilityModel::new(
            vec![AllowedBuildTimeOperation::OptionRead],
            Vec::new(),
        ),
        "pkg",
        declarations,
        resolved,
        Vec::new(),
        BuildGraph::new(),
    );

    assert_eq!(result.option_declarations.declarations().len(), 1);
    assert_eq!(
        result.resolved_options.get("optimize"),
        Some("release-fast")
    );
}

#[test]
fn build_evaluation_result_keeps_declared_dependency_requests() {
    let dependencies = vec![DependencyRequest {
        alias: "core".to_string(),
        source_kind: crate::api::DependencySourceKind::PackageStore,
        package: "org/core".to_string(),
        args: std::collections::BTreeMap::from([(
            "jobs".to_string(),
            DependencyArgValue::Int(4),
        )]),
        evaluation_mode: Some(crate::DependencyBuildEvaluationMode::Lazy),
        git_version: None,
        git_hash: None,
        surface: None,
    }];
    let result = BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        BuildRuntimeCapabilityModel::new(
            vec![AllowedBuildTimeOperation::GraphMutation],
            Vec::new(),
        ),
        "pkg",
        BuildOptionDeclarationSet::new(),
        ResolvedBuildOptionSet::new(),
        dependencies.clone(),
        BuildGraph::new(),
    );

    assert_eq!(result.dependency_requests, dependencies);
}
