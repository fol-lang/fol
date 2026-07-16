use fol_frontend::{
    load_workspace_config, work_info, work_list, FrontendWorkspace, FrontendWorkspaceConfig,
    WorkspaceRoot,
};
use std::fs;
use std::path::PathBuf;

fn package_build(name: &str) -> String {
    format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
            "    return;\n",
            "}};\n",
        ),
        name = name
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_workspace_model_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

#[test]
fn workspace_config_and_model_round_trip_through_public_api() {
    let root = temp_root("round_trip");
    let app = root.join("app");
    let lib = root.join("lib");
    fs::create_dir_all(&app).expect("should create app member");
    fs::create_dir_all(&lib).expect("should create lib member");
    fs::write(app.join("build.fol"), package_build("app")).expect("should write app build");
    fs::write(lib.join("build.fol"), package_build("lib")).expect("should write lib build");
    fs::write(
        root.join("fol.work.yaml"),
        concat!(
            "members:\n",
            "  - app\n",
            "  - lib\n",
            "std_root: std\n",
            "package_store_root: .fol/pkg\n",
            "build_root: .artifacts/build\n",
            "cache_root: .artifacts/cache\n",
        ),
    )
    .expect("should write workspace config");

    let root_model = WorkspaceRoot::new(root.clone());
    let config = load_workspace_config(&root_model).expect("config should load");
    let workspace =
        FrontendWorkspace::from_config(root_model, &config).expect("workspace should build");

    assert_eq!(workspace.members.len(), 2);
    assert_eq!(workspace.std_root_override, Some(root.join("std")));
    assert_eq!(
        workspace.package_store_root_override,
        Some(root.join(".fol/pkg"))
    );
    assert_eq!(workspace.build_root, root.join(".artifacts/build"));
    assert_eq!(workspace.cache_root, root.join(".artifacts/cache"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn workspace_info_and_list_summaries_are_stable_through_public_api() {
    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(PathBuf::from("/tmp/demo")),
        members: vec![
            fol_frontend::PackageRoot::new(PathBuf::from("/tmp/demo/app")),
            fol_frontend::PackageRoot::new(PathBuf::from("/tmp/demo/lib")),
        ],
        std_root_override: Some(PathBuf::from("/tmp/demo/std")),
        package_store_root_override: Some(PathBuf::from("/tmp/demo/.fol/pkg")),
        build_root: PathBuf::from("/tmp/demo/.fol/build"),
        cache_root: PathBuf::from("/tmp/demo/.fol/cache"),
        git_cache_root: PathBuf::from("/tmp/demo/.fol/cache/git"),
        install_prefix: PathBuf::from("/tmp/demo/.fol/install"),
    };

    let info = work_info(&workspace);
    let list = work_list(&workspace);

    assert!(info.summary.contains("root=/tmp/demo"));
    assert!(info.summary.contains("members=2"));
    assert!(list.summary.contains("/tmp/demo/app"));
    assert!(list.summary.contains("/tmp/demo/lib"));
    assert_eq!(list.artifacts.len(), 2);
}

#[test]
fn workspace_model_defaults_are_stable_without_loaded_config() {
    let workspace = FrontendWorkspace::from_config(
        WorkspaceRoot::new(PathBuf::from("/tmp/demo")),
        &FrontendWorkspaceConfig::default(),
    )
    .expect("empty config should build an empty workspace");

    assert!(workspace.members.is_empty());
    assert_eq!(workspace.build_root, PathBuf::from("/tmp/demo/.fol/build"));
    assert_eq!(workspace.cache_root, PathBuf::from("/tmp/demo/.fol/cache"));
    assert_eq!(
        workspace.git_cache_root,
        PathBuf::from("/tmp/demo/.fol/cache/git")
    );
}
