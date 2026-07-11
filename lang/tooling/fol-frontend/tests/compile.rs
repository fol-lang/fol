use fol_frontend::{
    build_workspace, build_workspace_with_config, check_workspace, check_workspace_with_config,
    emit_lowered, emit_rust, run_command_from_args_in_dir, run_workspace,
    run_workspace_with_config, test_workspace, test_workspace_with_config, FrontendArtifactKind,
    FrontendConfig, FrontendWorkspace, PackageRoot, WorkspaceRoot,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn semantic_bin_build() -> &'static str {
    concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
        "    graph.install(app);\n",
        "    graph.add_run(app);\n",
        "    graph.add_test({ name = \"app_test\", root = \"src/main.fol\" });\n",
        "};\n",
    )
}

fn semantic_lib_build(name: &str) -> String {
    format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
            "    var graph = build.graph();\n",
            "    var lib = graph.add_static_lib({{ name = \"{name}\", root = \"src/lib.fol\" }});\n",
            "    graph.install(lib);\n",
            "}};\n",
        ),
        name = name
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_compile_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

fn host_machine_target() -> String {
    FrontendConfig::host_rust_target_triple()
        .expect("host target triple should be known")
        .to_string()
}

fn non_host_machine_target() -> String {
    if FrontendConfig::host_rust_target_triple() == Some("aarch64-apple-darwin") {
        "x86_64-unknown-linux-gnu".to_string()
    } else {
        "aarch64-apple-darwin".to_string()
    }
}

fn sample_workspace(root: &PathBuf) -> FrontendWorkspace {
    let app = root.join("app");
    let src = app.join("src");
    fs::create_dir_all(&src).expect("should create source tree");
    fs::write(app.join("build.fol"), semantic_bin_build()).expect("should write build file");
    fs::write(
        src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .expect("should write main");

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
fn check_command_round_trips_workspace_members_through_public_api() {
    let root = temp_root("check");
    let workspace = sample_workspace(&root);

    let result = check_workspace(&workspace).expect("check should succeed");

    assert_eq!(result.command, "check");
    assert_eq!(result.artifacts.len(), 1);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::PackageRoot);

    fs::remove_dir_all(root).ok();
}

#[test]
fn locked_check_build_run_and_test_use_existing_lockfile() {
    let root = temp_root("locked_workflow");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep(&app, &remote);
    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app)],
        std_root_override: None,
        package_store_root_override: Some(root.join(".fol/pkg")),
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    fol_frontend::fetch_workspace(&workspace).expect("initial fetch should succeed");
    let locked = FrontendConfig {
        locked_fetch: true,
        ..FrontendConfig::default()
    };

    assert!(check_workspace_with_config(&workspace, &locked).is_ok());
    assert!(build_workspace_with_config(&workspace, &locked).is_ok());
    assert!(run_workspace_with_config(&workspace, &locked).is_ok());
    assert!(test_workspace_with_config(&workspace, &locked).is_ok());

    fs::remove_dir_all(root).ok();
}

fn create_app_with_git_dep(app: &Path, remote: &Path) {
    fs::create_dir_all(app.join("src")).expect("should create app package");
    fs::write(
        app.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{ alias = \"logtiny\", source = \"git\", target = \"git+file://{}\" }});\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    graph.install(app);\n",
                "    graph.add_run(app);\n",
                "    graph.add_test({{ name = \"app_test\", root = \"src/main.fol\" }});\n",
                "}};\n",
            ),
            remote.display()
        ),
    )
    .expect("should write app build");
    fs::write(
        app.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .expect("should write app source");
}

fn create_git_package_repo(root: &Path, name: &str, version: &str) {
    fs::create_dir_all(root.join("src")).expect("package repo should be creatable");
    fs::write(
        root.join("build.fol"),
        semantic_lib_build(name).replace("0.1.0", version),
    )
    .expect("package build should be writable");
    fs::write(root.join("src/lib.fol"), "var[exp] level: int = 1;\n")
        .expect("package source should be writable");
    git(root, &["init"]);
    git(root, &["config", "user.name", "FOL"]);
    git(root, &["config", "user.email", "fol@example.com"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "init"]);
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git {:?} should succeed", args);
}

#[test]
fn build_command_reports_emitted_crate_and_binary_through_public_api() {
    let root = temp_root("build");
    let workspace = sample_workspace(&root);

    let result = build_workspace_with_config(
        &workspace,
        &FrontendConfig {
            keep_build_dir: true,
            ..FrontendConfig::default()
        },
    )
    .expect("build should succeed");

    assert_eq!(result.command, "build");
    assert_eq!(result.artifacts.len(), 3);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(result.artifacts[1].kind, FrontendArtifactKind::EmittedRust);
    assert_eq!(result.artifacts[2].kind, FrontendArtifactKind::Binary);
    assert!(result.summary.contains("install_prefix="));
    assert!(result.summary.contains("outputs=2"));
    assert!(result.artifacts[2]
        .path
        .as_ref()
        .expect("binary path should exist")
        .is_file());

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_command_scopes_binary_outputs_by_selected_target() {
    let root = temp_root("target_layout");
    let workspace = sample_workspace(&root);
    let config = FrontendConfig {
        build_target_override: Some(host_machine_target()),
        ..FrontendConfig::default()
    };

    let result = build_workspace_with_config(&workspace, &config).expect("build should succeed");
    let binary = result
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .and_then(|artifact| artifact.path.as_ref())
        .expect("binary path should exist");

    assert!(
        binary
            .display()
            .to_string()
            .contains(&format!("/bin/{}/", host_machine_target())),
        "{}",
        binary.display()
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_command_summary_surfaces_install_prefix_and_outputs() {
    let root = temp_root("build_summary");
    let workspace = sample_workspace(&root);

    let result = build_workspace_with_config(
        &workspace,
        &FrontendConfig {
            keep_build_dir: true,
            ..FrontendConfig::default()
        },
    )
    .expect("build should succeed");

    assert!(result.summary.contains(&format!(
        "install_prefix={}",
        workspace.install_prefix.display()
    )));
    assert!(result.summary.contains("outputs=2"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn run_command_executes_single_workspace_members_through_public_api() {
    let root = temp_root("run");
    let workspace = sample_workspace(&root);

    let result = run_workspace(&workspace).expect("run should succeed");

    assert_eq!(result.command, "run");
    assert_eq!(result.artifacts.len(), 1);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::Binary);

    fs::remove_dir_all(root).ok();
}

#[test]
fn run_command_rejects_non_host_targets_through_public_api() {
    let root = temp_root("run_cross_target");
    let workspace = sample_workspace(&root);
    let config = FrontendConfig {
        build_target_override: Some(non_host_machine_target()),
        ..FrontendConfig::default()
    };

    let error = run_workspace_with_config(&workspace, &config).unwrap_err();

    assert!(error
        .to_string()
        .contains("run command cannot execute target"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn test_command_traverses_all_runnable_workspace_members_through_public_api() {
    let root = temp_root("test_workspace");
    let app = sample_workspace(&root);
    let tools_root = root.join("tools");
    let tools_src = tools_root.join("src");
    fs::create_dir_all(&tools_src).expect("should create tools source tree");
    fs::write(
        tools_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"tools\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"tools\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    graph.add_run(app);\n",
            "    graph.add_test({ name = \"tools_test\", root = \"src/main.fol\" });\n",
            "};\n",
        ),
    )
    .expect("should write tools build");
    fs::write(
        tools_src.join("main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .expect("should write tools main");

    let workspace = FrontendWorkspace {
        members: vec![app.members[0].clone(), PackageRoot::new(tools_root)],
        ..app
    };

    let result = test_workspace(&workspace).expect("workspace test should succeed");

    assert_eq!(result.command, "test");
    assert_eq!(
        result.summary,
        "tested 2 workspace package(s) (capability_mode=memo, bundled_std=0/1)"
    );
    assert_eq!(result.artifacts.len(), 2);
    assert!(result
        .artifacts
        .iter()
        .all(|artifact| artifact.kind == FrontendArtifactKind::Binary));

    fs::remove_dir_all(root).ok();
}

#[test]
fn test_command_rejects_non_host_targets_through_public_api() {
    let root = temp_root("test_cross_target");
    let workspace = sample_workspace(&root);
    let config = FrontendConfig {
        build_target_override: Some(non_host_machine_target()),
        ..FrontendConfig::default()
    };

    let error = test_workspace_with_config(&workspace, &config).unwrap_err();

    assert!(error
        .to_string()
        .contains("test command cannot execute target"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn emit_rust_command_reports_generated_crate_paths_through_public_api() {
    let root = temp_root("emit_rust");
    let workspace = sample_workspace(&root);

    let result = emit_rust(&workspace).expect("emit rust should succeed");

    assert_eq!(result.command, "emit rust");
    assert_eq!(result.artifacts.len(), 2);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(result.artifacts[1].kind, FrontendArtifactKind::EmittedRust);
    assert!(result.artifacts[1]
        .path
        .as_ref()
        .expect("crate path should exist")
        .is_dir());

    fs::remove_dir_all(root).ok();
}

#[test]
fn emit_lowered_command_reports_snapshot_paths_through_public_api() {
    let root = temp_root("emit_lowered");
    let workspace = sample_workspace(&root);

    let result = emit_lowered(&workspace).expect("emit lowered should succeed");

    assert_eq!(result.command, "emit lowered");
    assert_eq!(result.artifacts.len(), 2);
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    assert_eq!(
        result.artifacts[1].kind,
        FrontendArtifactKind::LoweredSnapshot
    );
    assert!(result.artifacts[1]
        .path
        .as_ref()
        .expect("snapshot path should exist")
        .is_file());

    fs::remove_dir_all(root).ok();
}

#[test]
fn direct_file_or_folder_compilation_is_code_subcommand_owned() {
    let root = temp_root("direct_compile");
    let workspace = sample_workspace(&root);
    let entry_file = workspace.members[0].root.join("src/main.fol");

    let (_, built) = run_command_from_args_in_dir(
        [
            "fol",
            "code",
            "build",
            entry_file.to_string_lossy().as_ref(),
        ],
        &root,
    )
    .expect("direct package compile should succeed");
    let (_, emitted) = run_command_from_args_in_dir(
        [
            "fol",
            "code",
            "emit",
            "rust",
            entry_file.to_string_lossy().as_ref(),
        ],
        &root,
    )
    .expect("direct emit rust should succeed");

    assert_eq!(built.command, "build");
    assert!(built
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == FrontendArtifactKind::Binary));
    assert_eq!(emitted.command, "emit rust");
    assert!(emitted
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == FrontendArtifactKind::EmittedRust));

    fs::remove_dir_all(root).ok();
}
