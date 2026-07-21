use crate::{EditorConfig, EditorDocument, EditorError, EditorErrorKind, EditorResult};
use fol_package::{
    build_api::DependencySourceKind, build_artifact::BuildArtifactFolModel,
    build_runtime::BuildRuntimeArtifact, build_runtime::BuildRuntimeDependency,
    evaluate_build_source, BuildEvaluationInputs, BuildEvaluationRequest,
};
use fol_typecheck::TypecheckCapabilityModel;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorWorkspaceRoots {
    pub package_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub analysis_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorWorkspaceMapping {
    pub document_path: PathBuf,
    pub package_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub analysis_root: PathBuf,
    /// Canonical directory recursively owned by the selected build artifact.
    ///
    /// This is intentionally separate from `package_root`: overlays still need
    /// the formal package root for declared dependencies, while compiler-backed
    /// source analysis must stay inside the selected artifact's source scope.
    pub artifact_source_scope: Option<PathBuf>,
    pub active_fol_model: Option<TypecheckCapabilityModel>,
    /// Bundled-standard aliases that were actually registered by the evaluated
    /// build graph for this editor configuration.
    ///
    /// Keep this evaluated set beside the model inference so overlay setup does
    /// not reparse static declarations and accidentally expose an inactive
    /// conditional dependency.
    pub active_internal_standard_aliases: Vec<String>,
    /// The build graph declares artifacts, but this document cannot be assigned
    /// one honest capability model (for example because mixed-model scopes
    /// overlap, an artifact escapes the package, or the document is outside all
    /// scopes in a mixed package).
    pub fol_model_scope_unresolved: bool,
}

#[derive(Debug)]
pub struct EditorAnalysisOverlay {
    temp_root: PathBuf,
    analysis_root: PathBuf,
    package_root: Option<PathBuf>,
    artifact_source_scope: Option<PathBuf>,
    document_path: PathBuf,
}

impl EditorAnalysisOverlay {
    pub fn analysis_root(&self) -> &Path {
        &self.analysis_root
    }

    pub fn package_root(&self) -> Option<&Path> {
        self.package_root.as_deref()
    }

    pub fn artifact_source_scope(&self) -> Option<&Path> {
        self.artifact_source_scope.as_deref()
    }

    pub fn document_path(&self) -> &Path {
        &self.document_path
    }
}

impl Drop for EditorAnalysisOverlay {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_root);
    }
}

pub fn map_document_workspace(
    path: &Path,
    config: &EditorConfig,
) -> EditorResult<EditorWorkspaceMapping> {
    let absolute = canonical_document_path(path)?;
    let roots = discover_workspace_roots(
        absolute.parent().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidDocumentPath,
                format!("document '{}' has no parent directory", absolute.display()),
            )
        })?,
        config,
    );
    let inference = roots
        .package_root
        .as_ref()
        .map(|package_root| recover_active_fol_model(package_root, &absolute))
        .unwrap_or_default();
    Ok(EditorWorkspaceMapping {
        document_path: absolute,
        artifact_source_scope: inference.artifact_source_scope,
        active_fol_model: inference.active_fol_model,
        active_internal_standard_aliases: inference.active_internal_standard_aliases,
        fol_model_scope_unresolved: inference.unresolved,
        package_root: roots.package_root,
        workspace_root: roots.workspace_root,
        analysis_root: roots.analysis_root,
    })
}

pub(crate) fn canonical_document_path(path: &Path) -> EditorResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::fs::canonicalize(path).map_err(|error| {
            EditorError::new(
                EditorErrorKind::InvalidDocumentPath,
                format!("failed to resolve '{}': {error}", path.display()),
            )
        })
    }
}

pub(crate) fn discover_workspace_roots(
    directory: &Path,
    config: &EditorConfig,
) -> EditorWorkspaceRoots {
    let package_root = find_upward_marker(directory, "build.fol");
    let workspace_root = config
        .root_markers
        .iter()
        .filter(|marker| marker.as_str() != "build.fol")
        .find_map(|marker| find_upward_marker(directory, marker))
        // Test runners and shell environments may put a generic `.git`
        // directory directly in their ambient temporary directory. Treating
        // that shared scratch directory as a FOL workspace makes overlays copy
        // unrelated temporary trees and can recursively copy themselves.
        // A real workspace nested under the temporary directory still wins
        // because its root is more specific than the ambient temp root.
        .filter(|root| !std::env::temp_dir().starts_with(root));
    let analysis_root = workspace_root
        .clone()
        .or_else(|| package_root.clone())
        .unwrap_or_else(|| directory.to_path_buf());
    EditorWorkspaceRoots {
        package_root,
        workspace_root,
        analysis_root,
    }
}

pub fn materialize_analysis_overlay(
    mapping: &EditorWorkspaceMapping,
    document: &EditorDocument,
) -> EditorResult<EditorAnalysisOverlay> {
    // Multi-package editor analysis needs the full analysis tree, not only the
    // current package root, so sibling local-import targets remain available.
    let overlay_source_root = &mapping.analysis_root;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!(
        "fol_editor_overlay_{}_{}_{}",
        std::process::id(),
        mapping
            .document_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("doc"),
        stamp
    ));
    fs::create_dir_all(&temp_root).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to create overlay root '{}': {error}",
                temp_root.display()
            ),
        )
    })?;

    copy_directory_tree(overlay_source_root, &temp_root)?;

    let relative_document = relative_overlay_path(&mapping.document_path, overlay_source_root)
        .ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "document '{}' is not inside analysis root '{}'",
                    mapping.document_path.display(),
                    overlay_source_root.display()
                ),
            )
        })?;
    let overlay_document = temp_root.join(relative_document);
    if let Some(parent) = overlay_document.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to create overlay parent '{}': {error}",
                    parent.display()
                ),
            )
        })?;
    }
    fs::write(&overlay_document, &document.text).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to write overlay document '{}': {error}",
                overlay_document.display()
            ),
        )
    })?;

    let package_root = mapping
        .package_root
        .as_ref()
        .and_then(|package_root| relative_overlay_path(package_root, overlay_source_root))
        .map(|relative| temp_root.join(relative));
    let artifact_source_scope = match mapping.artifact_source_scope.as_ref() {
        Some(source_scope) => {
            let relative =
                relative_overlay_path(source_scope, overlay_source_root).ok_or_else(|| {
                    EditorError::new(
                        EditorErrorKind::Internal,
                        format!(
                            "artifact source scope '{}' is not inside analysis root '{}'",
                            source_scope.display(),
                            overlay_source_root.display()
                        ),
                    )
                })?;
            Some(temp_root.join(relative))
        }
        None => None,
    };

    // Source imports intentionally address the bundled standard library through
    // the dependency alias declared in build.fol (`use alias: pkg = {"alias"}`).
    // Normal builds materialize that alias through `fol pack fetch`, but editor
    // analysis must also work in a freshly checked-out package before a fetch has
    // populated `.fol/pkg`. Materialize only aliases actually registered by
    // this build evaluation, and only inside the disposable analysis overlay.
    // Core artifacts still need the active alias to resolve so typecheck can
    // publish the intended model-boundary diagnostic for an illegal import.
    if let Some(package_root) = package_root.as_deref() {
        materialize_active_internal_standard_aliases(
            package_root,
            &mapping.active_internal_standard_aliases,
        )?;
    }

    Ok(EditorAnalysisOverlay {
        temp_root: temp_root.clone(),
        analysis_root: temp_root,
        package_root,
        artifact_source_scope,
        document_path: overlay_document,
    })
}

fn relative_overlay_path(path: &Path, root: &Path) -> Option<PathBuf> {
    path.strip_prefix(root)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| {
            let canonical_root = root.canonicalize().ok()?;
            let canonical_path = path.canonicalize().ok()?;
            canonical_path
                .strip_prefix(canonical_root)
                .ok()
                .map(Path::to_path_buf)
        })
}

fn find_upward_marker(start: &Path, marker: &str) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(path) = current {
        let candidate = path.join(marker);
        if candidate.is_file() || candidate.is_dir() {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

fn copy_directory_tree(from: &Path, to: &Path) -> EditorResult<()> {
    for entry in fs::read_dir(from).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to read analysis root '{}': {error}", from.display()),
        )
    })? {
        let entry = entry.map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to enumerate analysis root '{}': {error}",
                    from.display()
                ),
            )
        })?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to inspect '{}': {error}", source.display()),
            )
        })?;
        // Analysis overlays only need source packages, not build artifacts or
        // version-control metadata. When a document resolves its analysis root
        // to a repository root (for example via a `.git` root marker), copying
        // the live `target/` build tree is both wasteful and racy: cargo can
        // rewrite or delete object files mid-copy, surfacing spurious
        // "No such file" failures. Skip those non-source directories entirely.
        if file_type.is_dir()
            && entry.file_name().to_str().is_some_and(|name| {
                matches!(
                    name,
                    "target" | ".tmp" | ".git" | ".jj" | ".hg" | ".svn" | "node_modules"
                )
            })
        {
            continue;
        }
        if file_type.is_dir() {
            fs::create_dir_all(&target).map_err(|error| {
                EditorError::new(
                    EditorErrorKind::Internal,
                    format!("failed to create '{}': {error}", target.display()),
                )
            })?;
            copy_directory_tree(&source, &target)?;
        } else if file_type.is_file() {
            fs::copy(&source, &target).map_err(|error| {
                EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to copy '{}' to '{}': {error}",
                        source.display(),
                        target.display()
                    ),
                )
            })?;
        }
    }
    Ok(())
}

fn materialize_active_internal_standard_aliases(
    package_root: &Path,
    aliases: &[String],
) -> EditorResult<()> {
    if aliases.is_empty() {
        return Ok(());
    }
    let Some(std_root) = fol_package::available_bundled_std_root() else {
        // The resolver will produce the canonical missing-bundled-std
        // diagnostic if a declared internal dependency cannot be found.
        return Ok(());
    };

    for alias in aliases {
        let alias_root = package_root.join(".fol/pkg").join(alias);
        if alias_root.exists() {
            continue;
        }
        fs::create_dir_all(&alias_root).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to create bundled std analysis alias '{}': {error}",
                    alias_root.display()
                ),
            )
        })?;
        copy_directory_tree(&std_root, &alias_root)?;
    }

    Ok(())
}

#[derive(Debug, Default)]
struct ActiveFolModelInference {
    active_fol_model: Option<TypecheckCapabilityModel>,
    artifact_source_scope: Option<PathBuf>,
    active_internal_standard_aliases: Vec<String>,
    unresolved: bool,
}

#[derive(Debug)]
struct EvaluatedArtifactSourceScope {
    source_scope: PathBuf,
    active_fol_model: TypecheckCapabilityModel,
}

fn recover_active_fol_model(package_root: &Path, document_path: &Path) -> ActiveFolModelInference {
    let build_path = package_root.join("build.fol");
    let Some(build_source) = fs::read_to_string(&build_path).ok() else {
        return ActiveFolModelInference {
            unresolved: true,
            ..ActiveFolModelInference::default()
        };
    };
    let Some(package_root_text) = package_root.to_str().map(str::to_string) else {
        return ActiveFolModelInference {
            unresolved: true,
            ..ActiveFolModelInference::default()
        };
    };
    let Some(evaluated) = evaluate_build_source(
        &BuildEvaluationRequest {
            package_root: package_root_text.clone(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root_text,
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        },
        &build_path,
        &build_source,
    )
    .ok()
    .flatten() else {
        return ActiveFolModelInference {
            unresolved: true,
            ..ActiveFolModelInference::default()
        };
    };
    let active_internal_standard_aliases = evaluated
        .evaluated
        .dependencies
        .iter()
        .filter(|dependency| is_internal_standard_dependency(dependency))
        .map(|dependency| dependency.alias.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if document_path.file_name().and_then(|name| name.to_str()) == Some("build.fol") {
        return ActiveFolModelInference {
            active_internal_standard_aliases,
            ..ActiveFolModelInference::default()
        };
    }
    let mut inference = infer_active_fol_model(
        package_root,
        document_path,
        &evaluated.evaluated.artifacts,
        &evaluated.evaluated.dependencies,
    );
    inference.active_internal_standard_aliases = active_internal_standard_aliases;
    inference
}

fn infer_active_fol_model(
    package_root: &Path,
    document_path: &Path,
    artifacts: &[BuildRuntimeArtifact],
    dependencies: &[BuildRuntimeDependency],
) -> ActiveFolModelInference {
    if artifacts.is_empty() {
        return ActiveFolModelInference::default();
    }

    let Some(canonical_package_root) = package_root.canonicalize().ok() else {
        return ActiveFolModelInference {
            unresolved: true,
            ..ActiveFolModelInference::default()
        };
    };
    let canonical_document = document_path
        .canonicalize()
        .unwrap_or_else(|_| document_path.to_path_buf());
    let mut evaluated_scopes = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        if artifact.root_module.trim().is_empty() {
            return ActiveFolModelInference {
                unresolved: true,
                ..ActiveFolModelInference::default()
            };
        }
        let Some(canonical_root) = package_root.join(&artifact.root_module).canonicalize().ok()
        else {
            return ActiveFolModelInference {
                unresolved: true,
                ..ActiveFolModelInference::default()
            };
        };
        if !canonical_root.is_file() || !canonical_root.starts_with(&canonical_package_root) {
            return ActiveFolModelInference {
                unresolved: true,
                ..ActiveFolModelInference::default()
            };
        }
        let Some(source_scope) = canonical_root.parent().map(Path::to_path_buf) else {
            return ActiveFolModelInference {
                unresolved: true,
                ..ActiveFolModelInference::default()
            };
        };
        evaluated_scopes.push(EvaluatedArtifactSourceScope {
            source_scope,
            active_fol_model: typecheck_model_for_build_model(artifact.fol_model, dependencies),
        });
    }

    for (index, left) in evaluated_scopes.iter().enumerate() {
        for right in evaluated_scopes.iter().skip(index + 1) {
            if left.active_fol_model != right.active_fol_model
                && artifact_scopes_overlap(&left.source_scope, &right.source_scope)
            {
                return ActiveFolModelInference {
                    unresolved: true,
                    ..ActiveFolModelInference::default()
                };
            }
        }
    }

    let matching_scopes = evaluated_scopes
        .iter()
        .filter(|artifact| canonical_document.starts_with(&artifact.source_scope))
        .collect::<Vec<_>>();
    if !matching_scopes.is_empty() {
        let active_fol_model = matching_scopes[0].active_fol_model;
        if matching_scopes
            .iter()
            .any(|artifact| artifact.active_fol_model != active_fol_model)
        {
            return ActiveFolModelInference {
                unresolved: true,
                ..ActiveFolModelInference::default()
            };
        }
        // Same-model artifacts may intentionally nest. The broadest matching
        // scope is the only choice that cannot hide files recursively parsed by
        // one of those artifact roots.
        let artifact_source_scope = matching_scopes
            .iter()
            .map(|artifact| artifact.source_scope.clone())
            .min_by_key(|scope| scope.components().count());
        return ActiveFolModelInference {
            active_fol_model: Some(active_fol_model),
            artifact_source_scope,
            unresolved: false,
            ..ActiveFolModelInference::default()
        };
    }

    let first = evaluated_scopes[0].active_fol_model;
    if evaluated_scopes
        .iter()
        .all(|artifact| artifact.active_fol_model == first)
    {
        return ActiveFolModelInference {
            active_fol_model: Some(first),
            artifact_source_scope: None,
            unresolved: false,
            ..ActiveFolModelInference::default()
        };
    }

    ActiveFolModelInference {
        unresolved: true,
        ..ActiveFolModelInference::default()
    }
}

fn artifact_scopes_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn typecheck_model_for_build_model(
    model: BuildArtifactFolModel,
    dependencies: &[BuildRuntimeDependency],
) -> TypecheckCapabilityModel {
    // Mirrors the CLI: a memo artifact that declares the bundled internal
    // `std` dependency typechecks at the effective hosted capability, which
    // legalizes the low-level hosted substrate (`.echo`) exactly like
    // `fol code check` does (see examples/std_substrate_echo).
    match model {
        BuildArtifactFolModel::Core => TypecheckCapabilityModel::Core,
        BuildArtifactFolModel::Memo => {
            if dependencies.iter().any(is_internal_standard_dependency) {
                TypecheckCapabilityModel::Std
            } else {
                TypecheckCapabilityModel::Memo
            }
        }
    }
}

fn is_internal_standard_dependency(dependency: &BuildRuntimeDependency) -> bool {
    dependency.source_kind == DependencySourceKind::Internal && dependency.package == "standard"
}

#[cfg(test)]
mod tests {
    use super::{copy_directory_tree, map_document_workspace, materialize_analysis_overlay};
    use crate::{EditorConfig, EditorDocument, EditorDocumentUri};
    use fol_typecheck::TypecheckCapabilityModel;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_workspace_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(root.join(".git")).unwrap();
        root
    }

    fn copy_dir_all(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap();
        for entry in fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir_all(&from, &to);
            } else {
                fs::copy(&from, &to).unwrap();
            }
        }
    }

    fn copied_example_root(example_path: &str) -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(example_path)
            .canonicalize()
            .expect("checked-in example path should canonicalize");
        let root = temp_root(&format!("example_copy_{}", example_path.replace('/', "_")));
        copy_dir_all(&source, &root);
        root
    }

    #[test]
    fn workspace_mapping_finds_package_and_workspace_roots() {
        let root = temp_root("mapping");
        let package = root.join("app");
        let src = package.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("fol.work.yaml"), "members:\n  - app\n").unwrap();
        fs::write(package.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&src.join("main.fol"), &EditorConfig::default())
            .expect("mapping should succeed");

        assert_eq!(mapping.package_root, Some(package.clone()));
        assert_eq!(mapping.workspace_root, Some(root.clone()));
        assert_eq!(mapping.analysis_root, root);
        assert_eq!(mapping.active_fol_model, None);
        assert_eq!(mapping.artifact_source_scope, None);
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(package.parent().unwrap()).ok();
    }

    #[test]
    fn workspace_mapping_marks_unreadable_build_source_unresolved() {
        let root = temp_root("mapping_unreadable_build");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("build.fol"), [0xff]).unwrap();
        let document = src.join("main.fol");
        fs::write(&document, "fun[] main(): int = { return 0; };\n").unwrap();

        let mapping = map_document_workspace(&document, &EditorConfig::default())
            .expect("workspace discovery should preserve an unresolved build mapping");

        assert_eq!(mapping.active_fol_model, None);
        assert!(mapping.active_internal_standard_aliases.is_empty());
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_single_artifact_fol_model() {
        let root = temp_root("mapping_model_core");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&src.join("main.fol"), &EditorConfig::default())
            .expect("mapping should succeed");

        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Core)
        );
        assert_eq!(
            mapping.artifact_source_scope,
            Some(src.canonicalize().unwrap())
        );
        assert!(!mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_bundled_std_example_model_without_override() {
        let root = copied_example_root("examples/std_bundled_fmt");
        let document = root.join("src/main.fol");

        let mapping = map_document_workspace(&document, &EditorConfig::default())
            .expect("mapping should succeed");

        assert_eq!(mapping.package_root, Some(root.clone()));
        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_bundled_std_alias_example_model_without_override() {
        let root = copied_example_root("examples/std_alias_pkg");
        let document = root.join("src/main.fol");

        let mapping = map_document_workspace(&document, &EditorConfig::default())
            .expect("mapping should succeed");

        assert_eq!(mapping.package_root, Some(root.clone()));
        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_uses_matching_artifact_root_in_mixed_model_package() {
        let root = temp_root("mapping_model_mixed");
        let src = root.join("src");
        let tests = root.join("test");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&tests).unwrap();
        fs::write(root.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.add_test({ name = \"tests\", root = \"test/app.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .unwrap();
        fs::write(
            tests.join("app.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .unwrap();

        let src_mapping = map_document_workspace(&src.join("main.fol"), &EditorConfig::default())
            .expect("mapping should succeed");
        let test_mapping = map_document_workspace(&tests.join("app.fol"), &EditorConfig::default())
            .expect("mapping should succeed");
        let build_mapping =
            map_document_workspace(&root.join("build.fol"), &EditorConfig::default())
                .expect("mapping should succeed");

        assert_eq!(
            src_mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );
        assert_eq!(
            test_mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Core)
        );
        assert_eq!(build_mapping.active_fol_model, None);
        assert_eq!(
            src_mapping.artifact_source_scope,
            Some(src.canonicalize().unwrap())
        );
        assert_eq!(
            test_mapping.artifact_source_scope,
            Some(tests.canonicalize().unwrap())
        );
        assert!(!build_mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_routes_helpers_through_disjoint_artifact_scopes() {
        let root = temp_root("mapping_model_scoped_helpers");
        let core = root.join("core");
        let memo = root.join("memo");
        fs::create_dir_all(&core).unwrap();
        fs::create_dir_all(&memo).unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"core\", root = \"core/main.fol\", fol_model = \"core\" });\n",
                "    graph.add_exe({ name = \"memo\", root = \"memo/main.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        for file in [core.join("main.fol"), core.join("helper.fol")] {
            fs::write(file, "fun[] value(): int = { return 1; };\n").unwrap();
        }
        for file in [memo.join("main.fol"), memo.join("helper.fol")] {
            fs::write(file, "fun[] value(): str = { return \"memo\"; };\n").unwrap();
        }

        for file in [core.join("main.fol"), core.join("helper.fol")] {
            let mapping = map_document_workspace(&file, &EditorConfig::default()).unwrap();
            assert_eq!(
                mapping.active_fol_model,
                Some(TypecheckCapabilityModel::Core)
            );
            assert_eq!(
                mapping.artifact_source_scope,
                Some(core.canonicalize().unwrap())
            );
            assert!(!mapping.fol_model_scope_unresolved);
        }
        for file in [memo.join("main.fol"), memo.join("helper.fol")] {
            let mapping = map_document_workspace(&file, &EditorConfig::default()).unwrap();
            assert_eq!(
                mapping.active_fol_model,
                Some(TypecheckCapabilityModel::Memo)
            );
            assert_eq!(
                mapping.artifact_source_scope,
                Some(memo.canonicalize().unwrap())
            );
            assert!(!mapping.fol_model_scope_unresolved);
        }

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_rejects_overlapping_mixed_artifact_scopes() {
        let root = temp_root("mapping_model_overlap");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"core\", root = \"src/core.fol\", fol_model = \"core\" });\n",
                "    graph.add_exe({ name = \"memo\", root = \"src/memo.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(src.join("core.fol"), "fun[] core(): int = { return 1; };\n").unwrap();
        fs::write(
            src.join("memo.fol"),
            "fun[] memo(): str = { return \"memo\"; };\n",
        )
        .unwrap();

        let mapping =
            map_document_workspace(&src.join("core.fol"), &EditorConfig::default()).unwrap();
        assert_eq!(mapping.active_fol_model, None);
        assert_eq!(mapping.artifact_source_scope, None);
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_rejects_artifact_roots_that_escape_the_package() {
        let root = temp_root("mapping_model_escape");
        let memo = root.join("memo");
        fs::create_dir_all(&memo).unwrap();
        let outside = root.with_extension("outside.fol");
        fs::write(&outside, "fun[] escaped(): int = { return 0; };\n").unwrap();
        let escaped_root = format!(
            "../{}",
            outside
                .file_name()
                .and_then(|name| name.to_str())
                .expect("temporary fixture name should be UTF-8")
        );
        fs::write(
            root.join("build.fol"),
            format!(
                concat!(
                    "pro[] build(): non = {{\n",
                    "    var build = .build();\n",
                    "    build.meta({{ name = \"sample\", version = \"0.1.0\" }});\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_exe({{ name = \"escaped\", root = \"{escaped_root}\", fol_model = \"core\" }});\n",
                    "    graph.add_exe({{ name = \"memo\", root = \"memo/main.fol\", fol_model = \"memo\" }});\n",
                    "}};\n",
                ),
                escaped_root = escaped_root,
            ),
        )
        .unwrap();
        fs::write(
            memo.join("main.fol"),
            "fun[] main(): str = { return \"memo\"; };\n",
        )
        .unwrap();

        let mapping =
            map_document_workspace(&memo.join("main.fol"), &EditorConfig::default()).unwrap();
        assert_eq!(mapping.active_fol_model, None);
        assert_eq!(mapping.artifact_source_scope, None);
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_file(outside).ok();
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_routed_models_from_real_mixed_example_workspace() {
        let root = copied_example_root("examples/mixed_models_workspace");
        let app_mapping =
            map_document_workspace(&root.join("app/main.fol"), &EditorConfig::default()).unwrap();
        let core_mapping =
            map_document_workspace(&root.join("core/lib.fol"), &EditorConfig::default()).unwrap();
        let memo_mapping =
            map_document_workspace(&root.join("memo/lib.fol"), &EditorConfig::default()).unwrap();

        // The workspace-level build.fol declares the bundled internal `std`
        // dependency, so memo-model members typecheck at the effective hosted
        // capability while the core member keeps its no-heap boundary.
        assert_eq!(
            app_mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );
        assert_eq!(
            core_mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Core)
        );
        assert_eq!(
            memo_mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_uses_uniform_package_model_for_unmapped_files() {
        let root = temp_root("uniform_unmapped_model");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): str = {\n    return \"ok\";\n};\n",
        )
        .unwrap();
        fs::write(
            root.join("notes.fol"),
            "fun[] helper(): int = {\n    return 7;\n};\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&root.join("notes.fol"), &EditorConfig::default())
            .expect("mapping should succeed for package-local helper file");
        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Memo)
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_returns_unknown_model_for_ambiguous_unmapped_files() {
        let root = temp_root("ambiguous_unmapped_model");
        let src = root.join("src");
        let tests = root.join("test");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&tests).unwrap();
        fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.add_test({ name = \"suite\", root = \"test/app.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 7;\n};\n",
        )
        .unwrap();
        fs::write(
            tests.join("app.fol"),
            "fun[] main(): int = {\n    return 9;\n};\n",
        )
        .unwrap();
        fs::write(
            root.join("notes.fol"),
            "fun[] helper(): int = {\n    return 1;\n};\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&root.join("notes.fol"), &EditorConfig::default())
            .expect("mapping should succeed for package-local helper file");
        assert_eq!(mapping.active_fol_model, None);
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_keeps_real_mixed_workspace_helper_files_conservative() {
        let root = copied_example_root("examples/mixed_models_workspace");
        let helper = root.join("notes.fol");
        fs::write(&helper, "fun[] helper(): int = {\n    return 7;\n};\n").unwrap();

        let mapping = map_document_workspace(&helper, &EditorConfig::default())
            .expect("mapping should succeed for real mixed-workspace helper file");
        assert_eq!(mapping.active_fol_model, None);
        assert!(mapping.fol_model_scope_unresolved);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn overlay_materialization_rewrites_the_open_document_text() {
        let root = temp_root("overlay");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(root.join("build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n",
        )
        .unwrap();
        fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .unwrap();

        let path = src.join("main.fol");
        let mapping =
            map_document_workspace(&path, &EditorConfig::default()).expect("mapping should work");
        let uri = EditorDocumentUri::from_file_path(path.clone()).unwrap();
        let document = EditorDocument::new(
            uri,
            2,
            "fun[] main(): int = {\n    return 7;\n};\n".to_string(),
        )
        .unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();
        let mirrored = overlay.analysis_root().join("src/main.fol");

        assert_eq!(
            fs::read_to_string(mirrored).unwrap(),
            "fun[] main(): int = {\n    return 7;\n};\n"
        );
        assert_eq!(overlay.package_root(), Some(overlay.analysis_root()));
        assert_eq!(
            overlay.document_path(),
            overlay.analysis_root().join("src/main.fol")
        );
        assert_eq!(
            overlay.artifact_source_scope(),
            Some(overlay.analysis_root().join("src").as_path())
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn analysis_overlay_copy_skips_tmp_but_preserves_source_trees() {
        let source = temp_root("overlay_copy_source");
        let destination = temp_root("overlay_copy_destination");
        let app_source = source.join("app/src");
        let shared_source = source.join("shared/src");
        let scratch = source.join(".tmp/stale-cargo-target");
        fs::create_dir_all(&app_source).unwrap();
        fs::create_dir_all(&shared_source).unwrap();
        fs::create_dir_all(&scratch).unwrap();
        fs::write(
            app_source.join("main.fol"),
            "fun[] main(): int = { return 0; };\n",
        )
        .unwrap();
        fs::write(
            shared_source.join("lib.fol"),
            "fun[exp] helper(): int = { return 7; };\n",
        )
        .unwrap();
        fs::write(scratch.join("artifact.o"), "build output\n").unwrap();

        copy_directory_tree(&source, &destination).unwrap();

        assert!(destination.join("app/src/main.fol").is_file());
        assert!(destination.join("shared/src/lib.fol").is_file());
        assert!(!destination.join(".tmp").exists());

        fs::remove_dir_all(source).ok();
        fs::remove_dir_all(destination).ok();
    }

    #[test]
    fn overlay_materializes_declared_internal_standard_alias() {
        let root = copied_example_root("examples/std_alias_pkg");
        let path = root.join("src/main.fol");
        assert!(!root.join(".fol/pkg/standard_lib").exists());

        let mapping =
            map_document_workspace(&path, &EditorConfig::default()).expect("mapping should work");
        let uri = EditorDocumentUri::from_file_path(path.clone()).unwrap();
        let document = EditorDocument::new(uri, 1, fs::read_to_string(&path).unwrap()).unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();
        let alias_root = overlay.analysis_root().join(".fol/pkg/standard_lib");

        assert!(alias_root.join("io/lib.fol").is_file());
        assert!(alias_root.join("fmt/root.fol").is_file());
        assert!(!root.join(".fol/pkg/standard_lib").exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn overlay_materializes_active_standard_for_core_artifacts() {
        let root = temp_root("overlay_core_std_boundary");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"core\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        let path = src.join("main.fol");
        fs::write(&path, "fun[] main(): int = { return 0; };\n").unwrap();

        let mapping = map_document_workspace(&path, &EditorConfig::default()).unwrap();
        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Core)
        );
        assert_eq!(mapping.active_internal_standard_aliases, ["std"]);
        let uri = EditorDocumentUri::from_file_path(path.clone()).unwrap();
        let document = EditorDocument::new(uri, 1, fs::read_to_string(path).unwrap()).unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();

        assert!(overlay
            .analysis_root()
            .join(".fol/pkg/std/fmt/root.fol")
            .is_file());
        assert!(!root.join(".fol/pkg/std").exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn overlay_materializes_only_evaluated_internal_standard_aliases() {
        let root = temp_root("overlay_conditional_std_aliases");
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    var optimize = graph.standard_optimize();\n",
                "    when(optimize == \"release-fast\") {\n",
                "        { build.add_dep({ alias = \"release_std\", source = \"internal\", target = \"standard\" }); }\n",
                "    };\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
        )
        .unwrap();
        let path = src.join("main.fol");
        fs::write(
            &path,
            "use std: pkg = {\"std\"};\nfun[] main(): int = { return std::fmt::answer(); };\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&path, &EditorConfig::default()).unwrap();
        assert_eq!(
            mapping.active_fol_model,
            Some(TypecheckCapabilityModel::Std)
        );
        assert_eq!(mapping.active_internal_standard_aliases, ["std"]);
        let uri = EditorDocumentUri::from_file_path(path.clone()).unwrap();
        let document = EditorDocument::new(uri, 1, fs::read_to_string(path).unwrap()).unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();

        assert!(overlay
            .analysis_root()
            .join(".fol/pkg/std/fmt/root.fol")
            .is_file());
        assert!(!overlay
            .analysis_root()
            .join(".fol/pkg/release_std")
            .exists());
        assert!(!root.join(".fol/pkg/std").exists());
        assert!(!root.join(".fol/pkg/release_std").exists());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_overlay_copies_the_full_analysis_tree_for_sibling_imports() {
        let root = temp_root("workspace_overlay");
        let app_src = root.join("app/src");
        let shared_src = root.join("shared/src");
        fs::create_dir_all(&app_src).unwrap();
        fs::create_dir_all(&shared_src).unwrap();
        fs::write(
            root.join("fol.work.yaml"),
            "members:\n  - app\n  - shared\n",
        )
        .unwrap();
        fs::write(root.join("app/build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
        fs::write(
            root.join("app/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n",
        )
        .unwrap();
        fs::write(
            app_src.join("main.fol"),
            "use shared: loc = {\"../shared\"};\n\nfun[] main(): int = {\n    return shared::helper();\n};\n",
        )
        .unwrap();
        fs::write(
            root.join("shared/build.fol"),
            "name: shared\nversion: 0.1.0\n",
        )
        .unwrap();
        fs::write(
            root.join("shared/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n",
        )
        .unwrap();
        fs::write(
            shared_src.join("lib.fol"),
            "fun[exp] helper(): int = {\n    return 7;\n};\n",
        )
        .unwrap();

        let path = app_src.join("main.fol");
        let mapping =
            map_document_workspace(&path, &EditorConfig::default()).expect("mapping should work");
        let uri = EditorDocumentUri::from_file_path(path.clone()).unwrap();
        let document = EditorDocument::new(uri, 2, fs::read_to_string(&path).unwrap()).unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();

        // The overlay keeps the whole analysis tree so sibling local-import
        // targets (`use shared: loc = {"../shared"}`) stay resolvable.
        assert_eq!(
            overlay.package_root(),
            Some(overlay.analysis_root().join("app").as_path())
                .map(|path| path.to_path_buf())
                .as_deref()
        );
        assert!(overlay.analysis_root().join("app/src/main.fol").is_file());
        assert!(overlay.analysis_root().join("app/build.fol").is_file());
        assert!(overlay.analysis_root().join("shared/src/lib.fol").is_file());

        fs::remove_dir_all(root).ok();
    }
}
