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
