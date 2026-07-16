use crate::api::{BuildArtifactConfigValue, PathHandle, PathHandleClass, PathHandleProvenance};
use crate::artifact::BuildArtifactFolModel;
use crate::runtime::BuildRuntimeGeneratedFileKind;

// ---- Extraction output types (public so eval.rs can build EvaluatedBuildProgram) ---

pub type ExecConfigValue = BuildArtifactConfigValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecArtifact {
    pub name: String,
    pub root_module: ExecConfigValue,
    pub fol_model: BuildArtifactFolModel,
    pub target: Option<ExecConfigValue>,
    pub optimize: Option<ExecConfigValue>,
}

// ---- Internal value type for the execution scope ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExecValue {
    Build,
    Graph,
    Target(String),
    Optimize(String),
    OptionRef {
        name: String,
        kind: crate::graph::BuildOptionKind,
    },
    Str(String),
    Bool(bool),
    Artifact(ExecArtifact),
    Module {
        name: String,
    },
    SourceFile {
        path: String,
        provenance: PathHandleProvenance,
    },
    SourceDir {
        path: String,
        provenance: PathHandleProvenance,
    },
    GeneratedFile {
        name: String,
        path: String,
        kind: BuildRuntimeGeneratedFileKind,
        provenance: PathHandleProvenance,
    },
    Step {
        name: String,
    },
    Run {
        name: String,
    },
    Install {
        name: String,
    },
    Dependency {
        alias: String,
    },
    SystemLibrary {
        request: crate::native::SystemLibraryRequest,
    },
    DependencyModule {
        alias: String,
        query_name: String,
    },
    DependencyArtifact {
        alias: String,
        query_name: String,
    },
    DependencyStep {
        alias: String,
        query_name: String,
    },
    List(Vec<ExecValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedPathHandle {
    pub descriptor: PathHandle,
    pub generated_name: Option<String>,
}

impl ResolvedPathHandle {
    pub fn file(relative_path: impl Into<String>, provenance: PathHandleProvenance) -> Self {
        Self {
            descriptor: PathHandle {
                class: PathHandleClass::File,
                provenance,
                relative_path: relative_path.into(),
            },
            generated_name: None,
        }
    }

    pub fn dir(relative_path: impl Into<String>, provenance: PathHandleProvenance) -> Self {
        Self {
            descriptor: PathHandle {
                class: PathHandleClass::Dir,
                provenance,
                relative_path: relative_path.into(),
            },
            generated_name: None,
        }
    }

    pub fn generated(
        relative_path: impl Into<String>,
        provenance: PathHandleProvenance,
        generated_name: impl Into<String>,
    ) -> Self {
        Self {
            descriptor: PathHandle {
                class: PathHandleClass::File,
                provenance,
                relative_path: relative_path.into(),
            },
            generated_name: Some(generated_name.into()),
        }
    }
}

// ---- Helper routine representation ---

pub(super) struct HelperRoutine {
    pub(super) params: Vec<String>,
    pub(super) body: Vec<fol_parser::ast::AstNode>,
}
