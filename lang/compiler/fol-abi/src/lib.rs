//! The canonical FOL C ABI model.
//!
//! Dependency-foundation crate: `fol-types` is its only dependency, because
//! both the compiler and the interop stack consume this schema and anything it
//! pulled in would become a dependency of both. Enforced by
//! `crate_graph_tests`.

pub mod compat;
pub mod interface;
pub mod manifest;
pub mod types;

pub use compat::{
    classify_duplicate_symbols, classify_external_symbol, is_reserved_c_identifier,
    AbiClassification, AbiRejection,
};
pub use interface::{
    AbiCallingConvention, AbiDirection, AbiErrorContract, AbiFacing, AbiParameter, AbiSourceOrigin,
    ForeignInterface, ForeignInterfaceTemplate, ForeignRoutine, ResolvedAbiSurface,
};
pub use manifest::{
    canonical_interface_json, canonical_type_table_json, compare_surfaces, digest,
    AbiCompatibility, AbiManifest, BuildProvenance, MANIFEST_SCHEMA, MANIFEST_SCHEMA_VERSION,
};
pub use types::{
    AbiEscape, AbiField, AbiMutability, AbiNullability, AbiOwnership, AbiScalar, AbiType,
    AbiTypeId, AbiTypeTable, AbiVariant,
};
