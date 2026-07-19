use crate::{PackageError, PackageErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageLocatorKind {
    InstalledStore,
    Git,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageGitTransport {
    Https,
    Ssh,
    Git,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageGitSelector {
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageGitLocator {
    pub transport: PackageGitTransport,
    pub repository: String,
    pub selector: PackageGitSelector,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageLocator {
    pub kind: PackageLocatorKind,
    pub raw: String,
    pub path_segments: Vec<String>,
    pub git: Option<PackageGitLocator>,
}

impl PackageLocator {
    pub fn installed_store(raw: impl Into<String>, path_segments: Vec<String>) -> Self {
        Self {
            kind: PackageLocatorKind::InstalledStore,
            raw: raw.into(),
            path_segments,
            git: None,
        }
    }

    pub fn git(
        raw: impl Into<String>,
        transport: PackageGitTransport,
        repository: impl Into<String>,
        selector: PackageGitSelector,
    ) -> Self {
        Self {
            kind: PackageLocatorKind::Git,
            raw: raw.into(),
            path_segments: Vec::new(),
            git: Some(PackageGitLocator {
                transport,
                repository: repository.into(),
                selector,
            }),
        }
    }

    pub fn normalized_git_identity(&self) -> Option<String> {
        self.git
            .as_ref()
            .map(PackageGitLocator::normalized_identity)
    }
}

impl PackageGitLocator {
    pub fn normalized_identity(&self) -> String {
        normalize_git_repository_identity(&self.repository)
    }
}

pub fn parse_package_locator(raw: &str) -> Result<PackageLocator, PackageError> {
    let trimmed = raw.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return parse_https_git_locator(trimmed);
    }
    if trimmed.starts_with("git@") {
        return parse_ssh_git_locator(trimmed);
    }
    if trimmed.starts_with("git+") {
        return parse_git_scheme_locator(trimmed);
    }
    if looks_like_future_git_locator(trimmed) {
        return Err(unsupported_remote_locator(raw));
    }
    let parts = trimmed.split('/').map(str::trim).collect::<Vec<_>>();
    if trimmed.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package dependency locator '{}' must contain non-empty slash-separated segments",
                raw
            ),
        ));
    }

    Ok(PackageLocator::installed_store(
        trimmed.to_string(),
        parts.into_iter().map(str::to_string).collect(),
    ))
}

fn parse_https_git_locator(raw: &str) -> Result<PackageLocator, PackageError> {
    reject_selector_query(raw)?;
    let repository = raw.to_string();
    let without_scheme = repository
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or_else(|| unsupported_remote_locator(raw))?;
    let mut segments = without_scheme
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 3 {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must include a host, owner, and repository path",
                raw
            ),
        ));
    }
    let host = segments.remove(0);
    if host.is_empty() {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("git package locator '{}' is missing a host", raw),
        ));
    }
    if !is_valid_hostname(host) {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' has invalid hostname '{}'; hostnames may only contain alphanumeric characters, dots, and hyphens",
                raw, host
            ),
        ));
    }

    Ok(PackageLocator::git(
        raw.to_string(),
        PackageGitTransport::Https,
        repository,
        PackageGitSelector::default(),
    ))
}

fn parse_ssh_git_locator(raw: &str) -> Result<PackageLocator, PackageError> {
    reject_selector_query(raw)?;
    let repository = raw.to_string();
    let Some((user_host, repo_path)) = repository.split_once(':') else {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must follow 'git@host:owner/repo.git' form",
                raw
            ),
        ));
    };
    let Some((_user, host)) = user_host.split_once('@') else {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must include a user and host before ':'",
                raw
            ),
        ));
    };
    let path_segments = repo_path
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if host.trim().is_empty() || path_segments.len() < 2 {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must include a host, owner, and repository path",
                raw
            ),
        ));
    }
    if !is_valid_hostname(host.trim()) {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' has invalid hostname '{}'; hostnames may only contain alphanumeric characters, dots, and hyphens",
                raw, host.trim()
            ),
        ));
    }

    Ok(PackageLocator::git(
        raw.to_string(),
        PackageGitTransport::Ssh,
        repository,
        PackageGitSelector::default(),
    ))
}

fn parse_git_scheme_locator(raw: &str) -> Result<PackageLocator, PackageError> {
    let git_prefixed = raw.trim_start_matches("git+").trim();
    reject_selector_query(git_prefixed)?;
    let repository = git_prefixed.to_string();
    if repository.is_empty()
        || !(repository.starts_with("https://")
            || repository.starts_with("http://")
            || repository.starts_with("ssh://")
            || repository.starts_with("file://"))
    {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must follow 'git+https://...', 'git+ssh://...', or 'git+file://...' form",
                raw
            ),
        ));
    }

    Ok(PackageLocator::git(
        raw.to_string(),
        PackageGitTransport::Git,
        repository,
        PackageGitSelector::default(),
    ))
}

fn reject_selector_query(raw: &str) -> Result<(), PackageError> {
    if raw.split_once('?').is_some() {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "git package locator '{}' must not embed selector query params; use dependency fields 'version' and 'hash' instead",
                raw
            ),
        ));
    }
    Ok(())
}

fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    host.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-')
}

fn looks_like_future_git_locator(raw: &str) -> bool {
    raw.contains("://")
        || raw.starts_with("git+")
        || raw.starts_with("git@")
        || raw.ends_with(".git")
}

fn unsupported_remote_locator(raw: &str) -> PackageError {
    PackageError::new(
        PackageErrorKind::Unsupported,
        format!(
            "package dependency locator '{}' looks like a future git or remote locator; only installed-store slash paths are supported today",
            raw
        ),
    )
}

fn normalize_git_repository_identity(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let normalized = if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("ssh://"))
    {
        // Scheme URLs may still embed a user (ssh://git@host/...); the
        // user is transport detail, not repository identity.
        match rest.split_once('/') {
            Some((authority, path)) => {
                let host = authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host);
                format!("{host}/{path}")
            }
            None => rest
                .rsplit_once('@')
                .map_or(rest, |(_, host)| host)
                .to_string(),
        }
    } else if let Some(rest) = trimmed.strip_prefix("git@") {
        rest.replace(':', "/")
    } else {
        trimmed.to_string()
    };

    let normalized = normalized
        .trim_start_matches('/')
        .trim_end_matches(".git")
        .to_string();

    if let Some((host, rest)) = normalized.split_once('/') {
        format!("{}/{}", host.to_ascii_lowercase(), rest)
    } else {
        normalized.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_package_locator, PackageGitSelector, PackageGitTransport, PackageLocator,
        PackageLocatorKind,
    };

    #[test]
    fn package_locator_parses_installed_store_paths() {
        let locator = parse_package_locator("org/tools").expect(
            "Slash-separated package dependency paths should parse as installed-store locators",
        );

        assert_eq!(
            locator,
            PackageLocator::installed_store(
                "org/tools",
                vec!["org".to_string(), "tools".to_string()]
            )
        );
    }

    #[test]
    fn package_locator_rejects_empty_store_segments() {
        let error = parse_package_locator("org//tools")
            .expect_err("Package locators should reject empty slash-separated segments");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must contain non-empty slash-separated segments"),
            "Invalid locators should explain the accepted slash-separated path form",
        );
    }

    #[test]
    fn package_locator_reports_remote_git_forms_as_explicit_placeholders() {
        // The `git@host:owner/repo.git` and `git+ssh://...` forms are now
        // supported transports. Raw `ssh://...` remote locators remain
        // unsupported and must still surface an explicit placeholder diagnostic.
        let error = parse_package_locator("ssh://github.com/follang/json.git").expect_err(
            "Remote git-like locators should fail with an explicit placeholder diagnostic",
        );

        assert_eq!(error.kind(), crate::PackageErrorKind::Unsupported);
        assert!(
            error.to_string().contains("future git or remote locator"),
            "Remote locators should fail with an explicit future-support diagnostic",
        );
    }

    #[test]
    fn package_locator_can_represent_git_locators_without_store_segments() {
        let locator = PackageLocator::git(
            "https://github.com/follang/json.git",
            PackageGitTransport::Https,
            "https://github.com/follang/json.git",
            PackageGitSelector {
                branch: Some("main".to_string()),
                tag: None,
                rev: None,
                hash: None,
            },
        );

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert!(locator.path_segments.is_empty());
        assert_eq!(
            locator.git.as_ref().map(|git| git.transport),
            Some(PackageGitTransport::Https)
        );
        assert_eq!(
            locator
                .git
                .as_ref()
                .and_then(|git| git.selector.branch.as_deref()),
            Some("main")
        );
    }

    #[test]
    fn package_locator_parses_https_git_locators() {
        let locator = parse_package_locator("https://github.com/follang/json.git")
            .expect("HTTPS git locators should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            locator.git.as_ref().map(|git| git.transport),
            Some(PackageGitTransport::Https)
        );
        assert_eq!(
            locator.git.as_ref().map(|git| git.repository.as_str()),
            Some("https://github.com/follang/json.git")
        );
        assert!(locator.path_segments.is_empty());
    }

    #[test]
    fn package_locator_parses_ssh_git_locators() {
        let locator = parse_package_locator("git@github.com:follang/json.git")
            .expect("SSH git locators should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            locator.git.as_ref().map(|git| git.transport),
            Some(PackageGitTransport::Ssh)
        );
        assert_eq!(
            locator.git.as_ref().map(|git| git.repository.as_str()),
            Some("git@github.com:follang/json.git")
        );
    }

    #[test]
    fn package_locator_parses_git_scheme_locators() {
        let locator = parse_package_locator("git+https://github.com/follang/json.git")
            .expect("git+ locators should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            locator.git.as_ref().map(|git| git.transport),
            Some(PackageGitTransport::Git)
        );
        assert_eq!(
            locator.git.as_ref().map(|git| git.repository.as_str()),
            Some("https://github.com/follang/json.git")
        );
    }

    #[test]
    fn package_locator_parses_git_file_scheme_locators() {
        let locator = parse_package_locator("git+file:///tmp/logtiny")
            .expect("git+file locators should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            locator.git.as_ref().map(|git| git.transport),
            Some(PackageGitTransport::Git)
        );
        assert_eq!(
            locator.git.as_ref().map(|git| git.repository.as_str()),
            Some("file:///tmp/logtiny")
        );
    }

    #[test]
    fn package_locator_rejects_branch_selector_query_params() {
        let error = parse_package_locator("https://github.com/follang/json.git?branch=main")
            .expect_err("branch selector query params should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "rejection should explain the structured version/hash contract",
        );
    }

    #[test]
    fn package_locator_rejects_tag_selector_query_params() {
        let error = parse_package_locator("https://github.com/follang/json.git?tag=v0.1.0")
            .expect_err("tag selector query params should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "rejection should explain the structured version/hash contract",
        );
    }

    #[test]
    fn package_locator_rejects_revision_selector_query_params() {
        let error =
            parse_package_locator("https://github.com/follang/json.git?rev=0123456789abcdef")
                .expect_err("revision selector query params should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "rejection should explain the structured version/hash contract",
        );
    }

    #[test]
    fn package_locator_rejects_hash_selector_query_params() {
        let error =
            parse_package_locator("https://github.com/follang/json.git?hash=0123456789abcdef")
                .expect_err("hash selector query params should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "rejection should explain the structured version/hash contract",
        );
    }

    #[test]
    fn package_locator_rejects_multiple_selector_query_params() {
        let error = parse_package_locator(
            "https://github.com/follang/json.git?branch=main&hash=0123456789abcdef",
        )
        .expect_err("selector query params should be rejected even when otherwise well-formed");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "rejection should explain the structured version/hash contract",
        );
    }

    #[test]
    fn package_locator_normalizes_git_identity_across_transport_forms() {
        let https = parse_package_locator("https://GitHub.com/follang/json.git")
            .expect("https git locator should parse");
        let ssh = parse_package_locator("git@github.com:follang/json.git")
            .expect("ssh git locator should parse");
        let git = parse_package_locator("git+ssh://git@github.com/follang/json.git")
            .expect("git+ git locator should parse");

        assert_eq!(
            https.normalized_git_identity().as_deref(),
            Some("github.com/follang/json")
        );
        assert_eq!(
            ssh.normalized_git_identity().as_deref(),
            Some("github.com/follang/json")
        );
        // Every transport spelling of the same repository normalizes to one
        // identity; embedded users are transport detail, not identity.
        assert_eq!(
            git.normalized_git_identity().as_deref(),
            Some("github.com/follang/json")
        );
    }

    #[test]
    fn package_locator_rejects_invalid_https_hostname() {
        let error = parse_package_locator("https://git hub.com/follang/json.git")
            .expect_err("HTTPS locators with invalid hostnames should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("invalid hostname"),
            "invalid hostname errors should mention 'invalid hostname', got: {error}",
        );
    }

    #[test]
    fn package_locator_rejects_invalid_ssh_hostname() {
        let error = parse_package_locator("git@git_hub.com:follang/json.git")
            .expect_err("SSH locators with invalid hostnames should be rejected");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("invalid hostname"),
            "invalid hostname errors should mention 'invalid hostname', got: {error}",
        );
    }

    #[test]
    fn package_locator_rejects_conflicting_git_selector_query_params() {
        let error =
            parse_package_locator("https://github.com/follang/json.git?branch=main&tag=v0.1.0")
                .expect_err("git locators should reject selector query params entirely");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "selector query params should point callers at version/hash fields",
        );
    }

    #[test]
    fn package_locator_rejects_duplicate_git_selector_query_params() {
        let error =
            parse_package_locator("https://github.com/follang/json.git?branch=main&branch=stable")
                .expect_err("git locators should reject selector query params entirely");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "selector query params should point callers at version/hash fields",
        );
    }

    #[test]
    fn package_locator_rejects_duplicate_hash_selector_query_params() {
        let error = parse_package_locator("https://github.com/follang/json.git?hash=abc&hash=def")
            .expect_err("git locators should reject selector query params entirely");

        assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("must not embed selector query params"),
            "selector query params should point callers at version/hash fields",
        );
    }

    #[test]
    fn package_locator_acceptance_matrix_stays_stable() {
        let cases = [
            ("core/tools", PackageLocatorKind::InstalledStore, None, None),
            (
                "https://github.com/follang/json.git",
                PackageLocatorKind::Git,
                Some(PackageGitTransport::Https),
                Some("github.com/follang/json"),
            ),
            (
                "git@github.com:follang/json.git",
                PackageLocatorKind::Git,
                Some(PackageGitTransport::Ssh),
                Some("github.com/follang/json"),
            ),
            (
                "git+https://github.com/follang/json.git",
                PackageLocatorKind::Git,
                Some(PackageGitTransport::Git),
                Some("github.com/follang/json"),
            ),
        ];

        for (raw, kind, transport, identity) in cases {
            let locator =
                parse_package_locator(raw).unwrap_or_else(|error| panic!("{raw}: {error}"));

            assert_eq!(locator.kind, kind, "{raw}");
            assert_eq!(
                locator.git.as_ref().map(|git| git.transport),
                transport,
                "{raw}"
            );
            assert_eq!(
                locator.normalized_git_identity().as_deref(),
                identity,
                "{raw}"
            );
        }
    }
}
