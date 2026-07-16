use std::path::{Component, Path, PathBuf};

use linc::contract::{
    AnalysisPolicy, ContractError, ProbeEnvironmentIdentity, ProbeEnvironmentPolicy,
    ProbeExecutionPolicy, ProbePolicy, ProbeResourceLimits, ResolutionPolicy, RunnerPolicy,
};

const PROBE_WALL_TIME_MILLIS: u64 = 30_000;
const PROBE_MAX_MEMORY_BYTES: u64 = 1024 * 1024 * 1024;
const PROBE_MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const PROBE_MAX_PROCESSES: u32 = 16;

pub(crate) fn certification_resource_limits() -> Result<ProbeResourceLimits, ContractError> {
    ProbeResourceLimits::try_new(
        PROBE_WALL_TIME_MILLIS,
        PROBE_MAX_MEMORY_BYTES,
        PROBE_MAX_OUTPUT_BYTES,
        PROBE_MAX_PROCESSES,
    )
}

/// Validate the caller-owned probe directory before any sibling API runs.
pub(crate) fn preflight_temporary_parent(
    temporary_parent: &Path,
) -> Result<ValidatedTemporaryParent, InteropAnalysisPolicyError> {
    if !normalized_absolute(temporary_parent) {
        return Err(InteropAnalysisPolicyError::InvalidTemporaryParent(
            temporary_parent.to_owned(),
        ));
    }
    let temporary_parent = std::fs::canonicalize(temporary_parent).map_err(|source| {
        InteropAnalysisPolicyError::Io {
            path: temporary_parent.to_owned(),
            source,
        }
    })?;
    if !temporary_parent.is_dir() || temporary_parent.parent().is_none() {
        return Err(InteropAnalysisPolicyError::InvalidTemporaryParent(
            temporary_parent,
        ));
    }

    Ok(ValidatedTemporaryParent(temporary_parent))
}

fn normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
        && path.as_os_str() == path.components().collect::<PathBuf>().as_os_str()
}

#[derive(Debug)]
pub(crate) struct ValidatedTemporaryParent(PathBuf);

/// Construct FOL's initial fail-closed LINC policy. No runner, inferred
/// evidence, weak symbol, name search, or inherited environment is enabled.
pub(crate) fn strict_compile_only_policy(
    temporary_parent: ValidatedTemporaryParent,
) -> Result<AnalysisPolicy, InteropAnalysisPolicyError> {
    let environment = ProbeEnvironmentIdentity::try_new(ProbeEnvironmentPolicy::Empty, Vec::new())?;
    let limits = certification_resource_limits()?;
    let execution = ProbeExecutionPolicy::try_new(temporary_parent.0, environment, limits)?;
    AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        execution,
    )
    .map_err(Into::into)
}

#[derive(Debug)]
pub enum InteropAnalysisPolicyError {
    InvalidTemporaryParent(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Contract(ContractError),
}

impl std::fmt::Display for InteropAnalysisPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTemporaryParent(path) => write!(
                formatter,
                "LINC temporary parent must be an absolute directory other than root: {}",
                path.display()
            ),
            Self::Io { path, source } => write!(
                formatter,
                "could not canonicalize LINC temporary parent {}: {source}",
                path.display()
            ),
            Self::Contract(error) => write!(formatter, "invalid strict LINC policy: {error}"),
        }
    }
}

impl std::error::Error for InteropAnalysisPolicyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Contract(error) => Some(error),
            Self::InvalidTemporaryParent(_) => None,
        }
    }
}

impl From<ContractError> for InteropAnalysisPolicyError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{preflight_temporary_parent, InteropAnalysisPolicyError};

    #[test]
    fn rejects_relative_temporary_parent_before_io() {
        let error = preflight_temporary_parent(Path::new("target/interop")).unwrap_err();
        assert!(matches!(
            error,
            InteropAnalysisPolicyError::InvalidTemporaryParent(_)
        ));
    }

    #[test]
    fn rejects_filesystem_root_as_temporary_parent() {
        let error = preflight_temporary_parent(Path::new("/")).unwrap_err();
        assert!(matches!(
            error,
            InteropAnalysisPolicyError::InvalidTemporaryParent(_)
        ));
    }
}
