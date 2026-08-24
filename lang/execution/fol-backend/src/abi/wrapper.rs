//! Generated `extern "C"` wrappers.
//!
//! Every exported routine gets a public wrapper with the exact allowlisted
//! symbol, and the FOL routine it calls stays private and ID-mangled. The
//! wrapper owns four things the FOL routine does not: the uniform status
//! return, validation of inbound scalar bit patterns, panic containment, and
//! the out-parameter initialization rules.

use fol_abi::{AbiErrorContract, AbiScalar, AbiType, AbiTypeId, AbiTypeTable, ForeignRoutine};

/// The Rust spelling of an ABI type, for a generated wrapper signature.
///
/// These are the C representation types, not FOL's runtime types: a wrapper
/// takes `u8` for a boolean because that is what crosses, and converts.
pub fn rust_repr(table: &AbiTypeTable, id: AbiTypeId) -> String {
    match table.get(id) {
        Some(AbiType::Scalar(AbiScalar::Int(width))) => width.rust_primitive().to_string(),
        Some(AbiType::Scalar(AbiScalar::Float(width))) => width.rust_primitive().to_string(),
        Some(AbiType::Scalar(AbiScalar::Bool)) => "u8".to_string(),
        Some(AbiType::Scalar(AbiScalar::Char)) => "u32".to_string(),
        Some(AbiType::Void) => "()".to_string(),
        Some(AbiType::Record { name, .. }) | Some(AbiType::Entry { name, .. }) => {
            c_repr_struct_name(name)
        }
        _ => "()".to_string(),
    }
}

/// The generated `#[repr(C)]` struct standing for one exported record.
///
/// A record crosses as a C struct, so the wrapper cannot take FOL's internal
/// record type: that one is `#[repr(Rust)]` and its layout is not guaranteed.
/// The wrapper takes this instead and converts field by field.
pub fn c_repr_struct_name(record: &str) -> String {
    format!("FolAbi{record}")
}

/// Render the `#[repr(C)]` definitions for every record in the surface.
///
/// Fields are emitted in the ABI table's order, which is the FOL declaration
/// order, because that is what decides every offset.
pub fn render_record_structs(table: &AbiTypeTable) -> String {
    let mut out = String::new();
    for (_, ty) in table.iter() {
        if let AbiType::Entry { name, variants, .. } = ty {
            out.push_str(&render_entry_struct(table, name, variants));
            continue;
        }
        let AbiType::Record { name, fields } = ty else {
            continue;
        };
        out.push_str(&format!(
            "/// The C representation of FOL's `{name}`.\n\
             ///\n\
             /// `repr(C)` because the field order and padding below are the ABI;\n\
             /// FOL's own record type is `repr(Rust)` and may be laid out any way.\n\
             #[repr(C)]\n#[derive(Clone, Copy)]\npub struct {} {{\n",
            c_repr_struct_name(name)
        ));
        for field in fields {
            out.push_str(&format!(
                "    pub {}: {},\n",
                crate::mangle::escape_rust_field_ident(&field.name),
                rust_repr(table, field.type_id)
            ));
        }
        out.push_str("}\n\n");
    }
    out
}

/// The union member holding one entry's payloads.
fn c_repr_union_name(entry: &str) -> String {
    format!("FolAbi{entry}Payload")
}

/// Render the `#[repr(C)]` twin of one entry: a tag beside a payload union.
///
/// The union is `repr(C)` and its members are `Copy`, so no Rust drop glue is
/// involved. Reading the wrong member is what the tag exists to prevent, and
/// the wrapper checks the tag before reading anything.
fn render_entry_struct(
    table: &AbiTypeTable,
    name: &str,
    variants: &[fol_abi::AbiVariant],
) -> String {
    let struct_name = c_repr_struct_name(name);
    let union_name = c_repr_union_name(name);
    let payloads: Vec<&fol_abi::AbiVariant> = variants
        .iter()
        .filter(|variant| variant.payload.is_some())
        .collect();

    let mut out = String::new();
    if payloads.is_empty() {
        out.push_str(&format!(
            "/// The C representation of FOL's `{name}`. Every variant is tag-only.\n\
             #[repr(C)]\n#[derive(Clone, Copy)]\npub struct {struct_name} {{\n    pub tag: i32,\n}}\n\n"
        ));
        return out;
    }

    out.push_str(&format!(
        "/// Payloads of FOL's `{name}`. Only the member the tag names is live.\n\
         #[repr(C)]\n#[derive(Clone, Copy)]\npub union {union_name} {{\n"
    ));
    for variant in &payloads {
        let payload = variant.payload.expect("filtered to payload variants");
        out.push_str(&format!(
            "    pub {}: {},\n",
            crate::mangle::escape_rust_field_ident(&variant.name),
            rust_repr(table, payload)
        ));
    }
    out.push_str("}\n\n");

    out.push_str(&format!(
        "/// The C representation of FOL's `{name}`.\n\
         #[repr(C)]\n#[derive(Clone, Copy)]\npub struct {struct_name} {{\n    \
         pub tag: i32,\n    pub payload: {union_name},\n}}\n\n"
    ));
    out
}

/// Validation and conversion from the C representation to the FOL value.
///
/// Returns `None` when no validation is needed. A boolean and a character are
/// the two scalars where a C caller can hand over a bit pattern FOL has no
/// value for, so those are checked rather than transmuted.
fn inbound_conversion(
    table: &AbiTypeTable,
    id: AbiTypeId,
    name: &str,
    record_paths: &std::collections::BTreeMap<String, String>,
) -> String {
    // A record arrives as its `repr(C)` twin and is rebuilt field by field
    // into FOL's own type. Transmuting instead would assume the two layouts
    // match, which is exactly what `repr(Rust)` does not promise.
    // An entry's tag is checked before any payload is read. C can hand over a
    // tag that names no variant, and reading the union for one would be
    // reading whatever bytes happen to be there.
    if let Some(AbiType::Entry {
        name: entry,
        variants,
        ..
    }) = table.get(id)
    {
        let Some(path) = record_paths.get(entry) else {
            return String::new();
        };
        let mut arms = String::new();
        for variant in variants {
            let ident = crate::mangle::escape_rust_field_ident(&variant.name);
            let construction = match variant.payload {
                // The union read is the one place `unsafe` is needed, and it
                // happens only on the arm whose tag was just matched.
                Some(_) => format!("{path}::{ident}(unsafe {{ {name}.payload.{ident} }})"),
                None => format!("{path}::{ident}"),
            };
            arms.push_str(&format!(
                "        {} => {construction},\n",
                variant.discriminant
            ));
        }
        return format!(
            "    let {name} = match {name}.tag {{\n{arms}        \
             // A tag naming no variant is not an entry value.\n        \
             _ => return {},\n    }};\n",
            super::status::INVALID_ARGUMENT
        );
    }

    if let Some(AbiType::Record {
        name: record,
        fields,
    }) = table.get(id)
    {
        let Some(path) = record_paths.get(record) else {
            return String::new();
        };
        let assignments = fields
            .iter()
            .map(|field| {
                let ident = crate::mangle::escape_rust_field_ident(&field.name);
                format!("{ident}: {name}.{ident}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        return format!("    let {name} = {path} {{ {assignments} }};\n");
    }
    match table.get(id) {
        Some(AbiType::Scalar(AbiScalar::Bool)) => format!(
            "    let {name} = match {name} {{\n        \
             0 => false,\n        \
             1 => true,\n        \
             // Any other bit pattern is not a boolean. C has no such type, so\n        \
             // this is the boundary that has to say no.\n        \
             _ => return {},\n    }};\n",
            super::status::INVALID_ARGUMENT
        ),
        Some(AbiType::Scalar(AbiScalar::Char)) => format!(
            "    let {name} = match char::from_u32({name}) {{\n        \
             Some(value) => value,\n        \
             // A surrogate or an out-of-range code point is not a Unicode\n        \
             // scalar value, which is what FOL's `chr` holds.\n        \
             None => return {},\n    }};\n",
            super::status::INVALID_ARGUMENT
        ),
        _ => String::new(),
    }
}

/// Conversion from the FOL value back to the C representation.
fn outbound_conversion(
    table: &AbiTypeTable,
    id: AbiTypeId,
    expr: &str,
    record_paths: &std::collections::BTreeMap<String, String>,
) -> String {
    if let Some(AbiType::Entry { name, variants, .. }) = table.get(id) {
        let Some(path) = record_paths.get(name) else {
            return expr.to_string();
        };
        let struct_name = c_repr_struct_name(name);
        let union_name = c_repr_union_name(name);
        let has_payload = variants.iter().any(|variant| variant.payload.is_some());
        let mut arms = String::new();
        for variant in variants {
            let ident = crate::mangle::escape_rust_field_ident(&variant.name);
            let tag = variant.discriminant;
            let arm = match (variant.payload.is_some(), has_payload) {
                (true, _) => format!(
                    "{path}::{ident}(__fol_payload) => {struct_name} {{ tag: {tag}, \
                     payload: {union_name} {{ {ident}: __fol_payload }} }}"
                ),
                // A tag-only variant of an entry that has payloads still needs
                // a union value; any member will do, and the tag says it is
                // not to be read.
                (false, true) => format!(
                    "{path}::{ident} => {struct_name} {{ tag: {tag}, \
                     payload: unsafe {{ core::mem::zeroed() }} }}"
                ),
                (false, false) => format!("{path}::{ident} => {struct_name} {{ tag: {tag} }}"),
            };
            arms.push_str(&format!("        {arm},\n"));
        }
        return format!("match {expr} {{\n{arms}    }}");
    }

    if let Some(AbiType::Record { name, fields }) = table.get(id) {
        let assignments = fields
            .iter()
            .map(|field| {
                let ident = crate::mangle::escape_rust_field_ident(&field.name);
                format!("{ident}: __fol_record.{ident}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            "{{ let __fol_record = {expr}; {} {{ {assignments} }} }}",
            c_repr_struct_name(name)
        );
    }
    match table.get(id) {
        Some(AbiType::Scalar(AbiScalar::Bool)) => format!("if {expr} {{ 1u8 }} else {{ 0u8 }}"),
        Some(AbiType::Scalar(AbiScalar::Char)) => format!("({expr}) as u32"),
        _ => expr.to_string(),
    }
}

/// Render one exported wrapper.
///
/// `internal_path` is the private Rust path of the FOL routine, which keeps its
/// ID-mangled name: section 4.10 requires a public symbol to carry no internal
/// ID, and the wrapper is what carries the public name.
///
/// `record_paths` maps an exported record's name to the private Rust path of
/// FOL's own type for it, which is what the wrapper converts to and from.
pub fn render_wrapper(
    table: &AbiTypeTable,
    routine: &ForeignRoutine,
    internal_path: &str,
    record_paths: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut out = String::new();

    let mut params: Vec<String> = routine
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                rust_repr(table, parameter.type_id)
            )
        })
        .collect();

    let has_result = !matches!(table.get(routine.result), Some(AbiType::Void));
    if has_result {
        params.push(format!(
            "out_result: *mut {}",
            rust_repr(table, routine.result)
        ));
    }
    if let AbiErrorContract::Recoverable { error_type } = &routine.error {
        params.push(format!("out_error: *mut {}", rust_repr(table, *error_type)));
    }

    // `#[unsafe(no_mangle)]` is the 2024 spelling; the generated crate pins its
    // edition, so this stays valid rather than depending on the caller's.
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str(&format!(
        "pub unsafe extern \"C\" fn {}({}) -> i32 {{\n",
        routine.symbol,
        params.join(", ")
    ));

    // A null required out pointer is a caller error, checked before any work
    // so a failing call has no side effects.
    if has_result {
        out.push_str(&format!(
            "    if out_result.is_null() {{\n        return {};\n    }}\n",
            super::status::INVALID_ARGUMENT
        ));
    }
    if matches!(routine.error, AbiErrorContract::Recoverable { .. }) {
        out.push_str(&format!(
            "    if out_error.is_null() {{\n        return {};\n    }}\n",
            super::status::INVALID_ARGUMENT
        ));
    }

    for parameter in &routine.parameters {
        out.push_str(&inbound_conversion(
            table,
            parameter.type_id,
            &parameter.name,
            record_paths,
        ));
    }

    let call_args: Vec<String> = routine
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect();

    // The call runs inside `catch_unwind`: a FOL panic must not unwind into C,
    // where unwinding across an `extern "C"` frame is undefined.
    out.push_str("    let __fol_outcome = std::panic::catch_unwind(move || {\n");
    out.push_str(&format!(
        "        {internal_path}({})\n",
        call_args.join(", ")
    ));
    out.push_str("    });\n");
    out.push_str("    match __fol_outcome {\n");

    match &routine.error {
        AbiErrorContract::Infallible => {
            if has_result {
                out.push_str(&format!(
                    "        Ok(__fol_value) => {{\n            unsafe {{ *out_result = {}; }}\n            {}\n        }}\n",
                    outbound_conversion(table, routine.result, "__fol_value", record_paths),
                    super::status::OK
                ));
            } else {
                out.push_str(&format!("        Ok(_) => {},\n", super::status::OK));
            }
        }
        AbiErrorContract::Recoverable { error_type } => {
            // On a report only the error out is written: section 4.7 requires
            // the success out to stay uninitialized, so a caller that reads it
            // anyway is reading its own memory rather than a value FOL claimed
            // to produce.
            let write_result = if has_result {
                format!(
                    "                    unsafe {{ *out_result = {}; }}\n",
                    outbound_conversion(table, routine.result, "__fol_ok", record_paths)
                )
            } else {
                String::new()
            };
            out.push_str(&format!(
                "        Ok(__fol_recover) => match rt::abi::split_recoverable(__fol_recover) {{\n\
                 \x20           Ok(__fol_ok) => {{\n{write_result}                    {}\n                }}\n\
                 \x20           Err(__fol_error) => {{\n                    unsafe {{ *out_error = {}; }}\n                    {}\n                }}\n        }},\n",
                super::status::OK,
                outbound_conversion(table, *error_type, "__fol_error", record_paths),
                super::status::REPORT
            ));
        }
    }

    out.push_str(&format!(
        "        Err(_) => {},\n    }}\n}}\n",
        super::status::PANIC
    ));
    out
}
