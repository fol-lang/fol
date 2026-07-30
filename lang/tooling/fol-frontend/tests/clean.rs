use fol_frontend::{clean_workspace_with_config, FrontendConfig, FrontendWorkspace, WorkspaceRoot};
use std::fs;

fn temp_root(label: &str) -> fol_testkit::TempFixture {
    fol_testkit::TempFixture::new(&format!("fol_frontend_clean_{label}"))
}

#[test]
fn clean_command_removes_workspace_build_and_cache_roots_through_public_api() {
    let root = temp_root("roots");
    let build_root = root.join(".fol/build");
    let cache_root = root.join(".fol/cache");
    fs::create_dir_all(&build_root).expect("should create build root");
    fs::create_dir_all(&cache_root).expect("should create cache root");

    let mut workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.to_path_buf()));
    workspace.build_root = build_root.clone();
    workspace.cache_root = cache_root.clone();

    let result = clean_workspace_with_config(&workspace, &FrontendConfig::default())
        .expect("clean should succeed");

    assert_eq!(result.command, "clean");
    assert!(!build_root.exists());
    assert!(!cache_root.exists());

    fs::remove_dir_all(root).ok();
}

#[test]
fn clean_command_skips_external_package_store_roots_through_public_api() {
    let root = temp_root("external_store");
    let external_store = temp_root("pkg_store");
    fs::create_dir_all(&external_store).expect("should create external package store");

    let workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.to_path_buf()));
    let result = clean_workspace_with_config(
        &workspace,
        &FrontendConfig {
            package_store_root_override: Some(external_store.to_path_buf()),
            ..FrontendConfig::default()
        },
    )
    .expect("clean should succeed");

    assert!(result.summary.contains("skipped external package store"));
    assert!(external_store.exists());

    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(external_store).ok();
}

#[test]
fn clean_command_handles_git_cache_boundaries_through_public_api() {
    let root = temp_root("git_cache");
    let local_git_cache = root.join(".fol/custom-git");
    let external_git_cache = temp_root("shared_git");
    fs::create_dir_all(&local_git_cache).expect("should create local git cache");
    fs::create_dir_all(&external_git_cache).expect("should create external git cache");

    let mut workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.to_path_buf()));
    workspace.git_cache_root = local_git_cache.clone();

    let local = clean_workspace_with_config(&workspace, &FrontendConfig::default())
        .expect("clean should succeed");
    assert!(local.summary.contains("git source cache"));
    assert!(!local_git_cache.exists());

    let external = clean_workspace_with_config(
        &workspace,
        &FrontendConfig {
            git_cache_root_override: Some(external_git_cache.to_path_buf()),
            ..FrontendConfig::default()
        },
    )
    .expect("clean should succeed");
    assert!(external
        .summary
        .contains("skipped external git source cache"));
    assert!(external_git_cache.exists());

    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(external_git_cache).ok();
}
