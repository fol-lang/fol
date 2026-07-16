use crate::{
    FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult, FrontendConfig,
    FrontendError, FrontendResult, FrontendWorkspace,
};
use fol_package::{DependencyBuildEvaluationMode, PackageDependencySourceKind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendPreparedPackage {
    pub root: PathBuf,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendPackagePreparation {
    pub package_config: fol_package::PackageConfig,
    pub packages: Vec<FrontendPreparedPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedDependencyPackage {
    alias: String,
    root: PathBuf,
    name: String,
    version: String,
    source_kind: PackageDependencySourceKind,
    evaluation_mode: DependencyBuildEvaluationMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchResolution {
    preparation: FrontendPackagePreparation,
    resolved_packages: Vec<ResolvedDependencyPackage>,
    resolved_dependencies: Vec<ResolvedDependencyPackage>,
    lockfile: fol_package::PackageLockfile,
    lockfile_path: PathBuf,
    package_store_root: PathBuf,
    repaired_materializations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockRevisionChange {
    alias: String,
    previous_revision: String,
    next_revision: String,
}

pub fn select_package_store_root(
    config: &FrontendConfig,
    workspace: &FrontendWorkspace,
) -> PathBuf {
    config
        .package_store_root_override
        .clone()
        .or_else(|| workspace.package_store_root_override.clone())
        .unwrap_or_else(|| workspace.root.root.join(".fol").join("pkg"))
}

fn select_git_store_root(config: &FrontendConfig, workspace: &FrontendWorkspace) -> PathBuf {
    select_package_store_root(config, workspace).join("git")
}

fn lockfile_path(workspace: &FrontendWorkspace) -> PathBuf {
    workspace.root.root.join("fol.lock")
}

fn copy_package_projection(from: &Path, to: &Path) -> FrontendResult<()> {
    std::fs::create_dir_all(to).map_err(FrontendError::from)?;
    for entry in std::fs::read_dir(from).map_err(FrontendError::from)? {
        let entry = entry.map_err(FrontendError::from)?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        let file_type = entry.file_type().map_err(FrontendError::from)?;
        if file_type.is_dir() {
            copy_package_projection(&from_path, &to_path)?;
        } else {
            std::fs::copy(&from_path, &to_path).map_err(FrontendError::from)?;
        }
    }
    Ok(())
}

fn render_dependency_mode_summary(dependencies: &[ResolvedDependencyPackage]) -> Option<String> {
    if dependencies.is_empty() {
        return None;
    }
    let eager = dependencies
        .iter()
        .filter(|dep| dep.evaluation_mode == DependencyBuildEvaluationMode::Eager)
        .count();
    let lazy = dependencies
        .iter()
        .filter(|dep| dep.evaluation_mode == DependencyBuildEvaluationMode::Lazy)
        .count();
    let on_demand = dependencies
        .iter()
        .filter(|dep| dep.evaluation_mode == DependencyBuildEvaluationMode::OnDemand)
        .count();
    Some(format!(
        "dependency modes: eager={eager}, lazy={lazy}, on-demand={on_demand}"
    ))
}

fn project_git_dependency_alias(
    package_store_root: &Path,
    alias: &str,
    materialized_root: &Path,
) -> FrontendResult<PathBuf> {
    let alias_root = package_store_root.join(alias);
    if alias_root.is_dir() {
        std::fs::remove_dir_all(&alias_root).map_err(FrontendError::from)?;
    } else if alias_root.exists() {
        std::fs::remove_file(&alias_root).map_err(FrontendError::from)?;
    }
    copy_package_projection(materialized_root, &alias_root)?;
    Ok(alias_root)
}

fn dependency_git_locator(
    dependency: &fol_package::PackageDependencyDecl,
) -> FrontendResult<fol_package::PackageLocator> {
    let base =
        fol_package::parse_package_locator(&dependency.target).map_err(FrontendError::from)?;
    let Some(mut git) = base.git.clone() else {
        return Err(FrontendError::new(
            crate::FrontendErrorKind::InvalidInput,
            format!(
                "git dependency '{}' must use a git repository target, got '{}'",
                dependency.alias, dependency.target
            ),
        ));
    };
    git.selector = match &dependency.git_version {
        Some(fol_package::GitDependencyVersionSelector::Branch(value)) => {
            fol_package::PackageGitSelector {
                branch: Some(value.clone()),
                tag: None,
                rev: None,
                hash: dependency.git_hash.clone(),
            }
        }
        Some(fol_package::GitDependencyVersionSelector::Tag(value)) => {
            fol_package::PackageGitSelector {
                branch: None,
                tag: Some(value.clone()),
                rev: None,
                hash: dependency.git_hash.clone(),
            }
        }
        Some(fol_package::GitDependencyVersionSelector::Commit(value)) => {
            fol_package::PackageGitSelector {
                branch: None,
                tag: None,
                rev: Some(value.clone()),
                hash: dependency.git_hash.clone(),
            }
        }
        None => fol_package::PackageGitSelector {
            branch: None,
            tag: None,
            rev: None,
            hash: dependency.git_hash.clone(),
        },
    };
    Ok(fol_package::PackageLocator::git(
        dependency.git_locator_string(),
        git.transport,
        git.repository,
        git.selector,
    ))
}

pub fn prepare_workspace_packages(
    workspace: &FrontendWorkspace,
) -> FrontendResult<FrontendPackagePreparation> {
    let package_config = fol_package::PackageConfig {
        std_root: workspace
            .std_root_override
            .as_ref()
            .cloned()
            .or_else(fol_package::available_bundled_std_root)
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        package_store_root: workspace
            .package_store_root_override
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        package_cache_root: Some(workspace.cache_root.to_string_lossy().to_string()),
        package_git_cache_root: Some(workspace.git_cache_root.to_string_lossy().to_string()),
    };

    let packages = workspace
        .members
        .iter()
        .map(|member| {
            let build_path = member.root.join("build.fol");
            let metadata = fol_package::parse_package_metadata_from_build(&build_path)
                .map_err(FrontendError::from)?;
            fol_package::parse_package_build(&build_path).map_err(FrontendError::from)?;

            Ok(FrontendPreparedPackage {
                root: member.root.clone(),
                name: metadata.name,
                version: metadata.version,
            })
        })
        .collect::<FrontendResult<Vec<_>>>()?;

    Ok(FrontendPackagePreparation {
        package_config,
        packages,
    })
}

pub fn fetch_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let resolution = resolve_workspace_fetch(workspace, config)
        .map_err(|error| with_fetch_guidance(error, config))?;
    if !config.locked_fetch {
        std::fs::write(
            &resolution.lockfile_path,
            fol_package::render_package_lockfile(&resolution.lockfile),
        )
        .map_err(|error| {
            FrontendError::new(
                crate::FrontendErrorKind::CommandFailed,
                format!(
                    "failed to write lockfile '{}': {}",
                    resolution.lockfile_path.display(),
                    error
                ),
            )
        })?;
    }
    let stale_pruned = prune_stale_git_materializations(workspace, config, &resolution.lockfile)?;

    let mut result = FrontendCommandResult::new(
        "fetch",
        format!(
            "prepared {} workspace package(s) and resolved {} package root(s) into {}{}{}{}",
            resolution.preparation.packages.len(),
            resolution.resolved_packages.len(),
            resolution.package_store_root.display(),
            if resolution.repaired_materializations > 0 {
                format!(
                    "; repaired {} missing pinned materialization(s)",
                    resolution.repaired_materializations
                )
            } else {
                String::new()
            },
            if stale_pruned > 0 {
                format!("; pruned {} stale git materialization(s)", stale_pruned)
            } else {
                String::new()
            },
            resolution
                .resolved_dependencies
                .is_empty()
                .then(String::new)
                .unwrap_or_else(|| {
                    render_dependency_mode_summary(&resolution.resolved_dependencies)
                        .map(|summary| format!("; {summary}"))
                        .unwrap_or_default()
                })
        ),
    );
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::PackageRoot,
        "package-store-root",
        Some(resolution.package_store_root),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::PackageRoot,
        "package-cache-root",
        Some(workspace.cache_root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::CacheRoot,
        "git-cache-root",
        Some(workspace.git_cache_root.clone()),
    ));
    result.artifacts.push(FrontendArtifactSummary::new(
        FrontendArtifactKind::PackageRoot,
        "lockfile",
        Some(resolution.lockfile_path),
    ));
    for package in &resolution.resolved_packages {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            package.name.clone(),
            Some(package.root.clone()),
        ));
    }
    for package in resolution.resolved_dependencies {
        result.artifacts.push(FrontendArtifactSummary::new(
            FrontendArtifactKind::PackageRoot,
            format!(
                "{}:{}:{}",
                package.alias,
                match package.source_kind {
                    PackageDependencySourceKind::Local => "loc",
                    PackageDependencySourceKind::PackageStore => "pkg",
                    PackageDependencySourceKind::Git => "git",
                    PackageDependencySourceKind::Internal => "internal",
                },
                package.evaluation_mode.as_str()
            ),
            Some(package.root),
        ));
    }
    Ok(result)
}

pub fn fetch_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    fetch_workspace_with_config(workspace, &FrontendConfig::default())
}

pub fn update_workspace_with_config(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let previous_lockfile = if lockfile_path(workspace).is_file() {
        Some(load_existing_lockfile(workspace)?)
    } else {
        None
    };
    let mut update_config = config.clone();
    update_config.refresh_fetch = true;
    update_config.locked_fetch = false;
    let mut result = fetch_workspace_with_config(workspace, &update_config)?;
    result.command = "update".to_string();
    let revision_changes = if let Some(previous) = previous_lockfile.as_ref() {
        let next = load_existing_lockfile(workspace)?;
        diff_lockfile_revisions(previous, &next)?
    } else {
        Vec::new()
    };
    result.summary = render_update_summary(&result.summary, &revision_changes);
    Ok(result)
}

pub fn update_workspace(workspace: &FrontendWorkspace) -> FrontendResult<FrontendCommandResult> {
    update_workspace_with_config(workspace, &FrontendConfig::default())
}

fn resolve_workspace_fetch(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FetchResolution> {
    let preparation = prepare_workspace_packages(workspace)?;
    let package_store_root = select_package_store_root(config, workspace);
    let git_store_root = select_git_store_root(config, workspace);
    let git_session =
        fol_package::PackageGitSourceSession::new(workspace.git_cache_root.clone(), git_store_root);
    let mut package_session =
        fol_package::PackageSession::with_config(preparation.package_config.clone());
    let mut queued_roots = preparation
        .packages
        .iter()
        .map(|package| package.root.clone())
        .collect::<Vec<_>>();
    let mut seen_roots = BTreeSet::new();
    let mut resolved_packages = Vec::new();
    let mut resolved_dependencies = Vec::new();
    let mut git_lock_entries = Vec::new();
    let mut seen_lock_keys = BTreeSet::new();
    let existing_lockfile = if config.locked_fetch {
        Some(load_existing_lockfile(workspace)?)
    } else {
        None
    };
    let mut seen_git_aliases = BTreeSet::new();
    let fetch_options = fol_package::PackageGitFetchOptions {
        offline: config.offline_fetch,
        refresh: config.refresh_fetch,
    };
    let mut repaired_materializations = 0usize;

    while let Some(root) = queued_roots.pop() {
        let canonical_root = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        let canonical_key = canonical_root.to_string_lossy().to_string();
        if !seen_roots.insert(canonical_key) {
            continue;
        }

        let build_path = canonical_root.join("build.fol");
        let metadata = fol_package::parse_package_metadata_from_build(&build_path)
            .map_err(FrontendError::from)?;
        fol_package::parse_package_build(&build_path).map_err(FrontendError::from)?;

        resolved_packages.push(ResolvedDependencyPackage {
            alias: metadata.name.clone(),
            root: canonical_root.clone(),
            name: metadata.name.clone(),
            version: metadata.version.clone(),
            source_kind: PackageDependencySourceKind::Local,
            evaluation_mode: DependencyBuildEvaluationMode::Eager,
        });

        for dependency in metadata.dependencies {
            match dependency.source_kind {
                fol_package::PackageDependencySourceKind::Local => {
                    let dependency_root =
                        absolute_dependency_root(&canonical_root, &dependency.target);
                    let dependency_build = dependency_root.join("build.fol");
                    fol_package::parse_package_metadata_from_build(&dependency_build)
                        .map_err(FrontendError::from)?;
                    fol_package::parse_package_build(&dependency_build)
                        .map_err(FrontendError::from)?;
                    resolved_dependencies.push(ResolvedDependencyPackage {
                        alias: dependency.alias.clone(),
                        root: dependency_root.clone(),
                        name: dependency.alias.clone(),
                        version: String::new(),
                        source_kind: PackageDependencySourceKind::Local,
                        evaluation_mode: dependency.evaluation_mode,
                    });
                    queued_roots.push(dependency_root);
                }
                fol_package::PackageDependencySourceKind::PackageStore => {
                    let locator = fol_package::parse_package_locator(&dependency.target)
                        .map_err(FrontendError::from)?;
                    let loaded = package_session
                        .load_package_from_store(
                            &package_store_root,
                            &locator_use_path_segments(&locator),
                        )
                        .map_err(FrontendError::from)?;
                    resolved_dependencies.push(ResolvedDependencyPackage {
                        alias: dependency.alias.clone(),
                        root: PathBuf::from(loaded.identity.canonical_root.clone()),
                        name: loaded.identity.display_name.clone(),
                        version: String::new(),
                        source_kind: PackageDependencySourceKind::PackageStore,
                        evaluation_mode: dependency.evaluation_mode,
                    });
                    queued_roots.push(PathBuf::from(loaded.identity.canonical_root));
                }
                fol_package::PackageDependencySourceKind::Git => {
                    let locator = dependency_git_locator(&dependency)?;
                    let materialization = if let Some(lockfile) = &existing_lockfile {
                        let entry = lockfile
                            .entries
                            .iter()
                            .find(|entry| entry.alias == dependency.alias)
                            .ok_or_else(|| {
                                lock_mismatch_error(format!(
                                    "missing git dependency '{}' in fol.lock",
                                    dependency.alias
                                ))
                            })?;
                        if entry.locator != dependency.git_locator_string() {
                            return Err(lock_mismatch_error(format!(
                                "git dependency '{}' points to '{}' in build.fol metadata but '{}' in fol.lock",
                                dependency.alias,
                                dependency.git_locator_string(),
                                entry.locator
                            )));
                        }
                        let was_missing = !Path::new(&entry.materialized_root).is_dir();
                        git_session
                            .materialize_revision_with_options(
                                &locator,
                                &entry.selected_revision,
                                fetch_options,
                            )
                            .map_err(FrontendError::from)
                            .inspect(|_| {
                                if was_missing {
                                    repaired_materializations += 1;
                                }
                            })?
                    } else {
                        git_session
                            .materialize_selected_revision_with_options(&locator, fetch_options)
                            .map_err(FrontendError::from)?
                    };
                    seen_git_aliases.insert(dependency.alias.clone());
                    project_git_dependency_alias(
                        &package_store_root,
                        &dependency.alias,
                        &materialization.store_root,
                    )?;
                    let loaded = package_session
                        .load_materialized_package(&materialization.store_root)
                        .map_err(FrontendError::from)?;
                    resolved_dependencies.push(ResolvedDependencyPackage {
                        alias: dependency.alias.clone(),
                        root: PathBuf::from(loaded.identity.canonical_root.clone()),
                        name: loaded.identity.display_name.clone(),
                        version: String::new(),
                        source_kind: PackageDependencySourceKind::Git,
                        evaluation_mode: dependency.evaluation_mode,
                    });
                    queued_roots.push(PathBuf::from(loaded.identity.canonical_root.clone()));

                    let lock_key = format!(
                        "{}@{}",
                        locator
                            .normalized_git_identity()
                            .unwrap_or_else(|| locator.raw.clone()),
                        materialization.selected_revision
                    );
                    if seen_lock_keys.insert(lock_key) {
                        let dependency_alias = dependency.alias.clone();
                        git_lock_entries.push(fol_package::PackageLockEntry {
                            alias: dependency_alias,
                            source_kind: fol_package::PackageDependencySourceKind::Git,
                            locator: dependency.git_locator_string(),
                            selected_revision: materialization.selected_revision.clone(),
                            materialized_root: materialization
                                .store_root
                                .to_string_lossy()
                                .to_string(),
                        });
                    }
                }
                fol_package::PackageDependencySourceKind::Internal => {
                    if dependency.target != "standard" {
                        return Err(FrontendError::new(
                            crate::FrontendErrorKind::InvalidInput,
                            format!(
                                "internal dependency '{}' must use target 'standard' (got '{}')",
                                dependency.alias, dependency.target
                            ),
                        ));
                    }
                    let std_root = workspace
                        .std_root_override
                        .clone()
                        .or_else(fol_package::available_bundled_std_root)
                        .ok_or_else(|| {
                            FrontendError::new(
                                crate::FrontendErrorKind::InvalidInput,
                                format!(
                                    "internal dependency '{}' requires bundled std or an explicit --std-root <DIR> override",
                                    dependency.alias
                                ),
                            )
                        })?;
                    let alias_root = package_store_root.join(&dependency.alias);
                    if alias_root.is_dir() {
                        std::fs::remove_dir_all(&alias_root).map_err(FrontendError::from)?;
                    } else if alias_root.exists() {
                        std::fs::remove_file(&alias_root).map_err(FrontendError::from)?;
                    }
                    copy_package_projection(&std_root, &alias_root)?;
                    let loaded = package_session
                        .load_materialized_package(&alias_root)
                        .map_err(FrontendError::from)?;
                    resolved_dependencies.push(ResolvedDependencyPackage {
                        alias: dependency.alias.clone(),
                        root: PathBuf::from(loaded.identity.canonical_root.clone()),
                        name: loaded.identity.display_name.clone(),
                        version: String::new(),
                        source_kind: PackageDependencySourceKind::Internal,
                        evaluation_mode: dependency.evaluation_mode,
                    });
                    queued_roots.push(PathBuf::from(loaded.identity.canonical_root));
                }
            }
        }
    }

    git_lock_entries.sort_by(|left, right| left.alias.cmp(&right.alias));
    let lockfile = fol_package::PackageLockfile::new(git_lock_entries);
    if let Some(existing_lockfile) = &existing_lockfile {
        let existing_aliases = existing_lockfile
            .entries
            .iter()
            .map(|entry| entry.alias.as_str())
            .collect::<BTreeSet<_>>();
        let new_aliases = seen_git_aliases
            .iter()
            .map(|alias| alias.as_str())
            .collect::<BTreeSet<_>>();
        if existing_aliases != new_aliases {
            return Err(lock_mismatch_error(
                "build.fol git dependencies do not match the aliases pinned in fol.lock",
            ));
        }
    }

    Ok(FetchResolution {
        preparation,
        resolved_packages,
        resolved_dependencies,
        lockfile,
        lockfile_path: lockfile_path(workspace),
        package_store_root,
        repaired_materializations,
    })
}

fn load_existing_lockfile(
    workspace: &FrontendWorkspace,
) -> FrontendResult<fol_package::PackageLockfile> {
    let path = lockfile_path(workspace);
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        FrontendError::new(
            crate::FrontendErrorKind::InvalidInput,
            format!(
                "locked fetch requires an existing fol.lock at '{}': {}",
                path.display(),
                error
            ),
        )
        .with_note("run `fol pack fetch` first to create fol.lock")
    })?;
    fol_package::parse_package_lockfile(&raw).map_err(FrontendError::from)
}

fn lock_mismatch_error(message: impl Into<String>) -> FrontendError {
    FrontendError::new(crate::FrontendErrorKind::InvalidInput, message.into())
        .with_note("run `fol pack fetch` or `fol pack update` to refresh fol.lock")
        .with_note("use `fol pack fetch --locked` only when build.fol and fol.lock are intentionally in sync")
}

fn with_fetch_guidance(mut error: FrontendError, config: &FrontendConfig) -> FrontendError {
    let message = error.message().to_ascii_lowercase();
    if config.offline_fetch {
        error = error.with_note(
            "offline mode only works when the git source already exists in the local cache",
        );
    }
    if config.locked_fetch {
        error = error.with_note(
            "locked mode requires build.fol and fol.lock to describe the same git dependencies",
        );
    }
    if message.contains("permission denied")
        || message.contains("could not read from remote repository")
        || message.contains("authentication failed")
    {
        error = error.with_note(
            "check that your git remote credentials and SSH keys are available to `git`",
        );
    }
    if message.contains("could not resolve host")
        || message.contains("name or service not known")
        || message.contains("failed to connect")
    {
        error = error.with_note("check your network connection or rerun with `fol pack fetch --offline` after warming the cache");
    }
    error
}

fn prune_stale_git_materializations(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    lockfile: &fol_package::PackageLockfile,
) -> FrontendResult<usize> {
    let package_store_root = select_package_store_root(config, workspace);
    let workspace_local_root = workspace.root.root.join(".fol");
    if !package_store_root.starts_with(&workspace_local_root) {
        return Ok(0);
    }
    let git_store_root = package_store_root.join("git");
    if !git_store_root.is_dir() {
        return Ok(0);
    }

    let live_roots = lockfile
        .entries
        .iter()
        .map(|entry| {
            std::fs::canonicalize(&entry.materialized_root)
                .unwrap_or_else(|_| PathBuf::from(&entry.materialized_root))
        })
        .collect::<BTreeSet<_>>();
    let mut removed = 0usize;
    let mut stack = vec![git_store_root];

    while let Some(root) = stack.pop() {
        for entry in std::fs::read_dir(&root).map_err(FrontendError::from)? {
            let entry = entry.map_err(FrontendError::from)?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if file_name.starts_with("rev_") {
                let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                if !live_roots.contains(&canonical) {
                    std::fs::remove_dir_all(&path).map_err(FrontendError::from)?;
                    removed += 1;
                }
            } else {
                stack.push(path);
            }
        }
    }

    Ok(removed)
}

fn diff_lockfile_revisions(
    previous: &fol_package::PackageLockfile,
    next: &fol_package::PackageLockfile,
) -> FrontendResult<Vec<LockRevisionChange>> {
    let mut changes = Vec::new();
    for next_entry in &next.entries {
        if let Some(previous_entry) = previous
            .entries
            .iter()
            .find(|entry| entry.alias == next_entry.alias)
        {
            if previous_entry.selected_revision != next_entry.selected_revision {
                changes.push(LockRevisionChange {
                    alias: next_entry.alias.clone(),
                    previous_revision: previous_entry.selected_revision.clone(),
                    next_revision: next_entry.selected_revision.clone(),
                });
            }
        }
    }
    Ok(changes)
}

fn render_update_summary(fetch_summary: &str, revision_changes: &[LockRevisionChange]) -> String {
    let mut lines = vec![fetch_summary.replacen("prepared", "updated", 1)];
    if revision_changes.is_empty() {
        lines.push("pinned revisions unchanged".to_string());
    } else {
        lines.push(format!("revisions changed: {}", revision_changes.len()));
        for change in revision_changes {
            lines.push(format!(
                "{}: {} -> {}",
                change.alias, change.previous_revision, change.next_revision
            ));
        }
    }
    lines.join("\n")
}

fn absolute_dependency_root(package_root: &Path, target: &str) -> PathBuf {
    let path = Path::new(target);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        package_root.join(path)
    }
}

fn locator_use_path_segments(
    locator: &fol_package::PackageLocator,
) -> Vec<fol_parser::ast::UsePathSegment> {
    locator
        .path_segments
        .iter()
        .enumerate()
        .map(|(index, spelling)| fol_parser::ast::UsePathSegment {
            separator: (index > 0).then_some(fol_parser::ast::UsePathSeparator::Slash),
            spelling: spelling.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        fetch_workspace, fetch_workspace_with_config, prepare_workspace_packages,
        select_package_store_root,
    };
    use crate::{FrontendConfig, FrontendWorkspace, PackageRoot, WorkspaceRoot};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    fn semantic_bin_build() -> &'static str {
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = .graph();\n",
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
                "    var graph = .graph();\n",
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
    fn package_preparation_reads_formal_workspace_members() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_prepare_{}", std::process::id()));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();

        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(app.clone())],
            std_root_override: Some(root.join("std")),
            package_store_root_override: Some(root.join(".fol/pkg")),
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        };

        let preparation = prepare_workspace_packages(&workspace).unwrap();

        assert_eq!(preparation.packages.len(), 1);
        assert_eq!(preparation.packages[0].root, app);
        assert_eq!(preparation.packages[0].name, "app");
        assert_eq!(
            preparation.package_config.package_store_root,
            Some(root.join(".fol/pkg").to_string_lossy().to_string())
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn package_preparation_rejects_members_without_formal_build_files() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_prepare_missing_{}",
            std::process::id()
        ));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();

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

        let error = prepare_workspace_packages(&workspace).unwrap_err();

        assert_eq!(error.kind(), crate::FrontendErrorKind::PackageFailed);
        assert!(error.message().contains("build file"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fetch_workspace_returns_a_command_result_for_prepared_members() {
        let root = std::env::temp_dir().join(format!("fol_frontend_fetch_{}", std::process::id()));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();

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

        let result = fetch_workspace(&workspace).unwrap();

        assert_eq!(result.command, "fetch");
        assert!(result
            .summary
            .contains("prepared 1 workspace package(s) and resolved 1 package root(s) into"));
        assert_eq!(result.artifacts.len(), 5);
        assert_eq!(result.artifacts[4].path, Some(app));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fetch_workspace_materializes_internal_standard_dependencies_under_alias() {
        let root = temp_root("internal_standard");
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
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(
            app.join("src/main.fol"),
            "fun[] main(): int = { return 0; };\n",
        )
        .unwrap();

        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(app.clone())],
            std_root_override: None,
            package_store_root_override: Some(root.join(".fol/pkg")),
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        };

        let result = fetch_workspace(&workspace).unwrap();
        let std_alias_root = root.join(".fol/pkg/std");

        assert!(result
            .artifacts
            .iter()
            .any(|artifact| artifact.label == "std:internal:eager"));
        assert!(std_alias_root.join("build.fol").is_file());
        assert!(std_alias_root.join("fmt/root.fol").is_file());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn package_store_root_selection_prefers_frontend_config_then_workspace_then_default() {
        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(PathBuf::from("/tmp/demo")),
            members: Vec::new(),
            std_root_override: None,
            package_store_root_override: Some(PathBuf::from("/tmp/demo/ws-pkg")),
            build_root: PathBuf::from("/tmp/demo/.fol/build"),
            cache_root: PathBuf::from("/tmp/demo/.fol/cache"),
            git_cache_root: PathBuf::from("/tmp/demo/.fol/cache/git"),
            install_prefix: PathBuf::from("/tmp/demo/.fol/install"),
        };
        let config = FrontendConfig {
            package_store_root_override: Some(PathBuf::from("/tmp/demo/config-pkg")),
            ..FrontendConfig::default()
        };

        assert_eq!(
            select_package_store_root(&config, &workspace),
            PathBuf::from("/tmp/demo/config-pkg")
        );
        assert_eq!(
            select_package_store_root(&FrontendConfig::default(), &workspace),
            PathBuf::from("/tmp/demo/ws-pkg")
        );
        assert_eq!(
            select_package_store_root(
                &FrontendConfig::default(),
                &FrontendWorkspace {
                    package_store_root_override: None,
                    git_cache_root: PathBuf::from("/tmp/demo/.fol/cache/git"),
                    ..workspace
                }
            ),
            PathBuf::from("/tmp/demo/.fol/pkg")
        );
    }

    #[test]
    fn fetch_summary_prefers_configured_store_root_in_reported_artifacts() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_fetch_summary_{}", std::process::id()));
        let app = root.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("build.fol"), semantic_bin_build()).unwrap();

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

        let result = fetch_workspace_with_config(&workspace, &config).unwrap();

        assert_eq!(result.artifacts[0].path, Some(root.join(".fol/config-pkg")));
        assert_eq!(result.artifacts[1].path, Some(root.join(".fol/cache")));
        assert_eq!(result.artifacts[2].path, Some(root.join(".fol/cache/git")));
        assert_eq!(result.artifacts[4].path, Some(app));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fetch_workspace_materializes_git_manifest_dependencies_and_writes_lockfile() {
        let root = temp_root("git_dep");
        let app = root.join("app");
        let remote = root.join("remote-logtiny");
        create_package_repo(&remote, "logtiny", "0.1.0");
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
                    "}};\n",
                ),
                remote.display()
            ),
        )
        .expect("should write app build");
        fs::write(app.join("src/main.fol"), "var[exp] answer: int = 1;\n")
            .expect("should write app source");

        let workspace = FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(app.clone())],
            std_root_override: None,
            package_store_root_override: Some(root.join(".fol/pkg")),
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        };

        let result = fetch_workspace(&workspace).expect("fetch should succeed");

        assert!(root.join("fol.lock").is_file());
        let lockfile =
            fs::read_to_string(root.join("fol.lock")).expect("lockfile should be readable");
        assert!(lockfile.contains("alias: logtiny"));
        assert!(lockfile.contains("source: git"));
        assert!(result.artifacts.iter().any(|artifact| {
            artifact.label == "lockfile" && artifact.path == Some(root.join("fol.lock"))
        }));
        assert!(result.artifacts.iter().any(|artifact| {
            artifact
                .path
                .as_ref()
                .map(|path| path.to_string_lossy().contains("rev_"))
                .unwrap_or(false)
        }));

        fs::remove_dir_all(root).ok();
    }

    fn create_package_repo(root: &Path, name: &str, version: &str) {
        fs::create_dir_all(root.join("src")).expect("package repo should be creatable");
        fs::write(
            root.join("build.fol"),
            format!("name: {name}\nversion: {version}\n"),
        )
        .expect("package metadata should be writable");
        fs::write(root.join("build.fol"), semantic_lib_build(name))
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
}
