//! Tool fingerprinting and the executable-trust policy.
//!
//! Two separate concerns that share a subject. Fingerprinting records *which*
//! tool ran so a build can be attributed and compared; the trust policy decides
//! *whether* a tool may run at all.
//!
//! The policy is deliberately blunt: a dependency may not hand the build an
//! executable. Running one means an arbitrary package chosen transitively gets
//! to execute code on the build machine, and V4 has no mechanism to say which
//! packages are allowed to. Until it has one, the answer is no.

use std::path::{Path, PathBuf};

/// Where an executable came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolProvenance {
    /// Found on the system, outside any package tree.
    System,
    /// Supplied by a dependency package.
    DependencyProvided,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolTrustError {
    /// A dependency tried to supply an executable.
    DependencyProvidedToolsAreDisabled { tool: String },
    /// The tool is not an absolute path, so which binary runs would depend on
    /// `PATH` at execution time.
    ToolPathIsNotAbsolute { tool: String },
}

impl std::fmt::Display for ToolTrustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DependencyProvidedToolsAreDisabled { tool } => write!(
                f,
                "'{tool}' is provided by a dependency, and running a dependency-provided \
                 executable is disabled: V4 has no trust policy that could say which packages \
                 are allowed to execute code during a build"
            ),
            Self::ToolPathIsNotAbsolute { tool } => write!(
                f,
                "build tool '{tool}' must be an absolute path; a bare name would resolve \
                 through PATH and make the build depend on the caller's environment"
            ),
        }
    }
}

impl std::error::Error for ToolTrustError {}

/// Decide whether a tool may run.
pub fn check_tool_is_runnable(
    tool: &str,
    provenance: ToolProvenance,
) -> Result<(), ToolTrustError> {
    if provenance == ToolProvenance::DependencyProvided {
        return Err(ToolTrustError::DependencyProvidedToolsAreDisabled {
            tool: tool.to_string(),
        });
    }
    if !Path::new(tool).is_absolute() {
        return Err(ToolTrustError::ToolPathIsNotAbsolute {
            tool: tool.to_string(),
        });
    }
    Ok(())
}

/// What was recorded about a tool that ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFingerprint {
    pub path: PathBuf,
    /// Digest of the executable's bytes, or `None` when it could not be read.
    pub content_digest: Option<String>,
    pub size_bytes: Option<u64>,
    /// Names of the environment variables the action set, never their values.
    pub environment_names: Vec<String>,
}

/// Fingerprint a tool and the environment it was given.
///
/// The environment contributes **names only**. A build fingerprint is written
/// into build metadata and travels with the artifact, so a value that happens
/// to be a token would travel with it. `plan::identity` hashes values when they
/// must affect identity; this record exists to be read by a human.
pub fn fingerprint_tool(tool: &Path, environment: &[(String, String)]) -> ToolFingerprint {
    let metadata = std::fs::metadata(tool).ok();
    let content_digest = std::fs::read(tool).ok().map(|bytes| digest(&bytes));
    let mut environment_names: Vec<String> =
        environment.iter().map(|(name, _)| name.clone()).collect();
    environment_names.sort();
    ToolFingerprint {
        path: tool.to_path_buf(),
        content_digest,
        size_bytes: metadata.map(|metadata| metadata.len()),
        environment_names,
    }
}

fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_provided_tool_is_refused() {
        let error =
            check_tool_is_runnable("/opt/dep/bin/generator", ToolProvenance::DependencyProvided)
                .expect_err("a dependency-provided tool must be refused");
        assert!(matches!(
            error,
            ToolTrustError::DependencyProvidedToolsAreDisabled { .. }
        ));
        // The message has to say why, or the rule reads as arbitrary.
        assert!(error.to_string().contains("trust policy"));
    }

    #[test]
    fn a_system_tool_must_be_an_absolute_path() {
        assert!(check_tool_is_runnable("/usr/bin/cc", ToolProvenance::System).is_ok());
        assert!(matches!(
            check_tool_is_runnable("cc", ToolProvenance::System),
            Err(ToolTrustError::ToolPathIsNotAbsolute { .. })
        ));
    }

    /// A fingerprint records which variables were set, never what they held.
    #[test]
    fn a_fingerprint_records_environment_names_and_never_values() {
        let fixture = fol_testkit::TempFixture::new("fol_tool_fingerprint");
        std::fs::create_dir_all(fixture.path()).unwrap();
        let tool = fixture.path().join("tool");
        std::fs::write(&tool, b"#!/bin/sh\nexit 0\n").unwrap();

        let fingerprint = fingerprint_tool(
            &tool,
            &[
                ("GITHUB_TOKEN".to_string(), "ghp_secretvalue".to_string()),
                ("CC".to_string(), "/usr/bin/cc".to_string()),
            ],
        );

        assert_eq!(
            fingerprint.environment_names,
            vec!["CC".to_string(), "GITHUB_TOKEN".to_string()],
            "names are recorded, sorted"
        );
        let rendered = format!("{fingerprint:?}");
        assert!(
            !rendered.contains("ghp_secretvalue"),
            "a secret reached the fingerprint: {rendered}"
        );
        assert_eq!(fingerprint.size_bytes, Some(17));
        assert!(fingerprint.content_digest.is_some());
    }

    #[test]
    fn a_changed_tool_changes_its_fingerprint() {
        let fixture = fol_testkit::TempFixture::new("fol_tool_changed");
        std::fs::create_dir_all(fixture.path()).unwrap();
        let tool = fixture.path().join("tool");

        std::fs::write(&tool, b"first").unwrap();
        let first = fingerprint_tool(&tool, &[]);
        std::fs::write(&tool, b"second").unwrap();
        let second = fingerprint_tool(&tool, &[]);

        assert_ne!(first.content_digest, second.content_digest);
    }

    #[test]
    fn a_missing_tool_fingerprints_without_panicking() {
        let fingerprint = fingerprint_tool(Path::new("/nonexistent/tool"), &[]);
        assert_eq!(fingerprint.content_digest, None);
        assert_eq!(fingerprint.size_bytes, None);
    }
}
