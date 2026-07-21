use crate::{
    select_package_store_root, FrontendArtifactKind, FrontendArtifactSummary,
    FrontendCommandResult, FrontendConfig, FrontendError, FrontendErrorKind, FrontendResult,
    FrontendWorkspace,
};
use fol_build::{evaluate_build_source, BuildEvaluationInputs, BuildEvaluationRequest};
use fol_package::available_bundled_std_root;
use fol_package::build_artifact::BuildArtifactFolModel;
pub fn work_info(workspace: &FrontendWorkspace) -> FrontendCommandResult {
    let mut summary = workspace.info_summary_lines();
    if let Some(distribution) = artifact_model_distribution_line(workspace) {
        summary.push(distribution);
    }
    if let Some(distribution) = bundled_std_distribution_line(workspace) {
        summary.push(distribution);
    }
    let mut result = FrontendCommandResult::new("work info", summary.join("\n"));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::WorkspaceRoot,
        "workspace-root",
        Some(workspace.root.root.clone()),
    ));
    result
}

pub fn work_list(workspace: &FrontendWorkspace) -> FrontendCommandResult {
    let mut result = FrontendCommandResult::new(
        "work list",
        workspace
            .members
            .iter()
            .map(|member| member.root.display().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    for member in &workspace.members {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            member
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package"),
            Some(member.root.clone()),
        ));
    }
    result
}

pub fn work_deps(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    let mut lines = Vec::new();
    let mut result = FrontendCommandResult::new("work deps", "");

    for member in &workspace.members {
        let metadata =
            fol_package::parse_package_metadata_from_build(&member.root.join("build.fol"))
                .map_err(FrontendError::from)?;
        if metadata.dependencies.is_empty() {
            lines.push(format!("{}:", metadata.name));
            lines.push("  (no dependencies)".to_string());
        } else {
            lines.push(format!("{}:", metadata.name));
            for dependency in &metadata.dependencies {
                lines.push(format!(
                    "  {} [{}] -> {}",
                    dependency.alias,
                    dependency_kind_label(dependency.source_kind),
                    dependency.target
                ));
            }
        }
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            metadata.name,
            Some(member.root.clone()),
        ));
    }

    result.summary = lines.join("\n");
    Ok(result)
}

pub fn work_status(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let lockfile_path = workspace.root.root.join("fol.lock");
    let lockfile = if lockfile_path.is_file() {
        Some(
            fol_package::parse_package_lockfile(&std::fs::read_to_string(&lockfile_path).map_err(
                |error| {
                    FrontendError::new(
                        FrontendErrorKind::CommandFailed,
                        format!(
                            "could not read lockfile '{}': {}",
                            lockfile_path.display(),
                            error
                        ),
                    )
                },
            )?)
            .map_err(FrontendError::from)?,
        )
    } else {
        None
    };
    let package_store_root = select_package_store_root(config, workspace);
    let git_store_root = package_store_root.join("git");

    let mut summary = vec![
        format!("workspace-root={}", workspace.root.root.display()),
        format!("members={}", workspace.members.len()),
        format!(
            "lockfile={}",
            if lockfile.is_some() {
                "present"
            } else {
                "missing"
            }
        ),
        format!("build-root={}", workspace.build_root.display()),
        format!("cache-root={}", workspace.cache_root.display()),
        format!("git-cache-root={}", workspace.git_cache_root.display()),
        format!("package-store-root={}", package_store_root.display()),
        format!("git-store-root={}", git_store_root.display()),
    ];
    if let Some(std_root) = &workspace.std_root_override {
        summary.push(format!("std-root=override:{}", std_root.display()));
    } else if let Some(std_root) = available_bundled_std_root() {
        summary.push(format!("std-root=bundled:{}", std_root.display()));
    }

    if let Some(lockfile) = &lockfile {
        summary.push(format!(
            "locked-git-dependencies={}",
            lockfile.entries.len()
        ));
        for entry in &lockfile.entries {
            summary.push(format!(
                "  {} [{}] @ {}",
                entry.alias,
                entry.locator,
                shorten_revision(&entry.selected_revision)
            ));
        }
    }

    let mut result = FrontendCommandResult::new("work status", summary.join("\n"));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::WorkspaceRoot,
        "workspace-root",
        Some(workspace.root.root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::BuildRoot,
        "build-root",
        Some(workspace.build_root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::CacheRoot,
        "cache-root",
        Some(workspace.cache_root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::CacheRoot,
        "git-cache-root",
        Some(workspace.git_cache_root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::PackageRoot,
        "package-store-root",
        Some(package_store_root),
    ));
    if lockfile_path.is_file() {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            "lockfile",
            Some(lockfile_path),
        ));
    }
    Ok(result)
}

fn dependency_kind_label(kind: fol_package::PackageDependencySourceKind) -> &'static str {
    match kind {
        fol_package::PackageDependencySourceKind::Local => "loc",
        fol_package::PackageDependencySourceKind::PackageStore => "pkg",
        fol_package::PackageDependencySourceKind::Git => "git",
        fol_package::PackageDependencySourceKind::Internal => "internal",
    }
}

fn shorten_revision(revision: &str) -> String {
    revision.chars().take(12).collect()
}

fn artifact_model_distribution_line(workspace: &FrontendWorkspace) -> Option<String> {
    let mut core = 0usize;
    let mut memo = 0usize;

    for member in &workspace.members {
        let build_path = member.root.join("build.fol");
        let source = std::fs::read_to_string(&build_path).ok()?;
        let evaluated = evaluate_build_source(
            &BuildEvaluationRequest {
                package_root: member.root.display().to_string(),
                inputs: BuildEvaluationInputs {
                    working_directory: member.root.display().to_string(),
                    ..BuildEvaluationInputs::default()
                },
                operations: Vec::new(),
            },
            &build_path,
            &source,
        )
        .ok()??;

        for artifact in &evaluated.evaluated.artifacts {
            match artifact.fol_model {
                BuildArtifactFolModel::Core => core += 1,
                BuildArtifactFolModel::Memo => memo += 1,
            }
        }
    }

    Some(format!("artifact_models=core={core},memo={memo}"))
}

fn bundled_std_distribution_line(workspace: &FrontendWorkspace) -> Option<String> {
    let mut declared = 0usize;

    for member in &workspace.members {
        let metadata =
            fol_package::parse_package_metadata_from_build(&member.root.join("build.fol")).ok()?;
        if metadata.dependencies.iter().any(|dependency| {
            dependency.source_kind == fol_package::PackageDependencySourceKind::Internal
                && dependency.target == "standard"
        }) {
            declared += 1;
        }
    }

    Some(format!(
        "bundled_std_members={declared}/{}",
        workspace.members.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::{work_deps, work_info, work_list, work_status};
    use crate::{FrontendConfig, FrontendWorkspace, PackageRoot, WorkspaceRoot};
    use std::{fs, path::PathBuf};

    #[test]
    fn work_info_returns_workspace_summary_as_command_result() {
        let workspace = FrontendWorkspace::new(WorkspaceRoot::new(PathBuf::from("/tmp/demo")));
        let result = work_info(&workspace);

        assert_eq!(result.command, "work info");
        assert!(result.summary.contains("root=/tmp/demo"));
        assert!(result.summary.contains("std_root=bundled:"));
        assert_eq!(result.artifacts.len(), 1);
    }

    #[test]
    fn work_info_surfaces_artifact_model_distribution_for_valid_members() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_work_info_models_{}",
            std::process::id()
        ));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"tool\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.add_static_lib({ name = \"corelib\", root = \"src/core.fol\", fol_model = \"core\" });\n",
                "    graph.add_static_lib({ name = \"memolib\", root = \"src/memo.fol\", fol_model = \"memo\" });\n",
                "    return;\n",
                "};\n",
            ),
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

        let result = work_info(&workspace);

        assert!(result.summary.contains("artifact_models=core=1,memo=2"));
        assert!(result.summary.contains("bundled_std_members=1/1"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn work_list_returns_member_packages_as_command_result() {
        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(PathBuf::from("/tmp/demo")),
            members: vec![
                PackageRoot::new(PathBuf::from("/tmp/demo/app")),
                PackageRoot::new(PathBuf::from("/tmp/demo/lib")),
            ],
            std_root_override: None,
            package_store_root_override: None,
            build_root: PathBuf::from("/tmp/demo/.fol/build"),
            cache_root: PathBuf::from("/tmp/demo/.fol/cache"),
            git_cache_root: PathBuf::from("/tmp/demo/.fol/cache/git"),
            install_prefix: PathBuf::from("/tmp/demo/.fol/install"),
        };
        let result = work_list(&workspace);

        assert_eq!(result.command, "work list");
        assert!(result.summary.contains("/tmp/demo/app"));
        assert!(result.summary.contains("/tmp/demo/lib"));
        assert_eq!(result.artifacts.len(), 2);
        assert_eq!(
            result.artifacts[0].kind,
            crate::FrontendArtifactKind::PackageRoot
        );
    }

    #[test]
    fn work_deps_reports_member_dependency_graphs() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_work_deps_{}", std::process::id()));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"shared\", source = \"loc\", target = \"../shared\" });\n",
                "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://github.com/bresilla/logtiny\" });\n",
                "    return;\n",
                "};\n",
            ),
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

        let result = work_deps(&workspace).unwrap();

        assert_eq!(result.command, "work deps");
        assert!(result.summary.contains("app:"));
        assert!(result.summary.contains("shared [loc] -> ../shared"));
        assert!(result
            .summary
            .contains("logtiny [git] -> git+https://github.com/bresilla/logtiny"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn work_status_reports_lockfile_and_roots() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_work_status_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("fol.lock"),
            "version: 1\n- alias: logtiny\n  source: git\n  locator: git+https://github.com/bresilla/logtiny\n  revision: abcdef1234567890\n  root: /tmp/demo/.fol/pkg/git/logtiny/rev-abcdef1234567890\n",
        )
        .unwrap();
        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.join("app"))],
            std_root_override: None,
            package_store_root_override: Some(root.join(".fol/pkg")),
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        };

        let result = work_status(&workspace, &FrontendConfig::default()).unwrap();

        assert_eq!(result.command, "work status");
        assert!(result.summary.contains("lockfile=present"));
        assert!(result.summary.contains("locked-git-dependencies=1"));
        assert!(result.summary.contains("std-root=bundled:"));
        assert!(result
            .summary
            .contains("logtiny [git+https://github.com/bresilla/logtiny] @ abcdef123456"));

        fs::remove_dir_all(root).ok();
    }
}
