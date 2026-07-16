use crate::{
    compile::FrontendArtifactExecutionSelection, FrontendConfig, FrontendError, FrontendErrorKind,
    FrontendResult,
};
use std::path::{Component, Path, PathBuf};

pub(crate) struct PreparedH7Interop {
    pub backend_plan: fol_backend::BackendAuxiliaryRustPlan,
    pub report: fol_interop::H7InteropReport,
}

/// Convert one authoritative graph C-import into FOL's sole sibling pipeline
/// and then into the backend's generic ordered auxiliary-Rust plan.
///
/// The graph identity, target, optimization, compiler, and temporary parent
/// are checked before `interop-generated` creation or any external process.
pub(crate) fn prepare_h7_interop_for_selection(
    selection: &FrontendArtifactExecutionSelection,
    config: &FrontendConfig,
    output_root: &Path,
) -> FrontendResult<Option<PreparedH7Interop>> {
    let Some(binding) = selection.graph_binding.as_ref() else {
        if selection.c_imports.is_empty() {
            return Ok(None);
        }
        return Err(invalid_input(
            "C imports require an authoritative build-graph artifact binding",
        ));
    };
    let graph_imports = binding
        .graph
        .c_imports_for(binding.artifact_id)
        .cloned()
        .collect::<Vec<_>>();
    if graph_imports.is_empty() && selection.c_imports.is_empty() {
        return Ok(None);
    }
    if graph_imports.len() != 1 || selection.c_imports.len() != 1 {
        return Err(invalid_input(
            "the certified H7 lane requires exactly one C import per artifact",
        ));
    }
    if graph_imports != selection.c_imports {
        return Err(invalid_input(
            "selected C import does not match the authoritative build graph",
        ));
    }

    let artifact = binding
        .graph
        .artifact(binding.artifact_id)
        .ok_or_else(|| invalid_input("selected C import references an unknown graph artifact"))?;
    let graph_capability_model = match artifact.fol_model {
        fol_package::build_artifact::BuildArtifactFolModel::Core => {
            fol_backend::BackendFolModel::Core
        }
        fol_package::build_artifact::BuildArtifactFolModel::Memo => {
            fol_backend::BackendFolModel::Memo
        }
    };
    if artifact.id != selection.c_imports[0].artifact_id
        || artifact.name != selection.label
        || artifact.target != selection.target
        || artifact.optimize != selection.optimize
        || graph_capability_model != selection.capability_model
        || selection.root_module.as_deref() != Some(artifact.root_module.as_str())
    {
        return Err(invalid_input(
            "selected C import artifact identity drifted from its authoritative graph",
        ));
    }
    if selection.target.as_str() != fol_interop::CERTIFIED_INTEROP_TARGET {
        return Err(invalid_input(format!(
            "C import target '{}' is not certified; expected {}",
            selection.target,
            fol_interop::CERTIFIED_INTEROP_TARGET
        )));
    }
    if !matches!(
        selection.optimize,
        fol_package::BuildOptimizeMode::Debug | fol_package::BuildOptimizeMode::ReleaseSafe
    ) {
        return Err(invalid_input(format!(
            "C imports require debug or release-safe optimization, not {}",
            selection.optimize.as_str()
        )));
    }

    let compiler = required_normalized_path(
        config.interop_compiler_override.as_deref(),
        "FOL_INTEROP_GCC",
    )?;
    let temporary_parent = required_normalized_path(
        config.interop_temporary_parent_override.as_deref(),
        "FOL_INTEROP_TEMP",
    )?;
    if !normalized_absolute(&selection.package_root) || !normalized_absolute(output_root) {
        return Err(invalid_input(
            "C import package and build roots must be normalized absolute paths",
        ));
    }
    std::fs::create_dir_all(output_root).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "failed to establish C import build root '{}': {error}",
                output_root.display()
            ),
        )
    })?;

    let generated_output_root = output_root.join("interop-generated");
    let build = fol_interop::prepare_h7_interop(fol_interop::H7InteropRequest::new(
        &binding.graph,
        binding.artifact_id,
        &selection.package_root,
        compiler,
        temporary_parent,
        &generated_output_root,
    ))
    .map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!("certified C import pipeline failed: {error}"),
        )
    })?;

    let profile = selection.backend_build_profile();
    let raw_crate = fol_backend::BackendAuxiliaryRustCrate::try_new(
        build.raw_crate_name(),
        build.raw_crate_root().join("src/lib.rs"),
        Vec::new(),
    )
    .map_err(backend_plan_error)?;
    let anchor_crate = fol_backend::BackendAuxiliaryRustCrate::try_new(
        build.anchor_crate_name(),
        build.anchor_crate_root().join("src/lib.rs"),
        vec![build.raw_crate_name().to_owned()],
    )
    .map_err(backend_plan_error)?;
    let entry_call = fol_backend::BackendMainEntryCall::try_new_with_result_observation(
        build.anchor_crate_name(),
        vec![build.anchor_function_name().to_owned()],
        fol_backend::BackendMainEntryResultObservation::StdoutI32,
    )
    .map_err(backend_plan_error)?;
    let backend_plan = fol_backend::BackendAuxiliaryRustPlan::try_new(
        selection.target.clone(),
        profile,
        vec![raw_crate, anchor_crate],
        entry_call,
        build.rustc_link_arguments().to_vec(),
    )
    .map_err(backend_plan_error)?;

    Ok(Some(PreparedH7Interop {
        backend_plan,
        report: build.report(),
    }))
}

fn required_normalized_path<'a>(
    path: Option<&'a Path>,
    variable: &'static str,
) -> FrontendResult<&'a Path> {
    let path = path.ok_or_else(|| {
        invalid_input(format!(
            "certified C imports require explicit {variable} configuration"
        ))
    })?;
    if !normalized_absolute(path) {
        return Err(invalid_input(format!(
            "{variable} must be a normalized absolute path, got '{}'",
            path.display()
        )));
    }
    Ok(path)
}

fn normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
        && path.as_os_str() == path.components().collect::<PathBuf>().as_os_str()
}

fn backend_plan_error(error: fol_backend::BackendError) -> FrontendError {
    FrontendError::new(
        FrontendErrorKind::CommandFailed,
        format!("could not construct checked C import backend plan: {error}"),
    )
}

fn invalid_input(message: impl Into<String>) -> FrontendError {
    FrontendError::new(FrontendErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::{normalized_absolute, prepare_h7_interop_for_selection};
    use crate::compile::{FrontendArtifactExecutionSelection, FrontendArtifactGraphBinding};
    use crate::FrontendConfig;
    use std::path::{Path, PathBuf};

    #[test]
    fn interop_configuration_paths_are_normalized_absolute() {
        assert!(normalized_absolute(Path::new("/tmp/fol-interop")));
        assert!(!normalized_absolute(Path::new("tmp/fol-interop")));
        assert!(!normalized_absolute(Path::new("/tmp/../fol-interop")));
        assert!(!normalized_absolute(Path::new("/tmp/./fol-interop")));
    }

    #[test]
    fn target_mismatch_fails_before_output_or_tool_io() {
        let mut graph = fol_package::BuildGraph::new();
        let target = fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap();
        let artifact_id = graph.add_configured_artifact(
            fol_package::BuildArtifactKind::Executable,
            "app",
            "src/main.fol",
            fol_package::build_artifact::BuildArtifactFolModel::Core,
            target.clone(),
            fol_package::BuildOptimizeMode::Debug,
        );
        let c_import = graph
            .add_c_import(
                artifact_id,
                "native/provider.h",
                "native/provider.o",
                fol_package::BuildCImportProviderKind::Object,
            )
            .unwrap();
        let selection = FrontendArtifactExecutionSelection {
            package_root: PathBuf::from("/definitely/missing/fol-h7-package"),
            label: "app".to_owned(),
            root_module: Some("src/main.fol".to_owned()),
            artifact_capabilities: Vec::new(),
            capability_model: fol_backend::BackendFolModel::Core,
            fol_model: fol_backend::BackendFolModel::Core,
            has_bundled_std: false,
            target,
            optimize: fol_package::BuildOptimizeMode::Debug,
            c_imports: vec![c_import],
            graph_binding: Some(FrontendArtifactGraphBinding { graph, artifact_id }),
        };
        let output =
            std::env::temp_dir().join(format!("fol-h7-target-reject-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output);

        let error =
            prepare_h7_interop_for_selection(&selection, &FrontendConfig::default(), &output)
                .err()
                .expect("unsupported target must fail");

        assert!(error.message().contains("is not certified"));
        assert!(!output.exists());
    }

    #[test]
    fn graph_model_and_label_mismatches_fail_before_output_or_tool_io() {
        for (case, label, capability_model) in [
            ("label", "different-app", fol_backend::BackendFolModel::Core),
            ("model", "app", fol_backend::BackendFolModel::Memo),
        ] {
            let mut graph = fol_package::BuildGraph::new();
            let target =
                fol_types::ResolvedTarget::resolve(fol_interop::CERTIFIED_INTEROP_TARGET).unwrap();
            let artifact_id = graph.add_configured_artifact(
                fol_package::BuildArtifactKind::Executable,
                "app",
                "src/main.fol",
                fol_package::build_artifact::BuildArtifactFolModel::Core,
                target.clone(),
                fol_package::BuildOptimizeMode::Debug,
            );
            let c_import = graph
                .add_c_import(
                    artifact_id,
                    "native/provider.h",
                    "native/provider.o",
                    fol_package::BuildCImportProviderKind::Object,
                )
                .unwrap();
            let selection = FrontendArtifactExecutionSelection {
                package_root: PathBuf::from("/definitely/missing/fol-h7-package"),
                label: label.to_owned(),
                root_module: Some("src/main.fol".to_owned()),
                artifact_capabilities: Vec::new(),
                capability_model,
                fol_model: capability_model,
                has_bundled_std: false,
                target,
                optimize: fol_package::BuildOptimizeMode::Debug,
                c_imports: vec![c_import],
                graph_binding: Some(FrontendArtifactGraphBinding { graph, artifact_id }),
            };
            let output = std::env::temp_dir().join(format!(
                "fol-h7-{case}-identity-reject-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&output);

            let error =
                prepare_h7_interop_for_selection(&selection, &FrontendConfig::default(), &output)
                    .err()
                    .expect("graph identity mismatch must fail");

            assert!(
                error
                    .message()
                    .contains("artifact identity drifted from its authoritative graph"),
                "{case} mismatch failed at the wrong boundary: {error}"
            );
            assert!(!output.exists(), "{case} mismatch performed output I/O");
        }
    }
}
