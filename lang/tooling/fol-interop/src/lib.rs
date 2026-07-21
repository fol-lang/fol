//! FOL-owned orchestration for the checked `PARC -> LINC -> GERC` pipeline.
//!
//! This crate owns target routing and the handoff into the sibling contracts.
//! It does not parse C, inspect native artifacts, derive ABI evidence, or emit
//! raw Rust bindings itself.

#![forbid(unsafe_code)]

mod analysis;
mod anchor;
mod generation;
mod identity;
mod lock;
mod materialization;
mod pipeline;
mod source;
mod toolchain;

pub use analysis::InteropAnalysisPolicyError;
pub use anchor::H7InteropAnchorError;
pub use identity::InteropIdentityError;
pub use materialization::InteropMaterializationPlanError;
pub use pipeline::{
    prepare_h7_interop, H7InteropBuild, H7InteropError, H7InteropReport, H7InteropRequest,
};
pub use source::InteropSourceError;
pub use toolchain::InteropToolchainError;

/// The only platform promoted for the initial FOL interop handoff.
pub const CERTIFIED_INTEROP_TARGET: &str = "x86_64-unknown-linux-gnu";
