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
        Some(AbiType::BorrowedString) => STR_VIEW_STRUCT.to_string(),
        // C holds it as an address and nothing else, so the wrapper's ABI
        // spelling is an address and nothing else. What it points at is FOL's
        // own value, boxed by the producing wrapper.
        Some(AbiType::OpaqueHandle { .. }) => "*mut core::ffi::c_void".to_string(),
        _ => "()".to_string(),
    }
}

/// The `repr(C)` twin of `fol_str_view_t`.
pub const STR_VIEW_STRUCT: &str = "FolAbiStrView";

/// The definition of the borrowed-string view, emitted once per surface.
fn render_str_view_struct() -> String {
    format!(
        "/// The C representation of borrowed UTF-8 text.\n\
         ///\n\
         /// The caller owns the bytes for the duration of the call. Nothing here\n\
         /// keeps the pointer: each wrapper validates it and copies into FOL's own\n\
         /// owning string before doing anything else.\n\
         #[repr(C)]\n#[derive(Clone, Copy)]\npub struct {STR_VIEW_STRUCT} {{\n    \
         pub ptr: *const u8,\n    pub len: usize,\n}}\n\n"
    )
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
    if table
        .iter()
        .any(|(_, ty)| matches!(ty, AbiType::BorrowedString))
    {
        out.push_str(&render_str_view_struct());
    }
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

/// Wrap a C function pointer as the FOL closure the routine expects.
///
/// The closure captures the pointer and the context and calls straight through,
/// so FOL invokes C exactly as it would invoke any other routine value. It is
/// valid only for the duration of this call, which is the same contract the
/// import direction states -- and here FOL is the one honouring it, because
/// nothing keeps the closure past the call.
///
/// A null pointer is refused before anything captures it: calling through one
/// is undefined, and a caller passing null is making a mistake C cannot stop.
fn callback_inbound_conversion(
    table: &AbiTypeTable,
    name: &str,
    parameters: &[AbiTypeId],
    result: AbiTypeId,
) -> String {
    let arguments: Vec<String> = (0..parameters.len())
        .map(|index| format!("__fol_a{index}"))
        .collect();
    let typed: Vec<String> = parameters
        .iter()
        .enumerate()
        .map(|(index, id)| format!("__fol_a{index}: {}", rust_repr(table, *id)))
        .collect();
    let returns = match table.get(result) {
        Some(AbiType::Void) => String::new(),
        _ => format!(" -> {}", rust_repr(table, result)),
    };
    let forwarded = std::iter::once(format!("{name}_context"))
        .chain(arguments.iter().cloned())
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "    let Some({name}) = {name} else {{\n\
         \x20       return {invalid};\n\
         \x20   }};\n\
         \x20   let {name}_context = {name}_context;\n\
         \x20   let {name} = std::rc::Rc::new(move |{}|{returns} {{\n\
         \x20       // SAFETY: the pointer is non-null, and the context is the\n\
         \x20       // one the caller paired with it. Both are valid for this\n\
         \x20       // call, which is the only span the closure exists for.\n\
         \x20       unsafe {{ {name}({forwarded}) }}\n\
         \x20   }});\n",
        typed.join(", "),
        invalid = super::status::INVALID_ARGUMENT
    )
}

/// Turn an incoming address back into the FOL value behind it.
///
/// A borrower gets a reference and must not release anything; a consumer takes
/// the box back, which is what makes the release happen exactly once. Either
/// way the address is checked first: a null handle is a caller error C cannot
/// be stopped from making, and reading through it would be undefined rather
/// than wrong.
fn handle_inbound_conversion(
    domain: &str,
    name: &str,
    role: fol_abi::HandleRole,
    record_paths: &std::collections::BTreeMap<String, String>,
    symbol: &str,
) -> String {
    let Some(path) = record_paths.get(domain) else {
        return String::new();
    };
    let _ = symbol;
    let mut out = format!(
        "    if {name}.is_null() {{\n\
         \x20       return {};\n\
         \x20   }}\n",
        super::status::INVALID_ARGUMENT
    );
    match role {
        fol_abi::HandleRole::Consumes => {
            // The box comes back here and is dropped when the FOL routine that
            // receives it finishes with it. That is the release.
            out.push_str(&format!(
                "    // SAFETY: the address came from this library's own producing\n\
                 \x20   // wrapper, which handed out a `Box` of exactly this type.\n\
                 \x20   let {name} = *unsafe {{ Box::from_raw({name} as *mut {path}) }};\n"
            ));
        }
        _ => {
            out.push_str(&format!(
                "    // SAFETY: as above, and the reference does not outlive this\n\
                 \x20   // call, so the caller still owns the box afterwards.\n\
                 \x20   let {name} = unsafe {{ &*({name} as *const {path}) }};\n"
            ));
        }
    }
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
    // Borrowed text is the most dangerous thing a C caller can hand over, so
    // every way it can be wrong is checked before the bytes are touched:
    //
    //   - a null pointer with a non-zero length is a lie about the buffer
    //   - a length past `isize::MAX` cannot be a valid slice, and constructing
    //     one is undefined behaviour rather than a big slice
    //   - bytes that are not UTF-8 are not a FOL `str`
    //
    // A null pointer with length zero is *accepted* as the empty string: C
    // code routinely represents "no text" that way, and refusing it would make
    // the empty string unspellable from C.
    if matches!(table.get(id), Some(AbiType::BorrowedString)) {
        return format!(
            "    if {name}.ptr.is_null() && {name}.len != 0 {{\n        return {invalid};\n    }}\n\
             \x20   if {name}.len > isize::MAX as usize {{\n        return {invalid};\n    }}\n\
             \x20   let {name} = if {name}.len == 0 {{\n        \
             rt_model::FolStr::new(\"\")\n    }} else {{\n        \
             // Safe by the two checks above: non-null, and a length that fits\n        \
             // an `isize`. The slice lives only until the copy below.\n        \
             let __fol_bytes = unsafe {{ core::slice::from_raw_parts({name}.ptr, {name}.len) }};\n        \
             match core::str::from_utf8(__fol_bytes) {{\n            \
             Ok(__fol_text) => rt_model::FolStr::new(__fol_text),\n            \
             Err(_) => return {invalid},\n        }}\n    }};\n",
            invalid = super::status::INVALID_ARGUMENT
        );
    }

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
    // The value moves to the heap and the caller receives its address. Nothing
    // frees it here: that is the paired destroy routine's job, which is why a
    // producing export cannot be declared without naming one.
    if matches!(table.get(id), Some(AbiType::OpaqueHandle { .. })) {
        return format!("Box::into_raw(Box::new({expr})) as *mut core::ffi::c_void");
    }
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
        .flat_map(|parameter| match table.get(parameter.type_id) {
            // Two ABI slots, matching the header: the function pointer and the
            // context it is handed back. `Option<fn>` rather than a bare `fn`
            // so a null pointer is a value the wrapper can test rather than
            // undefined behaviour on the first call.
            Some(AbiType::Callback {
                parameters, result, ..
            }) => {
                let arguments = std::iter::once("*mut core::ffi::c_void".to_string())
                    .chain(parameters.iter().map(|id| rust_repr(table, *id)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let returns = match table.get(*result) {
                    Some(AbiType::Void) => String::new(),
                    _ => format!(" -> {}", rust_repr(table, *result)),
                };
                vec![
                    format!(
                        "{}: Option<unsafe extern \"C\" fn({arguments}){returns}>",
                        parameter.name
                    ),
                    format!("{}_context: *mut core::ffi::c_void", parameter.name),
                ]
            }
            _ => vec![format!(
                "{}: {}",
                parameter.name,
                rust_repr(table, parameter.type_id)
            )],
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
        if let Some(AbiType::Callback {
            parameters, result, ..
        }) = table.get(parameter.type_id)
        {
            out.push_str(&callback_inbound_conversion(
                table,
                &parameter.name,
                parameters,
                *result,
            ));
            continue;
        }
        if let Some(AbiType::OpaqueHandle { name }) = table.get(parameter.type_id) {
            out.push_str(&handle_inbound_conversion(
                name,
                &parameter.name,
                routine
                    .handle
                    .as_ref()
                    .map(|use_| use_.role)
                    .unwrap_or(fol_abi::HandleRole::Borrows),
                record_paths,
                &routine.symbol,
            ));
            continue;
        }
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
