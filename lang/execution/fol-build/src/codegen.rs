use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::graph::{
    BuildArtifactId, BuildArtifactInput, BuildGeneratedFileId, BuildGeneratedFileKind, BuildGraph,
    BuildGraphValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileDefinition {
    pub name: String,
    pub relative_path: String,
    pub action: GeneratedFileAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedFileAction {
    Write {
        contents: Vec<u8>,
    },
    Copy {
        source_path: String,
    },
    CaptureToolOutput {
        tool: String,
        args: Vec<String>,
        file_args: Vec<String>,
        env: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileMaterialization {
    pub generated_file: BuildGeneratedFileId,
    pub relative_path: String,
    pub action: GeneratedFileAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedFileMaterializationPlan {
    entries: Vec<GeneratedFileMaterialization>,
}

impl GeneratedFileMaterializationPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[GeneratedFileMaterialization] {
        &self.entries
    }

    pub fn add(&mut self, entry: GeneratedFileMaterialization) {
        self.entries.push(entry);
    }
}

#[derive(Debug)]
pub enum GeneratedFileMaterializationError {
    InvalidGraph {
        errors: Vec<BuildGraphValidationError>,
    },
    UnknownArtifact {
        artifact: BuildArtifactId,
    },
    DuplicateGraphAttachment {
        generated_file: BuildGeneratedFileId,
    },
    DuplicatePlanEntry {
        generated_file: BuildGeneratedFileId,
    },
    MissingPlanEntry {
        generated_file: BuildGeneratedFileId,
    },
    UnattachedPlanEntry {
        generated_file: BuildGeneratedFileId,
    },
    GraphPathMismatch {
        generated_file: BuildGeneratedFileId,
    },
    GraphKindMismatch {
        generated_file: BuildGeneratedFileId,
    },
    UnsafePath {
        path: String,
    },
    DuplicatePath {
        path: String,
    },
    UnsupportedAction {
        generated_file: BuildGeneratedFileId,
    },
    UnsafeFilesystemEntry {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for GeneratedFileMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGraph { errors } => {
                write!(
                    formatter,
                    "build graph has {} validation error(s)",
                    errors.len()
                )
            }
            Self::UnknownArtifact { artifact } => {
                write!(formatter, "build graph has no artifact {artifact}")
            }
            Self::DuplicateGraphAttachment { generated_file } => write!(
                formatter,
                "artifact attaches generated file {generated_file} more than once"
            ),
            Self::DuplicatePlanEntry { generated_file } => write!(
                formatter,
                "materialization plan repeats generated file {generated_file}"
            ),
            Self::MissingPlanEntry { generated_file } => write!(
                formatter,
                "artifact-attached generated file {generated_file} has no plan entry"
            ),
            Self::UnattachedPlanEntry { generated_file } => write!(
                formatter,
                "materialization plan entry {generated_file} is not attached to the artifact"
            ),
            Self::GraphPathMismatch { generated_file } => write!(
                formatter,
                "materialization path differs from graph node {generated_file}"
            ),
            Self::GraphKindMismatch { generated_file } => write!(
                formatter,
                "materialization action differs from graph node {generated_file}"
            ),
            Self::UnsafePath { path } => {
                write!(
                    formatter,
                    "generated path {path:?} is not a safe relative path"
                )
            }
            Self::DuplicatePath { path } => {
                write!(formatter, "generated file path {path:?} is duplicated")
            }
            Self::UnsupportedAction { generated_file } => write!(
                formatter,
                "generated file {generated_file} requires tool execution, not materialization"
            ),
            Self::UnsafeFilesystemEntry { path } => write!(
                formatter,
                "generated output path crosses a symlink or non-directory at {}",
                path.display()
            ),
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "generated output I/O failed at {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for GeneratedFileMaterializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidGraph { .. }
            | Self::UnknownArtifact { .. }
            | Self::DuplicateGraphAttachment { .. }
            | Self::DuplicatePlanEntry { .. }
            | Self::MissingPlanEntry { .. }
            | Self::UnattachedPlanEntry { .. }
            | Self::GraphPathMismatch { .. }
            | Self::GraphKindMismatch { .. }
            | Self::UnsafePath { .. }
            | Self::DuplicatePath { .. }
            | Self::UnsupportedAction { .. }
            | Self::UnsafeFilesystemEntry { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedFileSet {
    definitions: Vec<GeneratedFileDefinition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedOutputDependencySet {
    outputs: Vec<GeneratedFileDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileInstallProjection {
    pub generated_file_name: String,
    pub install_name: String,
    pub install_path: String,
}

impl GeneratedFileInstallProjection {
    pub fn new(
        generated_file_name: impl Into<String>,
        install_name: impl Into<String>,
        install_path: impl Into<String>,
    ) -> Self {
        Self {
            generated_file_name: generated_file_name.into(),
            install_name: install_name.into(),
            install_path: install_path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemToolRequest {
    pub tool: String,
    pub args: Vec<String>,
    pub file_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemToolResult {
    pub tool: String,
    pub exit_status: i32,
    pub generated_outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenKind {
    FolToFol,
    Schema,
    AssetPreprocess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenRequest {
    pub kind: CodegenKind,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenResult {
    pub kind: CodegenKind,
    pub output: String,
}

impl GeneratedFileSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn definitions(&self) -> &[GeneratedFileDefinition] {
        &self.definitions
    }

    pub fn add(&mut self, definition: GeneratedFileDefinition) {
        self.definitions.push(definition);
    }
}

/// Materialize exactly the generated files attached to one checked artifact.
///
/// The output root must already exist and remain under the caller's control,
/// without adversarial concurrent mutation, for the duration of this call.
/// This narrow handoff executor is not an atomic publisher, cache, or install
/// engine. It validates an exact graph-to-plan
/// bijection, never invokes a shell or tool, never removes the output root, and
/// rejects absolute paths, parent traversal, duplicate destinations, and
/// existing symlink crossings before writing. Tool-output actions belong to a
/// separate bounded process executor and are rejected here.
pub fn materialize_generated_action_plan(
    graph: &BuildGraph,
    artifact: BuildArtifactId,
    output_root: &Path,
    plan: &GeneratedFileMaterializationPlan,
) -> Result<Vec<PathBuf>, GeneratedFileMaterializationError> {
    let graph_errors = graph.validate();
    if !graph_errors.is_empty() {
        return Err(GeneratedFileMaterializationError::InvalidGraph {
            errors: graph_errors,
        });
    }
    if graph.artifacts().get(artifact.index()).is_none() {
        return Err(GeneratedFileMaterializationError::UnknownArtifact { artifact });
    }

    let mut attached = BTreeSet::new();
    for generated_file in graph
        .artifact_inputs_for(artifact)
        .filter_map(|input| match input {
            BuildArtifactInput::GeneratedFile(id) => Some(id),
            BuildArtifactInput::Module(_) => None,
        })
    {
        if !attached.insert(generated_file) {
            return Err(
                GeneratedFileMaterializationError::DuplicateGraphAttachment { generated_file },
            );
        }
    }

    let mut planned = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in plan.entries() {
        if !planned.insert(entry.generated_file) {
            return Err(GeneratedFileMaterializationError::DuplicatePlanEntry {
                generated_file: entry.generated_file,
            });
        }
        if !attached.contains(&entry.generated_file) {
            return Err(GeneratedFileMaterializationError::UnattachedPlanEntry {
                generated_file: entry.generated_file,
            });
        }
        let graph_file = &graph.generated_files()[entry.generated_file.index()];
        if graph_file.name != entry.relative_path {
            return Err(GeneratedFileMaterializationError::GraphPathMismatch {
                generated_file: entry.generated_file,
            });
        }
        if graph_file.kind != action_graph_kind(&entry.action) {
            return Err(GeneratedFileMaterializationError::GraphKindMismatch {
                generated_file: entry.generated_file,
            });
        }
        validate_relative_path(&entry.relative_path)?;
        if !paths.insert(entry.relative_path.as_str()) {
            return Err(GeneratedFileMaterializationError::DuplicatePath {
                path: entry.relative_path.clone(),
            });
        }
        if matches!(entry.action, GeneratedFileAction::CaptureToolOutput { .. }) {
            return Err(GeneratedFileMaterializationError::UnsupportedAction {
                generated_file: entry.generated_file,
            });
        }
    }
    if let Some(generated_file) = attached.difference(&planned).next().copied() {
        return Err(GeneratedFileMaterializationError::MissingPlanEntry { generated_file });
    }

    reject_symlink_or_non_directory(output_root)?;
    for entry in plan.entries() {
        preflight_destination(output_root, Path::new(&entry.relative_path))?;
    }
    for entry in plan.entries() {
        prepare_destination(output_root, Path::new(&entry.relative_path))?;
    }

    let mut materialized = Vec::with_capacity(plan.entries().len());
    for entry in plan.entries() {
        let destination = output_root.join(&entry.relative_path);
        match &entry.action {
            GeneratedFileAction::Write { contents } => {
                std::fs::write(&destination, contents).map_err(|source| {
                    GeneratedFileMaterializationError::Io {
                        path: destination.clone(),
                        source,
                    }
                })?;
            }
            GeneratedFileAction::Copy { source_path } => {
                std::fs::copy(source_path, &destination).map_err(|source| {
                    GeneratedFileMaterializationError::Io {
                        path: destination.clone(),
                        source,
                    }
                })?;
            }
            GeneratedFileAction::CaptureToolOutput { .. } => {
                unreachable!("tool actions were rejected before materialization began")
            }
        }
        materialized.push(destination);
    }
    Ok(materialized)
}

fn action_graph_kind(action: &GeneratedFileAction) -> BuildGeneratedFileKind {
    match action {
        GeneratedFileAction::Write { .. } => BuildGeneratedFileKind::Write,
        GeneratedFileAction::Copy { .. } => BuildGeneratedFileKind::Copy,
        GeneratedFileAction::CaptureToolOutput { .. } => BuildGeneratedFileKind::CaptureOutput,
    }
}

fn validate_relative_path(path: &str) -> Result<(), GeneratedFileMaterializationError> {
    let path_value = Path::new(path);
    if path.is_empty()
        || !path_value.is_relative()
        || path_value
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(GeneratedFileMaterializationError::UnsafePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn reject_symlink_or_non_directory(path: &Path) -> Result<(), GeneratedFileMaterializationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        GeneratedFileMaterializationError::Io {
            path: path.to_owned(),
            source,
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn prepare_destination(
    output_root: &Path,
    relative_path: &Path,
) -> Result<(), GeneratedFileMaterializationError> {
    let mut current = output_root.to_owned();
    if let Some(parent) = relative_path.parent() {
        for component in parent.components() {
            let Component::Normal(component) = component else {
                unreachable!("relative path was validated before materialization")
            };
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        return Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry {
                            path: current,
                        });
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&current).map_err(|source| {
                        GeneratedFileMaterializationError::Io {
                            path: current.clone(),
                            source,
                        }
                    })?;
                }
                Err(source) => {
                    return Err(GeneratedFileMaterializationError::Io {
                        path: current,
                        source,
                    });
                }
            }
        }
    }
    let destination = output_root.join(relative_path);
    if let Ok(metadata) = std::fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() || metadata.is_dir() {
            return Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry {
                path: destination,
            });
        }
    }
    Ok(())
}

fn preflight_destination(
    output_root: &Path,
    relative_path: &Path,
) -> Result<(), GeneratedFileMaterializationError> {
    let mut current = output_root.to_owned();
    if let Some(parent) = relative_path.parent() {
        for component in parent.components() {
            let Component::Normal(component) = component else {
                unreachable!("relative path was validated before materialization")
            };
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry {
                        path: current,
                    });
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(source) => {
                    return Err(GeneratedFileMaterializationError::Io {
                        path: current,
                        source,
                    });
                }
            }
        }
    }
    let destination = output_root.join(relative_path);
    if let Ok(metadata) = std::fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() || metadata.is_dir() {
            return Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry {
                path: destination,
            });
        }
    }
    Ok(())
}

impl GeneratedOutputDependencySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, definition: GeneratedFileDefinition) {
        self.outputs.push(definition);
    }

    pub fn get(&self, name: &str) -> Option<&GeneratedFileDefinition> {
        self.outputs
            .iter()
            .find(|definition| definition.name == name)
    }

    pub fn outputs(&self) -> &[GeneratedFileDefinition] {
        &self.outputs
    }
}

#[cfg(test)]
mod tests {
    use super::{
        materialize_generated_action_plan, CodegenKind, CodegenRequest, CodegenResult,
        GeneratedFileAction, GeneratedFileDefinition, GeneratedFileInstallProjection,
        GeneratedFileMaterialization, GeneratedFileMaterializationError,
        GeneratedFileMaterializationPlan, GeneratedFileSet, GeneratedOutputDependencySet,
        SystemToolRequest, SystemToolResult,
    };
    use crate::graph::{
        BuildArtifactId, BuildArtifactKind, BuildGeneratedFileId, BuildGeneratedFileKind,
        BuildGraph,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_ORDINAL: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let ordinal = TEMP_ORDINAL.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fol-build-{label}-{}-{ordinal}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create temporary root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn generated_file_set_starts_empty() {
        let set = GeneratedFileSet::new();

        assert!(set.definitions().is_empty());
    }

    #[test]
    fn generated_file_set_preserves_inserted_shell_definitions() {
        let mut set = GeneratedFileSet::new();
        set.add(GeneratedFileDefinition {
            name: "version".to_string(),
            relative_path: "gen/version.fol".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"con version = \"0.1.0\";".to_vec(),
            },
        });

        assert_eq!(set.definitions().len(), 1);
        assert_eq!(set.definitions()[0].name, "version");
        assert_eq!(set.definitions()[0].relative_path, "gen/version.fol");
        assert!(matches!(
            set.definitions()[0].action,
            GeneratedFileAction::Write { .. }
        ));
    }

    #[test]
    fn generated_file_materializer_writes_nested_actions_without_shells() {
        let root = TempRoot::new("materialize");
        let source = root.path().join("source.txt");
        std::fs::write(&source, "copied").expect("write copy source");
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let bindings =
            graph.add_generated_file(BuildGeneratedFileKind::Write, "generated/src/lib.rs");
        let metadata =
            graph.add_generated_file(BuildGeneratedFileKind::Copy, "generated/metadata.txt");
        let binary = graph.add_generated_file(BuildGeneratedFileKind::Write, "generated/blob.bin");
        graph.add_artifact_generated_file_input(artifact, bindings);
        graph.add_artifact_generated_file_input(artifact, metadata);
        graph.add_artifact_generated_file_input(artifact, binary);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: bindings,
            relative_path: "generated/src/lib.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"pub fn value() {}\n".to_vec(),
            },
        });
        plan.add(GeneratedFileMaterialization {
            generated_file: metadata,
            relative_path: "generated/metadata.txt".to_string(),
            action: GeneratedFileAction::Copy {
                source_path: source.display().to_string(),
            },
        });
        plan.add(GeneratedFileMaterialization {
            generated_file: binary,
            relative_path: "generated/blob.bin".to_string(),
            action: GeneratedFileAction::Write {
                contents: vec![0, 0xff, 1],
            },
        });

        let output = root.path().join("out");
        std::fs::create_dir(&output).expect("create trusted output root");
        let written = materialize_generated_action_plan(&graph, artifact, &output, &plan)
            .expect("materialize files");
        assert_eq!(written.len(), 3);
        assert_eq!(
            std::fs::read_to_string(output.join("generated/src/lib.rs")).unwrap(),
            "pub fn value() {}\n"
        );
        assert_eq!(
            std::fs::read_to_string(output.join("generated/metadata.txt")).unwrap(),
            "copied"
        );
        assert_eq!(
            std::fs::read(output.join("generated/blob.bin")).unwrap(),
            [0, 0xff, 1]
        );
    }

    #[test]
    fn generated_file_materializer_rejects_the_action_set_before_writing() {
        let root = TempRoot::new("reject");
        for (index, unsafe_path) in ["", "/tmp/escape", "../escape", "safe/../escape", "./file"]
            .into_iter()
            .enumerate()
        {
            let mut graph = BuildGraph::new();
            let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
            let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, unsafe_path);
            graph.add_artifact_generated_file_input(artifact, generated);
            let mut plan = GeneratedFileMaterializationPlan::new();
            plan.add(GeneratedFileMaterialization {
                generated_file: generated,
                relative_path: unsafe_path.to_string(),
                action: GeneratedFileAction::Write {
                    contents: b"bad".to_vec(),
                },
            });
            let output = root.path().join(format!("out-{index}"));
            std::fs::create_dir(&output).expect("create trusted output root");
            assert!(matches!(
                materialize_generated_action_plan(&graph, artifact, &output, &plan),
                Err(GeneratedFileMaterializationError::UnsafePath { .. })
            ));
        }

        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let safe = graph.add_generated_file(BuildGeneratedFileKind::Write, "safe.txt");
        let tool = graph.add_generated_file(BuildGeneratedFileKind::CaptureOutput, "tool.txt");
        graph.add_artifact_generated_file_input(artifact, safe);
        graph.add_artifact_generated_file_input(artifact, tool);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: safe,
            relative_path: "safe.txt".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"must not be written".to_vec(),
            },
        });
        plan.add(GeneratedFileMaterialization {
            generated_file: tool,
            relative_path: "tool.txt".to_string(),
            action: GeneratedFileAction::CaptureToolOutput {
                tool: "generator".to_string(),
                args: Vec::new(),
                file_args: Vec::new(),
                env: BTreeMap::new(),
            },
        });
        let output = root.path().join("unsupported");
        std::fs::create_dir(&output).expect("create trusted output root");
        assert!(matches!(
            materialize_generated_action_plan(&graph, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::UnsupportedAction { .. })
        ));
        assert!(!output.join("safe.txt").exists());
    }

    #[test]
    fn generated_file_materializer_requires_an_exact_graph_plan_bijection() {
        let root = TempRoot::new("bijection");
        let output = root.path().join("out");
        std::fs::create_dir(&output).expect("create trusted output root");
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        graph.add_artifact_generated_file_input(artifact, generated);

        assert!(matches!(
            materialize_generated_action_plan(
                &graph,
                artifact,
                &output,
                &GeneratedFileMaterializationPlan::new(),
            ),
            Err(GeneratedFileMaterializationError::MissingPlanEntry { .. })
        ));

        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "different.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"bad".to_vec(),
            },
        });
        assert!(matches!(
            materialize_generated_action_plan(&graph, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::GraphPathMismatch { .. })
        ));
        assert!(std::fs::read_dir(&output).unwrap().next().is_none());
    }

    #[test]
    fn generated_file_materializer_rejects_all_graph_plan_mismatch_shapes() {
        let root = TempRoot::new("graph-mismatch");
        let output = root.path().join("out");
        std::fs::create_dir(&output).expect("create trusted output root");

        let mut duplicate_attachment = BuildGraph::new();
        let artifact = duplicate_attachment.add_artifact(BuildArtifactKind::Executable, "app");
        let generated =
            duplicate_attachment.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        duplicate_attachment.add_artifact_generated_file_input(artifact, generated);
        duplicate_attachment.add_artifact_generated_file_input(artifact, generated);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "binding.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"binding".to_vec(),
            },
        });
        assert!(matches!(
            materialize_generated_action_plan(&duplicate_attachment, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::DuplicateGraphAttachment { .. })
        ));

        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        graph.add_artifact_generated_file_input(artifact, generated);
        let entry = GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "binding.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"binding".to_vec(),
            },
        };
        let mut duplicate_plan = GeneratedFileMaterializationPlan::new();
        duplicate_plan.add(entry.clone());
        duplicate_plan.add(entry);
        assert!(matches!(
            materialize_generated_action_plan(&graph, artifact, &output, &duplicate_plan),
            Err(GeneratedFileMaterializationError::DuplicatePlanEntry { .. })
        ));

        let mut duplicate_path = BuildGraph::new();
        let artifact = duplicate_path.add_artifact(BuildArtifactKind::Executable, "app");
        let first = duplicate_path.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        let second = duplicate_path.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        duplicate_path.add_artifact_generated_file_input(artifact, first);
        duplicate_path.add_artifact_generated_file_input(artifact, second);
        let mut plan = GeneratedFileMaterializationPlan::new();
        for generated_file in [first, second] {
            plan.add(GeneratedFileMaterialization {
                generated_file,
                relative_path: "binding.rs".to_string(),
                action: GeneratedFileAction::Write {
                    contents: b"binding".to_vec(),
                },
            });
        }
        assert!(matches!(
            materialize_generated_action_plan(&duplicate_path, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::DuplicatePath { .. })
        ));

        let mut unattached = BuildGraph::new();
        let artifact = unattached.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = unattached.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "binding.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"binding".to_vec(),
            },
        });
        assert!(matches!(
            materialize_generated_action_plan(&unattached, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::UnattachedPlanEntry { .. })
        ));

        let mut wrong_kind = BuildGraph::new();
        let artifact = wrong_kind.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = wrong_kind.add_generated_file(BuildGeneratedFileKind::Copy, "binding.rs");
        wrong_kind.add_artifact_generated_file_input(artifact, generated);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "binding.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"binding".to_vec(),
            },
        });
        assert!(matches!(
            materialize_generated_action_plan(&wrong_kind, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::GraphKindMismatch { .. })
        ));

        assert!(matches!(
            materialize_generated_action_plan(
                &BuildGraph::new(),
                BuildArtifactId::from_index(1),
                &output,
                &GeneratedFileMaterializationPlan::new(),
            ),
            Err(GeneratedFileMaterializationError::UnknownArtifact { .. })
        ));

        let mut invalid = BuildGraph::new();
        let artifact = invalid.add_artifact(BuildArtifactKind::Executable, "app");
        invalid.add_artifact_generated_file_input(artifact, BuildGeneratedFileId::from_index(99));
        assert!(matches!(
            materialize_generated_action_plan(
                &invalid,
                artifact,
                &output,
                &GeneratedFileMaterializationPlan::new(),
            ),
            Err(GeneratedFileMaterializationError::InvalidGraph { .. })
        ));
        assert!(std::fs::read_dir(&output).unwrap().next().is_none());
    }

    #[test]
    fn generated_file_materializer_requires_an_existing_root() {
        let root = TempRoot::new("missing-root");
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "binding.rs");
        graph.add_artifact_generated_file_input(artifact, generated);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "binding.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"binding".to_vec(),
            },
        });

        assert!(matches!(
            materialize_generated_action_plan(
                &graph,
                artifact,
                &root.path().join("missing"),
                &plan,
            ),
            Err(GeneratedFileMaterializationError::Io { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn generated_file_materializer_rejects_symlink_crossings() {
        use std::os::unix::fs::symlink;

        let root = TempRoot::new("symlink");
        let output = root.path().join("out");
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, output.join("generated")).unwrap();
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "generated/lib.rs");
        graph.add_artifact_generated_file_input(artifact, generated);
        let mut plan = GeneratedFileMaterializationPlan::new();
        plan.add(GeneratedFileMaterialization {
            generated_file: generated,
            relative_path: "generated/lib.rs".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"bad".to_vec(),
            },
        });

        assert!(matches!(
            materialize_generated_action_plan(&graph, artifact, &output, &plan),
            Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry { .. })
        ));
        assert!(!outside.join("lib.rs").exists());

        let root_link = root.path().join("root-link");
        symlink(&outside, &root_link).unwrap();
        assert!(matches!(
            materialize_generated_action_plan(&graph, artifact, &root_link, &plan),
            Err(GeneratedFileMaterializationError::UnsafeFilesystemEntry { .. })
        ));
    }

    #[test]
    fn generated_file_actions_cover_write_copy_and_captured_outputs() {
        let write = GeneratedFileAction::Write {
            contents: b"hello".to_vec(),
        };
        let copy = GeneratedFileAction::Copy {
            source_path: "assets/logo.svg".to_string(),
        };
        let capture = GeneratedFileAction::CaptureToolOutput {
            tool: "schema-gen".to_string(),
            args: vec!["api.yaml".to_string()],
            file_args: vec!["schema/api.yaml".to_string()],
            env: BTreeMap::from([("MODE".to_string(), "strict".to_string())]),
        };

        assert!(matches!(write, GeneratedFileAction::Write { .. }));
        assert!(matches!(copy, GeneratedFileAction::Copy { .. }));
        assert!(matches!(
            capture,
            GeneratedFileAction::CaptureToolOutput { .. }
        ));
    }

    #[test]
    fn generated_file_install_projection_keeps_install_helper_metadata() {
        let projection =
            GeneratedFileInstallProjection::new("config", "install-config", "share/config.json");

        assert_eq!(projection.generated_file_name, "config");
        assert_eq!(projection.install_name, "install-config");
        assert_eq!(projection.install_path, "share/config.json");
    }

    #[test]
    fn system_tool_models_keep_requests_and_results_stable() {
        let request = SystemToolRequest {
            tool: "flatc".to_string(),
            args: vec!["--fol".to_string(), "schema.fbs".to_string()],
            file_args: vec!["schema/api.fbs".to_string()],
            env: BTreeMap::from([("FLAVOR".to_string(), "strict".to_string())]),
            outputs: vec!["gen/schema.fol".to_string()],
        };
        let result = SystemToolResult {
            tool: "flatc".to_string(),
            exit_status: 0,
            generated_outputs: vec!["gen/schema.fol".to_string()],
        };

        assert_eq!(request.tool, "flatc");
        assert_eq!(request.file_args, vec!["schema/api.fbs".to_string()]);
        assert_eq!(
            request.env.get("FLAVOR").map(String::as_str),
            Some("strict")
        );
        assert_eq!(request.outputs, vec!["gen/schema.fol".to_string()]);
        assert_eq!(result.exit_status, 0);
        assert_eq!(result.generated_outputs.len(), 1);
    }

    #[test]
    fn codegen_models_cover_fol_schema_and_asset_flows() {
        let fol = CodegenRequest {
            kind: CodegenKind::FolToFol,
            input: "schema/source.fol".to_string(),
            output: "gen/source.fol".to_string(),
        };
        let schema = CodegenRequest {
            kind: CodegenKind::Schema,
            input: "schema/api.yaml".to_string(),
            output: "gen/api.fol".to_string(),
        };
        let asset = CodegenResult {
            kind: CodegenKind::AssetPreprocess,
            output: "gen/logo.bin".to_string(),
        };

        assert!(matches!(fol.kind, CodegenKind::FolToFol));
        assert!(matches!(schema.kind, CodegenKind::Schema));
        assert!(matches!(asset.kind, CodegenKind::AssetPreprocess));
        assert_eq!(asset.output, "gen/logo.bin");
    }

    #[test]
    fn generated_output_dependency_set_supports_named_lookup() {
        let mut outputs = GeneratedOutputDependencySet::new();
        outputs.add(GeneratedFileDefinition {
            name: "bindings".to_string(),
            relative_path: "gen/bindings.fol".to_string(),
            action: GeneratedFileAction::Write {
                contents: b"generated".to_vec(),
            },
        });

        assert_eq!(outputs.outputs().len(), 1);
        assert_eq!(
            outputs
                .get("bindings")
                .map(|definition| definition.relative_path.as_str()),
            Some("gen/bindings.fol")
        );
        assert!(outputs.get("missing").is_none());
    }
}
