//! Giving a synthesized foreign namespace its FOL types.
//!
//! The resolver mounts an import's routines as symbols with no declaration
//! behind them (see `fol_resolver::c_import`). This is where each one gets the
//! `RoutineType` a caller is checked against, built from the checked import
//! manifest rather than from any FOL source.
//!
//! The two shapes a routine can take come straight from section 4.13's error
//! subset:
//!
//! - infallible: every C parameter is a FOL parameter, and the C result is the
//!   FOL result
//! - status: the out-parameter is *not* a FOL parameter, because FOL supplies
//!   that storage itself; the FOL result is `err[T]` over the out-parameter's
//!   pointee, so a caller cannot read the success value without handling the
//!   failure first

use std::collections::BTreeMap;

use fol_abi::{
    AbiScalar, AbiType, AbiTypeId, AbiTypeTable, ImportErrorConvention, ImportedInterface,
    ImportedRoutine,
};
use fol_resolver::{SymbolId, SymbolKind};

use crate::{
    model::TypedProgram,
    types::{BuiltinType, CheckedType, CheckedTypeId, DeclaredTypeKind, RoutineType},
    TypecheckError, TypecheckErrorKind, TypecheckResult,
};

/// Type every foreign routine the resolver mounted.
pub(crate) fn hydrate_c_import_symbol_types(
    typed: &mut TypedProgram,
    interfaces: &[ImportedInterface],
) -> TypecheckResult<()> {
    if interfaces.is_empty() {
        return Ok(());
    }

    let mut errors = Vec::new();
    for interface in interfaces {
        let Some(scope_id) = typed.resolved().namespace_scope(&interface.alias) else {
            // The resolver declined to mount it, which it already reported.
            continue;
        };
        let symbols: BTreeMap<String, SymbolId> = typed
            .resolved()
            .symbols_in_scope(scope_id)
            .iter()
            .map(|symbol| (symbol.name.clone(), symbol.id))
            .collect();

        // Records before routines: a signature naming one resolves through the
        // symbol's declared type, so the structure must be attached first.
        for (name, fields) in interface.record_shapes() {
            let Some(symbol_id) = symbols.get(name).copied() else {
                continue;
            };
            let mut checked_fields = BTreeMap::new();
            let mut failed = false;
            for field in fields {
                match checked_type_for(
                    typed,
                    &interface.alias,
                    &interface.types,
                    field.type_id,
                    &format!("imported record field {:?}", field.name),
                ) {
                    Ok(type_id) => {
                        checked_fields.insert(field.name.clone(), type_id);
                    }
                    Err(error) => {
                        errors.push(error);
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                continue;
            }
            let type_id = typed.type_table_mut().intern(CheckedType::Record {
                fields: checked_fields,
            });
            if let Some(symbol) = typed.typed_symbol_mut(symbol_id) {
                symbol.declared_type = Some(type_id);
            }
        }

        for routine in &interface.routines {
            let Some(symbol_id) = symbols.get(&routine.fol_name).copied() else {
                continue;
            };
            match routine_type_for(typed, &interface.alias, &interface.types, routine) {
                Ok(signature) => {
                    let type_id = typed
                        .type_table_mut()
                        .intern(CheckedType::Routine(signature));
                    if let Some(symbol) = typed.typed_symbol_mut(symbol_id) {
                        symbol.declared_type = Some(type_id);
                        // A foreign routine has no defaults and no generics:
                        // C has neither, and inventing one would let a caller
                        // omit an argument the provider requires.
                        symbol.param_defaults = vec![None; routine.call_parameters().len()];
                    }
                }
                Err(error) => errors.push(error),
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Intern one handle domain and record its linear obligation.
///
/// The `lin` claim is what makes every later rule apply: the flow analysis
/// then proves the handle is consumed exactly once on every path, and
/// `[cpy]`/`[cln]` of one are refused as duplication. None of that needs a
/// FOL declaration, because the claim is on the type rather than on a syntax
/// node.
pub(crate) fn foreign_handle_type(
    typed: &mut TypedProgram,
    alias: &str,
    domain: &str,
) -> CheckedTypeId {
    let type_id = typed.type_table_mut().intern(CheckedType::ForeignHandle {
        alias: alias.to_string(),
        domain: domain.to_string(),
    });
    typed.record_lin_type(type_id);
    type_id
}

fn routine_type_for(
    typed: &mut TypedProgram,
    alias: &str,
    types: &AbiTypeTable,
    routine: &ImportedRoutine,
) -> Result<RoutineType, TypecheckError> {
    // A routine that only *borrows* a handle takes a loan, not the handle: a
    // by-value parameter would consume it, and `widget_size(w)` would leave the
    // caller with nothing to release. The role is the only thing that says
    // which, because a C pointer parameter looks identical either way.
    let borrows = routine
        .handle
        .as_ref()
        .is_some_and(|use_| use_.role == fol_abi::HandleRole::Borrows);

    let mut param_names = Vec::new();
    let mut params = Vec::new();
    for parameter in routine.call_parameters() {
        param_names.push(parameter.name.clone());
        let mut checked = checked_type_for(
            typed,
            alias,
            types,
            parameter.type_id,
            &format!("imported routine {:?}", routine.symbol),
        )?;
        if borrows
            && matches!(
                types.get(parameter.type_id),
                Some(AbiType::OpaqueHandle { .. })
            )
        {
            checked = typed.type_table_mut().intern(CheckedType::Borrowed {
                inner: checked,
                mutable: false,
            });
        }
        params.push(checked);
    }

    let (return_type, error_type) = match &routine.error {
        ImportErrorConvention::Infallible => {
            let result = checked_type_for(
                typed,
                alias,
                types,
                routine.result,
                &format!("imported routine {:?}", routine.symbol),
            )?;
            // A `void` C result is no FOL value at all, not a unit value.
            // Everything else is: a producer returning a handle has to hand it
            // back, or the resource would be created with nobody owing it.
            let return_type =
                (!matches!(types.get(routine.result), Some(AbiType::Void))).then_some(result);
            (return_type, None)
        }
        ImportErrorConvention::Status { .. } => {
            // FOL spells a fallible routine `(...): T / E`, keeping the two
            // types apart rather than wrapping one in the other. The success
            // type is the out-parameter's pointee; the failure carries the
            // provider's own status code, which is the only thing it reported.
            let success = success_type_for(typed, alias, types, routine)?;
            let status = checked_type_for(
                typed,
                alias,
                types,
                routine.result,
                &format!("imported routine {:?}", routine.symbol),
            )?;
            (Some(success), Some(status))
        }
    };

    Ok(RoutineType {
        generic_params: Vec::new(),
        generic_constraints: BTreeMap::new(),
        param_names,
        param_defaults: vec![None; params.len()],
        variadic_index: None,
        mutex_params: Default::default(),
        params,
        return_type,
        error_type,
        env_lifetime: false,
    })
}

/// Under a status mapping the FOL result is the out-parameter's pointee.
fn success_type_for(
    typed: &mut TypedProgram,
    alias: &str,
    types: &AbiTypeTable,
    routine: &ImportedRoutine,
) -> Result<CheckedTypeId, TypecheckError> {
    let index = routine.out_parameter_index().ok_or_else(|| {
        internal(format!(
            "imported routine '{}' has a status mapping whose out-parameter was not resolved",
            routine.symbol
        ))
    })?;
    let out = &routine.parameters[index];
    let Some(AbiType::Pointer { target, .. }) = types.get(out.type_id) else {
        return Err(internal(format!(
            "imported routine '{}' names a non-pointer out-parameter",
            routine.symbol
        )));
    };
    checked_type_for(
        typed,
        alias,
        types,
        *target,
        &format!("imported routine {:?}", routine.symbol),
    )
}

/// The nominal FOL type for a mounted C record, if the resolver mounted it.
fn foreign_record_type(typed: &mut TypedProgram, alias: &str, name: &str) -> Option<CheckedTypeId> {
    let scope_id = typed.resolved().namespace_scope(alias)?;
    let symbol = typed
        .resolved()
        .symbols_in_scope(scope_id)
        .iter()
        .find(|symbol| symbol.name == name && symbol.kind == SymbolKind::Type)
        .map(|symbol| symbol.id)?;
    Some(typed.type_table_mut().intern(CheckedType::Declared {
        symbol,
        name: name.to_string(),
        kind: DeclaredTypeKind::Type,
        args: Vec::new(),
    }))
}

fn checked_type_for(
    typed: &mut TypedProgram,
    alias: &str,
    types: &AbiTypeTable,
    type_id: AbiTypeId,
    context: &str,
) -> Result<CheckedTypeId, TypecheckError> {
    let abi_type = types.get(type_id).ok_or_else(|| {
        internal(format!(
            "{context} references a type the manifest does not define"
        ))
    })?;
    // Handled before the match because it needs the import's alias, which is
    // half the handle's identity and which the ABI type table does not carry.
    if let AbiType::OpaqueHandle { name } = abi_type {
        let name = name.clone();
        return Ok(foreign_handle_type(typed, alias, &name));
    }
    // A callback becomes an ordinary FOL routine value, so a caller passes a
    // closure and nothing about the C function-pointer shape is visible. The
    // context is already absent: it never reaches this function, because
    // `call_parameters` hides it.
    if let AbiType::Callback { parameters, result } = abi_type {
        let (parameters, result) = (parameters.clone(), *result);
        let mut params = Vec::new();
        for parameter in &parameters {
            params.push(checked_type_for(typed, alias, types, *parameter, context)?);
        }
        let return_type = match types.get(result) {
            Some(AbiType::Void) => None,
            _ => Some(checked_type_for(typed, alias, types, result, context)?),
        };
        let signature = RoutineType {
            generic_params: Vec::new(),
            generic_constraints: BTreeMap::new(),
            param_names: (0..params.len())
                .map(|index| format!("arg{index}"))
                .collect(),
            param_defaults: vec![None; params.len()],
            variadic_index: None,
            mutex_params: Default::default(),
            params,
            return_type,
            // A C callback has one result channel. A FOL closure that reported
            // would have nowhere to report to: the provider is mid-call and
            // takes only the return value.
            error_type: None,
            env_lifetime: false,
        };
        return Ok(typed
            .type_table_mut()
            .intern(CheckedType::Routine(signature)));
    }
    // A paired buffer reaches FOL as one borrowed vector. Borrowed rather than
    // owned because the provider is lent the storage for the call and FOL keeps
    // it: an owned `vec` would mean handing over memory the caller still holds.
    // The length is not a FOL parameter at all -- it comes off this value.
    if let AbiType::BorrowedSlice {
        element,
        mutability,
    } = abi_type
    {
        let (element, mutability) = (*element, *mutability);
        let element = checked_type_for(typed, alias, types, element, context)?;
        let inner = typed.type_table_mut().intern(CheckedType::Vector {
            element_type: element,
        });
        return Ok(typed.type_table_mut().intern(CheckedType::Borrowed {
            inner,
            mutable: matches!(mutability, fol_abi::AbiMutability::Mutable),
        }));
    }
    // A record is nominal: it reaches FOL as a reference to the type symbol the
    // resolver mounted, not as a fresh structural shape.
    if let AbiType::Record { name, .. } = abi_type {
        let name = name.clone();
        return foreign_record_type(typed, alias, &name).ok_or_else(|| {
            internal(format!(
                "{context} uses record '{name}', which the resolver did not mount"
            ))
        });
    }
    let checked = match abi_type {
        AbiType::Scalar(AbiScalar::Int(width)) => CheckedType::Builtin(BuiltinType::Int(*width)),
        AbiType::Scalar(AbiScalar::Float(width)) => {
            CheckedType::Builtin(BuiltinType::Float(*width))
        }
        AbiType::Scalar(AbiScalar::Bool) => CheckedType::Builtin(BuiltinType::Bool),
        AbiType::Scalar(AbiScalar::Char) => {
            CheckedType::Builtin(BuiltinType::Char(fol_types::CharEncoding::Utf32))
        }
        // `void` reaches here only as an infallible result, where the caller
        // turns it into "no return type" rather than a value.
        AbiType::Void => CheckedType::Builtin(BuiltinType::Never),
        other => {
            return Err(internal(format!(
                "{context} uses a {} type, which the C import path does not surface to FOL",
                other.kind_name()
            )))
        }
    };
    Ok(typed.type_table_mut().intern(checked))
}

fn internal(message: String) -> TypecheckError {
    TypecheckError::new(TypecheckErrorKind::Internal, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fol_abi::{
        AbiCallingConvention, AbiDirection, AbiEscape, AbiMutability, AbiNullability, AbiOwnership,
        AbiParameter, AbiSourceOrigin, ImportEffects,
    };
    use fol_resolver::model::ResolvedProgram;

    fn empty_program() -> TypedProgram {
        let parsed = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "lib.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: fol_parser::ast::ParsedSourceUnitKind::Ordinary,
                items: Vec::new(),
            }],
            syntax_index: Default::default(),
        };
        let mut resolved = ResolvedProgram::new(parsed);
        fol_resolver::inject_c_import_namespaces(&mut resolved, &[scalar_interface()]);
        TypedProgram::from_resolved(resolved)
    }

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
                            name: "rhs".to_string(),
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
                        failure: vec![1],
                        out_parameter: "result".to_string(),
                    },
                    effects: ImportEffects::default(),
                    handle: None,
                    callback: None,
                    buffer: None,
                    origin: AbiSourceOrigin::default(),
                },
            ],
        }
    }

    fn signature(typed: &TypedProgram, fol_name: &str) -> RoutineType {
        let scope = typed
            .resolved()
            .namespace_scope("c_math")
            .expect("the namespace should be mounted");
        let symbol = typed
            .resolved()
            .symbols_in_scope(scope)
            .into_iter()
            .find(|symbol| symbol.name == fol_name)
            .expect("routine symbol should exist");
        let type_id = typed
            .typed_symbol(symbol.id)
            .and_then(|symbol| symbol.declared_type)
            .expect("a hydrated foreign routine should have a declared type");
        match typed.type_table().get(type_id) {
            Some(CheckedType::Routine(signature)) => signature.clone(),
            other => panic!("expected a routine type, got {other:?}"),
        }
    }

    #[test]
    fn an_infallible_import_keeps_every_parameter_and_its_result() {
        let mut typed = empty_program();
        hydrate_c_import_symbol_types(&mut typed, &[scalar_interface()]).expect("hydration");

        let add_one = signature(&typed, "add_one");
        assert_eq!(add_one.param_names, vec!["value"]);
        assert_eq!(
            typed.type_table().get(add_one.params[0]),
            Some(&CheckedType::Builtin(BuiltinType::Int(
                fol_types::IntWidth::I32
            )))
        );
        assert_eq!(
            add_one
                .return_type
                .and_then(|id| typed.type_table().get(id)),
            Some(&CheckedType::Builtin(BuiltinType::Int(
                fol_types::IntWidth::I32
            )))
        );
        assert!(
            add_one.error_type.is_none(),
            "an infallible import is not recoverable"
        );
    }

    #[test]
    fn a_status_import_hides_its_out_parameter_and_is_fallible() {
        let mut typed = empty_program();
        hydrate_c_import_symbol_types(&mut typed, &[scalar_interface()]).expect("hydration");

        let div = signature(&typed, "checked_div");
        // The caller supplies lhs and rhs; FOL owns the out storage, so the
        // caller must not pass it.
        assert_eq!(div.param_names, vec!["lhs", "rhs"]);

        // FOL's fallible shape is `(...): T / E`, two separate types -- not a
        // success type wrapped in an error shell.
        let int32 = CheckedType::Builtin(BuiltinType::Int(fol_types::IntWidth::I32));
        assert_eq!(
            div.return_type.and_then(|id| typed.type_table().get(id)),
            Some(&int32),
            "the success type is the out-parameter's pointee"
        );
        assert_eq!(
            div.error_type.and_then(|id| typed.type_table().get(id)),
            Some(&int32),
            "the failure carries the provider's own status code"
        );
    }

    #[test]
    fn a_foreign_routine_is_never_generic_or_defaulted() {
        let mut typed = empty_program();
        hydrate_c_import_symbol_types(&mut typed, &[scalar_interface()]).expect("hydration");

        for name in ["add_one", "checked_div"] {
            let signature = signature(&typed, name);
            assert!(signature.generic_params.is_empty(), "{name} is not generic");
            assert!(signature.variadic_index.is_none(), "{name} is not variadic");
            assert!(
                signature.param_defaults.iter().all(Option::is_none),
                "{name} has no defaults: C has none to take"
            );
        }
    }

    #[test]
    fn hydrating_an_alias_the_resolver_did_not_mount_is_not_an_error() {
        let mut typed = empty_program();
        let mut absent = scalar_interface();
        absent.alias = "never_mounted".to_string();

        // The resolver reports its own failure; this pass must not report a
        // second, confusing one for the same cause.
        assert!(hydrate_c_import_symbol_types(&mut typed, &[absent]).is_ok());
    }
}
