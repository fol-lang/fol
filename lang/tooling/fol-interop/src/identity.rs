use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
};

use crate::lock::{
    LOCKED_GERC_PATH, LOCKED_GERC_REPOSITORY, LOCKED_GERC_REVISION, LOCKED_LINC_PATH,
    LOCKED_LINC_REPOSITORY, LOCKED_LINC_REVISION, LOCKED_PARC_PATH, LOCKED_PARC_REPOSITORY,
    LOCKED_PARC_REVISION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VerifiedSiblingRevisions {
    pub parc: &'static str,
    pub linc: &'static str,
    pub gerc: &'static str,
}

#[derive(Debug)]
pub enum InteropIdentityError {
    InvalidCheckout {
        component: &'static str,
        path: PathBuf,
    },
    GitUnavailable {
        component: &'static str,
        source: std::io::Error,
    },
    GitFailed {
        component: &'static str,
        operation: &'static str,
        status: Option<i32>,
        stderr: String,
    },
    NonUtf8GitOutput {
        component: &'static str,
        operation: &'static str,
    },
    GitRootMismatch {
        component: &'static str,
        expected: PathBuf,
        actual: PathBuf,
    },
    RevisionMismatch {
        component: &'static str,
        expected: &'static str,
        actual: String,
    },
    DirtyCheckout {
        component: &'static str,
    },
    OriginMismatch {
        component: &'static str,
        expected: &'static str,
        actual: String,
    },
}

impl std::fmt::Display for InteropIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCheckout { component, path } => write!(
                formatter,
                "locked {component} checkout is not an absolute canonical directory: {}",
                path.display()
            ),
            Self::GitUnavailable { component, source } => {
                write!(
                    formatter,
                    "could not launch Git for locked {component}: {source}"
                )
            }
            Self::GitFailed {
                component,
                operation,
                status,
                stderr,
            } => write!(
                formatter,
                "Git {operation} failed for locked {component} with status {status:?}: {stderr}"
            ),
            Self::NonUtf8GitOutput {
                component,
                operation,
            } => write!(
                formatter,
                "Git {operation} returned non-UTF-8 output for locked {component}"
            ),
            Self::GitRootMismatch {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "locked {component} Git root is {}, expected {}",
                actual.display(),
                expected.display()
            ),
            Self::RevisionMismatch {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "locked {component} revision is {actual}, expected {expected}"
            ),
            Self::DirtyCheckout { component } => {
                write!(formatter, "locked {component} checkout is dirty")
            }
            Self::OriginMismatch {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "locked {component} origin is {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for InteropIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GitUnavailable { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(crate) fn verify_locked_siblings() -> Result<VerifiedSiblingRevisions, InteropIdentityError> {
    let fol_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    verify_checkout(
        "PARC",
        &fol_root.join(LOCKED_PARC_PATH),
        LOCKED_PARC_REPOSITORY,
        LOCKED_PARC_REVISION,
    )?;
    verify_checkout(
        "LINC",
        &fol_root.join(LOCKED_LINC_PATH),
        LOCKED_LINC_REPOSITORY,
        LOCKED_LINC_REVISION,
    )?;
    verify_checkout(
        "GERC",
        &fol_root.join(LOCKED_GERC_PATH),
        LOCKED_GERC_REPOSITORY,
        LOCKED_GERC_REVISION,
    )?;
    Ok(VerifiedSiblingRevisions {
        parc: LOCKED_PARC_REVISION,
        linc: LOCKED_LINC_REVISION,
        gerc: LOCKED_GERC_REVISION,
    })
}

fn verify_checkout(
    component: &'static str,
    checkout: &Path,
    expected_repository: &'static str,
    expected_revision: &'static str,
) -> Result<(), InteropIdentityError> {
    let checkout =
        std::fs::canonicalize(checkout).map_err(|_| InteropIdentityError::InvalidCheckout {
            component,
            path: checkout.to_owned(),
        })?;
    if !checkout.is_absolute() || !checkout.is_dir() {
        return Err(InteropIdentityError::InvalidCheckout {
            component,
            path: checkout,
        });
    }

    let reported_root = PathBuf::from(git_text(
        component,
        &checkout,
        "root discovery",
        [OsStr::new("rev-parse"), OsStr::new("--show-toplevel")],
    )?);
    let reported_root = std::fs::canonicalize(&reported_root).map_err(|_| {
        InteropIdentityError::InvalidCheckout {
            component,
            path: reported_root.clone(),
        }
    })?;
    if reported_root != checkout {
        return Err(InteropIdentityError::GitRootMismatch {
            component,
            expected: checkout,
            actual: reported_root,
        });
    }

    let revision = git_text(
        component,
        &checkout,
        "HEAD discovery",
        [OsStr::new("rev-parse"), OsStr::new("HEAD")],
    )?;
    if revision != expected_revision {
        return Err(InteropIdentityError::RevisionMismatch {
            component,
            expected: expected_revision,
            actual: revision,
        });
    }

    let status = git_text(
        component,
        &checkout,
        "worktree status",
        [
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
            OsStr::new("--untracked-files=normal"),
            OsStr::new("--ignore-submodules=none"),
        ],
    )?;
    if !status.is_empty() {
        return Err(InteropIdentityError::DirtyCheckout { component });
    }

    let origin = git_text(
        component,
        &checkout,
        "origin discovery",
        [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("--get"),
            OsStr::new("remote.origin.url"),
        ],
    )?;
    let normalized = normalize_repository(&origin);
    if normalized != expected_repository {
        return Err(InteropIdentityError::OriginMismatch {
            component,
            expected: expected_repository,
            actual: normalized,
        });
    }
    Ok(())
}

fn git_text<const N: usize>(
    component: &'static str,
    checkout: &Path,
    operation: &'static str,
    args: [&OsStr; N],
) -> Result<String, InteropIdentityError> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin"));
    // Preserve Git's standard global-exclude classification while clearing
    // repository-affecting variables such as GIT_DIR and GIT_WORK_TREE.
    let output = Command::new("git")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.untrackedCache=false")
        .arg("-C")
        .arg(checkout)
        .args(args)
        .env_clear()
        .env("PATH", path)
        .env("LC_ALL", "C")
        .envs(
            [
                "HOME",
                "XDG_CONFIG_HOME",
                "GIT_CONFIG_GLOBAL",
                "GIT_CONFIG_SYSTEM",
                "GIT_CONFIG_NOSYSTEM",
            ]
            .into_iter()
            .filter_map(|name| std::env::var_os(name).map(|value| (name, value))),
        )
        .output()
        .map_err(|source| InteropIdentityError::GitUnavailable { component, source })?;
    if !output.status.success() {
        return Err(InteropIdentityError::GitFailed {
            component,
            operation,
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let value =
        String::from_utf8(output.stdout).map_err(|_| InteropIdentityError::NonUtf8GitOutput {
            component,
            operation,
        })?;
    Ok(value.trim().to_owned())
}

fn normalize_repository(raw: &str) -> String {
    let mut value = raw.trim();
    let mut recognized_github = false;
    for prefix in [
        "git@github.com:",
        "ssh://git@github.com/",
        "https://github.com/",
        "http://github.com/",
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest;
            recognized_github = true;
            break;
        }
    }
    if let Some(rest) = value.strip_prefix("github.com/") {
        value = rest;
        recognized_github = true;
    }
    value = value.trim_end_matches('/');
    let value = value.strip_suffix(".git").unwrap_or(value);
    let value = value.trim_end_matches('/');
    if recognized_github {
        format!("github.com/{value}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_repository, verify_locked_siblings};

    #[test]
    fn repository_spellings_normalize_to_the_locked_identity() {
        for spelling in [
            "git@github.com:follang/lang-c.git",
            "ssh://git@github.com/follang/lang-c/",
            "https://github.com/follang/lang-c.git",
            "https://github.com/follang/lang-c.git/",
            "http://github.com/follang/lang-c",
        ] {
            assert_eq!(normalize_repository(spelling), "github.com/follang/lang-c");
        }
    }

    #[test]
    fn non_github_repository_spellings_are_not_upgraded_to_github() {
        for spelling in [
            "follang/lang-c",
            "/tmp/follang/lang-c.git",
            "file:///tmp/follang/lang-c.git",
            "ssh://example.com/follang/lang-c.git",
        ] {
            assert_ne!(normalize_repository(spelling), "github.com/follang/lang-c");
        }
    }

    #[test]
    fn compiled_sibling_revisions_are_observed_from_clean_locked_checkouts() {
        let revisions = verify_locked_siblings().unwrap();
        assert_eq!(revisions.parc, crate::lock::LOCKED_PARC_REVISION);
        assert_eq!(revisions.linc, crate::lock::LOCKED_LINC_REVISION);
        assert_eq!(revisions.gerc, crate::lock::LOCKED_GERC_REVISION);
    }
}
