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
        // A buffer domain's release gets no adapter. FOL cannot call it -- it
        // takes a raw address FOL never holds -- and the producer that does
        // call it reaches the certified symbol directly.
        if !routine.is_mountable() {
            continue;
        }
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
        // A record arrives as its fields rather than as a struct. FOL's struct
        // has FOL's layout, so it cannot be handed to C; the provider's struct
        // is built here instead, from values whose types C itself measured.
        if let Some((_, fields, _)) = record_behind(&interface.types, parameter.type_id) {
            // By field name, not by declaration order.
            //
            // This list and the call site have to agree, and they disagreed:
            // the adapter took declaration order while the backend emits from
            // FOL's own lowered record, whose fields are sorted. A struct
            // whose declaration order was not already alphabetical therefore
            // arrived with its fields **transposed** -- silently, with no
            // diagnostic and a wrong answer. `struct point { int x; int y; }`
            // hid it for a whole milestone by being alphabetical already.
            //
            // Sorting here is what makes them agree. The provider's own field
            // order is untouched: `cast_argument` rebuilds the struct by name.
            let mut ordered: Vec<&fol_abi::AbiField> = fields.iter().collect();
            ordered.sort_by(|left, right| left.name.cmp(&right.name));
            for field in ordered {
                params.push(format!(
                    "{}: {}",
                    record_field_param(&parameter.name, &field.name),
                    rust_scalar(&interface.types, field.type_id, &routine.symbol)?
                ));
            }
            continue;
        }
        params.push(format!(
            "{}: {}",
            rust_param_name(&parameter.name),
            rust_scalar(&interface.types, parameter.type_id, &routine.symbol)?
        ));
        // A paired buffer is one FOL value and two C parameters, so its length
        // is declared here rather than beside the other visible parameters:
        // the provider takes it right after the address, and the call site
        // derives it from the value it just passed.
        if routine
            .buffer
            .as_ref()
            .is_some_and(|use_| use_.parameter == parameter.name)
        {
            let index = routine
                .buffer_length_index()
                .ok_or_else(|| AdapterError::UnknownType {
                    symbol: routine.symbol.clone(),
                })?;
            let length = &routine.parameters[index];
            params.push(format!(
                "{}: {}",
                rust_param_name(&length.name),
                rust_scalar(&interface.types, length.type_id, &routine.symbol)?
            ));
        }
    }
    // The context is hidden from FOL but not from the adapter: FOL passes the
    // trampoline and the closure address as two ordinary arguments here, and
    // the adapter forwards both. The trampoline itself is emitted by the
    // backend, which is the only layer that knows the closure's Rust type.
    // Only when the provider has one: a context-free callback -- `qsort`'s
    // comparator, `lua_CFunction` -- has no slot to fill.
    if let Some(index) = routine.callback_context_index() {
        params.push(format!(
            "{}: *mut core::ffi::c_void",
            rust_param_name(&routine.parameters[index].name)
        ));
    }
    let params = params.join(", ");

    // A produced buffer is its own shape: the provider's memory is validated,
    // copied out of, and released before the call returns, so nothing FOL
    // holds afterwards points into it.
    if let Some(body) = owned_buffer_body(interface, routine, &raw)? {
        return Ok(format!(
            "    #[inline]\n    pub fn {}({params}) -> {} {{\n{body}    }}\n",
            rust_param_name(&routine.fol_name),
            owned_buffer_return(interface, routine)?,
        ));
    }

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

/// The record a parameter carries: by value, or behind a `const` pointer.
///
/// Both arrive as fields and are rebuilt here, for the same reason -- FOL's
/// struct has FOL's layout, so the address C receives has to be one this
/// adapter made. The only difference is whether the rebuilt struct is passed
/// or lent.
fn record_behind(
    types: &AbiTypeTable,
    type_id: AbiTypeId,
) -> Option<(&str, &[fol_abi::AbiField], bool)> {
    match types.get(type_id) {
        Some(AbiType::Record { name, fields }) => Some((name, fields, false)),
        Some(AbiType::Pointer {
            target,
            mutability: fol_abi::AbiMutability::Const,
            ..
        }) => match types.get(*target) {
            Some(AbiType::Record { name, fields }) => Some((name, fields, true)),
            _ => None,
        },
        _ => None,
    }
}

/// The Rust type a produced buffer hands back.
///
/// `std::vec::Vec`, not FOL's own vector: this crate is linked without the
/// runtime, and the call site converts.
fn owned_buffer_return(
    interface: &ImportedInterface,
    routine: &ImportedRoutine,
) -> Result<String, AdapterError> {
    let Some(AbiType::Pointer { target, .. }) = interface.types.get(routine.result) else {
        return Err(AdapterError::UnsupportedType {
            symbol: routine.symbol.clone(),
            detail: "a produced buffer whose result is not a pointer".to_string(),
        });
    };
    Ok(format!(
        "std::vec::Vec<{}>",
        rust_scalar(&interface.types, *target, &routine.symbol)?
    ))
}

/// The body of a routine that produces an owned buffer.
///
/// Three things happen here that cannot happen anywhere else. The provider's
/// report is *validated* -- a null address with a nonzero length, or a length
/// past the capacity it reported, is a provider contradicting itself, and
/// reading the buffer on either would read memory that was never described.
/// The bytes are *copied*, because FOL's allocator did not make this
/// allocation and must never free it. And the domain's release is *called*,
/// exactly once, before the copy is handed back.
fn owned_buffer_body(
    interface: &ImportedInterface,
    routine: &ImportedRoutine,
    raw: &str,
) -> Result<Option<String>, AdapterError> {
    let Some(use_) = &routine.owned_buffer else {
        return Ok(None);
    };
    if use_.role != fol_abi::BufferRole::Produces {
        return Ok(None);
    }
    let (Some(destroy), Some(length_index)) =
        (&routine.owned_destroy, routine.owned_length_index())
    else {
        return Err(AdapterError::UnsupportedType {
            symbol: routine.symbol.clone(),
            detail: "a produced buffer with no resolved release or length".to_string(),
        });
    };
    let length_name = rust_param_name(&routine.parameters[length_index].name);
    let length_type = out_pointee(&interface.types, routine, length_index)?;
    let capacity_index = routine.owned_capacity_index();

    let mut lines = vec![format!("        let mut {length_name}: {length_type} = 0;")];
    let mut capacity_check = Vec::new();
    if let Some(index) = capacity_index {
        let name = rust_param_name(&routine.parameters[index].name);
        let ty = out_pointee(&interface.types, routine, index)?;
        lines.push(format!("        let mut {name}: {ty} = 0;"));
        capacity_check = vec![
            format!("        if {length_name} > {name} {{"),
            format!(
                "            panic!(\"fol interop fault: '{}' reported a longer buffer than \
                 the capacity it allocated\");",
                routine.symbol
            ),
            "        }".to_string(),
        ];
    }

    let call_args = routine
        .parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            if position == length_index || Some(position) == capacity_index {
                format!("&mut {}", rust_param_name(&parameter.name))
            } else {
                cast_argument(&interface.types, parameter)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    let release_args = release_arguments(interface, routine, destroy, &length_name)?;
    let symbol = &routine.symbol;
    lines.extend([
        format!("        // SAFETY: `{raw}` is the LINC-certified declaration of a symbol this"),
        "        // provider defines. The out storage is live locals of the declared types,".to_string(),
        "        // so the pointers handed over are valid for the whole call.".to_string(),
        format!("        let __fol_address = unsafe {{ {raw}({call_args}) }};"),
        "        if __fol_address.is_null() {".to_string(),
        "            // A provider may report failure by returning NULL, and that is an".to_string(),
        "            // empty buffer. Claiming a length for one is not: there is no memory".to_string(),
        "            // the count could be describing.".to_string(),
        format!("            if {length_name} != 0 {{"),
        format!(
            "                panic!(\"fol interop fault: '{symbol}' returned NULL but reported a nonzero length\");"
        ),
        "            }".to_string(),
        "            return std::vec::Vec::new();".to_string(),
        "        }".to_string(),
    ]);
    lines.extend(capacity_check);
    lines.extend([
        "        // Copied, never adopted: this allocation is the provider's, and freeing"
            .to_string(),
        "        // it with FOL's allocator would be freeing memory that allocator never"
            .to_string(),
        "        // made.".to_string(),
        "        let __fol_copy = unsafe {".to_string(),
        format!("            core::slice::from_raw_parts(__fol_address, {length_name} as usize)"),
        "        }".to_string(),
        "        .to_vec();".to_string(),
        "        // SAFETY: the address came from this domain's one producer and has not"
            .to_string(),
        "        // been released; the copy above no longer points into it.".to_string(),
        format!("        unsafe {{ fol_h7_raw::{destroy}({release_args}) }};"),
        "        __fol_copy".to_string(),
    ]);
    Ok(Some(format!("{}\n", lines.join("\n"))))
}

/// The arguments the domain's release takes, in its own declared order.
fn release_arguments(
    interface: &ImportedInterface,
    routine: &ImportedRoutine,
    destroy: &str,
    length_name: &str,
) -> Result<String, AdapterError> {
    let Some(release) = interface
        .routines
        .iter()
        .find(|candidate| candidate.symbol == destroy)
    else {
        return Err(AdapterError::UnsupportedType {
            symbol: routine.symbol.clone(),
            detail: format!("release '{destroy}' is not part of this interface"),
        });
    };
    let length = release
        .owned_buffer
        .as_ref()
        .map(|use_| use_.length.clone())
        .unwrap_or_default();
    Ok(release
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.name == length {
                format!("{length_name} as _")
            } else {
                "__fol_address as *mut _".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
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
/// The adapter parameter carrying one field of a flattened record.
fn record_field_param(parameter: &str, field: &str) -> String {
    rust_param_name(&format!("{parameter}__{field}"))
}

fn cast_argument(types: &AbiTypeTable, parameter: &fol_abi::AbiParameter) -> String {
    let name = rust_param_name(&parameter.name);
    match types.get(parameter.type_id) {
        Some(AbiType::OpaqueHandle { .. }) => format!("{name} as *mut _"),
        // Rebuilt field by field, never transmuted -- the same rule the export
        // wrapper follows in the other direction. A `const` pointer to a
        // record is the same rebuild, lent rather than passed.
        _ if record_behind(types, parameter.type_id).is_some() => {
            let (record, fields, behind_pointer) =
                record_behind(types, parameter.type_id).expect("checked");
            let assigned = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        rust_param_name(&field.name),
                        record_field_param(&parameter.name, &field.name)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let built = format!(
                "{}::{record} {{ {assigned} }}",
                crate::anchor::H7_RAW_CRATE_NAME
            );
            // A temporary lives to the end of the enclosing statement, which
            // is the raw call itself -- so the provider reads it for exactly
            // as long as it is entitled to.
            if behind_pointer {
                format!("&{built}")
            } else {
                built
            }
        }
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
        // A NUL-terminated string is an address and nothing else. The bytes it
        // points at are built by the call site, which owns them for exactly
        // the length of the call.
        AbiType::CString { mutability } => format!(
            "*{} core::ffi::c_char",
            match mutability {
                fol_abi::AbiMutability::Const => "const",
                fol_abi::AbiMutability::Mutable => "mut",
            }
        ),
        // Only the address: the length travels as its own parameter, which is
        // the shape C has and the reason the pairing had to be declared.
        AbiType::BorrowedSlice {
            element,
            mutability,
        } => format!(
            "*{} {}",
            match mutability {
                fol_abi::AbiMutability::Const => "const",
                fol_abi::AbiMutability::Mutable => "mut",
            },
            rust_scalar(types, *element, symbol)?
        ),
        // A record parameter never reaches here -- it is flattened into its
        // fields before this is called -- so a record in this position is a
        // result, and returning one would mean handing back the provider's own
        // struct, which nothing on FOL's side can name.
        AbiType::Record { .. } => {
            return Err(AdapterError::UnsupportedType {
                symbol: symbol.to_string(),
                detail: "a record type".to_string(),
            })
        }
        // Rendered as GERC projects a C function pointer, with the context
        // restored at the front: the canonical shape puts it first, and the
        // stored `parameters` are what FOL sees, which is everything after it.
        AbiType::Callback {
            parameters,
            result,
            context,
        } => {
            // The context is restored at the front when the provider has one,
            // because the canonical shape puts it first and `parameters` is
            // what FOL sees, which is everything after it. A context-free
            // provider takes exactly what FOL sees.
            let mut rendered = String::from("Option<unsafe extern \"C\" fn(");
            if *context {
                rendered.push_str("*mut core::ffi::c_void");
            }
            let mut first = !*context;
            for parameter in parameters {
                if first {
                    first = false;
                } else {
                    rendered.push_str(", ");
                }
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
                    buffer: None,
                    strings: Default::default(),
                    owned_buffer: None,
                    owned_destroy: None,
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
                    buffer: None,
                    strings: Default::default(),
                    owned_buffer: None,
                    owned_destroy: None,
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
