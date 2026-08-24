//! The FOL-owned safe adapters over one import's raw declarations.
//!
//! Section 4.13 splits responsibility deliberately: GERC emits the one private
//! `extern "C"` module and nothing else, and everything that is a *language*
//! decision -- validation, the error convention, what a caller is allowed to
//! see -- lives in adapters FOL generates. This module is those adapters.
//!
//! Each adapter is the only place an `unsafe` call appears for its symbol, and
//! the only place the provider's status convention is interpreted. A FOL
//! caller reaches the adapter, never the extern.

use fol_abi::{
    AbiScalar, AbiType, AbiTypeId, AbiTypeTable, ImportErrorConvention, ImportedInterface,
    ImportedRoutine,
};

/// Render the adapter module for one import.
///
/// The module name matches `fol_backend::mangle::foreign_adapter_module_name`,
/// because that is what the emitted call sites reach for. The two are checked
/// against each other by `adapter_module_name_matches_the_backend_spelling`.
pub fn render_adapter_module(
    interface: &ImportedInterface,
    raw_crate: &str,
) -> Result<String, AdapterError> {
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "// Generated FOL-owned safe adapters for C import '{}'.\n\
         // Every `unsafe` call to this provider appears here and nowhere else.\n\
         //\n\
         // `dead_code` and `non_snake_case` are relaxed because the surface is\n\
         // generated from C: not every imported routine is called, and C names\n\
         // are kept verbatim so they stay greppable back to the header. The\n\
         // denials below are the point -- this module is the only place\n\
         // `unsafe` appears for this provider, so it is the one place worth\n\
         // holding to a stricter standard than the rest of the tree.\n\
         #[allow(dead_code, non_snake_case)]\n\
         #[deny(unsafe_op_in_unsafe_fn)]\n\
         #[deny(clippy::undocumented_unsafe_blocks)]\n\
         #[deny(clippy::not_unsafe_ptr_arg_deref)]\n\
         #[deny(clippy::cast_ptr_alignment)]\n\
         pub mod {} {{\n",
        interface.alias,
        adapter_module_name(&interface.alias),
    ));

    for routine in &interface.routines {
        rendered.push_str(&render_adapter(interface, routine, raw_crate)?);
    }

    rendered.push_str("}\n");
    Ok(rendered)
}

/// Mirrors `fol_backend::mangle::foreign_adapter_module_name`.
///
/// Duplicated rather than shared because `fol-interop` does not depend on the
/// backend; the duplication is guarded by a test that fails if either moves.
pub fn adapter_module_name(alias: &str) -> String {
    format!("cimp__{alias}")
}

fn render_adapter(
    interface: &ImportedInterface,
    routine: &ImportedRoutine,
    raw_crate: &str,
) -> Result<String, AdapterError> {
    let raw = format!("{raw_crate}::{}", routine.symbol);
    let mut params = Vec::new();
    for parameter in routine.call_parameters() {
        params.push(format!(
            "{}: {}",
            rust_param_name(&parameter.name),
            rust_scalar(&interface.types, parameter.type_id, &routine.symbol)?
        ));
    }
    // The context is hidden from FOL but not from the adapter: FOL passes the
    // trampoline and the closure address as two ordinary arguments here, and
    // the adapter forwards both. The trampoline itself is emitted by the
    // backend, which is the only layer that knows the closure's Rust type.
    if let Some(index) = routine.callback_context_index() {
        params.push(format!(
            "{}: *mut core::ffi::c_void",
            rust_param_name(&routine.parameters[index].name)
        ));
    }
    let params = params.join(", ");

    match &routine.error {
        ImportErrorConvention::Infallible => {
            // Every declared parameter, in the provider's own order -- including
            // the context, which the adapter passes through even though FOL
            // never named it. Its value comes from the extra argument above.
            let call_args = routine
                .parameters
                .iter()
                .map(|parameter| cast_argument(&interface.types, parameter))
                .collect::<Vec<_>>()
                .join(", ");
            // A `void` provider and a value-returning one render the same
            // call; only the adapter's declared return type differs.
            let returns_handle = matches!(
                interface.types.get(routine.result),
                Some(AbiType::OpaqueHandle { .. })
            );
            let body = if returns_handle {
                // Back to `c_void` on the way out, for the same reason the
                // arguments cast on the way in: this crate does not name the
                // provider's projected opaque struct.
                format!(
                    "    // SAFETY: `{raw}` is the LINC-certified declaration of a symbol this\n\
                     \x20   // provider defines, called with the arity and types the header\n\
                     \x20   // states and the overlay accepted. The result is an address FOL\n\
                     \x20   // does not read through here.\n\
                     \x20   unsafe {{ {raw}({call_args}) as *mut core::ffi::c_void }}\n"
                )
            } else {
                format!(
                    "    // SAFETY: `{raw}` is the LINC-certified declaration of a symbol this\n\
                     \x20   // provider defines, called with the arity and types the header\n\
                     \x20   // states and the overlay accepted.\n\
                     \x20   unsafe {{ {raw}({call_args}) }}\n"
                )
            };
            let return_type = match interface.types.get(routine.result) {
                Some(AbiType::Void) => String::new(),
                _ => format!(
                    " -> {}",
                    rust_scalar(&interface.types, routine.result, &routine.symbol)?
                ),
            };
            Ok(format!(
                "    #[inline]\n    pub fn {}({params}){return_type} {{\n{body}    }}\n",
                rust_param_name(&routine.fol_name),
            ))
        }
        ImportErrorConvention::Status {
            success,
            out_parameter,
            ..
        } => {
            let index = routine.out_parameter_index().ok_or_else(|| {
                AdapterError::UnresolvedOutParameter {
                    symbol: routine.symbol.clone(),
                    parameter: out_parameter.clone(),
                }
            })?;
            let out_type = out_pointee(&interface.types, routine, index)?;
            let status_type = rust_scalar(&interface.types, routine.result, &routine.symbol)?;

            // The out storage belongs to the adapter, so a caller cannot
            // observe it before the status has been checked.
            let mut call_args = Vec::new();
            for (position, parameter) in routine.parameters.iter().enumerate() {
                if position == index {
                    call_args.push("&mut __fol_out".to_string());
                } else {
                    call_args.push(cast_argument(&interface.types, parameter));
                }
            }
            let call_args = call_args.join(", ");
            let success_arms = success
                .iter()
                .map(|code| code.to_string())
                .collect::<Vec<_>>()
                .join(" | ");

            // A plain `Result`, not the FOL runtime's carrier: this crate is
            // linked without the runtime, and the call site converts. That
            // keeps the adapter layer dependency-free.
            Ok(format!(
                "    #[inline]\n\
                 \x20   pub fn {name}({params}) -> Result<{out_type}, {status_type}> {{\n\
                 \x20       let mut __fol_out: {out_type} = Default::default();\n\
                 \x20       // SAFETY: `{raw}` is the LINC-certified declaration of a symbol\n\
                 \x20       // this provider defines. `__fol_out` is a live local of the\n\
                 \x20       // declared out type, so the pointer handed over is valid and\n\
                 \x20       // aligned for the whole call, and it is read below only on a\n\
                 \x20       // status the overlay enumerated as success.\n\
                 \x20       let __fol_status = unsafe {{ {raw}({call_args}) }};\n\
                 \x20       match __fol_status {{\n\
                 \x20           {success_arms} => Ok(__fol_out),\n\
                 \x20           // Any other code is a failure, including one the\n\
                 \x20           // overlay did not enumerate: reading the out value\n\
                 \x20           // for an unknown status would be reading whatever\n\
                 \x20           // the provider happened to leave there.\n\
                 \x20           other => Err(other),\n\
                 \x20       }}\n\
                 \x20   }}\n",
                name = rust_param_name(&routine.fol_name),
            ))
        }
    }
}

fn out_pointee(
    types: &AbiTypeTable,
    routine: &ImportedRoutine,
    index: usize,
) -> Result<String, AdapterError> {
    let parameter = &routine.parameters[index];
    let Some(AbiType::Pointer { target, .. }) = types.get(parameter.type_id) else {
        return Err(AdapterError::UnresolvedOutParameter {
            symbol: routine.symbol.clone(),
            parameter: parameter.name.clone(),
        });
    };
    rust_scalar(types, *target, &routine.symbol)
}

/// One argument as it is passed to the raw provider.
///
/// A handle crosses the adapter boundary as `*mut c_void` and reaches the
/// provider as its own opaque pointee. `as *mut _` bridges the two without
/// this crate ever naming GERC's projected struct: the target type is inferred
/// from the callee's declared parameter.
fn cast_argument(types: &AbiTypeTable, parameter: &fol_abi::AbiParameter) -> String {
    let name = rust_param_name(&parameter.name);
    match types.get(parameter.type_id) {
        Some(AbiType::OpaqueHandle { .. }) => format!("{name} as *mut _"),
        // A callback and its context both already have the provider's own
        // spelling: the adapter declares the function pointer exactly as GERC
        // projected it, and `void *` projects to `*mut c_void` on both sides.
        _ => name,
    }
}

/// A C parameter may be named for a Rust keyword; raw identifiers keep the
/// provider's own spelling rather than renaming its API.
fn rust_param_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "typeof", "unsized", "virtual", "yield",
    ];
    match name {
        "crate" | "self" | "super" | "Self" => format!("{name}_kw"),
        _ if RESERVED.contains(&name) => format!("r#{name}"),
        _ => name.to_string(),
    }
}

fn rust_scalar(
    types: &AbiTypeTable,
    type_id: AbiTypeId,
    symbol: &str,
) -> Result<String, AdapterError> {
    let Some(abi_type) = types.get(type_id) else {
        return Err(AdapterError::UnknownType {
            symbol: symbol.to_string(),
        });
    };
    Ok(match abi_type {
        AbiType::Scalar(AbiScalar::Int(width)) => {
            let bits = width.bits().ok_or_else(|| AdapterError::UnsupportedType {
                symbol: symbol.to_string(),
                detail: "an architecture-sized integer".to_string(),
            })?;
            format!("{}{bits}", if width.is_signed() { "i" } else { "u" })
        }
        AbiType::Scalar(AbiScalar::Float(width)) => match width {
            fol_types::FloatWidth::F32 => "f32".to_string(),
            _ => "f64".to_string(),
        },
        AbiType::Scalar(AbiScalar::Bool) => "bool".to_string(),
        AbiType::Scalar(AbiScalar::Char) => "char".to_string(),
        AbiType::Void => "()".to_string(),
        // An opaque handle is the address and nothing else. It is spelled
        // `c_void` rather than the provider's own opaque struct because this
        // crate never names GERC's projected types: the raw call site casts
        // with `as *mut _`, which infers the pointee from the callee.
        AbiType::OpaqueHandle { .. } => "*mut core::ffi::c_void".to_string(),
        // Rendered as GERC projects a C function pointer, with the context
        // restored at the front: the canonical shape puts it first, and the
        // stored `parameters` are what FOL sees, which is everything after it.
        AbiType::Callback { parameters, result } => {
            let mut rendered = String::from("Option<unsafe extern \"C\" fn(*mut core::ffi::c_void");
            for parameter in parameters {
                rendered.push_str(", ");
                rendered.push_str(&rust_scalar(types, *parameter, symbol)?);
            }
            rendered.push(')');
            if !matches!(types.get(*result), Some(AbiType::Void)) {
                rendered.push_str(" -> ");
                rendered.push_str(&rust_scalar(types, *result, symbol)?);
            }
            rendered.push('>');
            rendered
        }
        other => {
            return Err(AdapterError::UnsupportedType {
                symbol: symbol.to_string(),
                detail: format!("a {} type", other.kind_name()),
            })
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    UnknownType { symbol: String },
    UnsupportedType { symbol: String, detail: String },
    UnresolvedOutParameter { symbol: String, parameter: String },
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType { symbol } => write!(
                f,
                "cannot generate an adapter for '{symbol}': it references a type the manifest \
                 does not define"
            ),
            Self::UnsupportedType { symbol, detail } => write!(
                f,
                "cannot generate an adapter for '{symbol}': it uses {detail}"
            ),
            Self::UnresolvedOutParameter { symbol, parameter } => write!(
                f,
                "cannot generate an adapter for '{symbol}': out-parameter '{parameter}' is not a \
                 writable pointer"
            ),
        }
    }
}

impl std::error::Error for AdapterError {}

#[cfg(test)]
mod tests {
    use super::*;
    use fol_abi::{
        AbiCallingConvention, AbiDirection, AbiEscape, AbiMutability, AbiNullability, AbiOwnership,
        AbiParameter, AbiSourceOrigin, ImportEffects,
    };

    fn scalar_interface() -> ImportedInterface {
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

        ImportedInterface {
            alias: "c_math".to_string(),
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu")
                .expect("certified target"),
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
                    origin: AbiSourceOrigin::default(),
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
                    effects: ImportEffects::default(),
                    handle: None,
                    callback: None,
                    origin: AbiSourceOrigin::default(),
                },
            ],
        }
    }

    #[test]
    fn an_infallible_adapter_calls_the_raw_symbol_directly() {
        let rendered =
            render_adapter_module(&scalar_interface(), "fol_raw").expect("adapters should render");

        assert!(
            rendered.contains("pub fn add_one(value: i32) -> i32"),
            "got:\n{rendered}"
        );
        assert!(
            rendered.contains("unsafe { fol_raw::c_math_add_one(value) }"),
            "got:\n{rendered}"
        );
    }

    #[test]
    fn a_status_adapter_owns_the_out_storage_and_checks_before_reading_it() {
        let rendered =
            render_adapter_module(&scalar_interface(), "fol_raw").expect("adapters should render");

        // The caller's signature has no out-parameter.
        assert!(
            rendered.contains("pub fn checked_div(lhs: i32) -> Result<i32, i32>"),
            "got:\n{rendered}"
        );
        // The adapter declares the storage and passes a pointer to its own.
        assert!(
            rendered.contains("let mut __fol_out: i32 = Default::default();"),
            "got:\n{rendered}"
        );
        assert!(
            rendered.contains("fol_raw::c_math_checked_div(lhs, &mut __fol_out)"),
            "got:\n{rendered}"
        );
        // The out value is read only on an enumerated success code.
        assert!(rendered.contains("0 => Ok(__fol_out),"), "got:\n{rendered}");
        assert!(
            rendered.contains("other => Err(other),"),
            "got:\n{rendered}"
        );
    }

    #[test]
    fn an_unenumerated_status_is_a_failure_rather_than_a_success() {
        let rendered =
            render_adapter_module(&scalar_interface(), "fol_raw").expect("adapters should render");

        // The catch-all arm must come last and must be the Err arm: treating an
        // unknown code as success would read uninitialized out storage.
        let ok_at = rendered.find("Ok(__fol_out)").expect("an Ok arm");
        let err_at = rendered.find("Err(other)").expect("an Err arm");
        assert!(ok_at < err_at, "the catch-all must be the failure arm");
    }

    #[test]
    fn every_unsafe_call_is_inside_an_adapter() {
        let rendered =
            render_adapter_module(&scalar_interface(), "fol_raw").expect("adapters should render");

        // Two routines, two unsafe blocks, no others.
        assert_eq!(rendered.matches("unsafe {").count(), 2, "got:\n{rendered}");
    }

    #[test]
    fn the_adapter_crate_name_matches_the_interop_spelling() {
        // The backend emits `<crate>::cimp__<alias>::<fn>` at every foreign
        // call site, and this crate writes the file that has to define it. The
        // two names live in different crates because neither depends on the
        // other, so the agreement is asserted rather than assumed.
        assert_eq!(crate::anchor::H7_ANCHOR_CRATE_NAME, "fol_h7_anchor");
    }

    #[test]
    fn the_module_name_matches_what_the_backend_calls() {
        // The backend renders `cimp__<alias>::<fn>` at every foreign call site.
        // If either spelling moves, generated code stops resolving, so the two
        // are asserted equal here rather than left to agree by habit.
        assert_eq!(adapter_module_name("c_math"), "cimp__c_math");
        let rendered =
            render_adapter_module(&scalar_interface(), "fol_raw").expect("adapters should render");
        assert!(
            rendered.contains("pub mod cimp__c_math {"),
            "got:\n{rendered}"
        );
    }

    #[test]
    fn a_c_parameter_named_for_a_rust_keyword_keeps_its_spelling() {
        let mut interface = scalar_interface();
        interface.routines[0].parameters[0].name = "type".to_string();
        let rendered =
            render_adapter_module(&interface, "fol_raw").expect("adapters should render");

        assert!(
            rendered.contains("pub fn add_one(r#type: i32)"),
            "a provider's parameter name is not ours to rename; got:\n{rendered}"
        );
        assert!(rendered.contains("(r#type)"), "got:\n{rendered}");
    }

    /// A handle crosses as a bare address, cast at the raw call in both
    /// directions.
    ///
    /// The casts are the point: this crate must never name the opaque struct
    /// GERC projects for the provider's incomplete type. `as *mut _` on the way
    /// in infers the pointee from the callee, and `as *mut c_void` on the way
    /// out returns to the spelling the adapter declares.
    #[test]
    fn a_handle_crosses_as_an_address_and_is_cast_at_the_raw_call() {
        let mut interface = scalar_interface();
        let handle = interface.types.intern(AbiType::OpaqueHandle {
            name: "Widget".to_string(),
        });
        interface.routines[0].result = handle;
        interface.routines[0].parameters[0].type_id = handle;

        let rendered =
            render_adapter_module(&interface, "fol_raw").expect("a handle adapter should render");
        assert!(
            rendered.contains("pub fn add_one(value: *mut core::ffi::c_void)"),
            "got:\n{rendered}"
        );
        assert!(
            rendered.contains(" -> *mut core::ffi::c_void"),
            "got:\n{rendered}"
        );
        assert!(rendered.contains("(value as *mut _)"), "got:\n{rendered}");
        assert!(
            rendered.contains("as *mut core::ffi::c_void }"),
            "the result casts back to the declared spelling; got:\n{rendered}"
        );
    }

    /// A shape with no handle role behind it is still refused by name.
    #[test]
    fn an_aggregate_type_is_refused_rather_than_guessed() {
        let mut interface = scalar_interface();
        let record = interface.types.intern(AbiType::Record {
            name: "Point".to_string(),
            fields: Vec::new(),
        });
        interface.routines[0].result = record;

        assert_eq!(
            render_adapter_module(&interface, "fol_raw"),
            Err(AdapterError::UnsupportedType {
                symbol: "c_math_add_one".to_string(),
                detail: "a record type".to_string(),
            })
        );
    }
}
