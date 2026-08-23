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
    format!(
        "{{\"convention\":{},\"error\":{error},\"facing\":{},\"fol_path\":{},\
         \"parameters\":[{parameters}],\"result\":{},\"symbol\":{}}}",
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

/// How two interfaces relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiCompatibility {
    /// Byte-identical public facts.
    Identical,
    /// Only disjoint additions; existing symbols unchanged.
    MinorCompatible,
    /// A public symbol, type, layout, ownership, or error rule changed.
    Breaking,
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
    // Cross-target manifests are never comparable as if layout-compatible.
    if baseline.interface.target != candidate.interface.target {
        return AbiCompatibility::Breaking;
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
    }
}
