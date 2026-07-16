use fol_frontend::{
    fetch_workspace, fetch_workspace_with_config, prepare_workspace_packages,
    update_workspace_with_config, FrontendConfig, FrontendWorkspace, PackageRoot, WorkspaceRoot,
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
        "fol_frontend_fetch_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

#[test]
fn fetch_round_trip_prepares_and_reports_local_workspace_packages() {
    let root = temp_root("round_trip");
    let app = root.join("app");
    let lib = root.join("lib");
    fs::create_dir_all(&app).expect("should create app package");
    fs::create_dir_all(&lib).expect("should create lib package");
    fs::write(app.join("build.fol"), semantic_bin_build()).expect("should write app build");
    fs::write(lib.join("build.fol"), semantic_lib_build("lib")).expect("should write lib build");

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app.clone()), PackageRoot::new(lib.clone())],
        std_root_override: Some(root.join("std")),
        package_store_root_override: Some(root.join(".fol/pkg")),
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let preparation = prepare_workspace_packages(&workspace).expect("preparation should succeed");
    let result = fetch_workspace(&workspace).expect("fetch should succeed");

    assert_eq!(preparation.packages.len(), 2);
    assert!(result.summary.contains("prepared 2 workspace package(s)"));
    assert_eq!(result.artifacts.len(), 6);
    assert!(result
        .artifacts
        .iter()
        .any(|artifact| artifact.path == Some(app.clone())));
    assert!(result
        .artifacts
        .iter()
        .any(|artifact| artifact.path == Some(lib.clone())));

    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_summary_surfaces_dependency_modes_for_mixed_sources() {
    let root = temp_root("mode_summary");
    let app = root.join("app");
    let shared = root.join("deps/shared");
    let store_root = root.join(".fol/pkg");
    let remote = root.join("remote-logtiny");

    fs::create_dir_all(app.join("src")).expect("should create app package");
    fs::create_dir_all(shared.join("src")).expect("should create local dependency");
    fs::create_dir_all(store_root.join("core/src")).expect("should create store dependency");
    create_git_package_repo(&remote, "logtiny", "0.1.0");

    fs::write(
        app.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{ alias = \"shared\", source = \"loc\", target = \"../deps/shared\", mode = \"lazy\" }});\n",
                "    build.add_dep({{ alias = \"core\", source = \"pkg\", target = \"core\", mode = \"eager\" }});\n",
                "    build.add_dep({{ alias = \"logtiny\", source = \"git\", target = \"git+file://{}\", mode = \"on-demand\" }});\n",
                "    var graph = build.graph();\n",
                "    var exe = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    graph.install(exe);\n",
                "}};\n",
            ),
            remote.display()
        ),
    )
    .expect("should write app build");
    fs::write(
        app.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");
    fs::write(shared.join("build.fol"), semantic_lib_build("shared"))
        .expect("should write local dep build");
    fs::write(shared.join("src/lib.fol"), "var[exp] level: int = 1;\n")
        .expect("should write local dep source");
    fs::write(
        store_root.join("core/build.fol"),
        semantic_lib_build("core"),
    )
    .expect("should write store dep build");
    fs::write(
        store_root.join("core/src/lib.fol"),
        "var[exp] base: int = 2;\n",
    )
    .expect("should write store dep source");

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app.clone())],
        std_root_override: None,
        package_store_root_override: Some(store_root.clone()),
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };

    let result = fetch_workspace(&workspace).expect("fetch should succeed");

    assert!(result
        .summary
        .contains("dependency modes: eager=1, lazy=1, on-demand=1"));
    assert!(result
        .artifacts
        .iter()
        .any(|artifact| artifact.label == "shared:loc:lazy"));
    assert!(result
        .artifacts
        .iter()
        .any(|artifact| artifact.label == "core:pkg:eager"));
    assert!(result
        .artifacts
        .iter()
        .any(|artifact| artifact.label == "logtiny:git:on-demand"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_round_trip_prefers_frontend_config_store_root_in_artifacts() {
    let root = temp_root("config_store");
    let app = root.join("app");
    fs::create_dir_all(&app).expect("should create app package");
    fs::write(app.join("build.fol"), semantic_bin_build()).expect("should write app build");

    let workspace = FrontendWorkspace {
        root: WorkspaceRoot::new(root.clone()),
        members: vec![PackageRoot::new(app.clone())],
        std_root_override: None,
        package_store_root_override: Some(root.join(".fol/ws-pkg")),
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    };
    let config = FrontendConfig {
        package_store_root_override: Some(root.join(".fol/config-pkg")),
        ..FrontendConfig::default()
    };

    let result = fetch_workspace_with_config(&workspace, &config).expect("fetch should succeed");

    assert_eq!(result.artifacts[0].path, Some(root.join(".fol/config-pkg")));
    assert_eq!(result.artifacts[1].path, Some(root.join(".fol/cache")));
    assert_eq!(result.artifacts[4].path, Some(app));

    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_locked_requires_existing_lockfile() {
    let root = temp_root("locked_missing");
    let app = root.join("app");
    fs::create_dir_all(&app).expect("should create app package");
    fs::write(app.join("build.fol"), semantic_bin_build()).expect("should write app build");

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
    let config = FrontendConfig {
        locked_fetch: true,
        ..FrontendConfig::default()
    };

    let error = fetch_workspace_with_config(&workspace, &config)
        .expect_err("locked fetch should require fol.lock");

    assert!(error
        .message()
        .contains("locked fetch requires an existing fol.lock"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_offline_uses_warm_git_cache() {
    let root = temp_root("offline_warm");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep(&app, &remote);

    let workspace = git_dep_workspace(&root, &app);
    fetch_workspace(&workspace).expect("initial fetch should warm the cache");

    let config = FrontendConfig {
        offline_fetch: true,
        ..FrontendConfig::default()
    };
    let result = fetch_workspace_with_config(&workspace, &config)
        .expect("offline fetch should use warm cache");

    assert_eq!(result.command, "fetch");
    assert!(root.join("fol.lock").is_file());
    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_offline_without_a_warm_cache_adds_guidance_notes() {
    let root = temp_root("offline_cold");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep(&app, &remote);

    let workspace = git_dep_workspace(&root, &app);
    let error = fetch_workspace_with_config(
        &workspace,
        &FrontendConfig {
            offline_fetch: true,
            ..FrontendConfig::default()
        },
    )
    .expect_err("offline fetch should fail without a warm cache");

    assert!(error
        .notes()
        .iter()
        .any(|note| note.contains("offline mode only works")));
    fs::remove_dir_all(root).ok();
}

#[test]
fn update_workspace_refreshes_git_dependencies_through_public_api() {
    let root = temp_root("update");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep_spec(&app, &remote, Some("branch:main"), None, None);
    let workspace = git_dep_workspace(&root, &app);

    fetch_workspace(&workspace).expect("initial fetch should succeed");
    let before = fs::read_to_string(root.join("fol.lock")).expect("lockfile should exist");

    fs::write(remote.join("src/lib.fol"), "var[exp] level: int = 2;\n")
        .expect("should update remote source");
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "-m", "bump"]);

    let result = update_workspace_with_config(&workspace, &FrontendConfig::default())
        .expect("update should succeed");
    let after = fs::read_to_string(root.join("fol.lock")).expect("lockfile should still exist");

    assert_eq!(result.command, "update");
    assert_ne!(before, after);
    assert!(result.summary.contains("revisions changed: 1"));
    assert!(result.summary.contains("logtiny:"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn update_prunes_stale_workspace_local_git_materializations() {
    let root = temp_root("update_prune");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep_spec(&app, &remote, Some("branch:main"), None, None);
    let workspace = git_dep_workspace(&root, &app);

    fetch_workspace(&workspace).expect("initial fetch should succeed");
    let before = fol_package::parse_package_lockfile(
        &fs::read_to_string(root.join("fol.lock")).expect("lockfile should exist"),
    )
    .expect("lockfile should parse");
    let old_root = PathBuf::from(before.entries[0].materialized_root.clone());
    assert!(old_root.is_dir());

    fs::write(remote.join("src/lib.fol"), "var[exp] level: int = 3;\n")
        .expect("should update remote source");
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "-m", "bump-again"]);

    let result = update_workspace_with_config(&workspace, &FrontendConfig::default())
        .expect("update should succeed");

    assert!(result
        .summary
        .contains("pruned 1 stale git materialization(s)"));
    assert!(!old_root.exists());
    fs::remove_dir_all(root).ok();
}

#[test]
fn locked_fetch_repairs_missing_pinned_materializations_from_warm_cache() {
    let root = temp_root("locked_repair");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep(&app, &remote);
    let workspace = git_dep_workspace(&root, &app);

    fetch_workspace(&workspace).expect("initial fetch should succeed");
    let lockfile = fol_package::parse_package_lockfile(
        &fs::read_to_string(root.join("fol.lock")).expect("lockfile should exist"),
    )
    .expect("lockfile should parse");
    let materialized_root = PathBuf::from(lockfile.entries[0].materialized_root.clone());
    fs::remove_dir_all(&materialized_root).expect("should remove pinned materialization");

    let result = fetch_workspace_with_config(
        &workspace,
        &FrontendConfig {
            locked_fetch: true,
            offline_fetch: true,
            ..FrontendConfig::default()
        },
    )
    .expect("locked fetch should repair from the warm cache");

    assert!(result
        .summary
        .contains("repaired 1 missing pinned materialization(s)"));
    assert!(materialized_root.is_dir());
    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_supports_git_dependency_version_and_hash_fields() {
    let root = temp_root("git_versions");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    let revision = git_output(&remote, &["rev-parse", "HEAD"]);
    let short_hash = &revision[..12];

    let cases = vec![
        (Some("branch:main".to_string()), None),
        (Some("tag:v0.1.0".to_string()), None),
        (Some(format!("commit:{revision}")), None),
        (
            Some("branch:main".to_string()),
            Some(short_hash.to_string()),
        ),
        (None, None),
    ];

    for (index, (version, hash)) in cases.iter().enumerate() {
        let case_root = root.join(format!("case-{index}"));
        let case_app = case_root.join("app");
        create_app_with_git_dep_spec(
            &case_app,
            &remote,
            version.as_deref(),
            hash.as_deref(),
            None,
        );
        let workspace = git_dep_workspace(&case_root, &case_app);

        let result = fetch_workspace(&workspace).expect("fetch should succeed");
        assert_eq!(result.command, "fetch");
        let lockfile = fol_package::parse_package_lockfile(
            &fs::read_to_string(case_root.join("fol.lock")).expect("lockfile should exist"),
        )
        .expect("lockfile should parse");
        assert_eq!(lockfile.entries.len(), 1);
        if let Some(version) = version {
            assert!(
                lockfile.entries[0].locator.contains(version),
                "lockfile locator should keep requested version selector"
            );
        }
        if let Some(hash) = hash {
            assert!(
                lockfile.entries[0].locator.contains(hash),
                "lockfile locator should keep requested hash"
            );
        }
    }

    fs::remove_dir_all(root).ok();
}

#[test]
fn fetch_reports_hash_mismatch_for_structured_git_dependencies() {
    let root = temp_root("git_hash_mismatch");
    let app = root.join("app");
    let remote = root.join("remote-logtiny");
    create_git_package_repo(&remote, "logtiny", "0.1.0");
    create_app_with_git_dep_spec(&app, &remote, Some("branch:main"), Some("deadbeef"), None);

    let workspace = git_dep_workspace(&root, &app);
    let error = fetch_workspace(&workspace).expect_err("hash mismatch should fail");

    assert!(
        error
            .message()
            .contains("does not match required hash 'deadbeef'"),
        "hash mismatch should keep the required hash in the diagnostic",
    );
    fs::remove_dir_all(root).ok();
}

fn git_dep_workspace(root: &Path, app: &Path) -> FrontendWorkspace {
    FrontendWorkspace {
        root: WorkspaceRoot::new(root.to_path_buf()),
        members: vec![PackageRoot::new(app.to_path_buf())],
        std_root_override: None,
        package_store_root_override: Some(root.join(".fol/pkg")),
        build_root: root.join(".fol/build"),
        cache_root: root.join(".fol/cache"),
        git_cache_root: root.join(".fol/cache/git"),
        install_prefix: root.join(".fol/install"),
    }
}

fn create_app_with_git_dep(app: &Path, remote: &Path) {
    create_app_with_git_dep_spec(app, remote, None, None, None);
}

fn create_app_with_git_dep_spec(
    app: &Path,
    remote: &Path,
    version: Option<&str>,
    hash: Option<&str>,
    mode: Option<&str>,
) {
    fs::create_dir_all(app.join("src")).expect("should create app package");
    let version_field = version
        .map(|value| format!("        version = \"{value}\",\n"))
        .unwrap_or_default();
    let hash_field = hash
        .map(|value| format!("        hash = \"{value}\",\n"))
        .unwrap_or_default();
    let mode_field = mode
        .map(|value| format!(", mode = \"{value}\""))
        .unwrap_or_default();
    fs::write(
        app.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{\n",
                "        alias = \"logtiny\",\n",
                "        source = \"git\",\n",
                "        target = \"git+file://{}\"{mode_field},\n",
                "{version_field}",
                "{hash_field}",
                "    }});\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    graph.install(app);\n",
                "    graph.add_run(app);\n",
                "}};\n",
            ),
            remote.display(),
            mode_field = mode_field,
            version_field = version_field,
            hash_field = hash_field,
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
    git(root, &["init", "--initial-branch=main"]);
    git(root, &["config", "user.name", "FOL"]);
    git(root, &["config", "user.email", "fol@example.com"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "init"]);
    git(root, &["tag", &format!("v{version}")]);
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git {:?} should succeed", args);
}

fn git_output(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git command should run");
    assert!(output.status.success(), "git {:?} should succeed", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
