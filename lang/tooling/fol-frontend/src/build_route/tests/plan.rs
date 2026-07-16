use super::super::{
    execute_workspace_build_route, plan_workspace_build_route, FrontendBuildStep,
    FrontendBuildWorkflowMode, FrontendMemberBuildRoute, FrontendStepExecutionKind,
    FrontendWorkspaceBuildRequest, FrontendWorkspaceBuildRoute,
};
use crate::{FrontendConfig, FrontendWorkspace, PackageRoot, WorkspaceRoot};
use std::{fs, path::PathBuf};

fn plan_member_execution(
    member: &FrontendMemberBuildRoute,
    config: &FrontendConfig,
) -> crate::FrontendResult<super::super::FrontendMemberExecutionPlan> {
    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(member.member_root.clone()),
        members: vec![PackageRoot::new(member.member_root.clone())],
        std_root_override: None,
        package_store_root_override: None,
        build_root: member.member_root.join(".fol/build"),
        cache_root: member.member_root.join(".fol/cache"),
        git_cache_root: member.member_root.join(".fol/cache/git"),
        install_prefix: member.member_root.join(".fol/install"),
    };
    super::super::plan_member_execution(&workspace, member, config)
}

pub(super) fn absorbed_build_workspace_fixture(
    label: &str,
    with_bundled_std: bool,
) -> FrontendWorkspace {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_{label}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    let app = root.join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    let bundled_std_dependency = with_bundled_std
        .then_some(
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
        )
        .unwrap_or("");
    fs::write(
        app.join("build.fol"),
        [
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            bundled_std_dependency,
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    graph.add_run(app);\n",
            "    return;\n",
            "};\n",
        ]
        .concat(),
    )
    .unwrap();
    fs::write(
        app.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    }
}

#[test]
fn workflow_mode_maps_package_build_modes_into_frontend_route_modes() {
    assert_eq!(
        FrontendBuildWorkflowMode::from_package_build_mode(fol_package::PackageBuildMode::Empty),
        None
    );
    assert_eq!(
        FrontendBuildWorkflowMode::from_package_build_mode(
            fol_package::PackageBuildMode::ModernOnly
        ),
        Some(FrontendBuildWorkflowMode::Modern)
    );
}

#[test]
fn member_build_route_keeps_package_name_and_workflow_mode() {
    let route = FrontendMemberBuildRoute {
        member_root: PathBuf::from("/tmp/demo/app"),
        package_name: "app".to_string(),
        mode: FrontendBuildWorkflowMode::Modern,
    };

    assert_eq!(route.member_root, PathBuf::from("/tmp/demo/app"));
    assert_eq!(route.package_name, "app");
    assert_eq!(route.mode, FrontendBuildWorkflowMode::Modern);
}

#[test]
fn workspace_build_route_keeps_requested_step_and_members() {
    let route = FrontendWorkspaceBuildRoute {
        requested_step: "build".to_string(),
        members: vec![FrontendMemberBuildRoute {
            member_root: PathBuf::from("/tmp/demo/app"),
            package_name: "app".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        }],
    };

    assert_eq!(route.requested_step, "build");
    assert_eq!(route.members.len(), 1);
    assert_eq!(route.members[0].package_name, "app");
}

#[test]
fn shared_graph_projection_helper_keeps_graph_steps_and_synthesizes_check() {
    let mut graph = fol_package::BuildGraph::new();
    graph.add_step(fol_package::BuildStepKind::Default, "build", None);
    let member = FrontendMemberBuildRoute {
        member_root: PathBuf::from("/tmp/demo/app"),
        package_name: "app".to_string(),
        mode: FrontendBuildWorkflowMode::Modern,
    };
    let evaluated = fol_package::build_eval::EvaluatedBuildProgram {
        program: fol_package::BuildRuntimeProgram::new(
            fol_package::BuildExecutionRepresentation::RestrictedRuntimeIr,
        ),
        artifacts: Vec::new(),
        generated_files: Vec::new(),
        dependencies: Vec::new(),
        dependency_exports: Vec::new(),
        dependency_queries: Vec::new(),
        step_bindings: Vec::new(),
        result: fol_package::BuildEvaluationResult::new(
            fol_package::BuildEvaluationBoundary::GraphConstructionSubset,
            fol_package::canonical_graph_construction_capabilities(),
            "/tmp/demo/app",
            fol_package::BuildOptionDeclarationSet::new(),
            fol_package::ResolvedBuildOptionSet::new(),
            Vec::new(),
            graph.clone(),
        ),
    };

    let plan = super::super::plan_member_execution_from_graph(&member, &graph, &evaluated, false)
        .expect("graph projection should succeed");

    assert!(plan.steps.iter().any(|step| step.name == "build"));
    assert!(plan.steps.iter().any(|step| step.name == "check"));
}

#[test]
fn resolve_requested_step_execution_accepts_untargeted_core_routes() {
    let member_plans = vec![super::super::FrontendMemberExecutionPlan {
        steps: vec![super::super::FrontendMemberPlannedStep {
            name: "run".to_string(),
            description: Some("Run the default executable artifact".to_string()),
            default_kind: Some(fol_package::BuildDefaultStepKind::Run),
            execution: Some(super::super::FrontendStepExecutionKind::Run),
            selection: None,
            ambiguous_selection: false,
            available_models: vec![fol_backend::BackendFolModel::Core],
        }],
    }];

    let resolved = super::super::resolve_requested_step_execution("run", &member_plans)
        .expect("untargeted routed run step should resolve");

    assert_eq!(
        resolved.execution,
        super::super::FrontendStepExecutionKind::Run
    );
    assert!(resolved.selections.is_empty());
}

#[test]
fn projected_step_plans_keep_step_descriptions() {
    let mut graph = fol_package::BuildGraph::new();
    graph.add_step(
        fol_package::BuildStepKind::CustomCommand,
        "docs",
        Some("Generate documentation".to_string()),
    );
    let member = FrontendMemberBuildRoute {
        member_root: PathBuf::from("/tmp/demo/app"),
        package_name: "app".to_string(),
        mode: FrontendBuildWorkflowMode::Modern,
    };
    let evaluated = fol_package::build_eval::EvaluatedBuildProgram {
        program: fol_package::BuildRuntimeProgram::new(
            fol_package::BuildExecutionRepresentation::RestrictedRuntimeIr,
        ),
        artifacts: Vec::new(),
        generated_files: Vec::new(),
        dependencies: Vec::new(),
        dependency_exports: Vec::new(),
        dependency_queries: Vec::new(),
        step_bindings: Vec::new(),
        result: fol_package::BuildEvaluationResult::new(
            fol_package::BuildEvaluationBoundary::GraphConstructionSubset,
            fol_package::canonical_graph_construction_capabilities(),
            "/tmp/demo/app",
            fol_package::BuildOptionDeclarationSet::new(),
            fol_package::ResolvedBuildOptionSet::new(),
            Vec::new(),
            graph.clone(),
        ),
    };

    let plan = super::super::plan_member_execution_from_graph(&member, &graph, &evaluated, false)
        .expect("graph projection should keep descriptions");

    let docs = plan
        .steps
        .iter()
        .find(|step| step.name == "docs")
        .expect("docs step should exist");
    assert_eq!(docs.description.as_deref(), Some("Generate documentation"));
}

#[test]
fn semantic_member_planning_uses_graph_projected_build_run_and_check_steps() {
    let workspace = absorbed_build_workspace_fixture("compat_graph_plan", false);

    let plan = plan_member_execution(
        &FrontendMemberBuildRoute {
            member_root: workspace.members[0].root.clone(),
            package_name: "app".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        },
        &FrontendConfig::default(),
    )
    .expect("semantic member planning should succeed");

    assert!(plan.steps.iter().any(|step| step.name == "build"));
    assert!(plan.steps.iter().any(|step| step.name == "run"));
    assert!(plan.steps.iter().any(|step| step.name == "check"));

    fs::remove_dir_all(&workspace.root.root).ok();
}

#[test]
fn semantic_member_planning_rejects_dependency_queries_for_missing_exports() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_missing_export_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    let app = root.join("app");
    let shared = app.join("deps/shared");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir_all(shared.join("src")).unwrap();
    fs::write(
        app.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var dep = build.add_dep({ alias = \"shared\", source = \"loc\", target = \"deps/shared\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    var bindings = dep.generated(\"bindings\");\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        app.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();
    fs::write(
        shared.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"shared\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var lib = graph.add_static_lib({ name = \"shared\", root = \"src/root.fol\" });\n",
            "    graph.install(lib);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        shared.join("src/root.fol"),
        "fun[] helper(): int = {\n    return 1\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app.clone())],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let error = super::super::plan_member_execution(
        &workspace,
        &FrontendMemberBuildRoute {
            member_root: app,
            package_name: "app".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        },
        &FrontendConfig::default(),
    )
    .expect_err("missing exported dependency surfaces should fail planning");

    assert!(error
        .message()
        .contains("dependency 'shared' does not export generated output 'bindings'"));

    fs::remove_dir_all(&root).ok();
}

#[test]
fn build_steps_keep_stable_cli_facing_names() {
    assert_eq!(FrontendBuildStep::Build.as_str(), "build");
    assert_eq!(FrontendBuildStep::Run.as_str(), "run");
    assert_eq!(FrontendBuildStep::Test.as_str(), "test");
    assert_eq!(FrontendBuildStep::Check.as_str(), "check");
}

#[test]
fn build_steps_map_code_subcommands_to_default_requested_steps() {
    assert_eq!(
        FrontendBuildStep::default_for_code_subcommand(&crate::CodeSubcommand::Build(
            crate::BuildCommand::default()
        )),
        FrontendBuildStep::Build
    );
    assert_eq!(
        FrontendBuildStep::default_for_code_subcommand(&crate::CodeSubcommand::Run(
            crate::RunCommand::default()
        )),
        FrontendBuildStep::Run
    );
    assert_eq!(
        FrontendBuildStep::default_for_code_subcommand(&crate::CodeSubcommand::Test(
            crate::TestCommand::default()
        )),
        FrontendBuildStep::Test
    );
    assert_eq!(
        FrontendBuildStep::default_for_code_subcommand(&crate::CodeSubcommand::Check(
            crate::CheckCommand::default()
        )),
        FrontendBuildStep::Check
    );
}

#[test]
fn requested_workspace_step_prefers_explicit_override_and_falls_back_to_command_default() {
    let build = crate::CodeSubcommand::Build(crate::BuildCommand::default());
    let run = crate::CodeSubcommand::Run(crate::RunCommand::default());

    assert_eq!(
        super::super::requested_workspace_step(&build, None),
        "build"
    );
    assert_eq!(super::super::requested_workspace_step(&run, None), "run");
    assert_eq!(
        super::super::requested_workspace_step(&build, Some("docs")),
        "docs"
    );
}

#[test]
fn workspace_route_planner_accepts_only_semantic_members() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_plan_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    let modern = root.join("modern");
    fs::create_dir_all(modern.join("src")).unwrap();
    fs::write(
        modern.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"modern\", version = \"0.1.0\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        modern.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(modern.clone())],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let route = plan_workspace_build_route(&workspace, "build").unwrap();

    assert_eq!(route.requested_step, "build");
    assert_eq!(route.members.len(), 1);
    assert_eq!(route.members[0].mode, FrontendBuildWorkflowMode::Modern);

    fs::remove_dir_all(root).ok();
}

#[test]
fn workspace_route_planner_rejects_old_build_members() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_old_build_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: old\nversion: 0.1.0\n").unwrap();
    fs::write(root.join("build.fol"), "var[] answer: int = 42;\n").unwrap();

    let error = plan_workspace_build_route(
        &FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.clone())],
            std_root_override: None,
            package_store_root_override: None,
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        },
        "build",
    )
    .expect_err("build file without canonical entry should be rejected");

    assert_eq!(error.kind(), crate::FrontendErrorKind::PackageFailed);
    assert!(error
        .message()
        .contains("canonical `pro[] build(): non` entry"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn workspace_route_planner_rejects_broken_modern_builds() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_broken_modern_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: modern\nversion: 0.1.0\n").unwrap();
    fs::write(root.join("build.fol"), "pro[] build(): non = {\n").unwrap();

    let error = plan_workspace_build_route(
        &FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.clone())],
            std_root_override: None,
            package_store_root_override: None,
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        },
        "build",
    )
    .expect_err("broken modern-only build should stay a parse failure");

    assert_eq!(error.kind(), crate::FrontendErrorKind::PackageFailed);
    assert!(error
        .message()
        .contains("package loader could not parse package build file"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn modern_members_plan_custom_steps_from_semantic_builds() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_modern_custom_steps_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: modern\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"modern\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"modern\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    var docs = graph.step(\"docs\", \"Generate documentation\");\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let route = plan_workspace_build_route(
        &FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.clone())],
            std_root_override: None,
            package_store_root_override: None,
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        },
        "docs",
    )
    .expect("modern workspace route should classify successfully");
    assert_eq!(route.members[0].mode, FrontendBuildWorkflowMode::Modern);

    let plan = plan_member_execution(&route.members[0], &FrontendConfig::default())
        .expect("modern member should plan custom graph steps");
    let docs = plan
        .steps
        .iter()
        .find(|step| step.name == "docs")
        .expect("modern member should keep the custom docs step");
    assert_eq!(docs.execution, Some(FrontendStepExecutionKind::Build));

    fs::remove_dir_all(root).ok();
}

#[test]
fn absorbed_build_executor_maps_build_steps_back_onto_existing_workspace_commands() {
    let workspace = absorbed_build_workspace_fixture("compat_exec_build", false);

    let result = execute_workspace_build_route(
        &workspace,
        &FrontendConfig::default(),
        &FrontendWorkspaceBuildRequest {
            requested_step: "build".to_string(),
            profile: crate::FrontendProfile::Debug,
            run_args: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(result.command, "build");
    assert!(result
        .summary
        .contains("built 1 workspace package(s) into "));

    fs::remove_dir_all(&workspace.root.root).ok();
}

#[test]
fn absorbed_build_executor_routes_run_steps_with_arguments() {
    let workspace = absorbed_build_workspace_fixture("compat_exec_run", true);

    let result = execute_workspace_build_route(
        &workspace,
        &FrontendConfig::default(),
        &FrontendWorkspaceBuildRequest {
            requested_step: "run".to_string(),
            profile: crate::FrontendProfile::Debug,
            run_args: vec!["--demo".to_string()],
        },
    )
    .unwrap();

    assert_eq!(result.command, "run");
    assert!(result.summary.contains("ran "));

    fs::remove_dir_all(&workspace.root.root).ok();
}

#[test]
fn absorbed_build_executor_rejects_unknown_named_steps() {
    let workspace = absorbed_build_workspace_fixture("compat_exec_unknown", false);

    let error = execute_workspace_build_route(
        &workspace,
        &FrontendConfig::default(),
        &FrontendWorkspaceBuildRequest {
            requested_step: "docs".to_string(),
            profile: crate::FrontendProfile::Debug,
            run_args: Vec::new(),
        },
    )
    .expect_err("unknown absorbed-build step should fail");

    assert_eq!(error.kind(), crate::FrontendErrorKind::InvalidInput);
    assert!(error
        .message()
        .contains("workspace build execution does not define step 'docs'"));
    assert!(error.message().contains("known steps:"));
    assert!(error.message().contains("build [default:build]"));
    assert!(error.message().contains("run [default:run]"));
    assert!(error.message().contains("[artifact:app]"));
    assert!(error.message().contains("[models:memo]"));
    assert!(error
        .message()
        .contains("Build default executable artifacts"));

    fs::remove_dir_all(&workspace.root.root).ok();
}

#[test]
fn build_body_step_calls_flow_into_member_execution_plans() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_body_steps_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    var docs = graph.step(\"docs\", \"Generate documentation\");\n",
            "    var lint = graph.step(\"lint\", \"Lint the sources\");\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let plan = plan_member_execution(
        &FrontendMemberBuildRoute {
            member_root: root.clone(),
            package_name: "demo".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        },
        &FrontendConfig::default(),
    )
    .unwrap();

    assert!(plan.steps.iter().any(|step| step.name == "docs"));
    assert!(plan.steps.iter().any(|step| step.name == "lint"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_body_step_dependencies_are_accepted_during_member_planning() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_body_step_deps_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    var gen = graph.step(\"gen\", \"Generate sources\");\n",
            "    var docs = graph.step(\"docs\", \"Generate documentation\");\n",
            "    docs.depend_on(gen);\n",
            "    var run = graph.add_run(app);\n",
            "    run.depend_on(docs);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let plan = plan_member_execution(
        &FrontendMemberBuildRoute {
            member_root: root.clone(),
            package_name: "demo".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        },
        &FrontendConfig::default(),
    )
    .unwrap();

    assert!(plan.steps.iter().any(|step| step.name == "gen"));
    assert!(plan.steps.iter().any(|step| step.name == "docs"));
    assert!(plan.steps.iter().any(|step| step.name == "run"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn custom_build_steps_plan_as_build_execution() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_build_route_custom_step_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var docs = graph.step(\"docs\", \"Generate documentation\");\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();
    let plan = plan_member_execution(
        &FrontendMemberBuildRoute {
            member_root: root.clone(),
            package_name: "demo".to_string(),
            mode: FrontendBuildWorkflowMode::Modern,
        },
        &FrontendConfig::default(),
    )
    .expect("custom build-like step should plan successfully");

    let docs = plan
        .steps
        .iter()
        .find(|step| step.name == "docs")
        .expect("custom docs step should be present");
    assert_eq!(docs.execution, Some(FrontendStepExecutionKind::Build));
    assert!(docs.selection.is_none());

    fs::remove_dir_all(root).ok();
}
