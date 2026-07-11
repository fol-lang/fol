use crate::{EditorConfig, EditorDocument, EditorError, EditorErrorKind, EditorResult};
use fol_package::{
    build_artifact::BuildArtifactFolModel, build_runtime::BuildRuntimeArtifact,
    build_api::DependencySourceKind, build_runtime::BuildRuntimeDependency,
    evaluate_build_source, BuildEvaluationInputs, BuildEvaluationRequest,
};
use fol_typecheck::TypecheckCapabilityModel;
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
    pub active_fol_model: Option<TypecheckCapabilityModel>,
}

#[derive(Debug)]
pub struct EditorAnalysisOverlay {
    temp_root: PathBuf,
    analysis_root: PathBuf,
    package_root: Option<PathBuf>,
    document_path: PathBuf,
}

impl EditorAnalysisOverlay {
    pub fn analysis_root(&self) -> &Path {
        &self.analysis_root
    }

    pub fn package_root(&self) -> Option<&Path> {
        self.package_root.as_deref()
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
    let active_fol_model = roots
        .package_root
        .as_ref()
        .and_then(|package_root| recover_active_fol_model(package_root, &absolute));
    Ok(EditorWorkspaceMapping {
        document_path: absolute,
        active_fol_model,
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
        .find_map(|marker| find_upward_marker(directory, marker));
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

    let relative_document = mapping
        .document_path
        .strip_prefix(overlay_source_root)
        .map_err(|_| {
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
        .and_then(|package_root| package_root.strip_prefix(overlay_source_root).ok())
        .map(|relative| temp_root.join(relative));

    Ok(EditorAnalysisOverlay {
        temp_root: temp_root.clone(),
        analysis_root: temp_root,
        package_root,
        document_path: overlay_document,
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

fn recover_active_fol_model(
    package_root: &Path,
    document_path: &Path,
) -> Option<TypecheckCapabilityModel> {
    let build_path = package_root.join("build.fol");
    let build_source = fs::read_to_string(&build_path).ok()?;
    let package_root_text = package_root.to_str()?.to_string();
    let evaluated = evaluate_build_source(
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
    .flatten()?;
    infer_active_fol_model(
        package_root,
        document_path,
        &evaluated.evaluated.artifacts,
        &evaluated.evaluated.dependencies,
    )
}

fn infer_active_fol_model(
    package_root: &Path,
    document_path: &Path,
    artifacts: &[BuildRuntimeArtifact],
    dependencies: &[BuildRuntimeDependency],
) -> Option<TypecheckCapabilityModel> {
    if artifacts.is_empty() {
        return None;
    }

    let relative_document = document_path.strip_prefix(package_root).ok();
    let matching_artifacts = relative_document
        .iter()
        .flat_map(|relative| {
            artifacts
                .iter()
                .filter(move |artifact| Path::new(&artifact.root_module) == *relative)
        })
        .collect::<Vec<_>>();
    if matching_artifacts.len() == 1 {
        return Some(typecheck_model_for_build_model(
            matching_artifacts[0].fol_model,
            dependencies,
        ));
    }

    let first = artifacts[0].fol_model;
    if artifacts.iter().all(|artifact| artifact.fol_model == first) {
        return Some(typecheck_model_for_build_model(first, dependencies));
    }

    if artifacts.len() == 1 {
        return Some(typecheck_model_for_build_model(first, dependencies));
    }

    None
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
    use super::{map_document_workspace, materialize_analysis_overlay};
    use crate::{EditorConfig, EditorDocument, EditorDocumentUri};
    use fol_typecheck::TypecheckCapabilityModel;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fol_editor_workspace_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
            .as_nanos()
        ))
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

        fs::remove_dir_all(package.parent().unwrap()).ok();
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

        assert_eq!(mapping.active_fol_model, Some(TypecheckCapabilityModel::Core));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_bundled_std_example_model_without_override() {
        let root = copied_example_root("examples/std_bundled_fmt");
        let document = root.join("src/main.fol");

        let mapping =
            map_document_workspace(&document, &EditorConfig::default()).expect("mapping should succeed");

        assert_eq!(mapping.package_root, Some(root.clone()));
        assert_eq!(mapping.active_fol_model, Some(TypecheckCapabilityModel::Std));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_recovers_bundled_std_alias_example_model_without_override() {
        let root = copied_example_root("examples/std_alias_pkg");
        let document = root.join("src/main.fol");

        let mapping =
            map_document_workspace(&document, &EditorConfig::default()).expect("mapping should succeed");

        assert_eq!(mapping.package_root, Some(root.clone()));
        assert_eq!(mapping.active_fol_model, Some(TypecheckCapabilityModel::Std));

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
        let test_mapping =
            map_document_workspace(&tests.join("app.fol"), &EditorConfig::default())
                .expect("mapping should succeed");
        let build_mapping = map_document_workspace(&root.join("build.fol"), &EditorConfig::default())
            .expect("mapping should succeed");

        assert_eq!(src_mapping.active_fol_model, Some(TypecheckCapabilityModel::Std));
        assert_eq!(test_mapping.active_fol_model, Some(TypecheckCapabilityModel::Core));
        assert_eq!(build_mapping.active_fol_model, None);

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
        assert_eq!(app_mapping.active_fol_model, Some(TypecheckCapabilityModel::Std));
        assert_eq!(core_mapping.active_fol_model, Some(TypecheckCapabilityModel::Core));
        assert_eq!(memo_mapping.active_fol_model, Some(TypecheckCapabilityModel::Std));

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
        assert_eq!(mapping.active_fol_model, Some(TypecheckCapabilityModel::Memo));

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

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_mapping_keeps_real_mixed_workspace_helper_files_conservative() {
        let root = copied_example_root("examples/mixed_models_workspace");
        let helper = root.join("notes.fol");
        fs::write(
            &helper,
            "fun[] helper(): int = {\n    return 7;\n};\n",
        )
        .unwrap();

        let mapping = map_document_workspace(&helper, &EditorConfig::default())
            .expect("mapping should succeed for real mixed-workspace helper file");
        assert_eq!(mapping.active_fol_model, None);

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

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_overlay_copies_the_full_analysis_tree_for_sibling_imports() {
        let root = temp_root("workspace_overlay");
        let app_src = root.join("app/src");
        let shared_src = root.join("shared/src");
        fs::create_dir_all(&app_src).unwrap();
        fs::create_dir_all(&shared_src).unwrap();
        fs::write(root.join("fol.work.yaml"), "members:\n  - app\n  - shared\n").unwrap();
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
        fs::write(root.join("shared/build.fol"), "name: shared\nversion: 0.1.0\n").unwrap();
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
        let document = EditorDocument::new(
            uri,
            2,
            fs::read_to_string(&path).unwrap(),
        )
        .unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();

        // The overlay keeps the whole analysis tree so sibling local-import
        // targets (`use shared: loc = {"../shared"}`) stay resolvable.
        assert_eq!(
            overlay.package_root(),
            Some(overlay.analysis_root().join("app").as_path()).map(|path| path.to_path_buf()).as_deref()
        );
        assert!(overlay.analysis_root().join("app/src/main.fol").is_file());
        assert!(overlay.analysis_root().join("app/build.fol").is_file());
        assert!(overlay.analysis_root().join("shared/src/lib.fol").is_file());

        fs::remove_dir_all(root).ok();
    }
}
