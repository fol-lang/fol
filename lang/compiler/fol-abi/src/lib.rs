//! The canonical FOL C ABI model.
//!
//! Dependency-foundation crate: `fol-types` is its only dependency, because
//! both the compiler and the interop stack consume this schema and anything it
//! pulled in would become a dependency of both. Enforced by
//! `crate_graph_tests`.

pub mod annotation;
pub mod compat;
pub mod import;
pub mod import_manifest;
pub mod interface;
pub mod json;
pub mod layout;
pub mod manifest;
pub mod metadata;
pub mod types;
pub mod verify;

pub use annotation::{
    AnnotationError, AnnotationOverlay, CallbackUse, HandleDomain, HandleRole, HandleUse,
    ImportEffects, ImportErrorConvention, RoutineAnnotation, ANNOTATION_SCHEMA_VERSION,
};
pub use compat::{
    classify_duplicate_symbols, classify_external_symbol, is_reserved_c_identifier,
    AbiClassification, AbiRejection,
};
pub use import::{
    scalar_for_measured_float, scalar_for_measured_integer, verify_effects, verify_status_mapping,
    CapabilityModel, ImportRejection, ImportedInterface, ImportedRoutine,
};
pub use import_manifest::{
    canonical_import_interface_json, ImportManifest, ImportManifestError, ImportProvenance,
    IMPORT_MANIFEST_SCHEMA, IMPORT_MANIFEST_SCHEMA_VERSION,
};
pub use interface::{
    AbiCallingConvention, AbiDirection, AbiEffects, AbiErrorContract, AbiFacing, AbiParameter,
    AbiSourceOrigin, ExportSelection, ForeignInterface, ForeignInterfaceTemplate, ForeignRoutine,
    ResolvedAbiSurface,
};
pub use json::{JsonError, JsonValue};
pub use layout::{
    record_layout, record_layouts, size_and_align, FieldPlacement, LayoutError, RecordLayout,
};
pub use manifest::{
    canonical_interface_json, canonical_type_table_json, compare_surfaces, digest,
    AbiCompatibility, AbiManifest, BuildProvenance, ManifestError, MANIFEST_SCHEMA,
    MANIFEST_SCHEMA_VERSION,
};
pub use metadata::{
    aggregate_matrix, c_projection_for, full_matrix, scalar_matrix, AbiTypeMatrixRow, STATUS_VALUES,
};
pub use types::{
    AbiEscape, AbiField, AbiMutability, AbiNullability, AbiOwnership, AbiScalar, AbiType,
    AbiTypeId, AbiTypeTable, AbiVariant,
};
pub use verify::{verify_export_set, verify_type, verify_type_at, AbiPosition, CandidateType};
