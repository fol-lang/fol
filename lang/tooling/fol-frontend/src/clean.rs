use crate::{
    FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult, FrontendConfig,
    FrontendResult, FrontendWorkspace,
};
use std::fs;

pub fn clean_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    remove_dir_if_present(&workspace.build_root)?;
    remove_dir_if_present(&workspace.cache_root)?;
    let git_cache_root = config
        .git_cache_root_override
        .clone()
        .unwrap_or_else(|| workspace.git_cache_root.clone());
    let git_cache_removed = clean_workspace_local_root(workspace, Some(&git_cache_root))?;
    let package_store_root = config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone());
    let package_store_removed =
        clean_workspace_local_root(workspace, package_store_root.as_deref())?;

    let mut result = FrontendCommandResult::new(
        "clean",
        format!(
            "cleaned build root {} and cache root {}{}{}",
            workspace.build_root.display(),
            workspace.cache_root.display(),
            if git_cache_root != workspace.cache_root {
                if git_cache_removed {
                    format!(", and git source cache {}", git_cache_root.display())
                } else {
                    format!(
                        ", skipped external git source cache {}",
                        git_cache_root.display()
                    )
                }
            } else {
                String::new()
            },
            package_store_root
                .as_ref()
                .map(|path| {
                    if package_store_removed {
                        format!(", and package store {}", path.display())
                    } else {
                        format!(", skipped external package store {}", path.display())
                    }
                })
                .unwrap_or_default()
        ),
    );
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
    if git_cache_removed && git_cache_root != workspace.cache_root {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::CacheRoot,
            "git-cache-root",
            Some(git_cache_root),
        ));
    }
    if package_store_removed {
        if let Some(package_store_root) = package_store_root {
            result.artifacts.push(FrontendArtifactSummary::new(
                FrontendArtifactKind::PackageRoot,
                "package-store",
                Some(package_store_root),
            ));
        }
    }
    Ok(result)
}

pub fn clean_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    clean_workspace_with_config(workspace, &FrontendConfig::default())
}

fn remove_dir_if_present(path: &std::path::Path) -> FrontendResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn clean_workspace_local_root(
    workspace: &FrontendWorkspace,
    root: Option<&std::path::Path>,
) -> FrontendResult<bool> {
    let Some(root) = root else {
        return Ok(false);
    };
    let workspace_local_root = workspace.root.root.join(".fol");
    if root.starts_with(&workspace_local_root) {
        remove_dir_if_present(root)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::clean_workspace_with_config;
    use crate::{FrontendConfig, FrontendWorkspace, WorkspaceRoot};
    use std::fs;

    #[test]
    fn clean_workspace_exposes_a_stable_command_shell() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_clean_build_{}", std::process::id()));
        let build_root = root.join(".fol/build");
        let cache_root = root.join(".fol/cache");
        fs::create_dir_all(&build_root).unwrap();
        fs::create_dir_all(&cache_root).unwrap();
        let mut workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.clone()));
        workspace.build_root = build_root.clone();
        workspace.cache_root = cache_root.clone();

        let result = clean_workspace_with_config(&workspace, &FrontendConfig::default()).unwrap();

        assert_eq!(result.command, "clean");
        assert!(result.summary.contains(&build_root.display().to_string()));
        assert!(result.summary.contains(&cache_root.display().to_string()));
        assert!(!build_root.exists());
        assert!(!cache_root.exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn clean_workspace_only_removes_workspace_local_package_stores() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_clean_pkg_{}", std::process::id()));
        let local_store = root.join(".fol/pkg");
        let external_store = std::env::temp_dir().join(format!(
            "fol_frontend_clean_external_pkg_{}",
            std::process::id()
        ));
        fs::create_dir_all(&local_store).unwrap();
        fs::create_dir_all(&external_store).unwrap();

        let mut workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.clone()));
        workspace.build_root = root.join(".fol/build");
        workspace.cache_root = root.join(".fol/cache");
        workspace.package_store_root_override = Some(local_store.clone());

        let local = clean_workspace_with_config(&workspace, &FrontendConfig::default()).unwrap();
        assert!(local.summary.contains("package store"));
        assert!(!local_store.exists());

        fs::create_dir_all(&external_store).unwrap();
        let external = clean_workspace_with_config(
            &workspace,
            &FrontendConfig {
                package_store_root_override: Some(external_store.clone()),
                ..FrontendConfig::default()
            },
        )
        .unwrap();
        assert!(external.summary.contains("skipped external package store"));
        assert!(external_store.exists());

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(external_store).ok();
    }

    #[test]
    fn clean_workspace_removes_workspace_local_git_cache_overrides() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_clean_git_{}", std::process::id()));
        let git_cache = root.join(".fol/custom-git-cache");
        fs::create_dir_all(&git_cache).unwrap();

        let mut workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.clone()));
        workspace.build_root = root.join(".fol/build");
        workspace.cache_root = root.join(".fol/cache");
        workspace.git_cache_root = git_cache.clone();

        let result = clean_workspace_with_config(&workspace, &FrontendConfig::default()).unwrap();

        assert!(result.summary.contains("git source cache"));
        assert!(!git_cache.exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn clean_workspace_skips_external_git_cache_overrides() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_clean_git_external_{}",
            std::process::id()
        ));
        let external_git_cache = std::env::temp_dir().join(format!(
            "fol_frontend_clean_git_shared_{}",
            std::process::id()
        ));
        fs::create_dir_all(&external_git_cache).unwrap();

        let workspace = FrontendWorkspace::new(WorkspaceRoot::new(root.clone()));
        let result = clean_workspace_with_config(
            &workspace,
            &FrontendConfig {
                git_cache_root_override: Some(external_git_cache.clone()),
                ..FrontendConfig::default()
            },
        )
        .unwrap();

        assert!(result.summary.contains("skipped external git source cache"));
        assert!(external_git_cache.exists());

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(external_git_cache).ok();
    }
}
