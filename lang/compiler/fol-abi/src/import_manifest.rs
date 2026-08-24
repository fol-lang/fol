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
//! The manifest is *checked*, never trusted, at two costs. `stale_input`
//! compares the recorded input digests against the files on disk, which is
//! cheap enough for every compile and catches the case that happens: the
//! header was edited and nobody re-bound. `verify_against` compares a whole
//! freshly produced manifest, which needs the C pipeline.

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
    /// Content digest of the entry header.
    ///
    /// A path alone cannot tell a reader whether the header still says what it
    /// said when the manifest was written, so a changed header would go on
    /// looking current.
    pub header_digest: String,
    /// The provider artifact, package-relative.
    pub provider: String,
    /// Content digest of the provider artifact.
    ///
    /// Without it a swapped archive is invisible: the manifest would still
    /// name the same path and still verify against itself, and an archive can
    /// carry the same symbol names with different code behind them.
    ///
    /// This assumes the provider is built reproducibly. GNU `ar` has defaulted
    /// to deterministic mode -- zeroed member timestamps, uid, and gid -- for
    /// years, so rebuilding the same source produces the same bytes. A toolchain
    /// that stamps its archives makes this report staleness after every rebuild;
    /// the diagnostic names the command that fixes it rather than failing
    /// obscurely.
    pub provider_digest: String,
    /// `object`, `static`, or `shared`.
    pub provider_kind: String,
    /// The annotation overlay, package-relative, when there is one.
    pub annotations: Option<String>,
    /// Content digest of the annotation overlay, when there is one.
    pub annotations_digest: Option<String>,
    /// The C compiler that measured the layouts.
    pub compiler: String,
    /// The target triple the layouts were measured for.
    pub target: String,
    /// The C standard the header was read as.
    pub dialect: String,
    /// Quoted-include roots, in declaration order.
    pub include_roots: Vec<String>,
    /// Angled-include roots, in declaration order.
    pub system_include_roots: Vec<String>,
    /// `NAME` or `NAME=VALUE`, in declaration order.
    pub defines: Vec<String>,
    /// The external sysroot the header was read against, when there is one.
    pub sysroot: Option<String>,
    /// Pinned sibling revisions, as `parc=<rev>` style entries in order.
    pub components: Vec<String>,
}

/// A recorded input whose bytes no longer match the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleImportInput {
    pub alias: String,
    /// What changed, for the message: `header` or `annotation overlay`.
    pub input: &'static str,
    /// The package-relative path the manifest recorded.
    pub path: String,
}

impl std::fmt::Display for StaleImportInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the {} '{}' has changed since C import '{}' was bound",
            self.input, self.path, self.alias
        )
    }
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
        let list = |values: &[String]| {
            values
                .iter()
                .map(|value| quoted(value))
                .collect::<Vec<_>>()
                .join(",")
        };
        let optional = |value: &Option<String>| match value {
            Some(value) => quoted(value),
            None => "null".to_string(),
        };
        format!(
            "{{\"interface\":{interface},\"interface_fingerprint\":{},\
             \"provenance\":{{\"annotations\":{},\"annotations_digest\":{},\
             \"compiler\":{},\"components\":[{components}],\"defines\":[{}],\
             \"dialect\":{},\"header\":{},\"header_digest\":{},\
             \"include_roots\":[{}],\"provider\":{},\"provider_digest\":{},\"provider_kind\":{},\
             \"system_include_roots\":[{}],\"sysroot\":{},\"target\":{}}},\
             \"provenance_fingerprint\":{}}}",
            quoted(&self.interface_fingerprint()),
            optional(&self.provenance.annotations),
            optional(&self.provenance.annotations_digest),
            quoted(&self.provenance.compiler),
            list(&self.provenance.defines),
            quoted(&self.provenance.dialect),
            quoted(&self.provenance.header),
            quoted(&self.provenance.header_digest),
            list(&self.provenance.include_roots),
            quoted(&self.provenance.provider),
            quoted(&self.provenance.provider_digest),
            quoted(&self.provenance.provider_kind),
            list(&self.provenance.system_include_roots),
            optional(&self.provenance.sysroot),
            quoted(&self.provenance.target),
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
            self.provenance.header_digest.as_str(),
            self.provenance.provider.as_str(),
            self.provenance.provider_digest.as_str(),
            self.provenance.provider_kind.as_str(),
            self.provenance.annotations.as_deref().unwrap_or(""),
            self.provenance.annotations_digest.as_deref().unwrap_or(""),
            self.provenance.compiler.as_str(),
            self.provenance.target.as_str(),
            self.provenance.dialect.as_str(),
            self.provenance.sysroot.as_deref().unwrap_or(""),
        ] {
            rendered.push_str(part);
            rendered.push('\n');
        }
        // Order is significant for all three: an include root that comes first
        // shadows a later one, and a define can be redefined.
        for group in [
            &self.provenance.include_roots,
            &self.provenance.system_include_roots,
            &self.provenance.defines,
            &self.provenance.components,
        ] {
            for entry in group {
                rendered.push_str(entry);
                rendered.push('\n');
            }
            rendered.push('\u{1e}');
        }
        digest(rendered.as_bytes())
    }

    /// Read one back.
    pub fn parse(text: &str) -> Result<Self, ImportManifestError> {
        let document = JsonValue::parse(text)?;
        let interface = read_interface(document.field("interface")?)?;
        let provenance_value = document.field("provenance")?;
        let optional = |field: &'static str| -> Result<Option<String>, ImportManifestError> {
            match provenance_value.field(field)? {
                JsonValue::Null => Ok(None),
                other => Ok(Some(
                    other
                        .as_str()
                        .ok_or(ImportManifestError::Json(JsonError::WrongType {
                            field: field.to_string(),
                            expected: "string or null",
                        }))?
                        .to_string(),
                )),
            }
        };
        let list = |field: &'static str| -> Result<Vec<String>, ImportManifestError> {
            provenance_value
                .array_field(field)?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or(ImportManifestError::Json(JsonError::WrongType {
                            field: field.to_string(),
                            expected: "string",
                        }))
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let provenance = ImportProvenance {
            header: provenance_value.string_field("header")?.to_string(),
            header_digest: provenance_value.string_field("header_digest")?.to_string(),
            provider: provenance_value.string_field("provider")?.to_string(),
            provider_digest: provenance_value
                .string_field("provider_digest")?
                .to_string(),
            provider_kind: provenance_value.string_field("provider_kind")?.to_string(),
            annotations: optional("annotations")?,
            annotations_digest: optional("annotations_digest")?,
            compiler: provenance_value.string_field("compiler")?.to_string(),
            target: provenance_value.string_field("target")?.to_string(),
            dialect: provenance_value.string_field("dialect")?.to_string(),
            include_roots: list("include_roots")?,
            system_include_roots: list("system_include_roots")?,
            defines: list("defines")?,
            sysroot: optional("sysroot")?,
            components: list("components")?,
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

    /// Which recorded input no longer matches what is on disk.
    ///
    /// Comparing digests costs two file reads, so ordinary compilation can
    /// afford it on every build; re-running the C pipeline to compare
    /// interfaces cannot. This catches the case that actually happens -- the
    /// header was edited and the manifest was not regenerated -- before the
    /// stale interface is used to compile a caller.
    pub fn stale_input(
        &self,
        header_digest: &str,
        annotations_digest: Option<&str>,
        provider_digest: &str,
    ) -> Option<StaleImportInput> {
        if self.provenance.header_digest != header_digest {
            return Some(StaleImportInput {
                alias: self.interface.alias.clone(),
                input: "header",
                path: self.provenance.header.clone(),
            });
        }
        if self.provenance.provider_digest != provider_digest {
            return Some(StaleImportInput {
                alias: self.interface.alias.clone(),
                input: "provider",
                path: self.provenance.provider.clone(),
            });
        }
        if self.provenance.annotations_digest.as_deref() != annotations_digest {
            return Some(StaleImportInput {
                alias: self.interface.alias.clone(),
                input: "annotation overlay",
                path: self
                    .provenance
                    .annotations
                    .clone()
                    .unwrap_or_else(|| "<none>".to_string()),
            });
        }
        None
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
    // The handle fact is part of the interface, not the provenance: changing a
    // routine from borrowing a handle to consuming it changes who owes the
    // release, which every caller has to be recompiled against.
    let handle = match &routine.handle {
        Some(use_) => format!(
            "{{\"domain\":{},\"role\":{}}}",
            quoted(&use_.domain),
            quoted(use_.role.as_str())
        ),
        None => "null".to_string(),
    };
    let callback = match &routine.callback {
        Some(use_) => format!(
            "{{\"context\":{},\"parameter\":{}}}",
            quoted(&use_.context),
            quoted(&use_.parameter)
        ),
        None => "null".to_string(),
    };
    format!(
        "{{\"callback\":{callback},\"convention\":{},\
         \"effects\":{{\"allocates\":{},\"hosted\":{}}},\
         \"error\":{},\"fol_name\":{},\"handle\":{handle},\
         \"origin\":{{\"column\":{},\"file\":{},\"line\":{}}},\
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
        AbiType::Scalar(scalar) => format!(
            "\"kind\":\"scalar\",\"scalar\":{}",
            quoted(&scalar_name(*scalar))
        ),
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
        // An opaque handle is a name and nothing else. That is the whole point:
        // the domain is the identity, and a consumer may only pass it back.
        AbiType::OpaqueHandle { name } => {
            format!("\"kind\":\"opaque-handle\",\"name\":{}", quoted(name))
        }
        AbiType::Callback { parameters, result } => {
            let rendered = parameters
                .iter()
                .map(|id| id.0.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "\"kind\":\"callback\",\"parameters\":[{rendered}],\"result\":{}",
                result.0
            )
        }
        // M6 imports scalars, void, and the pointers that carry an out-value.
        // The remaining aggregate shapes are not read back, and writing a
        // placeholder for one would put a type in the manifest nothing can
        // parse.
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
            target: AbiTypeId(
                usize::try_from(entry.integer_field("target")?).map_err(|_| {
                    ImportManifestError::Json(JsonError::WrongType {
                        field: "target".to_string(),
                        expected: "a type index",
                    })
                })?,
            ),
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
        "opaque-handle" => AbiType::OpaqueHandle {
            name: entry.string_field("name")?.to_string(),
        },
        "callback" => {
            let mut parameters = Vec::new();
            for parameter in entry.array_field("parameters")? {
                let index =
                    parameter
                        .as_i64()
                        .ok_or(ImportManifestError::Json(JsonError::WrongType {
                            field: "parameters".to_string(),
                            expected: "a type index",
                        }))?;
                parameters.push(callback_type_index(index)?);
            }
            AbiType::Callback {
                parameters,
                result: callback_type_index(entry.integer_field("result")?)?,
            }
        }
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
        handle: read_handle(entry.field("handle")?)?,
        callback: read_callback(entry.field("callback")?)?,
        origin: AbiSourceOrigin {
            file: origin_value.string_field("file")?.to_string(),
            line: u32::try_from(origin_value.integer_field("line")?).unwrap_or(0),
            column: u32::try_from(origin_value.integer_field("column")?).unwrap_or(0),
        },
        parameters,
        symbol,
    })
}

/// A callback's parameter and result ids, which are positions in the same
/// table and are checked against it by the caller's `read_type_id`.
fn callback_type_index(raw: i64) -> Result<AbiTypeId, ImportManifestError> {
    usize::try_from(raw).map(AbiTypeId).map_err(|_| {
        ImportManifestError::Json(JsonError::WrongType {
            field: "type".to_string(),
            expected: "a type index",
        })
    })
}

fn read_callback(
    value: &JsonValue,
) -> Result<Option<crate::annotation::CallbackUse>, ImportManifestError> {
    if matches!(value, JsonValue::Null) {
        return Ok(None);
    }
    Ok(Some(crate::annotation::CallbackUse {
        parameter: value.string_field("parameter")?.to_string(),
        context: value.string_field("context")?.to_string(),
    }))
}

fn read_handle(
    value: &JsonValue,
) -> Result<Option<crate::annotation::HandleUse>, ImportManifestError> {
    if matches!(value, JsonValue::Null) {
        return Ok(None);
    }
    let role = value.string_field("role")?;
    let role = match role {
        "produces" => crate::annotation::HandleRole::Produces,
        "borrows" => crate::annotation::HandleRole::Borrows,
        "consumes" => crate::annotation::HandleRole::Consumes,
        other => {
            return Err(ImportManifestError::UnknownHandleRole {
                role: other.to_string(),
            })
        }
    };
    Ok(Some(crate::annotation::HandleUse {
        domain: value.string_field("domain")?.to_string(),
        role,
    }))
}

fn read_type_id(
    raw: i64,
    types: &AbiTypeTable,
    symbol: &str,
) -> Result<AbiTypeId, ImportManifestError> {
    let index = usize::try_from(raw)
        .ok()
        .filter(|index| *index < types.len());
    index
        .map(AbiTypeId)
        .ok_or(ImportManifestError::DanglingTypeId {
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
            item.as_i64()
                .ok_or(ImportManifestError::Json(JsonError::WrongType {
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
    UnsupportedTypeKind {
        kind: String,
    },
    UnknownConvention {
        convention: String,
    },
    UnknownErrorKind {
        kind: String,
    },
    UnknownHandleRole {
        role: String,
    },
    TypeTableOutOfOrder {
        expected: usize,
        found: i64,
    },
    DanglingTypeId {
        symbol: String,
        id: i64,
    },
    FingerprintMismatch {
        field: &'static str,
        recorded: String,
        actual: String,
    },
    AliasMismatch {
        recorded: String,
        actual: String,
    },
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
            Self::UnknownErrorKind { kind } => {
                write!(f, "the import manifest names error convention '{kind}'")
            }
            Self::UnknownHandleRole { role } => {
                write!(f, "the import manifest names handle role '{role}'")
            }
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
                        handle: None,
                        callback: None,
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
                        handle: None,
                        callback: None,
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
                header_digest: digest(b"int c_math_add_one(int);\n"),
                provider: "native/libc_math.a".to_string(),
                provider_digest: digest(b"!<arch>\n"),
                provider_kind: "static".to_string(),
                annotations: Some("interop/c_math.toml".to_string()),
                annotations_digest: Some(digest(b"[routine.c_math_add_one]\n")),
                compiler: "/usr/bin/gcc".to_string(),
                target: "x86_64-unknown-linux-gnu".to_string(),
                dialect: "c17".to_string(),
                include_roots: vec!["native".to_string()],
                system_include_roots: Vec::new(),
                defines: vec!["NDEBUG".to_string(), "WIDTH=64".to_string()],
                sysroot: None,
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

    /// A handle domain survives the round trip and is part of the interface.
    ///
    /// Being *in the interface* rather than the provenance is the point: a
    /// routine that changes from borrowing a handle to consuming it changes who
    /// owes the release, so every caller must be recompiled against it. That
    /// only happens if the change moves the interface fingerprint.
    #[test]
    fn a_handle_domain_round_trips_and_moves_the_interface_fingerprint() {
        let mut manifest = scalar_manifest();
        let handle = manifest.interface.types.intern(AbiType::OpaqueHandle {
            name: "Widget".to_string(),
        });
        manifest.interface.routines[0].result = handle;
        manifest.interface.routines[0].handle = Some(crate::annotation::HandleUse {
            domain: "Widget".to_string(),
            role: crate::annotation::HandleRole::Produces,
        });

        let parsed =
            ImportManifest::parse(&manifest.canonical_json()).expect("manifest should read back");
        let routine = parsed
            .interface
            .routine("add_one")
            .expect("the producer should be present");
        assert_eq!(
            routine.handle,
            Some(crate::annotation::HandleUse {
                domain: "Widget".to_string(),
                role: crate::annotation::HandleRole::Produces,
            })
        );

        let mut borrowing = manifest.clone();
        borrowing.interface.routines[0].handle = Some(crate::annotation::HandleUse {
            domain: "Widget".to_string(),
            role: crate::annotation::HandleRole::Borrows,
        });
        assert_ne!(
            manifest.interface_fingerprint(),
            borrowing.interface_fingerprint(),
            "changing who owes the release must move the interface fingerprint"
        );
    }
}
