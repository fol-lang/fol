use super::{
    backend_config, build_workspace, build_workspace_for_profile_with_config,
    build_workspace_with_config, check_workspace, declared_capability_model_for_package,
    effective_runtime_model_for_package, emit_lowered, emit_rust, profile_build_root,
    run_workspace, run_workspace_with_args_and_config, test_package, test_workspace,
    test_workspace_with_config, typecheck_capability_model,
};
use crate::{
    FrontendArtifactKind, FrontendConfig, FrontendProfile, FrontendWorkspace, PackageRoot,
    WorkspaceRoot,
};
use fol_backend::{BackendFolModel, BackendMachineTarget};
use std::{fs, path::PathBuf};

fn semantic_bin_build() -> &'static str {
    concat!(
        "pro[] build(): non = {\n",
        "    var graph = .graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
        "    graph.install(app);\n",
        "    graph.add_run(app);\n",
        "    graph.add_test({ name = \"app_test\", root = \"src/main.fol\" });\n",
        "};\n",
    )
}

fn non_host_machine_target() -> String {
    if FrontendConfig::host_rust_target_triple() == Some("aarch64-apple-darwin") {
        "x86_64-unknown-linux-gnu".to_string()
    } else {
        "aarch64-apple-darwin".to_string()
    }
}

#[test]
fn check_workspace_runs_the_real_pipeline_for_workspace_members() {
    let root = std::env::temp_dir().join(format!("fol_frontend_check_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
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

    let result = check_workspace(&workspace).unwrap();

    assert_eq!(result.command, "check");
    assert_eq!(result.summary, "checked 1 workspace package(s)");
    assert_eq!(result.artifacts[0].path, Some(app));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_workspace_runs_the_backend_for_runnable_members() {
    let root = std::env::temp_dir().join(format!("fol_frontend_build_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = build_workspace(&workspace).unwrap();

    assert_eq!(result.command, "build");
    assert!(result
        .summary
        .contains("built 1 workspace package(s) into "));
    assert!(result.summary.contains("capability_mode=memo"));
    assert!(result.summary.contains("bundled_std=0/1"));
    assert_eq!(result.artifacts.len(), 2);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(result.artifacts[1].kind, FrontendArtifactKind::Binary);
    assert!(result.artifacts[1]
        .path
        .as_ref()
        .expect("binary path")
        .is_file());

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_output_roots_are_profile_scoped() {
    let workspace = FrontendWorkspace::new(WorkspaceRoot::new(PathBuf::from("/tmp/demo")));

    assert_eq!(
        profile_build_root(&workspace, FrontendProfile::Debug),
        PathBuf::from("/tmp/demo/.fol/build/debug")
    );
    assert_eq!(
        profile_build_root(&workspace, FrontendProfile::Release),
        PathBuf::from("/tmp/demo/.fol/build/release")
    );
}

#[test]
fn backend_config_threads_frontend_machine_target_selection() {
    let default_config = FrontendConfig::default();
    let cross_config = FrontendConfig {
        build_target_override: Some("aarch64-macos-gnu".to_string()),
        ..FrontendConfig::default()
    };

    assert_eq!(
        backend_config(
            &default_config,
            FrontendProfile::Debug,
            fol_backend::BackendFolModel::Std,
        )
        .machine_target,
        BackendMachineTarget::Host
    );
    assert_eq!(
        backend_config(
            &cross_config,
            FrontendProfile::Release,
            fol_backend::BackendFolModel::Core,
        )
        .machine_target,
        BackendMachineTarget::Triple("aarch64-macos-gnu".to_string())
    );
    assert_eq!(
        backend_config(
            &cross_config,
            FrontendProfile::Release,
            fol_backend::BackendFolModel::Core,
        )
        .fol_model,
        fol_backend::BackendFolModel::Core
    );
}

#[test]
fn frontend_maps_backend_fol_models_into_typecheck_models() {
    assert_eq!(
        typecheck_capability_model(BackendFolModel::Core),
        fol_typecheck::TypecheckCapabilityModel::Core
    );
    assert_eq!(
        typecheck_capability_model(BackendFolModel::Memo),
        fol_typecheck::TypecheckCapabilityModel::Memo
    );
    assert_eq!(
        typecheck_capability_model(BackendFolModel::Std),
        fol_typecheck::TypecheckCapabilityModel::Std
    );
}

#[test]
fn declared_capability_model_defaults_to_memo_and_ignores_bundled_std_dependency() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_capability_default_{}", std::process::id()));
    let app = root.join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::write(
        app.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(app.join("src/main.fol"), "fun[] main(): int = { return 0; };\n").unwrap();

    assert_eq!(declared_capability_model_for_package(&app), BackendFolModel::Memo);
    assert_eq!(
        effective_runtime_model_for_package(&app, BackendFolModel::Memo),
        BackendFolModel::Std
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_workspace_uses_profile_specific_output_roots() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_build_profile_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = build_workspace_for_profile_with_config(
        &workspace,
        &crate::FrontendConfig::default(),
        FrontendProfile::Release,
    )
    .unwrap();

    let binary = result.artifacts[1].path.as_ref().expect("binary path");
    assert!(binary
        .display()
        .to_string()
        .contains("/.fol/build/release/"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn run_workspace_executes_a_single_runnable_member() {
    let root = std::env::temp_dir().join(format!("fol_frontend_run_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = run_workspace(&workspace).unwrap();

    assert_eq!(result.command, "run");
    assert!(result.summary.contains("ran "));
    assert_eq!(result.artifacts.len(), 1);
    assert!(result.artifacts[0]
        .path
        .as_ref()
        .expect("binary path")
        .is_file());

    fs::remove_dir_all(root).ok();
}

#[test]
fn run_workspace_passes_through_binary_arguments() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_run_args_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = run_workspace_with_args_and_config(
        &workspace,
        &crate::FrontendConfig::default(),
        &["--demo".to_string(), "123".to_string()],
    )
    .unwrap();

    assert_eq!(result.command, "run");
    assert_eq!(result.artifacts.len(), 1);

    fs::remove_dir_all(root).ok();
}

#[test]
fn run_workspace_rejects_non_host_machine_targets_before_execution() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_run_cross_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };
    let config = crate::FrontendConfig {
        build_target_override: Some(non_host_machine_target()),
        ..crate::FrontendConfig::default()
    };

    let error = run_workspace_with_args_and_config(&workspace, &config, &[]).unwrap_err();

    assert!(error
        .to_string()
        .contains("run command cannot execute target"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_workspace_keeps_generated_crate_dirs_when_requested() {
    let root = std::env::temp_dir().join(format!(
        "fol_frontend_keep_build_dir_{}",
        std::process::id()
    ));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };
    let config = crate::FrontendConfig {
        keep_build_dir: true,
        ..crate::FrontendConfig::default()
    };

    let result = build_workspace_with_config(&workspace, &config).unwrap();
    let crate_root = result.artifacts[0].path.as_ref().unwrap();

    assert!(crate_root.exists());

    fs::remove_dir_all(root).ok();
}

#[test]
fn test_workspace_runs_single_workspace_members() {
    let root = std::env::temp_dir().join(format!("fol_frontend_test_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = test_workspace(&workspace).unwrap();

    assert_eq!(result.command, "test");
    assert_eq!(
        result.summary,
        "tested 1 workspace package(s) (capability_mode=memo, bundled_std=0/1)"
    );
    assert_eq!(result.artifacts.len(), 1);

    fs::remove_dir_all(root).ok();
}

#[test]
fn test_package_selects_a_single_named_workspace_member() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_test_package_{}", std::process::id()));
    let app = root.join("app");
    let lib = root.join("lib");
    for package in [&app, &lib] {
        let src = package.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(package.join("build.fol"), "name: pkg\nversion: 0.1.0\n").unwrap();
        fs::write(package.join("build.fol"), semantic_bin_build()).unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0\n};\n",
        )
        .unwrap();
    }

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app), PackageRoot::new(lib)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = test_package(&workspace, "lib").unwrap();

    assert_eq!(result.command, "test");
    assert_eq!(
        result.summary,
        "tested 1 workspace package(s) (capability_mode=memo, bundled_std=0/1)"
    );
    assert_eq!(result.artifacts.len(), 1);

    fs::remove_dir_all(root).ok();
}

#[test]
fn test_workspace_rejects_non_host_machine_targets_before_execution() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_test_cross_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };
    let config = crate::FrontendConfig {
        build_target_override: Some(non_host_machine_target()),
        ..crate::FrontendConfig::default()
    };

    let error = test_workspace_with_config(&workspace, &config).unwrap_err();

    assert!(error
        .to_string()
        .contains("test command cannot execute target"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn emit_rust_materializes_generated_crates_for_workspace_members() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_emit_rust_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = emit_rust(&workspace).unwrap();

    assert_eq!(result.command, "emit rust");
    assert_eq!(
        result.summary,
        format!(
            "emitted 1 Rust crate(s) into {}",
            workspace.build_root.join("emit").join("rust").display()
        )
    );
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(result.artifacts[1].kind, FrontendArtifactKind::EmittedRust);
    assert!(result.artifacts[1].path.as_ref().unwrap().is_dir());

    fs::remove_dir_all(root).ok();
}

#[test]
fn emit_lowered_materializes_rendered_workspace_snapshots() {
    let root =
        std::env::temp_dir().join(format!("fol_frontend_emit_lowered_{}", std::process::id()));
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(app.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .unwrap();

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: None,
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = emit_lowered(&workspace).unwrap();

    assert_eq!(result.command, "emit lowered");
    assert_eq!(
        result.summary,
        format!(
            "emitted 1 lowered snapshot(s) into {}",
            workspace.build_root.join("emit").join("lowered").display()
        )
    );
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(
        result.artifacts[1].kind,
        FrontendArtifactKind::LoweredSnapshot
    );
    assert!(result.artifacts[1].path.as_ref().unwrap().is_file());

    fs::remove_dir_all(root).ok();
}
