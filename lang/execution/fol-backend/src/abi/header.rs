//! C header generation.
//!
//! Generated from the same `ResolvedAbiSurface` the manifest is, so the two
//! cannot describe different things. The shape is frozen in section 4.16 and
//! checked against `examples/v4_contract_header/demo.h`.

use fol_abi::{AbiErrorContract, AbiType, AbiTypeId, AbiTypeTable, ResolvedAbiSurface};

/// The include guard for an artifact, per section 4.16.
///
/// `FOL_` prefixed and uppercased with non-alphanumerics replaced, so a short
/// artifact name cannot collide with a consumer's own guard.
pub fn include_guard(artifact: &str) -> String {
    let mut guard = String::from("FOL_");
    for ch in artifact.chars() {
        if ch.is_ascii_alphanumeric() {
            guard.push(ch.to_ascii_uppercase());
        } else {
            guard.push('_');
        }
    }
    guard.push_str("_H");
    guard
}

fn c_type(table: &AbiTypeTable, id: AbiTypeId) -> String {
    match table.get(id) {
        Some(AbiType::Scalar(scalar)) => scalar.c_type(),
        Some(AbiType::Void) => "void".to_string(),
        Some(AbiType::Record { name, .. }) | Some(AbiType::Entry { name, .. }) => {
            c_struct_name(name)
        }
        Some(AbiType::BorrowedString) => "fol_str_view_t".to_string(),
        // A pointer to an incomplete type: the consumer may hold it and hand
        // it back, and cannot read through it. The domain *is* the identity,
        // so two domains are two C types and the compiler keeps them apart.
        Some(AbiType::OpaqueHandle { name }) => format!("{} *", c_handle_name(name)),
        _ => "void".to_string(),
    }
}

/// One incomplete struct per exported handle domain.
///
/// Declared and never defined, which is the whole contract: a consumer can hold
/// the pointer and hand it back, and the C compiler refuses to let it read
/// through or copy what is behind it. Nothing here says how big it is, because
/// nothing outside this library is entitled to know.
fn render_handle_definitions(table: &fol_abi::AbiTypeTable) -> String {
    let mut domains: Vec<&str> = table
        .iter()
        .filter_map(|(_, ty)| match ty {
            AbiType::OpaqueHandle { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    domains.sort_unstable();
    domains.dedup();
    if domains.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "/* Opaque resources this library owns. Each is declared and never\n \
         * defined: hold the pointer, hand it back, and release it through the\n \
         * routine paired with it. */\n",
    );
    for domain in domains {
        let name = c_handle_name(domain);
        out.push_str(&format!("typedef struct {name} {name};\n"));
    }
    out.push('\n');
    out
}

/// The C spelling of an exported handle domain.
fn c_handle_name(domain: &str) -> String {
    format!("fol_{}_t", domain.to_lowercase())
}

/// The C spelling of an entry's tag enum.
fn c_tag_name(entry: &str) -> String {
    format!("fol_{}_tag_t", entry.to_lowercase())
}

/// The C spelling of one variant's tag constant.
fn c_tag_constant(entry: &str, variant: &str) -> String {
    format!("FOL_{}_{}", entry.to_uppercase(), variant.to_uppercase())
}

/// Render one entry as an explicitly tagged struct.
///
/// A bare C `union` cannot say which member is live, so the generated form is
/// a struct pairing a fixed-width tag with the payload union. The tag values
/// are the FOL discriminants, written out rather than left to C's implicit
/// numbering: section 12.2 requires that reordering the declaration cannot
/// renumber a tag, and only explicit values give that.
fn render_entry_definition(
    table: &AbiTypeTable,
    name: &str,
    variants: &[fol_abi::AbiVariant],
) -> String {
    let mut out = String::new();
    let tag_name = c_tag_name(name);
    let struct_name = c_struct_name(name);

    out.push_str(&format!(
        "/* FOL `{name}`. The tag values are the FOL discriminants; they are written\n \
         * out so reordering the declaration cannot renumber them. */\ntypedef enum {{\n"
    ));
    for variant in variants {
        out.push_str(&format!(
            "    {} = {},\n",
            c_tag_constant(name, &variant.name),
            variant.discriminant
        ));
    }
    out.push_str(&format!("}} {tag_name};\n\n"));

    let payloads: Vec<&fol_abi::AbiVariant> = variants
        .iter()
        .filter(|variant| variant.payload.is_some())
        .collect();

    out.push_str(&format!("typedef struct {{\n    {tag_name} tag;\n"));
    if payloads.is_empty() {
        // A tag-only entry gets no union member: an empty union is not C.
        out.push_str("    /* Every variant is tag-only, so there is no payload. */\n");
    } else {
        out.push_str("    union {\n");
        for variant in &payloads {
            let payload = variant.payload.expect("filtered to payload variants");
            out.push_str(&format!(
                "        {} {};\n",
                c_type(table, payload),
                variant.name
            ));
        }
        out.push_str("    } payload;\n");
    }
    out.push_str(&format!("}} {struct_name};\n\n"));
    out
}

/// The C spelling of an exported record.
///
/// Suffixed `_t` because the header emits a `typedef`, and prefixed with the
/// artifact-neutral `fol_` so a consumer including two FOL headers does not
/// collide on a common record name.
pub fn c_struct_name(record: &str) -> String {
    format!("fol_{}_t", record.to_lowercase())
}

/// Render the struct definitions and their layout assertions.
///
/// The `_Static_assert`s are the point: they make the C compiler check the
/// size, alignment, and every field offset against what FOL computed. A
/// disagreement is a compile error in the consumer's own translation unit,
/// which is the only place the question can be settled honestly.
fn render_record_definitions(table: &AbiTypeTable) -> String {
    let mut out = String::new();
    for (type_id, ty) in table.iter() {
        if let AbiType::Entry { name, variants, .. } = ty {
            out.push_str(&render_entry_definition(table, name, variants));
            continue;
        }
        let AbiType::Record { name, fields } = ty else {
            continue;
        };
        let struct_name = c_struct_name(name);
        out.push_str(&format!(
            "/* FOL `{name}`. Field order is the FOL declaration order and decides\n \
             * every offset; it is not sorted. */\ntypedef struct {{\n"
        ));
        for field in fields {
            out.push_str(&format!(
                "    {} {};\n",
                c_type(table, field.type_id),
                field.name
            ));
        }
        out.push_str(&format!("}} {struct_name};\n\n"));

        // FOL computed these from the System V rules; the C compiler recomputes
        // them from its own. If the two ever disagree the consumer fails to
        // compile, which is the only honest place to settle the question.
        let Ok(layout) = fol_abi::record_layout(table, type_id) else {
            continue;
        };
        out.push_str(&format!(
            "_Static_assert(sizeof({struct_name}) == {}, \"FOL and C disagree on sizeof({struct_name})\");\n",
            layout.size
        ));
        out.push_str(&format!(
            "_Static_assert(_Alignof({struct_name}) == {}, \"FOL and C disagree on _Alignof({struct_name})\");\n",
            layout.align
        ));
        for placement in &layout.fields {
            out.push_str(&format!(
                "_Static_assert(offsetof({struct_name}, {}) == {}, \"FOL and C disagree on {struct_name}.{}\");\n",
                placement.name, placement.offset, placement.name
            ));
        }
        out.push('\n');
    }
    out
}

/// Render the complete header for one surface.
pub fn render_header(surface: &ResolvedAbiSurface) -> String {
    let guard = include_guard(&surface.artifact);
    let table = &surface.interface.types;
    let mut out = String::new();

    out.push_str(&format!(
        "/* {}.h -- generated by FOL. Do not edit.\n *\n * ABI {}.{} for {}.\n */\n\n",
        surface.artifact,
        surface.major,
        surface.minor,
        surface.interface.target.rust_target_triple()
    ));
    out.push_str(&format!("#ifndef {guard}\n#define {guard}\n\n"));
    out.push_str("#include <stdint.h>\n#include <stddef.h>\n\n");
    out.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");

    out.push_str(
        "/* Every exported FOL routine returns this. Ordinary results travel through out\n \
         * parameters, so an infallible-looking function still has a panic and\n \
         * validation channel. */\n",
    );
    out.push_str("typedef int32_t fol_status_t;\n\n");
    out.push_str(
        "/* Only 0 and 1 are valid. Imports validate. */\ntypedef uint8_t fol_bool_t;\n\n",
    );
    out.push_str(
        "/* A Unicode scalar value. Imports validate. */\ntypedef uint32_t fol_char_t;\n\n",
    );
    out.push_str(
        "/* UTF-8 text the caller owns and lends for the duration of one call.\n \
         *\n \
         * The callee copies what it needs before returning and never retains `ptr`,\n \
         * so the caller may free or reuse the buffer as soon as the call returns.\n \
         * `ptr` may be NULL only when `len` is 0. The bytes must be valid UTF-8;\n \
         * they are validated on entry and the call is refused if they are not. */\n",
    );
    out.push_str(
        "typedef struct {\n    const uint8_t *ptr;\n    size_t len;\n} fol_str_view_t;\n\n",
    );

    for (name, value, description) in fol_abi::STATUS_VALUES {
        // Negative macros are parenthesized: `x == -1` and `x == (-1)` differ
        // once the macro is pasted into a larger expression.
        let rendered = if *value < 0 {
            format!("({value})")
        } else {
            value.to_string()
        };
        out.push_str(&format!("/* {description} */\n#define {name} {rendered}\n"));
    }
    out.push('\n');

    out.push_str(&render_handle_definitions(table));
    out.push_str(&render_record_definitions(table));

    out.push_str(
        "/* On any failure the success out values are left uninitialized. The caller\n \
         * must not read or free them. On FOL_STATUS_REPORT, and only then, the typed\n \
         * error out parameter is initialized. */\n\n",
    );

    // Sorted by symbol, matching the manifest's canonical ordering, so a
    // reordered source file does not produce a different header.
    let mut routines: Vec<_> = surface
        .interface
        .facing(fol_abi::AbiFacing::Export)
        .collect();
    routines.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    for routine in routines {
        let mut params: Vec<String> = routine
            .parameters
            .iter()
            .map(|parameter| format!("{} {}", c_type(table, parameter.type_id), parameter.name))
            .collect();
        if !matches!(table.get(routine.result), Some(AbiType::Void)) {
            params.push(format!("{} *out_result", c_type(table, routine.result)));
        }
        if let AbiErrorContract::Recoverable { error_type } = &routine.error {
            params.push(format!("{} *out_error", c_type(table, *error_type)));
        }
        if params.is_empty() {
            params.push("void".to_string());
        }
        out.push_str(&format!(
            "/* {} */\nfol_status_t {}({});\n\n",
            routine.fol_path,
            routine.symbol,
            params.join(", ")
        ));
    }

    out.push_str("#ifdef __cplusplus\n} /* extern \"C\" */\n#endif\n\n");
    out.push_str(&format!("#endif /* {guard} */\n"));
    out
}
