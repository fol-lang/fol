use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendArtifactKind {
    WorkspaceRoot,
    PackageRoot,
    BuildRoot,
    CacheRoot,
    Binary,
    Installed,
    EmittedRust,
    LoweredSnapshot,
    InteropEvidence,
}

impl FrontendArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceRoot => "workspace-root",
            Self::PackageRoot => "package-root",
            Self::BuildRoot => "build-root",
            Self::CacheRoot => "cache-root",
            Self::Binary => "binary",
            Self::Installed => "installed",
            Self::EmittedRust => "emitted-rust",
            Self::LoweredSnapshot => "lowered-snapshot",
            Self::InteropEvidence => "interop-evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendArtifactSummary {
    pub kind: FrontendArtifactKind,
    pub path: Option<PathBuf>,
    pub label: String,
}

impl FrontendArtifactSummary {
    pub fn new(
        kind: FrontendArtifactKind,
        label: impl Into<String>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            kind,
            path,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontendCommandResult {
    pub command: String,
    pub summary: String,
    pub artifacts: Vec<FrontendArtifactSummary>,
    /// Verbatim command output that is the point of the command rather than a
    /// description of it — a shell completion script, a completion match list.
    /// Human and plain rendering print it raw with no status envelope so it can
    /// be piped or `eval`'d; JSON carries it in a `payload` field.
    pub payload: Option<String>,
}

impl FrontendCommandResult {
    pub fn new(command: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            summary: summary.into(),
            artifacts: Vec::new(),
            payload: None,
        }
    }

    pub fn with_artifact(mut self, artifact: FrontendArtifactSummary) -> Self {
        self.artifacts.push(artifact);
        self
    }

    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult};
    use std::path::PathBuf;

    #[test]
    fn command_result_tracks_artifacts_in_stable_order() {
        let result = FrontendCommandResult::new("build", "built binary").with_artifact(
            FrontendArtifactSummary::new(
                FrontendArtifactKind::Binary,
                "demo",
                Some(PathBuf::from("target/bin/demo")),
            ),
        );

        assert_eq!(result.command, "build");
        assert_eq!(result.summary, "built binary");
        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].kind.as_str(), "binary");
        assert_eq!(result.artifacts[0].label, "demo");
    }
}
