//! Rust backend for turning lowered FOL workspaces into runnable artifacts
//! across the shipped V1, V2, and V3 language surfaces.
//!
//! Runnable output does not imply hosted source capabilities. Public
//! `fol_model` remains `core` or `memo`; an evaluated bundled `standard`
//! dependency selects the backend's effective hosted runtime tier. A
//! host-compatible artifact can run without that dependency, while executing
//! a cross-target artifact requires an external runner.

mod auxiliary;
mod config;
mod control;
mod emit;
mod error;
mod identity;
mod instructions;
mod layout;
mod mangle;
mod model;
mod session;
mod signatures;
mod trace;
mod types;

#[cfg(test)]
mod testing;

pub const CRATE_NAME: &str = "fol-backend";

#[derive(Debug, Default)]
pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
    }
}

pub fn crate_name() -> &'static str {
    CRATE_NAME
}

pub use auxiliary::{
    BackendAuxiliaryRustCrate, BackendAuxiliaryRustPlan, BackendMainEntryCall,
    BackendMainEntryResultObservation,
};
pub use config::{
    BackendBuildProfile, BackendConfig, BackendFolModel, BackendMachineTarget, BackendMode,
    BackendRuntimeTier, BackendTarget,
};
pub use control::render_terminator;
pub use emit::{
    backend_build_paths, backend_runtime_build_dir, backend_runtime_manifest_path,
    backend_runtime_manifest_path_with_override, backend_runtime_source_entry,
    backend_runtime_source_entry_with_override, backend_runtime_source_root,
    backend_runtime_source_root_with_override, build_generated_crate_with_auxiliary_plan,
    build_generated_crate_with_rustc, build_generated_crate_with_rustc_args,
    build_runtime_rlib_with_rustc, emit_backend_artifact, emit_cargo_toml,
    emit_generated_crate_skeleton, emit_generated_crate_skeleton_for_config, emit_main_rs,
    emit_main_rs_for_config, emit_namespace_module_shells, emit_namespace_module_shells_for_config,
    emit_package_module_shells, prepare_backend_build_paths, prepare_backend_runtime_build_dir,
    prepare_generated_build_dir, summarize_emitted_artifact, write_generated_crate,
};
pub use error::{BackendError, BackendErrorKind};
pub use identity::{stable_workspace_hash, BackendWorkspaceIdentity};
pub use instructions::{render_core_instruction, render_core_instruction_in_workspace};
pub use layout::{
    plan_generated_crate_layout, plan_namespace_layouts, plan_package_layouts,
    GeneratedCrateLayoutPlan, NamespaceLayoutPlan, PackageLayoutPlan,
};
pub use mangle::{
    escape_rust_field_ident, mangle_global_name, mangle_local_name, mangle_package_module_name,
    mangle_routine_name, mangle_type_name, sanitize_backend_ident,
};
pub use model::{BackendArtifact, BackendBuildPaths, EmittedRustFile};
pub use session::BackendSession;
pub use signatures::{
    render_global_declaration, render_routine_definition, render_routine_shell,
    render_routine_signature,
};
pub use trace::{
    build_backend_trace, build_emitted_source_map, BackendEmittedSourceMap,
    BackendEmittedSourceMapEntry, BackendTrace, BackendTraceKind, BackendTraceRecord,
};
pub use types::{
    render_entry_definition, render_entry_trait_impl, render_record_definition,
    render_record_trait_impl, render_rust_type, render_rust_type_in_workspace,
};

pub type BackendResult<T> = Result<T, BackendError>;

#[cfg(test)]
mod tests {
    use super::{
        Backend, BackendArtifact, BackendBuildProfile, BackendConfig, BackendError,
        BackendErrorKind, BackendFolModel, BackendMachineTarget, BackendMode, BackendResult,
        BackendRuntimeTier, BackendSession, BackendTarget, EmittedRustFile,
    };
    use crate::testing::sample_lowered_workspace;

    #[test]
    fn backend_foundation_public_surface_is_constructible() {
        let backend = Backend::new();
        let config = BackendConfig::default();
        let session = BackendSession::new(sample_lowered_workspace());
        let result: BackendResult<()> = Ok(());
        let artifact = BackendArtifact::RustSourceCrate {
            root: "target/fol-backend/demo".to_string(),
            files: vec![EmittedRustFile {
                path: "src/main.rs".to_string(),
                module_name: "main".to_string(),
                contents: "fn main() {}".to_string(),
            }],
        };
        let error = BackendError::new(BackendErrorKind::Unsupported, "not implemented yet");

        assert_eq!(format!("{backend:?}"), "Backend");
        assert_eq!(config.target, BackendTarget::Rust);
        assert_eq!(config.fol_model, BackendFolModel::Std);
        assert_eq!(config.runtime_tier(), BackendRuntimeTier::Std);
        assert_eq!(config.machine_target, BackendMachineTarget::host().unwrap());
        assert_eq!(config.build_profile, BackendBuildProfile::Release);
        assert_eq!(config.mode, BackendMode::BuildArtifact);
        assert_eq!(session.workspace().package_count(), 2);
        assert!(result.is_ok());
        assert!(matches!(artifact, BackendArtifact::RustSourceCrate { .. }));
        assert_eq!(error.to_string(), "BackendUnsupported: not implemented yet");
    }
}
