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
pub use lock::{LOCKED_GERC_REVISION, LOCKED_LINC_REVISION, LOCKED_PARC_REVISION};
pub use materialization::InteropMaterializationPlanError;
pub use pipeline::{
    prepare_h7_interop, H7InteropBuild, H7InteropError, H7InteropReport, H7InteropRequest,
};
pub use source::InteropSourceError;
pub use toolchain::InteropToolchainError;

/// The only platform promoted for the initial FOL interop handoff.
pub const CERTIFIED_INTEROP_TARGET: &str = "x86_64-unknown-linux-gnu";

// The contract versions the pinned components must expose.
// These replace the deleted runtime git inspection: instead of asking whether a
// sibling checkout looks right, the compiler refuses to build against a
// component whose contract moved. The `rev` pins bind identity; these bind
// shape.
const _: () = assert!(
    parc::contract::SOURCE_PACKAGE_SCHEMA_VERSION == 2,
    "the pinned PARC exposes source-package schema version 2"
);
const _: () = assert!(
    linc::contract::LINK_ANALYSIS_SCHEMA_VERSION == 2,
    "the pinned LINC exposes link-analysis schema version 2"
);
const _: () = assert!(
    gerc::GENERATION_SCHEMA_VERSION == 1,
    "the pinned GERC exposes generation schema version 1"
);
