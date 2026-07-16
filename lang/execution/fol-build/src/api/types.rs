use crate::dependency::{DependencyBuildEvaluationMode, DependencyBuildSurface};
use crate::graph::{BuildOptionId, BuildOptionKind};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitDependencyVersionSelector {
    Branch(String),
    Tag(String),
    Commit(String),
}

impl GitDependencyVersionSelector {
    pub fn parse(raw: &str) -> Option<Self> {
        let (kind, value) = raw.split_once(':')?;
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        match kind.trim() {
            "branch" => Some(Self::Branch(value.to_string())),
            "tag" => Some(Self::Tag(value.to_string())),
            "commit" => Some(Self::Commit(value.to_string())),
            _ => None,
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Branch(value) => format!("branch:{value}"),
            Self::Tag(value) => format!("tag:{value}"),
            Self::Commit(value) => format!("commit:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardTargetRequest {
    pub name: String,
    pub default: Option<String>,
}

impl StandardTargetRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: None,
        }
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardOptimizeRequest {
    pub name: String,
    pub default: Option<String>,
}

impl StandardOptimizeRequest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: None,
        }
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardTargetOption {
    pub id: BuildOptionId,
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardOptimizeOption {
    pub id: BuildOptionId,
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOptionValue {
    Bool(bool),
    Int(i64),
    String(String),
    Enum(String),
    Path(String),
}

impl BuildOptionValue {
    pub fn kind(&self) -> crate::graph::BuildOptionKind {
        use crate::graph::BuildOptionKind;
        match self {
            Self::Bool(_) => BuildOptionKind::Bool,
            Self::Int(_) => BuildOptionKind::Int,
            Self::String(_) => BuildOptionKind::String,
            Self::Enum(_) => BuildOptionKind::Enum,
            Self::Path(_) => BuildOptionKind::Path,
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::String(value) | Self::Enum(value) | Self::Path(value) => value.clone(),
        }
    }

    pub fn parse_for_kind(kind: crate::graph::BuildOptionKind, raw: &str) -> Option<Self> {
        use crate::graph::BuildOptionKind;
        match kind {
            BuildOptionKind::Bool => match raw {
                "true" => Some(Self::Bool(true)),
                "false" => Some(Self::Bool(false)),
                _ => None,
            },
            BuildOptionKind::Int => raw.parse().ok().map(Self::Int),
            BuildOptionKind::String => Some(Self::String(raw.to_string())),
            BuildOptionKind::Enum => Some(Self::Enum(raw.to_string())),
            BuildOptionKind::Path => Some(Self::Path(raw.to_string())),
            BuildOptionKind::Target | BuildOptionKind::Optimize => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserOptionRequest {
    pub name: String,
    pub kind: BuildOptionKind,
    pub default: Option<BuildOptionValue>,
}

impl UserOptionRequest {
    pub fn bool(name: impl Into<String>, default: bool) -> Self {
        Self {
            name: name.into(),
            kind: BuildOptionKind::Bool,
            default: Some(BuildOptionValue::Bool(default)),
        }
    }

    pub fn string(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: BuildOptionKind::String,
            default: Some(BuildOptionValue::String(default.into())),
        }
    }

    pub fn int(name: impl Into<String>, default: i64) -> Self {
        Self {
            name: name.into(),
            kind: BuildOptionKind::Int,
            default: Some(BuildOptionValue::Int(default)),
        }
    }

    pub fn enumeration(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: BuildOptionKind::Enum,
            default: Some(BuildOptionValue::Enum(default.into())),
        }
    }

    pub fn path(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: BuildOptionKind::Path,
            default: Some(BuildOptionValue::Path(default.into())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserOption {
    pub id: BuildOptionId,
    pub name: String,
    pub kind: BuildOptionKind,
    pub default: Option<BuildOptionValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableRequest {
    pub name: String,
    pub root_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticLibraryRequest {
    pub name: String,
    pub root_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedLibraryRequest {
    pub name: String,
    pub root_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestArtifactRequest {
    pub name: String,
    pub root_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildApiNameError {
    Empty,
    InvalidCharacter(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildApiError {
    InvalidName(BuildApiNameError),
}

impl std::fmt::Display for BuildApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(BuildApiNameError::Empty) => {
                write!(f, "build API names must not be empty")
            }
            Self::InvalidName(BuildApiNameError::InvalidCharacter(ch)) => {
                write!(f, "build API names must not contain '{}'", ch)
            }
        }
    }
}

impl std::error::Error for BuildApiError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactHandle {
    pub artifact_id: crate::graph::BuildArtifactId,
    pub root_module_id: crate::graph::BuildModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLibraryHandle {
    pub request: crate::native::SystemLibraryRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRequest {
    pub name: String,
    pub description: Option<String>,
    pub depends_on: Vec<crate::graph::BuildStepId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepHandle {
    pub step_id: crate::graph::BuildStepId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub name: String,
    pub artifact: BuildArtifactHandle,
    pub depends_on: Vec<crate::graph::BuildStepId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHandle {
    pub step_id: crate::graph::BuildStepId,
    pub artifact_id: crate::graph::BuildArtifactId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallArtifactRequest {
    pub name: String,
    pub artifact: BuildArtifactHandle,
    pub depends_on: Vec<crate::graph::BuildStepId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallFileRequest {
    pub name: String,
    pub path: String,
    pub depends_on: Vec<crate::graph::BuildStepId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteFileRequest {
    pub name: String,
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFileRequest {
    pub name: String,
    pub source_path: String,
    pub destination_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDirRequest {
    pub name: String,
    pub path: String,
    pub depends_on: Vec<crate::graph::BuildStepId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallHandle {
    pub install_id: crate::graph::BuildInstallId,
    pub step_id: crate::graph::BuildStepId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputHandleKind {
    WrittenFile,
    CopiedFile,
    CapturedStdout,
    CodegenOutput,
    DependencyGeneratedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputHandleLocator {
    GeneratedFile(crate::graph::BuildGeneratedFileId),
    DependencyGeneratedOutput {
        dependency_alias: String,
        output_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputHandle {
    pub kind: OutputHandleKind,
    pub locator: OutputHandleLocator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathHandleClass {
    File,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathHandleProvenance {
    Source,
    Generated,
    DependencyGenerated,
    DependencyPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHandle {
    pub class: PathHandleClass,
    pub provenance: PathHandleProvenance,
    pub relative_path: String,
}

impl OutputHandle {
    pub fn generated_file_id(&self) -> Option<crate::graph::BuildGeneratedFileId> {
        match self.locator {
            OutputHandleLocator::GeneratedFile(id) => Some(id),
            OutputHandleLocator::DependencyGeneratedOutput { .. } => None,
        }
    }

    pub fn path_handle(&self, relative_path: impl Into<String>) -> PathHandle {
        let provenance = match self.locator {
            OutputHandleLocator::GeneratedFile(_) => PathHandleProvenance::Generated,
            OutputHandleLocator::DependencyGeneratedOutput { .. } => {
                PathHandleProvenance::DependencyGenerated
            }
        };
        PathHandle {
            class: PathHandleClass::File,
            provenance,
            relative_path: relative_path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileHandle {
    pub generated_file_id: crate::graph::BuildGeneratedFileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileHandle {
    pub relative_path: String,
}

impl SourceFileHandle {
    pub fn path_handle(&self) -> PathHandle {
        PathHandle {
            class: PathHandleClass::File,
            provenance: PathHandleProvenance::Source,
            relative_path: self.relative_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDirHandle {
    pub relative_path: String,
}

impl SourceDirHandle {
    pub fn path_handle(&self) -> PathHandle {
        PathHandle {
            class: PathHandleClass::Dir,
            provenance: PathHandleProvenance::Source,
            relative_path: self.relative_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySourceKind {
    Local,
    PackageStore,
    Git,
    Internal,
}

impl DependencySourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "loc",
            Self::PackageStore => "pkg",
            Self::Git => "git",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequest {
    pub alias: String,
    pub source_kind: DependencySourceKind,
    pub package: String,
    pub args: BTreeMap<String, DependencyArgValue>,
    pub evaluation_mode: Option<DependencyBuildEvaluationMode>,
    pub git_version: Option<GitDependencyVersionSelector>,
    pub git_hash: Option<String>,
    pub surface: Option<DependencyBuildSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyArgValue {
    Bool(bool),
    Int(i64),
    String(String),
    OptionRef(String),
}

impl DependencyArgValue {
    pub fn resolve(&self, options: &crate::option::ResolvedBuildOptionSet) -> Option<String> {
        match self {
            Self::Bool(value) => Some(value.to_string()),
            Self::Int(value) => Some(value.to_string()),
            Self::String(value) => Some(value.clone()),
            Self::OptionRef(name) => options.get(name.as_str()).map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyHandle {
    pub alias: String,
    pub package: String,
    pub root_module_id: crate::graph::BuildModuleId,
    pub evaluation_mode: Option<DependencyBuildEvaluationMode>,
    pub build: crate::dependency::DependencyBuildHandle,
    pub modules: crate::dependency::DependencyModuleSurfaceSet,
    pub artifacts: crate::dependency::DependencyArtifactSurfaceSet,
    pub steps: crate::dependency::DependencyStepSurfaceSet,
    pub files: crate::dependency::DependencyFileSurfaceSet,
    pub dirs: crate::dependency::DependencyDirSurfaceSet,
    pub paths: crate::dependency::DependencyPathSurfaceSet,
    pub generated_outputs: crate::dependency::DependencyGeneratedOutputSurfaceSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleHandle {
    pub module_id: crate::graph::BuildModuleId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddModuleRequest {
    pub name: String,
    pub root_module: String,
}

pub fn validate_build_name(name: &str) -> Result<(), BuildApiNameError> {
    if name.is_empty() {
        return Err(BuildApiNameError::Empty);
    }

    for ch in name.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_' | '.') {
            continue;
        }
        return Err(BuildApiNameError::InvalidCharacter(ch));
    }

    Ok(())
}
