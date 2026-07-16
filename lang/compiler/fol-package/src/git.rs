use crate::{git_cache_path, git_store_path, PackageError, PackageErrorKind, PackageLocator};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGitMaterialization {
    pub locator: PackageLocator,
    pub selected_revision: String,
    pub cache_root: PathBuf,
    pub store_root: PathBuf,
}

impl PackageGitMaterialization {
    pub fn new(
        cache_base_root: &Path,
        store_base_root: &Path,
        locator: PackageLocator,
        selected_revision: impl Into<String>,
    ) -> Self {
        let selected_revision = selected_revision.into();
        Self {
            cache_root: git_cache_path(cache_base_root, &locator),
            store_root: git_store_path(store_base_root, &locator, &selected_revision),
            locator,
            selected_revision,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGitSourceSession {
    cache_base_root: PathBuf,
    store_base_root: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackageGitFetchOptions {
    pub offline: bool,
    pub refresh: bool,
}

impl PackageGitSourceSession {
    pub fn new(cache_base_root: impl Into<PathBuf>, store_base_root: impl Into<PathBuf>) -> Self {
        Self {
            cache_base_root: cache_base_root.into(),
            store_base_root: store_base_root.into(),
        }
    }

    pub fn cache_base_root(&self) -> &Path {
        &self.cache_base_root
    }

    pub fn store_base_root(&self) -> &Path {
        &self.store_base_root
    }

    pub fn plan_materialization(
        &self,
        locator: &PackageLocator,
        selected_revision: impl Into<String>,
    ) -> Result<PackageGitMaterialization, PackageError> {
        if locator.git.is_none() {
            return Err(PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "git materialization requires a git locator, but '{}' is not remote git",
                    locator.raw
                ),
            ));
        }

        Ok(PackageGitMaterialization::new(
            &self.cache_base_root,
            &self.store_base_root,
            locator.clone(),
            selected_revision,
        ))
    }

    pub fn sync_source_cache_with_options(
        &self,
        locator: &PackageLocator,
        options: PackageGitFetchOptions,
    ) -> Result<PathBuf, PackageError> {
        if locator.git.is_none() {
            return Err(PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "git source cache sync requires a git locator, but '{}' is not remote git",
                    locator.raw
                ),
            ));
        }

        let cache_root = git_cache_path(&self.cache_base_root, locator);
        if cache_root.is_dir() {
            if !options.offline {
                run_git(
                    Some(&cache_root),
                    &["fetch", "--all", "--tags", "--prune"],
                    locator,
                    if options.refresh {
                        "refresh from source cache"
                    } else {
                        "fetch from source cache"
                    },
                )?;
            }
        } else {
            if options.offline {
                return Err(wrap_git_failure(
                    "read from offline git cache",
                    locator,
                    format!(
                        "cached source mirror '{}' does not exist",
                        cache_root.display()
                    ),
                ));
            }
            if let Some(parent) = cache_root.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    wrap_git_failure(
                        "create git cache parent directories",
                        locator,
                        error.to_string(),
                    )
                })?;
            }
            let repository = locator
                .git
                .as_ref()
                .expect("git locator was checked above")
                .repository
                .clone();
            run_git(
                None,
                &[
                    "clone",
                    repository.as_str(),
                    cache_root.to_string_lossy().as_ref(),
                ],
                locator,
                "clone into source cache",
            )?;
        }
        Ok(cache_root)
    }

    pub fn sync_source_cache(&self, locator: &PackageLocator) -> Result<PathBuf, PackageError> {
        self.sync_source_cache_with_options(locator, PackageGitFetchOptions::default())
    }

    pub fn resolve_revision(
        &self,
        locator: &PackageLocator,
        cache_root: &Path,
    ) -> Result<String, PackageError> {
        let git = locator.git.as_ref().ok_or_else(|| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "git revision resolution requires a git locator, but '{}' is not remote git",
                    locator.raw
                ),
            )
        })?;
        let spec = if let Some(rev) = &git.selector.rev {
            format!("{rev}^{{commit}}")
        } else if let Some(tag) = &git.selector.tag {
            format!("refs/tags/{tag}^{{commit}}")
        } else if let Some(branch) = &git.selector.branch {
            format!("refs/remotes/origin/{branch}^{{commit}}")
        } else {
            "HEAD^{commit}".to_string()
        };
        run_git_capture(
            Some(cache_root),
            &["rev-parse", spec.as_str()],
            locator,
            "resolve selected revision",
        )
        .map(|value| value.trim().to_string())
    }

    fn verify_selected_revision(
        &self,
        locator: &PackageLocator,
        selected_revision: &str,
    ) -> Result<(), PackageError> {
        let Some(expected_hash) = locator
            .git
            .as_ref()
            .and_then(|git| git.selector.hash.as_deref())
        else {
            return Ok(());
        };

        if selected_revision.starts_with(expected_hash) {
            return Ok(());
        }

        Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git dependency '{}' resolved revision '{}' does not match required hash '{}'",
                locator.raw, selected_revision, expected_hash
            ),
        ))
    }

    pub fn materialize_revision(
        &self,
        locator: &PackageLocator,
        revision: &str,
    ) -> Result<PackageGitMaterialization, PackageError> {
        self.materialize_revision_with_options(locator, revision, PackageGitFetchOptions::default())
    }

    pub fn materialize_revision_with_options(
        &self,
        locator: &PackageLocator,
        revision: &str,
        options: PackageGitFetchOptions,
    ) -> Result<PackageGitMaterialization, PackageError> {
        let cache_root = self.sync_source_cache_with_options(locator, options)?;
        let selected_revision = if revision.trim().is_empty() {
            self.resolve_revision(locator, &cache_root)?
        } else {
            revision.trim().to_string()
        };
        self.verify_selected_revision(locator, &selected_revision)?;
        let materialization = PackageGitMaterialization::new(
            &self.cache_base_root,
            &self.store_base_root,
            locator.clone(),
            selected_revision.clone(),
        );

        if materialization.store_root.is_dir() {
            return Ok(materialization);
        }

        if let Some(parent) = materialization.store_root.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                wrap_git_failure(
                    "create materialized package parent directories",
                    locator,
                    error.to_string(),
                )
            })?;
        }

        run_git(
            None,
            &[
                "clone",
                "--no-checkout",
                cache_root.to_string_lossy().as_ref(),
                materialization.store_root.to_string_lossy().as_ref(),
            ],
            locator,
            "clone pinned materialization",
        )?;
        run_git(
            Some(&materialization.store_root),
            &["checkout", "--force", selected_revision.as_str()],
            locator,
            "check out pinned revision",
        )?;

        Ok(materialization)
    }

    pub fn materialize_selected_revision(
        &self,
        locator: &PackageLocator,
    ) -> Result<PackageGitMaterialization, PackageError> {
        self.materialize_selected_revision_with_options(locator, PackageGitFetchOptions::default())
    }

    pub fn materialize_selected_revision_with_options(
        &self,
        locator: &PackageLocator,
        options: PackageGitFetchOptions,
    ) -> Result<PackageGitMaterialization, PackageError> {
        let cache_root = self.sync_source_cache_with_options(locator, options)?;
        let selected_revision = self.resolve_revision(locator, &cache_root)?;
        self.materialize_revision_with_options(locator, &selected_revision, options)
    }
}

pub fn wrap_git_failure(
    action: &str,
    locator: &PackageLocator,
    detail: impl Into<String>,
) -> PackageError {
    PackageError::new(
        PackageErrorKind::InvalidInput,
        format!(
            "git dependency '{}' failed while trying to {action}: {}",
            locator.raw,
            detail.into()
        ),
    )
}

fn run_git(
    cwd: Option<&Path>,
    args: &[&str],
    locator: &PackageLocator,
    action: &str,
) -> Result<(), PackageError> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| wrap_git_failure(action, locator, error.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(wrap_git_failure(
        action,
        locator,
        git_output_detail(&output.stderr, &output.stdout),
    ))
}

fn run_git_capture(
    cwd: Option<&Path>,
    args: &[&str],
    locator: &PackageLocator,
    action: &str,
) -> Result<String, PackageError> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| wrap_git_failure(action, locator, error.to_string()))?;
    if !output.status.success() {
        return Err(wrap_git_failure(
            action,
            locator,
            git_output_detail(&output.stderr, &output.stdout),
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| wrap_git_failure(action, locator, error.to_string()))
}

fn git_output_detail(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    "git command failed without output".to_string()
}

#[cfg(test)]
mod tests {
    use super::{wrap_git_failure, PackageGitFetchOptions, PackageGitSourceSession};
    use crate::{parse_package_locator, PackageGitSelector, PackageGitTransport, PackageLocator};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn git_source_session_plans_separate_cache_and_store_roots() {
        let session =
            PackageGitSourceSession::new("/tmp/demo/.fol/cache/git", "/tmp/demo/.fol/pkg/git");
        let locator = PackageLocator::git(
            "https://github.com/bresilla/logtiny.git#tag:v1",
            PackageGitTransport::Https,
            "https://github.com/bresilla/logtiny.git",
            PackageGitSelector {
                branch: None,
                tag: Some("v1".to_string()),
                rev: None,
                hash: None,
            },
        );

        let materialization = session
            .plan_materialization(&locator, "abc123")
            .expect("git materialization plan should succeed");

        assert_eq!(
            materialization.cache_root,
            PathBuf::from("/tmp/demo/.fol/cache/git/github.com/bresilla/logtiny.git")
        );
        assert_eq!(
            materialization.store_root,
            PathBuf::from("/tmp/demo/.fol/pkg/git/github.com/bresilla/logtiny/rev_abc123")
        );
        assert_eq!(materialization.selected_revision, "abc123");
    }

    #[test]
    fn git_source_session_rejects_non_git_locators() {
        let session =
            PackageGitSourceSession::new("/tmp/demo/.fol/cache/git", "/tmp/demo/.fol/pkg/git");
        let locator = parse_package_locator("org/core").expect("store locator should parse");

        let error = session
            .plan_materialization(&locator, "abc123")
            .expect_err("non-git locators should be rejected");

        assert!(error.message().contains("requires a git locator"));
    }

    #[test]
    fn wrapped_git_failures_keep_locator_context() {
        let locator = parse_package_locator("git@github.com:bresilla/logtiny.git")
            .expect("git locator should parse");

        let error = wrap_git_failure("fetch", &locator, "network timeout");

        assert!(error.message().contains(
            "git dependency 'git@github.com:bresilla/logtiny.git' failed while trying to fetch"
        ));
        assert!(error.message().contains("network timeout"));
    }

    #[test]
    fn git_source_session_clones_fetches_and_materializes_selected_revisions() {
        let temp_root = temp_root("materialize");
        let remote = temp_root.join("remote");
        create_package_repo(&remote, "0.1.0");

        let session =
            PackageGitSourceSession::new(temp_root.join("cache"), temp_root.join("store"));
        let locator = PackageLocator::git(
            format!("git+file://{}", remote.display()),
            PackageGitTransport::Git,
            format!("file://{}", remote.display()),
            PackageGitSelector::default(),
        );

        let materialization = session
            .materialize_selected_revision(&locator)
            .expect("materialization should succeed");

        assert!(materialization.cache_root.is_dir());
        assert!(materialization.store_root.is_dir());
        assert!(materialization.store_root.join("build.fol").is_file());
        assert!(!materialization.selected_revision.trim().is_empty());

        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn git_source_session_verifies_branch_plus_hash_selectors() {
        let temp_root = temp_root("branch_hash");
        let remote = temp_root.join("remote");
        create_package_repo(&remote, "0.1.0");
        let expected_revision = git_output(&remote, &["rev-parse", "HEAD"]);

        let session =
            PackageGitSourceSession::new(temp_root.join("cache"), temp_root.join("store"));
        let locator = PackageLocator::git(
            format!(
                "git+file://{}#branch:main#hash:{}",
                remote.display(),
                expected_revision
            ),
            PackageGitTransport::Git,
            format!("file://{}", remote.display()),
            PackageGitSelector {
                branch: Some("main".to_string()),
                tag: None,
                rev: None,
                hash: Some(expected_revision.clone()),
            },
        );

        let materialization = session
            .materialize_selected_revision(&locator)
            .expect("branch plus hash materialization should succeed");

        assert_eq!(materialization.selected_revision, expected_revision);
        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn git_source_session_verifies_tag_selectors() {
        let temp_root = temp_root("tag");
        let remote = temp_root.join("remote");
        create_package_repo(&remote, "0.1.0");
        let expected_revision = git_output(&remote, &["rev-parse", "HEAD"]);

        let session =
            PackageGitSourceSession::new(temp_root.join("cache"), temp_root.join("store"));
        let locator = PackageLocator::git(
            format!("git+file://{}#tag:v0.1.0", remote.display()),
            PackageGitTransport::Git,
            format!("file://{}", remote.display()),
            PackageGitSelector {
                branch: None,
                tag: Some("v0.1.0".to_string()),
                rev: None,
                hash: None,
            },
        );

        let materialization = session
            .materialize_selected_revision(&locator)
            .expect("tag materialization should succeed");

        assert_eq!(materialization.selected_revision, expected_revision);
        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn git_source_session_rejects_hash_mismatches() {
        let temp_root = temp_root("hash_mismatch");
        let remote = temp_root.join("remote");
        create_package_repo(&remote, "0.1.0");

        let session =
            PackageGitSourceSession::new(temp_root.join("cache"), temp_root.join("store"));
        let locator = PackageLocator::git(
            format!("git+file://{}#branch:main#hash:deadbeef", remote.display()),
            PackageGitTransport::Git,
            format!("file://{}", remote.display()),
            PackageGitSelector {
                branch: Some("main".to_string()),
                tag: None,
                rev: None,
                hash: Some("deadbeef".to_string()),
            },
        );

        let error = session
            .materialize_selected_revision(&locator)
            .expect_err("mismatched hash should fail");

        assert!(error
            .message()
            .contains("does not match required hash 'deadbeef'"));
        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn offline_git_source_session_requires_a_warm_cache() {
        let temp_root = temp_root("offline");
        let remote = temp_root.join("remote");
        create_package_repo(&remote, "0.1.0");

        let session =
            PackageGitSourceSession::new(temp_root.join("cache"), temp_root.join("store"));
        let locator = PackageLocator::git(
            format!("git+file://{}", remote.display()),
            PackageGitTransport::Git,
            format!("file://{}", remote.display()),
            PackageGitSelector::default(),
        );

        let error = session
            .materialize_selected_revision_with_options(
                &locator,
                PackageGitFetchOptions {
                    offline: true,
                    refresh: false,
                },
            )
            .expect_err("offline materialization should require a warm cache");

        assert!(error.message().contains("offline git cache"));
        std::fs::remove_dir_all(temp_root).ok();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fol_package_git_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be stable enough for temp dirs")
                .as_nanos()
        ))
    }

    fn create_package_repo(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("src")).expect("package repo should be creatable");
        std::fs::write(
            root.join("build.fol"),
            format!(
                "pro[] build(): non = {{\n    var build = .build();\n    build.meta({{ name = \"logtiny\", version = \"{version}\" }});\n}}\n"
            ),
        )
            .expect("package build should be writable");
        std::fs::write(root.join("src/lib.fol"), "var[exp] level: int = 1\n")
            .expect("package source should be writable");

        git(root, &["init"]);
        git(root, &["branch", "-M", "main"]);
        git(root, &["config", "user.name", "FOL"]);
        git(root, &["config", "user.email", "fol@example.com"]);
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "init"]);
        git(root, &["tag", "v0.1.0"]);
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
}
