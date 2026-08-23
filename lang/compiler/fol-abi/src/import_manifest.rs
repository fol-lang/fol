//! The checked import manifest.
//!
//! Section 4.13 gives `fol tool bind c` the job of running the C pipeline once
//! and writing its accepted result down, so that ordinary compilation reads a
//! file instead of invoking a C preprocessor. This module is that file's
//! format, in both directions.
//!
//! It is written canonically -- sorted keys, no incidental order -- for the
//! same reason the export manifest is: the document is fingerprinted, and two
//! runs that accepted the same surface must produce the same bytes.
//!
//! The manifest is *checked*, never trusted: the build action re-runs the
//! pipeline and compares fingerprints. Reading a stale file is what
//! `verify_against` exists to catch.

use crate::annotation::{ImportEffects, ImportErrorConvention};
use crate::import::{ImportedInterface, ImportedRoutine};
use crate::interface::{AbiCallingConvention, AbiDirection, AbiParameter, AbiSourceOrigin};
use crate::json::{escape, JsonError, JsonValue};
use crate::manifest::digest;
use crate::types::{
    AbiEscape, AbiMutability, AbiNullability, AbiOwnership, AbiScalar, AbiType, AbiTypeId,
    AbiTypeTable,
};

/// The schema name written into every import manifest.
pub const IMPORT_MANIFEST_SCHEMA: &str = "fol.abi.import";

/// The only schema version this compiler reads.
pub const IMPORT_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Where an import manifest came from, for cache identity.
///
/// Separate from the interface for the same reason section 4.11 separates the
/// two export fingerprints: a new compiler or a rebuilt provider changes how
/// the surface was produced without changing the surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportProvenance {
    /// The entry header, package-relative.
    pub header: String,
    /// The provider artifact, package-relative.
    pub provider: String,
    /// `object`, `static`, or `shared`.
    pub provider_kind: String,
    /// The annotation overlay, package-relative, when there is one.
    pub annotations: Option<String>,
    /// The C compiler that measured the layouts.
    pub compiler: String,
    /// Pinned sibling revisions, as `parc=<rev>` style entries in order.
    pub components: Vec<String>,
}

/// One accepted import, as written to `<alias>.folabi.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportManifest {
    pub interface: ImportedInterface,
    pub provenance: ImportProvenance,
}

impl ImportManifest {
    /// The canonical document.
    pub fn canonical_json(&self) -> String {
        let interface = canonical_import_interface_json(&self.interface);
        let components = self
            .provenance
            .components
            .iter()
            .map(|component| quoted(component))
            .collect::<Vec<_>>()
            .join(",");
        // The interface object is embedded verbatim, so the bytes hashed by
        // `interface_fingerprint` appear unchanged inside the whole document.
        format!(
            "{{\"interface\":{interface},\"interface_fingerprint\":{},\
             \"provenance\":{{\"annotations\":{},\"compiler\":{},\"components\":[{components}],\
             \"header\":{},\"provider\":{},\"provider_kind\":{}}},\
             \"provenance_fingerprint\":{}}}",
            quoted(&self.interface_fingerprint()),
            match &self.provenance.annotations {
                Some(path) => quoted(path),
                None => "null".to_string(),
            },
            quoted(&self.provenance.compiler),
            quoted(&self.provenance.header),
            quoted(&self.provenance.provider),
            quoted(&self.provenance.provider_kind),
            quoted(&self.provenance_fingerprint()),
        )
    }

    /// The facts a FOL caller can see. Controls whether callers still compile.
    pub fn interface_fingerprint(&self) -> String {
        digest(canonical_import_interface_json(&self.interface).as_bytes())
    }

    /// How the surface was produced. Controls cache identity.
    pub fn provenance_fingerprint(&self) -> String {
        let mut rendered = String::new();
        for part in [
            self.provenance.header.as_str(),
            self.provenance.provider.as_str(),
            self.provenance.provider_kind.as_str(),
            self.provenance.annotations.as_deref().unwrap_or(""),
            self.provenance.compiler.as_str(),
        ] {
            rendered.push_str(part);
            rendered.push('\n');
        }
        for component in &self.provenance.components {
            rendered.push_str(component);
            rendered.push('\n');
        }
        digest(rendered.as_bytes())
    }

    /// Read one back.
    pub fn parse(text: &str) -> Result<Self, ImportManifestError> {
        let document = JsonValue::parse(text)?;
        let interface = read_interface(document.field("interface")?)?;
        let provenance_value = document.field("provenance")?;
        let provenance = ImportProvenance {
            header: provenance_value.string_field("header")?.to_string(),
            provider: provenance_value.string_field("provider")?.to_string(),
            provider_kind: provenance_value.string_field("provider_kind")?.to_string(),
            annotations: match provenance_value.field("annotations")? {
                JsonValue::Null => None,
                other => Some(
                    other
                        .as_str()
                        .ok_or(ImportManifestError::Json(JsonError::WrongType {
                            field: "annotations".to_string(),
                            expected: "string or null",
                        }))?
                        .to_string(),
                ),
            },
            compiler: provenance_value.string_field("compiler")?.to_string(),
            components: provenance_value
                .array_field("components")?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_string).ok_or(ImportManifestError::Json(
                        JsonError::WrongType {
                            field: "components".to_string(),
                            expected: "string",
                        },
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let manifest = Self {
            interface,
            provenance,
        };

        // A manifest whose recorded fingerprint disagrees with its own body
        // was edited by hand or truncated. Either way it is not evidence.
        let recorded = document.string_field("interface_fingerprint")?;
        let actual = manifest.interface_fingerprint();
        if recorded != actual {
            return Err(ImportManifestError::FingerprintMismatch {
                field: "interface",
                recorded: recorded.to_string(),
                actual,
            });
        }
        let recorded = document.string_field("provenance_fingerprint")?;
        let actual = manifest.provenance_fingerprint();
        if recorded != actual {
            return Err(ImportManifestError::FingerprintMismatch {
                field: "provenance",
                recorded: recorded.to_string(),
                actual,
            });
        }
        Ok(manifest)
    }

    /// Check a checked-in manifest against a freshly produced one.
    ///
    /// This is what makes the file evidence rather than a cache: the build
    /// re-runs the pipeline and refuses to link a surface that no longer
    /// matches what the source was compiled against.
    pub fn verify_against(&self, fresh: &Self) -> Result<(), ImportManifestError> {
        if self.interface.alias != fresh.interface.alias {
            return Err(ImportManifestError::AliasMismatch {
                recorded: self.interface.alias.clone(),
                actual: fresh.interface.alias.clone(),
            });
        }
        let recorded = self.interface_fingerprint();
        let actual = fresh.interface_fingerprint();
        if recorded != actual {
            return Err(ImportManifestError::StaleInterface {
                alias: self.interface.alias.clone(),
                recorded,
                actual,
            });
        }
        Ok(())
    }
}

/// The interface half, which is what `interface_fingerprint` hashes.
pub fn canonical_import_interface_json(interface: &ImportedInterface) -> String {
    let types = interface
        .types
        .iter()
        .map(|(id, ty)| render_type(id, ty))
        .collect::<Vec<_>>()
        .join(",");
    // Sorted by symbol: header declaration order is not an ABI fact, and
    // reordering two declarations must not change the fingerprint.
    let mut routines: Vec<&ImportedRoutine> = interface.routines.iter().collect();
    routines.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    let routines = routines
        .into_iter()
        .map(render_routine)
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"alias\":{},\"routines\":[{routines}],\"schema\":{},\
         \"schema_version\":{IMPORT_MANIFEST_SCHEMA_VERSION},\"target\":{},\"types\":[{types}]}}",
        quoted(&interface.alias),
        quoted(IMPORT_MANIFEST_SCHEMA),
        quoted(interface.target.rust_target_triple()),
    )
}

fn render_routine(routine: &ImportedRoutine) -> String {
    let parameters = routine
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{{\"direction\":{},\"name\":{},\"type\":{}}}",
                quoted(parameter.direction.as_str()),
                quoted(&parameter.name),
                parameter.type_id.0
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"convention\":{},\"effects\":{{\"allocates\":{},\"hosted\":{}}},\
         \"error\":{},\"fol_name\":{},\"origin\":{{\"column\":{},\"file\":{},\"line\":{}}},\
         \"parameters\":[{parameters}],\"result\":{},\"symbol\":{}}}",
        quoted(routine.convention.as_str()),
        routine.effects.allocates,
        routine.effects.hosted,
        render_error(&routine.error),
        quoted(&routine.fol_name),
        routine.origin.column,
        quoted(&routine.origin.file),
        routine.origin.line,
        routine.result.0,
        quoted(&routine.symbol),
    )
}

fn render_error(error: &ImportErrorConvention) -> String {
    match error {
        ImportErrorConvention::Infallible => "{\"kind\":\"infallible\"}".to_string(),
        ImportErrorConvention::Status {
            success,
            failure,
            out_parameter,
        } => format!(
            "{{\"failure\":[{}],\"kind\":\"status\",\"out\":{},\"success\":[{}]}}",
            render_integers(failure),
            quoted(out_parameter),
            render_integers(success),
        ),
    }
}

fn render_integers(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn render_type(id: AbiTypeId, ty: &AbiType) -> String {
    let body = match ty {
        AbiType::Void => "\"kind\":\"void\"".to_string(),
        AbiType::Scalar(scalar) => format!("\"kind\":\"scalar\",\"scalar\":{}", quoted(&scalar_name(*scalar))),
        AbiType::Pointer {
            target,
            mutability,
            nullability,
            ownership,
            escape: escape_rule,
            destructor,
        } => format!(
            "\"destructor\":{},\"escape\":{},\"kind\":\"pointer\",\"mutability\":{},\
             \"nullability\":{},\"ownership\":{},\"target\":{}",
            match destructor {
                Some(name) => quoted(name),
                None => "null".to_string(),
            },
            quoted(match escape_rule {
                AbiEscape::CallScoped => "call-scoped",
                AbiEscape::Retained => "retained",
            }),
            quoted(match mutability {
                AbiMutability::Const => "const",
                AbiMutability::Mutable => "mutable",
            }),
            quoted(match nullability {
                AbiNullability::NonNull => "non-null",
                AbiNullability::Nullable => "nullable",
            }),
            quoted(match ownership {
                AbiOwnership::Borrowed => "borrowed",
                AbiOwnership::Transferred => "transferred",
            }),
            target.0,
        ),
        // M6 imports scalars, void, and the pointers that carry an out-value.
        // The aggregate shapes belong to M7, and writing a placeholder for one
        // would put a type in the manifest that nothing can read back.
        other => format!("\"kind\":{}", quoted(other.kind_name())),
    };
    format!("{{\"id\":{},{body}}}", id.0)
}

fn scalar_name(scalar: AbiScalar) -> String {
    match scalar {
        AbiScalar::Bool => "bool".to_string(),
        AbiScalar::Char => "char".to_string(),
        AbiScalar::Int(width) => format!(
            "{}{}",
            if width.is_signed() { "i" } else { "u" },
            width.bits().expect("arch widths never reach the ABI")
        ),
        AbiScalar::Float(width) => format!(
            "f{}",
            match width {
                fol_types::FloatWidth::F32 => 32,
                _ => 64,
            }
        ),
    }
}

fn scalar_for_name(name: &str) -> Option<AbiScalar> {
    Some(match name {
        "bool" => AbiScalar::Bool,
        "char" => AbiScalar::Char,
        "i8" => AbiScalar::Int(fol_types::IntWidth::I8),
        "i16" => AbiScalar::Int(fol_types::IntWidth::I16),
        "i32" => AbiScalar::Int(fol_types::IntWidth::I32),
        "i64" => AbiScalar::Int(fol_types::IntWidth::I64),
        "u8" => AbiScalar::Int(fol_types::IntWidth::U8),
        "u16" => AbiScalar::Int(fol_types::IntWidth::U16),
        "u32" => AbiScalar::Int(fol_types::IntWidth::U32),
        "u64" => AbiScalar::Int(fol_types::IntWidth::U64),
        "f32" => AbiScalar::Float(fol_types::FloatWidth::F32),
        "f64" => AbiScalar::Float(fol_types::FloatWidth::F64),
        _ => return None,
    })
}

fn read_interface(value: &JsonValue) -> Result<ImportedInterface, ImportManifestError> {
    let schema = value.string_field("schema")?;
    if schema != IMPORT_MANIFEST_SCHEMA {
        return Err(ImportManifestError::UnknownSchema {
            found: schema.to_string(),
        });
    }
    let version = value.integer_field("schema_version")?;
    if version != i64::from(IMPORT_MANIFEST_SCHEMA_VERSION) {
        return Err(ImportManifestError::UnsupportedVersion { found: version });
    }
    let triple = value.string_field("target")?;
    let target = fol_types::ResolvedTarget::resolve(triple).map_err(|_| {
        ImportManifestError::UnknownTarget {
            triple: triple.to_string(),
        }
    })?;

    // The table is written in id order and read back positionally, so an id
    // that does not match its position means the document was reordered.
    let mut types = AbiTypeTable::new();
    for (position, entry) in value.array_field("types")?.iter().enumerate() {
        let id = entry.integer_field("id")?;
        let ty = read_type(entry)?;
        let interned = types.intern(ty);
        if id != interned.0 as i64 || position != interned.0 {
            return Err(ImportManifestError::TypeTableOutOfOrder {
                expected: position,
                found: id,
            });
        }
    }

    let mut routines = Vec::new();
    for entry in value.array_field("routines")? {
        routines.push(read_routine(entry, &types)?);
    }

    Ok(ImportedInterface {
        alias: value.string_field("alias")?.to_string(),
        target,
        types,
        routines,
    })
}

fn read_type(entry: &JsonValue) -> Result<AbiType, ImportManifestError> {
    let kind = entry.string_field("kind")?;
    Ok(match kind {
        "void" => AbiType::Void,
        "scalar" => {
            let name = entry.string_field("scalar")?;
            AbiType::Scalar(scalar_for_name(name).ok_or_else(|| {
                ImportManifestError::UnknownScalar {
                    name: name.to_string(),
                }
            })?)
        }
        "pointer" => AbiType::Pointer {
            target: AbiTypeId(usize::try_from(entry.integer_field("target")?).map_err(|_| {
                ImportManifestError::Json(JsonError::WrongType {
                    field: "target".to_string(),
                    expected: "a type index",
                })
            })?),
            mutability: match entry.string_field("mutability")? {
                "const" => AbiMutability::Const,
                _ => AbiMutability::Mutable,
            },
            nullability: match entry.string_field("nullability")? {
                "nullable" => AbiNullability::Nullable,
                _ => AbiNullability::NonNull,
            },
            ownership: match entry.string_field("ownership")? {
                "transferred" => AbiOwnership::Transferred,
                _ => AbiOwnership::Borrowed,
            },
            escape: match entry.string_field("escape")? {
                "retained" => AbiEscape::Retained,
                _ => AbiEscape::CallScoped,
            },
            destructor: match entry.field("destructor")? {
                JsonValue::Null => None,
                other => other.as_str().map(str::to_string),
            },
        },
        other => {
            return Err(ImportManifestError::UnsupportedTypeKind {
                kind: other.to_string(),
            })
        }
    })
}

fn read_routine(
    entry: &JsonValue,
    types: &AbiTypeTable,
) -> Result<ImportedRoutine, ImportManifestError> {
    let symbol = entry.string_field("symbol")?.to_string();
    let mut parameters = Vec::new();
    for parameter in entry.array_field("parameters")? {
        parameters.push(AbiParameter {
            name: parameter.string_field("name")?.to_string(),
            type_id: read_type_id(parameter.integer_field("type")?, types, &symbol)?,
            direction: match parameter.string_field("direction")? {
                "out" => AbiDirection::Out,
                "inout" => AbiDirection::InOut,
                _ => AbiDirection::In,
            },
        });
    }
    let effects_value = entry.field("effects")?;
    let origin_value = entry.field("origin")?;

    Ok(ImportedRoutine {
        fol_name: entry.string_field("fol_name")?.to_string(),
        convention: match entry.string_field("convention")? {
            "C" => AbiCallingConvention::C,
            other => {
                return Err(ImportManifestError::UnknownConvention {
                    convention: other.to_string(),
                })
            }
        },
        result: read_type_id(entry.integer_field("result")?, types, &symbol)?,
        error: read_error(entry.field("error")?)?,
        effects: ImportEffects {
            allocates: matches!(effects_value.field("allocates")?, JsonValue::Bool(true)),
            hosted: matches!(effects_value.field("hosted")?, JsonValue::Bool(true)),
        },
        origin: AbiSourceOrigin {
            file: origin_value.string_field("file")?.to_string(),
            line: u32::try_from(origin_value.integer_field("line")?).unwrap_or(0),
            column: u32::try_from(origin_value.integer_field("column")?).unwrap_or(0),
        },
        parameters,
        symbol,
    })
}

fn read_type_id(
    raw: i64,
    types: &AbiTypeTable,
    symbol: &str,
) -> Result<AbiTypeId, ImportManifestError> {
    let index = usize::try_from(raw).ok().filter(|index| *index < types.len());
    index.map(AbiTypeId).ok_or(ImportManifestError::DanglingTypeId {
        symbol: symbol.to_string(),
        id: raw,
    })
}

fn read_error(value: &JsonValue) -> Result<ImportErrorConvention, ImportManifestError> {
    Ok(match value.string_field("kind")? {
        "infallible" => ImportErrorConvention::Infallible,
        "status" => ImportErrorConvention::Status {
            success: read_integers(value, "success")?,
            failure: read_integers(value, "failure")?,
            out_parameter: value.string_field("out")?.to_string(),
        },
        other => {
            return Err(ImportManifestError::UnknownErrorKind {
                kind: other.to_string(),
            })
        }
    })
}

fn read_integers(value: &JsonValue, field: &str) -> Result<Vec<i64>, ImportManifestError> {
    value
        .array_field(field)?
        .iter()
        .map(|item| {
            item.as_i64().ok_or(ImportManifestError::Json(JsonError::WrongType {
                field: field.to_string(),
                expected: "integer",
            }))
        })
        .collect()
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", escape(text))
}

/// Why an import manifest could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportManifestError {
    Json(JsonError),
    UnknownSchema { found: String },
    UnsupportedVersion { found: i64 },
    UnknownTarget { triple: String },
    UnknownScalar { name: String },
    UnsupportedTypeKind { kind: String },
    UnknownConvention { convention: String },
    UnknownErrorKind { kind: String },
    TypeTableOutOfOrder { expected: usize, found: i64 },
    DanglingTypeId { symbol: String, id: i64 },
    FingerprintMismatch {
        field: &'static str,
        recorded: String,
        actual: String,
    },
    AliasMismatch { recorded: String, actual: String },
    StaleInterface {
        alias: String,
        recorded: String,
        actual: String,
    },
}

impl ImportManifestError {
    /// The registered diagnostic code.
    pub const fn diagnostic_code(&self) -> &'static str {
        "A1009"
    }
}

impl From<JsonError> for ImportManifestError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl std::fmt::Display for ImportManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "the import manifest is not readable: {error}"),
            Self::UnknownSchema { found } => write!(
                f,
                "'{found}' is not an import manifest; expected schema '{IMPORT_MANIFEST_SCHEMA}'"
            ),
            Self::UnsupportedVersion { found } => write!(
                f,
                "import manifest schema version {found} is not supported; this compiler reads \
                 version {IMPORT_MANIFEST_SCHEMA_VERSION}"
            ),
            Self::UnknownTarget { triple } => {
                write!(f, "the import manifest names unknown target '{triple}'")
            }
            Self::UnknownScalar { name } => {
                write!(f, "the import manifest names unknown scalar '{name}'")
            }
            Self::UnsupportedTypeKind { kind } => write!(
                f,
                "the import manifest carries a '{kind}' type, which this compiler cannot read back"
            ),
            Self::UnknownConvention { convention } => write!(
                f,
                "the import manifest names calling convention '{convention}'"
            ),
            Self::UnknownErrorKind { kind } => write!(
                f,
                "the import manifest names error convention '{kind}'"
            ),
            Self::TypeTableOutOfOrder { expected, found } => write!(
                f,
                "the import manifest's type table is out of order: position {expected} carries \
                 id {found}"
            ),
            Self::DanglingTypeId { symbol, id } => write!(
                f,
                "routine '{symbol}' references type id {id}, which the manifest's table does not \
                 define"
            ),
            Self::FingerprintMismatch {
                field,
                recorded,
                actual,
            } => write!(
                f,
                "the import manifest's recorded {field} fingerprint {recorded} does not match its \
                 own contents ({actual}); the file was edited or truncated"
            ),
            Self::AliasMismatch { recorded, actual } => write!(
                f,
                "the checked-in import manifest is for alias '{recorded}', but the build produced \
                 '{actual}'"
            ),
            Self::StaleInterface {
                alias,
                recorded,
                actual,
            } => write!(
                f,
                "the checked-in import manifest for '{alias}' is stale: it records interface \
                 {recorded} and the provider now yields {actual}; re-run `fol tool bind c`"
            ),
        }
    }
}

impl std::error::Error for ImportManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> fol_types::ResolvedTarget {
        fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").expect("certified target")
    }

    fn scalar_manifest() -> ImportManifest {
        let mut types = AbiTypeTable::new();
        let i32_id = types.intern_int(fol_types::IntWidth::I32);
        let out_id = types.intern(AbiType::Pointer {
            target: i32_id,
            mutability: AbiMutability::Mutable,
            nullability: AbiNullability::NonNull,
            ownership: AbiOwnership::Borrowed,
            escape: AbiEscape::CallScoped,
            destructor: None,
        });
        // Interned but unreferenced, which is the shape the table has when a
        // header declares a void-returning routine the overlay does not select.
        types.intern(AbiType::Void);

        ImportManifest {
            interface: ImportedInterface {
                alias: "c_math".to_string(),
                target: target(),
                types,
                routines: vec![
                    ImportedRoutine {
                        symbol: "c_math_add_one".to_string(),
                        fol_name: "add_one".to_string(),
                        convention: AbiCallingConvention::C,
                        parameters: vec![AbiParameter {
                            name: "value".to_string(),
                            type_id: i32_id,
                            direction: AbiDirection::In,
                        }],
                        result: i32_id,
                        error: ImportErrorConvention::Infallible,
                        effects: ImportEffects::default(),
                        origin: AbiSourceOrigin {
                            file: "native/c_math.h".to_string(),
                            line: 7,
                            column: 5,
                        },
                    },
                    ImportedRoutine {
                        symbol: "c_math_checked_div".to_string(),
                        fol_name: "checked_div".to_string(),
                        convention: AbiCallingConvention::C,
                        parameters: vec![
                            AbiParameter {
                                name: "lhs".to_string(),
                                type_id: i32_id,
                                direction: AbiDirection::In,
                            },
                            AbiParameter {
                                name: "result".to_string(),
                                type_id: out_id,
                                direction: AbiDirection::Out,
                            },
                        ],
                        result: i32_id,
                        error: ImportErrorConvention::Status {
                            success: vec![0],
                            failure: vec![1, 2],
                            out_parameter: "result".to_string(),
                        },
                        effects: ImportEffects {
                            allocates: true,
                            hosted: false,
                        },
                        origin: AbiSourceOrigin {
                            file: "native/c_math.h".to_string(),
                            line: 11,
                            column: 5,
                        },
                    },
                ],
            },
            provenance: ImportProvenance {
                header: "native/c_math.h".to_string(),
                provider: "native/libc_math.a".to_string(),
                provider_kind: "static".to_string(),
                annotations: Some("interop/c_math.toml".to_string()),
                compiler: "/usr/bin/gcc".to_string(),
                components: vec!["parc=0f52aee".to_string(), "linc=38f73db".to_string()],
            },
        }
    }

    #[test]
    fn a_manifest_round_trips_through_its_canonical_json() {
        let manifest = scalar_manifest();
        let document = manifest.canonical_json();

        let parsed = ImportManifest::parse(&document).expect("a written manifest should read back");
        assert_eq!(parsed, manifest);
        // And the second rendering is byte-identical, which is what makes the
        // file comparable rather than merely equivalent.
        assert_eq!(parsed.canonical_json(), document);
    }

    #[test]
    fn the_interface_fingerprint_ignores_how_the_surface_was_produced() {
        let manifest = scalar_manifest();
        let mut rebuilt = manifest.clone();
        rebuilt.provenance.compiler = "/usr/bin/clang".to_string();
        rebuilt.provenance.components = vec!["parc=deadbee".to_string()];

        assert_eq!(
            manifest.interface_fingerprint(),
            rebuilt.interface_fingerprint(),
            "a different compiler does not change what a FOL caller sees"
        );
        assert_ne!(
            manifest.provenance_fingerprint(),
            rebuilt.provenance_fingerprint(),
            "but it does change cache identity"
        );
        assert_eq!(manifest.verify_against(&rebuilt), Ok(()));
    }

    #[test]
    fn a_changed_signature_changes_the_interface_fingerprint() {
        let manifest = scalar_manifest();
        let mut changed = manifest.clone();
        changed.interface.routines[0].parameters.clear();

        assert_ne!(
            manifest.interface_fingerprint(),
            changed.interface_fingerprint()
        );
        assert!(matches!(
            manifest.verify_against(&changed),
            Err(ImportManifestError::StaleInterface { .. })
        ));
    }

    #[test]
    fn routine_order_in_the_file_does_not_change_the_fingerprint() {
        let manifest = scalar_manifest();
        let mut reordered = manifest.clone();
        reordered.interface.routines.reverse();

        assert_eq!(
            manifest.interface_fingerprint(),
            reordered.interface_fingerprint(),
            "declaration order is not an ABI fact"
        );
    }

    #[test]
    fn a_hand_edited_manifest_is_refused() {
        let manifest = scalar_manifest();
        // Change a name without recomputing the fingerprint, which is exactly
        // what editing the generated file by hand looks like.
        let document = manifest
            .canonical_json()
            .replace("\"add_one\"", "\"add_two\"");

        assert!(matches!(
            ImportManifest::parse(&document),
            Err(ImportManifestError::FingerprintMismatch {
                field: "interface",
                ..
            })
        ));
    }

    #[test]
    fn a_manifest_from_another_schema_or_version_is_refused() {
        let manifest = scalar_manifest();
        let document = manifest.canonical_json();

        let wrong_schema = document.replace(IMPORT_MANIFEST_SCHEMA, "fol.abi.export");
        assert!(matches!(
            ImportManifest::parse(&wrong_schema),
            Err(ImportManifestError::UnknownSchema { .. })
        ));

        let wrong_version = document.replace("\"schema_version\":1", "\"schema_version\":2");
        assert!(matches!(
            ImportManifest::parse(&wrong_version),
            Err(ImportManifestError::UnsupportedVersion { found: 2 })
        ));
    }

    #[test]
    fn a_routine_referencing_a_missing_type_is_refused() {
        let manifest = scalar_manifest();
        let document = manifest
            .canonical_json()
            .replace("\"result\":0", "\"result\":99");

        assert!(matches!(
            ImportManifest::parse(&document),
            Err(ImportManifestError::DanglingTypeId { .. })
                | Err(ImportManifestError::FingerprintMismatch { .. })
        ));
    }

    #[test]
    fn an_alias_mismatch_is_reported_before_the_fingerprint() {
        let manifest = scalar_manifest();
        let mut other = manifest.clone();
        other.interface.alias = "c_string".to_string();

        assert_eq!(
            manifest.verify_against(&other),
            Err(ImportManifestError::AliasMismatch {
                recorded: "c_math".to_string(),
                actual: "c_string".to_string(),
            })
        );
    }

    #[test]
    fn the_status_mapping_survives_the_round_trip_intact() {
        let manifest = scalar_manifest();
        let parsed =
            ImportManifest::parse(&manifest.canonical_json()).expect("manifest should read back");
        let routine = parsed
            .interface
            .routine("checked_div")
            .expect("the status routine should be present");

        assert_eq!(
            routine.error,
            ImportErrorConvention::Status {
                success: vec![0],
                failure: vec![1, 2],
                out_parameter: "result".to_string(),
            }
        );
        assert_eq!(routine.out_parameter_index(), Some(1));
        assert!(routine.effects.allocates);
        assert_eq!(routine.origin.line, 11);
    }
}
