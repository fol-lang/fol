//! The `.folabi.json` manifest, its canonical encoding, and the two
//! fingerprints.
//!
//! Canonical encoding is written by hand rather than through a serializer.
//! Section 4.11 requires sorted object keys, semantic arrays in defined order,
//! no insignificant formatting, and byte-identical output across runs -- and a
//! derived serializer gives no guarantee about any of those, so the encoding
//! would depend on a dependency's field order.
//!
//! The two fingerprints exist because a compiler upgrade must not look like an
//! ABI break. `interface_fingerprint` covers only public target ABI facts;
//! `build_fingerprint` covers toolchain, profile, and native inputs.

use crate::interface::{AbiErrorContract, AbiFacing, ForeignRoutine, ResolvedAbiSurface};
use crate::json::{JsonError, JsonValue};
use crate::types::{AbiScalar, AbiType, AbiTypeId, AbiTypeTable};

/// The schema this manifest is written against.
pub const MANIFEST_SCHEMA: &str = "fol.abi.manifest";
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn render_type(id: AbiTypeId, ty: &AbiType) -> String {
    // Keys are emitted in sorted order by hand. A derived serializer would
    // order them by declaration, which is not a property this file can rely on.
    let mut fields: Vec<(String, String)> = vec![
        ("id".to_string(), id.0.to_string()),
        ("kind".to_string(), quoted(ty.kind_name())),
    ];
    match ty {
        AbiType::Scalar(scalar) => {
            fields.push(("c_type".to_string(), quoted(&scalar.c_type())));
            let spelling = match scalar {
                AbiScalar::Int(width) => width.as_str().to_string(),
                AbiScalar::Float(width) => width.as_str().to_string(),
                AbiScalar::Bool => "bol".to_string(),
                AbiScalar::Char => "chr".to_string(),
            };
            fields.push(("scalar".to_string(), quoted(&spelling)));
        }
        AbiType::Void => {}
        AbiType::Pointer {
            target,
            mutability,
            nullability,
            ownership,
            escape,
            destructor,
        } => {
            fields.push((
                "destructor".to_string(),
                destructor
                    .as_ref()
                    .map(|name| quoted(name))
                    .unwrap_or_else(|| "null".to_string()),
            ));
            fields.push(("escape".to_string(), quoted(&format!("{escape:?}"))));
            fields.push(("mutability".to_string(), quoted(&format!("{mutability:?}"))));
            fields.push((
                "nullability".to_string(),
                quoted(&format!("{nullability:?}")),
            ));
            fields.push(("ownership".to_string(), quoted(&format!("{ownership:?}"))));
            fields.push(("target".to_string(), target.0.to_string()));
        }
        AbiType::Record {
            name,
            fields: record_fields,
        } => {
            // Field order is semantic: it decides every offset, so it is
            // emitted as declared and never sorted.
            let rendered = record_fields
                .iter()
                .map(|field| {
                    format!(
                        "{{\"name\":{},\"type\":{}}}",
                        quoted(&field.name),
                        field.type_id.0
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            fields.push(("fields".to_string(), format!("[{rendered}]")));
            fields.push(("name".to_string(), quoted(name)));
        }
        AbiType::Entry {
            name,
            tag,
            variants,
        } => {
            let rendered = variants
                .iter()
                .map(|variant| {
                    format!(
                        "{{\"discriminant\":{},\"name\":{},\"payload\":{}}}",
                        variant.discriminant,
                        quoted(&variant.name),
                        variant
                            .payload
                            .map(|id| id.0.to_string())
                            .unwrap_or_else(|| "null".to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            fields.push(("name".to_string(), quoted(name)));
            fields.push(("tag".to_string(), quoted(tag.as_str())));
            fields.push(("variants".to_string(), format!("[{rendered}]")));
        }
        AbiType::BorrowedString => {}
        AbiType::BorrowedSlice {
            element,
            mutability,
        } => {
            fields.push(("element".to_string(), element.0.to_string()));
            fields.push(("mutability".to_string(), quoted(&format!("{mutability:?}"))));
        }
        AbiType::OpaqueHandle { name } => {
            fields.push(("name".to_string(), quoted(name)));
        }
        AbiType::Callback { parameters, result } => {
            // Positional, because a callback's parameter order is part of the
            // contract exactly as a routine's is.
            let rendered = parameters
                .iter()
                .map(|id| id.0.to_string())
                .collect::<Vec<_>>()
                .join(",");
            fields.push(("parameters".to_string(), format!("[{rendered}]")));
            fields.push(("result".to_string(), result.0.to_string()));
        }
    }
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    let body = fields
        .into_iter()
        .map(|(key, value)| format!("{}:{value}", quoted(&key)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

fn render_routine(routine: &ForeignRoutine) -> String {
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
    let error = match &routine.error {
        AbiErrorContract::Infallible => "null".to_string(),
        AbiErrorContract::Recoverable { error_type } => error_type.0.to_string(),
    };
    let facing = match routine.facing {
        AbiFacing::Export => "export",
        AbiFacing::Import => "import",
    };
    // Sorted keys, and `parameters` kept in declaration order because argument
    // order is part of the ABI.
    // A handle is part of the public contract -- it says who releases what --
    // so it belongs in the fingerprinted body rather than beside it.
    let handle = match &routine.handle {
        Some(use_) => format!(
            "{{\"domain\":{},\"role\":{}}}",
            quoted(&use_.domain),
            quoted(use_.role.as_str())
        ),
        None => "null".to_string(),
    };
    format!(
        "{{\"convention\":{},\"error\":{error},\"facing\":{},\"fol_path\":{},\
         \"handle\":{handle},\"parameters\":[{parameters}],\"result\":{},\"symbol\":{}}}",
        quoted(routine.convention.as_str()),
        quoted(facing),
        quoted(&routine.fol_path),
        routine.result.0,
        quoted(&routine.symbol)
    )
}

/// The canonical JSON for a surface's *interface* facts.
///
/// Deliberately excludes anything about how it was built: this is the input to
/// `interface_fingerprint`, and a compiler upgrade must not move it.
pub fn canonical_interface_json(surface: &ResolvedAbiSurface) -> String {
    let types = surface
        .interface
        .types
        .iter()
        .map(|(id, ty)| render_type(id, ty))
        .collect::<Vec<_>>()
        .join(",");
    // Routines are sorted by symbol: declaration order is not an ABI fact, and
    // reordering two exports in a source file must not change the fingerprint.
    let mut routines: Vec<&ForeignRoutine> = surface.interface.routines.iter().collect();
    routines.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    let routines = routines
        .into_iter()
        .map(render_routine)
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"abi\":{{\"major\":{},\"minor\":{}}},\"artifact\":{},\"routines\":[{routines}],\
         \"schema\":{},\"schema_version\":{MANIFEST_SCHEMA_VERSION},\"target\":{},\
         \"types\":[{types}]}}",
        surface.major,
        surface.minor,
        quoted(&surface.artifact),
        quoted(MANIFEST_SCHEMA),
        quoted(surface.interface.target.rust_target_triple()),
    )
}

/// How an artifact was built. Feeds `build_fingerprint` only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildProvenance {
    pub compiler: String,
    pub runtime: String,
    pub profile: String,
    /// Native inputs in link order. Order is significant.
    pub native_inputs: Vec<String>,
}

/// The complete manifest written as `<artifact>.folabi.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiManifest {
    pub surface: ResolvedAbiSurface,
    pub provenance: BuildProvenance,
}

impl AbiManifest {
    /// The full canonical document.
    pub fn canonical_json(&self) -> String {
        let interface = canonical_interface_json(&self.surface);
        let inputs = self
            .provenance
            .native_inputs
            .iter()
            .map(|input| quoted(input))
            .collect::<Vec<_>>()
            .join(",");
        // The interface object is embedded verbatim so the bytes hashed by
        // `interface_fingerprint` appear unchanged inside the full document.
        format!(
            "{{\"build_fingerprint\":{},\"interface\":{interface},\
             \"interface_fingerprint\":{},\"provenance\":{{\"compiler\":{},\
             \"native_inputs\":[{inputs}],\"profile\":{},\"runtime\":{}}}}}",
            quoted(&self.build_fingerprint()),
            quoted(&self.interface_fingerprint()),
            quoted(&self.provenance.compiler),
            quoted(&self.provenance.profile),
            quoted(&self.provenance.runtime),
        )
    }

    /// Public target ABI facts only. Controls compatibility.
    pub fn interface_fingerprint(&self) -> String {
        digest(canonical_interface_json(&self.surface).as_bytes())
    }

    /// Toolchain, profile, and native inputs. Controls cache identity.
    pub fn build_fingerprint(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(&self.provenance.compiler);
        rendered.push('\n');
        rendered.push_str(&self.provenance.runtime);
        rendered.push('\n');
        rendered.push_str(&self.provenance.profile);
        rendered.push('\n');
        for input in &self.provenance.native_inputs {
            rendered.push_str(input);
            rendered.push('\n');
        }
        digest(rendered.as_bytes())
    }
}

/// FNV-1a, matching every other fingerprint in the tree. Stable across
/// platforms and Rust versions, which `std`'s hasher is not.
pub fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Build a type table's canonical rendering, for tests and diagnostics.
pub fn canonical_type_table_json(table: &AbiTypeTable) -> String {
    let rendered = table
        .iter()
        .map(|(id, ty)| render_type(id, ty))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{rendered}]")
}

/// Why a written manifest could not be read back.
///
/// Reading is a separate concern from writing, and the errors say so: every
/// variant names the exact fact that did not survive, because a manifest is
/// evidence and "malformed" tells a reader nothing about what to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Json(JsonError),
    UnknownSchema {
        found: String,
    },
    UnsupportedVersion {
        found: i64,
    },
    UnknownTarget {
        triple: String,
    },
    UnknownScalar {
        name: String,
    },
    UnknownConvention {
        convention: String,
    },
    UnknownFacing {
        facing: String,
    },
    UnknownTagWidth {
        width: String,
    },
    UnsupportedTypeKind {
        kind: String,
    },
    TypeTableOutOfOrder {
        expected: usize,
        found: i64,
    },
    DanglingTypeId {
        symbol: String,
        id: i64,
    },
    /// The document's own recorded fingerprint disagrees with its body, which
    /// means it was hand-edited or truncated. Either way it is not evidence.
    FingerprintMismatch {
        field: &'static str,
        recorded: String,
        actual: String,
    },
}

impl From<JsonError> for ManifestError {
    fn from(error: JsonError) -> Self {
        Self::Json(error)
    }
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(f, "the ABI manifest is not readable: {error}"),
            Self::UnknownSchema { found } => write!(
                f,
                "the ABI manifest declares schema '{found}'; this compiler reads '{MANIFEST_SCHEMA}'"
            ),
            Self::UnsupportedVersion { found } => write!(
                f,
                "the ABI manifest declares schema version {found}; this compiler reads \
                 {MANIFEST_SCHEMA_VERSION}"
            ),
            Self::UnknownTarget { triple } => {
                write!(f, "the ABI manifest names unknown target '{triple}'")
            }
            Self::UnknownScalar { name } => {
                write!(f, "the ABI manifest names unknown scalar '{name}'")
            }
            Self::UnknownConvention { convention } => write!(
                f,
                "the ABI manifest names calling convention '{convention}'"
            ),
            Self::UnknownFacing { facing } => {
                write!(f, "the ABI manifest names direction '{facing}'")
            }
            Self::UnknownTagWidth { width } => {
                write!(f, "the ABI manifest names entry tag width '{width}'")
            }
            Self::UnsupportedTypeKind { kind } => {
                write!(f, "the ABI manifest names type kind '{kind}'")
            }
            Self::TypeTableOutOfOrder { expected, found } => write!(
                f,
                "the ABI manifest's type table is out of order: position {expected} carries id \
                 {found}"
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
                "the ABI manifest records {field} fingerprint {recorded} but its contents hash to \
                 {actual}; it was edited by hand or truncated"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

impl AbiManifest {
    /// Read a written manifest back.
    ///
    /// The inverse of `canonical_json`, and checked against it: both recorded
    /// fingerprints are recomputed from the reconstructed document and compared.
    /// That is what makes a checked-in manifest evidence rather than a note --
    /// `fol tool abi check` compares two of these, and comparing a hand-edited
    /// file would prove nothing.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let document = JsonValue::parse(text)?;
        let surface = read_surface(document.field("interface")?)?;
        let provenance_value = document.field("provenance")?;
        let provenance = BuildProvenance {
            compiler: provenance_value.string_field("compiler")?.to_string(),
            runtime: provenance_value.string_field("runtime")?.to_string(),
            profile: provenance_value.string_field("profile")?.to_string(),
            native_inputs: provenance_value
                .array_field("native_inputs")?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_string).ok_or(ManifestError::Json(
                        JsonError::WrongType {
                            field: "native_inputs".to_string(),
                            expected: "string",
                        },
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let manifest = Self {
            surface,
            provenance,
        };

        for (field, recorded) in [
            (
                "interface",
                document.string_field("interface_fingerprint")?.to_string(),
            ),
            (
                "build",
                document.string_field("build_fingerprint")?.to_string(),
            ),
        ] {
            let actual = match field {
                "interface" => manifest.interface_fingerprint(),
                _ => manifest.build_fingerprint(),
            };
            if recorded != actual {
                return Err(ManifestError::FingerprintMismatch {
                    field: match field {
                        "interface" => "interface",
                        _ => "build",
                    },
                    recorded,
                    actual,
                });
            }
        }
        Ok(manifest)
    }
}

fn read_surface(value: &JsonValue) -> Result<ResolvedAbiSurface, ManifestError> {
    let schema = value.string_field("schema")?;
    if schema != MANIFEST_SCHEMA {
        return Err(ManifestError::UnknownSchema {
            found: schema.to_string(),
        });
    }
    let version = value.integer_field("schema_version")?;
    if version != i64::from(MANIFEST_SCHEMA_VERSION) {
        return Err(ManifestError::UnsupportedVersion { found: version });
    }
    let triple = value.string_field("target")?;
    let target =
        fol_types::ResolvedTarget::resolve(triple).map_err(|_| ManifestError::UnknownTarget {
            triple: triple.to_string(),
        })?;

    // Written in id order and read back positionally: an id that does not match
    // its position means the document was reordered, which would silently
    // repoint every routine at a different type.
    let mut types = AbiTypeTable::new();
    for (position, entry) in value.array_field("types")?.iter().enumerate() {
        let id = entry.integer_field("id")?;
        let ty = read_type(entry)?;
        let interned = types.intern(ty);
        if id != interned.0 as i64 || position != interned.0 {
            return Err(ManifestError::TypeTableOutOfOrder {
                expected: position,
                found: id,
            });
        }
    }

    let mut routines = Vec::new();
    for entry in value.array_field("routines")? {
        routines.push(read_routine(entry, &types)?);
    }

    let abi = value.field("abi")?;
    Ok(ResolvedAbiSurface {
        artifact: value.string_field("artifact")?.to_string(),
        major: u32::try_from(abi.integer_field("major")?).unwrap_or(0),
        minor: u32::try_from(abi.integer_field("minor")?).unwrap_or(0),
        interface: crate::interface::ForeignInterface {
            target,
            types,
            routines,
        },
    })
}

fn read_type(entry: &JsonValue) -> Result<AbiType, ManifestError> {
    let kind = entry.string_field("kind")?;
    Ok(match kind {
        "void" => AbiType::Void,
        "scalar" => {
            let name = entry.string_field("scalar")?;
            AbiType::Scalar(export_scalar_for_name(name).ok_or_else(|| {
                ManifestError::UnknownScalar {
                    name: name.to_string(),
                }
            })?)
        }
        "borrowed-string" => AbiType::BorrowedString,
        "opaque-handle" => AbiType::OpaqueHandle {
            name: entry.string_field("name")?.to_string(),
        },
        "pointer" => AbiType::Pointer {
            target: read_type_index(entry.integer_field("target")?)?,
            mutability: match entry.string_field("mutability")? {
                "Const" => crate::types::AbiMutability::Const,
                _ => crate::types::AbiMutability::Mutable,
            },
            nullability: match entry.string_field("nullability")? {
                "Nullable" => crate::types::AbiNullability::Nullable,
                _ => crate::types::AbiNullability::NonNull,
            },
            ownership: match entry.string_field("ownership")? {
                "Transferred" => crate::types::AbiOwnership::Transferred,
                _ => crate::types::AbiOwnership::Borrowed,
            },
            escape: match entry.string_field("escape")? {
                "Retained" => crate::types::AbiEscape::Retained,
                _ => crate::types::AbiEscape::CallScoped,
            },
            destructor: match entry.field("destructor")? {
                JsonValue::Null => None,
                other => other.as_str().map(str::to_string),
            },
        },
        "borrowed-slice" => AbiType::BorrowedSlice {
            element: read_type_index(entry.integer_field("element")?)?,
            mutability: match entry.string_field("mutability")? {
                "Const" => crate::types::AbiMutability::Const,
                _ => crate::types::AbiMutability::Mutable,
            },
        },
        "record" => {
            let mut fields = Vec::new();
            for field in entry.array_field("fields")? {
                fields.push(crate::types::AbiField {
                    name: field.string_field("name")?.to_string(),
                    type_id: read_type_index(field.integer_field("type")?)?,
                });
            }
            AbiType::Record {
                name: entry.string_field("name")?.to_string(),
                fields,
            }
        }
        "entry" => {
            let mut variants = Vec::new();
            for variant in entry.array_field("variants")? {
                variants.push(crate::types::AbiVariant {
                    name: variant.string_field("name")?.to_string(),
                    discriminant: variant.integer_field("discriminant")?,
                    payload: match variant.field("payload")? {
                        JsonValue::Null => None,
                        other => Some(read_type_index(other.as_i64().unwrap_or(-1))?),
                    },
                });
            }
            let width = entry.string_field("tag")?;
            AbiType::Entry {
                name: entry.string_field("name")?.to_string(),
                tag: int_width_for_name(width).ok_or_else(|| ManifestError::UnknownTagWidth {
                    width: width.to_string(),
                })?,
                variants,
            }
        }
        other => {
            return Err(ManifestError::UnsupportedTypeKind {
                kind: other.to_string(),
            })
        }
    })
}

fn read_routine(entry: &JsonValue, types: &AbiTypeTable) -> Result<ForeignRoutine, ManifestError> {
    let symbol = entry.string_field("symbol")?.to_string();
    let mut parameters = Vec::new();
    for parameter in entry.array_field("parameters")? {
        parameters.push(crate::interface::AbiParameter {
            name: parameter.string_field("name")?.to_string(),
            type_id: read_referenced_type(parameter.integer_field("type")?, types, &symbol)?,
            direction: match parameter.string_field("direction")? {
                "out" => crate::interface::AbiDirection::Out,
                "inout" => crate::interface::AbiDirection::InOut,
                _ => crate::interface::AbiDirection::In,
            },
        });
    }
    let facing = entry.string_field("facing")?;
    // The domain and role, when the routine names one. Absent for every
    // routine that does not touch a handle, which is most of them.
    let handle = match entry.field("handle") {
        Ok(JsonValue::Null) | Err(_) => None,
        Ok(value) => Some(crate::annotation::HandleUse {
            domain: value.string_field("domain")?.to_string(),
            role: crate::annotation::HandleRole::from_keyword(value.string_field("role")?)
                .ok_or_else(|| ManifestError::UnknownConvention {
                    convention: value.string_field("role").unwrap_or_default().to_string(),
                })?,
        }),
    };
    Ok(ForeignRoutine {
        handle,
        fol_path: entry.string_field("fol_path")?.to_string(),
        convention: match entry.string_field("convention")? {
            "C" => crate::interface::AbiCallingConvention::C,
            other => {
                return Err(ManifestError::UnknownConvention {
                    convention: other.to_string(),
                })
            }
        },
        facing: match facing {
            "export" => AbiFacing::Export,
            "import" => AbiFacing::Import,
            other => {
                return Err(ManifestError::UnknownFacing {
                    facing: other.to_string(),
                })
            }
        },
        result: read_referenced_type(entry.integer_field("result")?, types, &symbol)?,
        error: match entry.field("error")? {
            JsonValue::Null => AbiErrorContract::Infallible,
            other => AbiErrorContract::Recoverable {
                error_type: read_referenced_type(other.as_i64().unwrap_or(-1), types, &symbol)?,
            },
        },
        // Neither selection nor effects nor origin is an ABI fact, so the
        // manifest does not carry them and a read-back surface uses the
        // defaults. `compare_surfaces` reads none of the three.
        selection: crate::interface::ExportSelection {
            package_visible: true,
            abi_selected: true,
        },
        effects: crate::interface::AbiEffects::default(),
        origin: crate::interface::AbiSourceOrigin::default(),
        parameters,
        symbol,
    })
}

fn read_type_index(raw: i64) -> Result<AbiTypeId, ManifestError> {
    usize::try_from(raw).map(AbiTypeId).map_err(|_| {
        ManifestError::Json(JsonError::WrongType {
            field: "type".to_string(),
            expected: "a type index",
        })
    })
}

fn read_referenced_type(
    raw: i64,
    types: &AbiTypeTable,
    symbol: &str,
) -> Result<AbiTypeId, ManifestError> {
    usize::try_from(raw)
        .ok()
        .filter(|index| *index < types.len())
        .map(AbiTypeId)
        .ok_or(ManifestError::DanglingTypeId {
            symbol: symbol.to_string(),
            id: raw,
        })
}

/// The export manifest's own scalar spellings.
///
/// Deliberately not shared with the import manifest's reader: that one writes
/// `bool`/`char` and this one writes FOL's `bol`/`chr`, and a shared mapper
/// accepting both would let a document mix the two.
fn export_scalar_for_name(name: &str) -> Option<AbiScalar> {
    Some(match name {
        "bol" => AbiScalar::Bool,
        "chr" => AbiScalar::Char,
        "f32" => AbiScalar::Float(fol_types::FloatWidth::F32),
        "f64" => AbiScalar::Float(fol_types::FloatWidth::F64),
        other => AbiScalar::Int(int_width_for_name(other)?),
    })
}

fn int_width_for_name(name: &str) -> Option<fol_types::IntWidth> {
    fol_types::IntWidth::ALL
        .iter()
        .copied()
        .find(|width| width.as_str() == name)
}

/// How two interfaces relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiCompatibility {
    /// Byte-identical public facts.
    Identical,
    /// Only disjoint additions; existing symbols unchanged.
    MinorCompatible,
    /// A public symbol, type, layout, ownership, or error rule changed.
    Breaking,
    /// The two surfaces describe different targets, so no comparison of their
    /// layouts means anything. Distinct from `Breaking` because nothing about
    /// the source changed -- the baseline is simply not evidence for this
    /// target.
    TargetMismatch,
}

/// Compare a candidate surface against a baseline.
///
/// Adding a disjoint symbol is minor-compatible; changing or removing one is
/// breaking. Section 4.11 requires a checked-in baseline comparison to fail a
/// breaking change unless the ABI major is explicitly incremented.
pub fn compare_surfaces(
    baseline: &ResolvedAbiSurface,
    candidate: &ResolvedAbiSurface,
) -> AbiCompatibility {
    if canonical_interface_json(baseline) == canonical_interface_json(candidate) {
        return AbiCompatibility::Identical;
    }
    // Cross-target manifests are never comparable as if layout-compatible, and
    // this is not a break: nothing about the source changed, the baseline is
    // simply not evidence for this target. Reporting it as breaking would send
    // a reader looking for a change that is not there.
    if baseline.interface.target != candidate.interface.target {
        return AbiCompatibility::TargetMismatch;
    }
    for existing in &baseline.interface.routines {
        let Some(updated) = candidate
            .interface
            .routines
            .iter()
            .find(|routine| routine.symbol == existing.symbol)
        else {
            return AbiCompatibility::Breaking;
        };
        if render_routine_with_types(existing, &baseline.interface.types)
            != render_routine_with_types(updated, &candidate.interface.types)
        {
            return AbiCompatibility::Breaking;
        }
    }
    AbiCompatibility::MinorCompatible
}

/// A routine rendered with its parameter types expanded.
///
/// Comparing type *ids* would be wrong: an id is a position in a table, and
/// inserting an unrelated type renumbers it without changing any ABI fact.
fn render_routine_with_types(routine: &ForeignRoutine, table: &AbiTypeTable) -> String {
    let mut rendered = String::new();
    rendered.push_str(&routine.symbol);
    rendered.push('|');
    rendered.push_str(routine.convention.as_str());
    rendered.push('|');
    for parameter in &routine.parameters {
        rendered.push_str(parameter.direction.as_str());
        rendered.push(':');
        rendered.push_str(&expand_type(parameter.type_id, table));
        rendered.push(',');
    }
    rendered.push('|');
    rendered.push_str(&expand_type(routine.result, table));
    rendered.push('|');
    match &routine.error {
        AbiErrorContract::Infallible => rendered.push_str("infallible"),
        AbiErrorContract::Recoverable { error_type } => {
            rendered.push_str(&expand_type(*error_type, table));
        }
    }
    rendered
}

/// A type rendered structurally, independent of its id.
fn expand_type(id: AbiTypeId, table: &AbiTypeTable) -> String {
    let Some(ty) = table.get(id) else {
        return "?".to_string();
    };
    match ty {
        AbiType::Scalar(scalar) => scalar.c_type(),
        AbiType::Void => "void".to_string(),
        AbiType::Pointer {
            target,
            mutability,
            nullability,
            ownership,
            escape,
            destructor,
        } => format!(
            "ptr({},{mutability:?},{nullability:?},{ownership:?},{escape:?},{})",
            expand_type(*target, table),
            destructor.as_deref().unwrap_or("-")
        ),
        AbiType::Record { name, fields } => format!(
            "record {name}({})",
            fields
                .iter()
                .map(|field| format!("{}:{}", field.name, expand_type(field.type_id, table)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        AbiType::Entry {
            name,
            tag,
            variants,
        } => format!(
            "entry {name}<{}>({})",
            tag.as_str(),
            variants
                .iter()
                .map(|variant| format!(
                    "{}={}:{}",
                    variant.name,
                    variant.discriminant,
                    variant
                        .payload
                        .map(|id| expand_type(id, table))
                        .unwrap_or_else(|| "-".to_string())
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        AbiType::BorrowedString => "str".to_string(),
        AbiType::BorrowedSlice {
            element,
            mutability,
        } => {
            format!("slice({},{mutability:?})", expand_type(*element, table))
        }
        AbiType::OpaqueHandle { name } => format!("handle {name}"),
        AbiType::Callback { parameters, result } => format!(
            "callback({}) -> {}",
            parameters
                .iter()
                .map(|id| expand_type(*id, table))
                .collect::<Vec<_>>()
                .join(","),
            expand_type(*result, table)
        ),
    }
}
