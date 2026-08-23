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
        _ => "()".to_string(),
    }
}

/// Validation and conversion from the C representation to the FOL value.
///
/// Returns `None` when no validation is needed. A boolean and a character are
/// the two scalars where a C caller can hand over a bit pattern FOL has no
/// value for, so those are checked rather than transmuted.
fn inbound_conversion(table: &AbiTypeTable, id: AbiTypeId, name: &str) -> String {
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
fn outbound_conversion(table: &AbiTypeTable, id: AbiTypeId, expr: &str) -> String {
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
pub fn render_wrapper(
    table: &AbiTypeTable,
    routine: &ForeignRoutine,
    internal_path: &str,
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
                    outbound_conversion(table, routine.result, "__fol_value"),
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
                    outbound_conversion(table, routine.result, "__fol_ok")
                )
            } else {
                String::new()
            };
            out.push_str(&format!(
                "        Ok(__fol_recover) => match rt::abi::split_recoverable(__fol_recover) {{\n\
                 \x20           Ok(__fol_ok) => {{\n{write_result}                    {}\n                }}\n\
                 \x20           Err(__fol_error) => {{\n                    unsafe {{ *out_error = {}; }}\n                    {}\n                }}\n        }},\n",
                super::status::OK,
                outbound_conversion(table, *error_type, "__fol_error"),
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
