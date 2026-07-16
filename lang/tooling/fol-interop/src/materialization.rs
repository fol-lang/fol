use fol_build::{
    BuildArtifactId, BuildGeneratedFileKind, BuildGraph, GeneratedFileAction,
    GeneratedFileMaterialization, GeneratedFileMaterializationPlan,
};
use gerc::GenerationBundle;

use crate::anchor::H7InteropAnchor;

/// Graph-bound write plan for one deterministic GERC raw-binding crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InteropGeneratedPlan {
    artifact: BuildArtifactId,
    raw_crate_root: String,
    anchor_crate_root: Option<String>,
    materialization: GeneratedFileMaterializationPlan,
}

impl InteropGeneratedPlan {
    pub fn raw_crate_root(&self) -> &str {
        &self.raw_crate_root
    }

    pub fn anchor_crate_root(&self) -> Option<&str> {
        self.anchor_crate_root.as_deref()
    }

    pub fn materialization(&self) -> &GeneratedFileMaterializationPlan {
        &self.materialization
    }
}

/// Attach every GERC file to the authoritative FOL graph as an exact byte
/// write. The graph is mutated only after all paths and collisions preflight.
pub(crate) fn attach_generated_bindings(
    graph: &mut BuildGraph,
    artifact: BuildArtifactId,
    bundle: &GenerationBundle,
) -> Result<InteropGeneratedPlan, InteropMaterializationPlanError> {
    if graph.artifacts().get(artifact.index()).is_none() {
        return Err(InteropMaterializationPlanError::UnknownArtifact(artifact));
    }

    let raw_crate_root = format!(
        "interop/{}/{}",
        artifact.index(),
        bundle.manifest().generation_fingerprint()
    );
    let mut files = Vec::with_capacity(bundle.files().files().len());
    for file in bundle.files().files() {
        let generated_path = file.path().as_path().to_str().ok_or_else(|| {
            InteropMaterializationPlanError::NonUtf8GeneratedPath(file.path().as_path().to_owned())
        })?;
        let relative_path = format!("{raw_crate_root}/{generated_path}");
        if graph
            .generated_files()
            .iter()
            .any(|existing| existing.name == relative_path)
            || files
                .iter()
                .any(|(existing, _): &(String, Vec<u8>)| existing == &relative_path)
        {
            return Err(InteropMaterializationPlanError::DuplicatePath(
                relative_path,
            ));
        }
        files.push((relative_path, file.contents().to_vec()));
    }

    let mut materialization = GeneratedFileMaterializationPlan::new();
    for (relative_path, contents) in files {
        let generated_file =
            graph.add_generated_file(BuildGeneratedFileKind::Write, relative_path.clone());
        graph.add_artifact_generated_file_input(artifact, generated_file);
        materialization.add(GeneratedFileMaterialization {
            generated_file,
            relative_path,
            action: GeneratedFileAction::Write { contents },
        });
    }

    Ok(InteropGeneratedPlan {
        artifact,
        raw_crate_root,
        anchor_crate_root: None,
        materialization,
    })
}

/// Add the FOL-owned H7 provider-call wrapper to the same exact graph/plan.
pub(crate) fn attach_h7_anchor(
    graph: &mut BuildGraph,
    artifact: BuildArtifactId,
    plan: &mut InteropGeneratedPlan,
    anchor: &H7InteropAnchor,
) -> Result<(), InteropMaterializationPlanError> {
    if plan.artifact != artifact {
        return Err(InteropMaterializationPlanError::ArtifactMismatch {
            expected: plan.artifact,
            actual: artifact,
        });
    }
    if graph.artifacts().get(artifact.index()).is_none() {
        return Err(InteropMaterializationPlanError::UnknownArtifact(artifact));
    }
    if plan.anchor_crate_root.is_some() {
        return Err(InteropMaterializationPlanError::AnchorAlreadyAttached);
    }

    let anchor_crate_root = format!("{}/anchor", plan.raw_crate_root);
    let relative_path = format!("{anchor_crate_root}/src/lib.rs");
    if graph
        .generated_files()
        .iter()
        .any(|existing| existing.name == relative_path)
        || plan
            .materialization
            .entries()
            .iter()
            .any(|entry| entry.relative_path == relative_path)
    {
        return Err(InteropMaterializationPlanError::DuplicatePath(
            relative_path,
        ));
    }

    let generated_file =
        graph.add_generated_file(BuildGeneratedFileKind::Write, relative_path.clone());
    graph.add_artifact_generated_file_input(artifact, generated_file);
    plan.materialization.add(GeneratedFileMaterialization {
        generated_file,
        relative_path,
        action: GeneratedFileAction::Write {
            contents: anchor.source().to_vec(),
        },
    });
    plan.anchor_crate_root = Some(anchor_crate_root);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteropMaterializationPlanError {
    UnknownArtifact(BuildArtifactId),
    ArtifactMismatch {
        expected: BuildArtifactId,
        actual: BuildArtifactId,
    },
    AnchorAlreadyAttached,
    DuplicatePath(String),
    NonUtf8GeneratedPath(std::path::PathBuf),
}

impl std::fmt::Display for InteropMaterializationPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArtifact(artifact) => {
                write!(
                    formatter,
                    "interop attachment references unknown {artifact}"
                )
            }
            Self::ArtifactMismatch { expected, actual } => write!(
                formatter,
                "interop plan belongs to {expected}, not requested {actual}"
            ),
            Self::AnchorAlreadyAttached => {
                formatter.write_str("H7 interop anchor is already attached")
            }
            Self::DuplicatePath(path) => {
                write!(formatter, "interop generated path is duplicated: {path}")
            }
            Self::NonUtf8GeneratedPath(path) => write!(
                formatter,
                "GERC generated path cannot be represented by the FOL graph: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for InteropMaterializationPlanError {}
