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
    /// The ordered native link plan for this artifact.
    ///
    /// Rendered to structured `rustc` arguments; never concatenated into a raw
    /// flag string. Absent on routes with no native inputs.
    pub native_link_plan: Option<fol_build::link_plan::NativeLinkPlan>,
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
            native_link_plan: None,
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

#[cfg(test)]
mod artifact_kind_layer_tests {
    use super::BackendConfig;
    use fol_build::graph::{BuildArtifactKind, BuildGraph};
    use fol_build::plan::{
        resolve_graph_artifacts, OutputRole, ResolvedArtifactKind, ResolvedProvenance,
    };

    /// Object remains object through every layer it passes.
    ///
    /// M1 requires this specifically because an object is the kind most easily
    /// mistaken for something else: it is not executable, not a library, and
    /// section 4.3 forbids it ever mapping to a test bundle.
    #[test]
    fn object_remains_object_from_graph_to_backend_config() {
        let mut graph = BuildGraph::new();
        graph.add_artifact(BuildArtifactKind::Object, "part");

        let plan = resolve_graph_artifacts(&graph, &ResolvedProvenance::default())
            .into_iter()
            .next()
            .expect("the graph declares one artifact");
        assert_eq!(plan.kind, ResolvedArtifactKind::Object);
        assert_eq!(plan.primary_output_role(), OutputRole::Object);
        assert_eq!(plan.primary_output_file_name(), "part.o");
        assert!(!plan.kind.is_executable());

        let config = BackendConfig {
            artifact_plan: Some(plan.clone()),
            ..BackendConfig::default()
        };
        let carried = config.artifact_plan.expect("the plan should reach here");
        assert_eq!(carried.kind, ResolvedArtifactKind::Object);
        assert_ne!(
            carried.kind,
            ResolvedArtifactKind::TestExecutable,
            "section 4.3: an object must never map to a test bundle"
        );
        assert_eq!(
            carried.outputs.first().map(|output| output.role),
            Some(OutputRole::Object)
        );
    }

    /// Mixed-target and mixed-profile artifacts keep independent values.
    ///
    /// One graph can declare artifacts for different targets and profiles, and
    /// each plan has to keep its own rather than adopting a shared default.
    #[test]
    fn mixed_target_and_profile_artifacts_keep_independent_values() {
        use fol_build::option::BuildOptimizeMode;

        let mut graph = BuildGraph::new();
        graph.add_configured_artifact(
            BuildArtifactKind::Executable,
            "gnu_debug",
            "src/main.fol",
            fol_build::artifact::BuildArtifactFolModel::Memo,
            fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap(),
            BuildOptimizeMode::Debug,
        );
        graph.add_configured_artifact(
            BuildArtifactKind::StaticLibrary,
            "musl_release",
            "src/lib.fol",
            fol_build::artifact::BuildArtifactFolModel::Core,
            fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap(),
            BuildOptimizeMode::ReleaseFast,
        );

        let plans = resolve_graph_artifacts(&graph, &ResolvedProvenance::default());
        assert_eq!(plans.len(), 2);

        assert_eq!(plans[0].target.as_str(), "x86_64-unknown-linux-gnu");
        assert_eq!(plans[0].optimize, BuildOptimizeMode::Debug);
        assert_eq!(
            plans[0].fol_model,
            fol_build::artifact::BuildArtifactFolModel::Memo
        );

        assert_eq!(plans[1].target.as_str(), "x86_64-unknown-linux-musl");
        assert_eq!(plans[1].optimize, BuildOptimizeMode::ReleaseFast);
        assert_eq!(
            plans[1].fol_model,
            fol_build::artifact::BuildArtifactFolModel::Core
        );

        // Independent values must also produce independent identities, or a
        // cache would serve one artifact's output for the other.
        let environment = std::collections::BTreeMap::new();
        assert_ne!(
            fol_build::plan::identity::plan_identity(&plans[0], &environment),
            fol_build::plan::identity::plan_identity(&plans[1], &environment)
        );
    }
}

impl BackendConfig {
    /// What this invocation is producing.
    ///
    /// Read from the resolved plan M1 threads in. Without a plan the backend is
    /// on a route that has no evaluated graph -- a direct compile, an emit --
    /// and those only ever produce an executable.
    pub fn product_kind(&self) -> crate::model::BackendProductKind {
        use crate::model::BackendProductKind;
        use fol_build::plan::ResolvedArtifactKind;
        match self.artifact_plan.as_ref().map(|plan| plan.kind) {
            Some(ResolvedArtifactKind::TestExecutable) => BackendProductKind::TestExecutable,
            Some(ResolvedArtifactKind::StaticLibrary) => BackendProductKind::StaticLibrary,
            Some(ResolvedArtifactKind::SharedLibrary) => BackendProductKind::SharedLibrary,
            Some(ResolvedArtifactKind::Object) => BackendProductKind::Object,
            Some(ResolvedArtifactKind::Executable) | None => BackendProductKind::Executable,
        }
    }
}

#[cfg(test)]
mod product_kind_config_tests {
    use super::BackendConfig;
    use crate::model::BackendProductKind;
    use fol_build::plan::ResolvedArtifactKind;

    fn plan_with(kind: ResolvedArtifactKind) -> fol_build::plan::ResolvedArtifactPlan {
        fol_build::plan::ResolvedArtifactPlan {
            name: "core".to_string(),
            kind,
            provenance: Default::default(),
            root_source: "src/lib.fol".to_string(),
            inputs: Vec::new(),
            fol_model: Default::default(),
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap(),
            optimize: fol_build::option::BuildOptimizeMode::Debug,
            abi: Default::default(),
            link_plan: Default::default(),
            native_attachments: Default::default(),
            outputs: Vec::new(),
            installs: Vec::new(),
        }
    }

    #[test]
    fn the_product_kind_follows_the_resolved_plan() {
        for (resolved, expected) in [
            (
                ResolvedArtifactKind::Executable,
                BackendProductKind::Executable,
            ),
            (
                ResolvedArtifactKind::TestExecutable,
                BackendProductKind::TestExecutable,
            ),
            (
                ResolvedArtifactKind::StaticLibrary,
                BackendProductKind::StaticLibrary,
            ),
            (
                ResolvedArtifactKind::SharedLibrary,
                BackendProductKind::SharedLibrary,
            ),
            (ResolvedArtifactKind::Object, BackendProductKind::Object),
        ] {
            let config = BackendConfig {
                artifact_plan: Some(plan_with(resolved)),
                ..BackendConfig::default()
            };
            assert_eq!(config.product_kind(), expected);
        }
    }

    /// A route with no evaluated graph builds an executable, which is what
    /// those routes have always produced.
    #[test]
    fn a_planless_config_produces_an_executable() {
        assert_eq!(
            BackendConfig::default().product_kind(),
            BackendProductKind::Executable
        );
    }
}
