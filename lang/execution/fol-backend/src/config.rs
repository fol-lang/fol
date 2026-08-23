#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendTarget {
    Rust,
}

impl BackendTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
        }
    }
}

/// Backend-internal effective model selected after build evaluation.
///
/// Public `fol_model` accepts only `core` and `memo`. `Std` represents the
/// effective hosted tier derived when a `memo` artifact declares the bundled
/// internal `standard` dependency; it is not a legal third public model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendFolModel {
    Core,
    Memo,
    #[default]
    Std,
}

impl BackendFolModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Memo => "memo",
            Self::Std => "std",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendRuntimeTier {
    Core,
    Memo,
    Std,
}

impl BackendRuntimeTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Memo => "memo",
            Self::Std => "std",
        }
    }

    pub fn runtime_module_path(self) -> &'static str {
        match self {
            Self::Core => "fol_runtime::core",
            Self::Memo => "fol_runtime::memo",
            Self::Std => "fol_runtime::std",
        }
    }
}

impl From<BackendFolModel> for BackendRuntimeTier {
    fn from(value: BackendFolModel) -> Self {
        match value {
            BackendFolModel::Core => Self::Core,
            BackendFolModel::Memo => Self::Memo,
            BackendFolModel::Std => Self::Std,
        }
    }
}

/// Compatibility name retained at the backend API surface. The value itself
/// is already resolved and cannot contain a host alias or unknown spelling.
pub type BackendMachineTarget = fol_types::ResolvedTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendBuildProfile {
    Debug,
    Release,
}

impl BackendBuildProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMode {
    EmitSource,
    BuildArtifact,
}

impl BackendMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmitSource => "emit-source",
            Self::BuildArtifact => "build-artifact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendConfig {
    pub target: BackendTarget,
    pub fol_model: BackendFolModel,
    pub machine_target: BackendMachineTarget,
    pub build_profile: BackendBuildProfile,
    pub mode: BackendMode,
    pub keep_build_dir: bool,
    /// Optional validated auxiliary `no_std` Rust compilation. The default
    /// backend route leaves this absent and is byte-for-byte unchanged.
    pub auxiliary_rust_plan: Option<crate::auxiliary::BackendAuxiliaryRustPlan>,
    /// The resolved artifact plan this compilation is for.
    ///
    /// Present whenever the build came from an evaluated graph. It is the whole
    /// plan rather than the few fields the backend happens to read today, so a
    /// later milestone that needs the artifact kind, the ABI exports, or the
    /// output roles finds them here instead of reconstructing them from a
    /// filename. `fol_model`, `machine_target`, and `build_profile` above stay
    /// for the routes that have no graph.
    pub artifact_plan: Option<fol_build::plan::ResolvedArtifactPlan>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            target: BackendTarget::Rust,
            fol_model: BackendFolModel::Std,
            machine_target: BackendMachineTarget::host()
                .expect("fol-backend requires a supported concrete host target"),
            build_profile: BackendBuildProfile::Release,
            mode: BackendMode::BuildArtifact,
            keep_build_dir: false,
            auxiliary_rust_plan: None,
            artifact_plan: None,
        }
    }
}

impl BackendConfig {
    pub fn runtime_tier(&self) -> BackendRuntimeTier {
        self.fol_model.into()
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendConfig, BackendFolModel, BackendMachineTarget, BackendRuntimeTier};

    #[test]
    fn machine_target_resolves_host_aliases_to_one_concrete_target() {
        let host = BackendMachineTarget::host().unwrap();
        assert_eq!(BackendMachineTarget::resolve("host").unwrap(), host);
        assert_eq!(BackendMachineTarget::resolve("native").unwrap(), host);
        assert_eq!(BackendMachineTarget::resolve("  host  ").unwrap(), host);
    }

    #[test]
    fn machine_target_normalization_canonicalizes_explicit_triples() {
        assert_eq!(
            BackendMachineTarget::resolve("aarch64-linux-gnu")
                .unwrap()
                .as_str(),
            "aarch64-unknown-linux-gnu"
        );
        assert_eq!(
            BackendMachineTarget::resolve("  x86_64-pc-windows-gnu  ")
                .unwrap()
                .as_str(),
            "x86_64-pc-windows-gnu"
        );
    }

    #[test]
    fn machine_target_rejects_unknown_target_spellings() {
        assert!(BackendMachineTarget::resolve("sparc-linux-gnu").is_err());
        assert!(BackendMachineTarget::resolve("aarch64-macos-msvc").is_err());
    }

    #[test]
    fn backend_config_defaults_to_effective_std_runtime_tier() {
        assert_eq!(BackendConfig::default().fol_model, BackendFolModel::Std);
        assert_eq!(BackendFolModel::Core.as_str(), "core");
        assert_eq!(BackendFolModel::Memo.as_str(), "memo");
        assert_eq!(BackendFolModel::Std.as_str(), "std");
        assert_eq!(
            BackendConfig::default().runtime_tier(),
            BackendRuntimeTier::Std
        );
    }

    #[test]
    fn backend_runtime_tier_tracks_internal_effective_model_and_module_paths() {
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Core).as_str(),
            "core"
        );
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Memo).as_str(),
            "memo"
        );
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Std).as_str(),
            "std"
        );
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Core).runtime_module_path(),
            "fol_runtime::core"
        );
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Memo).runtime_module_path(),
            "fol_runtime::memo"
        );
        assert_eq!(
            BackendRuntimeTier::from(BackendFolModel::Std).runtime_module_path(),
            "fol_runtime::std"
        );
    }
}

#[cfg(test)]
mod artifact_plan_tests {
    use super::BackendConfig;
    use fol_build::graph::{
        BuildArtifactKind, BuildGeneratedFileKind, BuildGraph, BuildModuleKind,
    };
    use fol_build::plan::{resolve_graph_artifacts, ResolvedInput, ResolvedProvenance};

    /// The plan reaches the backend whole. Section 4.3 requires one resolved
    /// plan to survive end to end; before this the backend received three
    /// fields copied out of it and reconstructed the rest.
    #[test]
    fn a_resolved_plan_survives_into_the_backend_config_unchanged() {
        let mut graph = BuildGraph::new();
        let module = graph.add_module(BuildModuleKind::Source, "src/main.fol");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "src/gen.fol");
        let artifact = graph.add_artifact(BuildArtifactKind::Test, "suite");
        graph.add_artifact_module_input(artifact, module);
        graph.add_artifact_generated_file_input(artifact, generated);

        let plan = resolve_graph_artifacts(&graph, &ResolvedProvenance::default())
            .into_iter()
            .next()
            .expect("the graph declares one artifact");

        let config = BackendConfig {
            artifact_plan: Some(plan.clone()),
            ..BackendConfig::default()
        };

        let carried = config
            .artifact_plan
            .as_ref()
            .expect("the plan should reach the backend");
        assert_eq!(carried, &plan, "the plan changed in transit");
        // The two facts the old projection lost are the ones worth naming.
        assert!(carried
            .inputs
            .contains(&ResolvedInput::Generated("src/gen.fol".to_string())));
        assert_eq!(
            carried.kind,
            fol_build::plan::ResolvedArtifactKind::TestExecutable,
            "a test artifact must not arrive as a plain executable"
        );
    }

    /// A route with no evaluated graph has no plan, and says so.
    #[test]
    fn the_default_config_carries_no_plan() {
        assert!(BackendConfig::default().artifact_plan.is_none());
    }
}
