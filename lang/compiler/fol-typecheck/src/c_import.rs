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
use fol_resolver::SymbolId;

use crate::{
    model::TypedProgram,
    types::{BuiltinType, CheckedType, CheckedTypeId, RoutineType},
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

        for routine in &interface.routines {
            let Some(symbol_id) = symbols.get(&routine.fol_name).copied() else {
                continue;
            };
            match routine_type_for(typed, &interface.types, routine) {
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

fn routine_type_for(
    typed: &mut TypedProgram,
    types: &AbiTypeTable,
    routine: &ImportedRoutine,
) -> Result<RoutineType, TypecheckError> {
    let mut param_names = Vec::new();
    let mut params = Vec::new();
    for parameter in routine.call_parameters() {
        param_names.push(parameter.name.clone());
        params.push(checked_type_for(typed, types, parameter.type_id, routine)?);
    }

    let (return_type, error_type) = match &routine.error {
        ImportErrorConvention::Infallible => {
            let result = checked_type_for(typed, types, routine.result, routine)?;
            // A `void` C result is no FOL value at all, not a unit value.
            let return_type =
                matches!(types.get(routine.result), Some(AbiType::Scalar(_))).then_some(result);
            (return_type, None)
        }
        ImportErrorConvention::Status { .. } => {
            let success = success_type_for(typed, types, routine)?;
            let shell = typed.type_table_mut().intern(CheckedType::Error {
                inner: Some(success),
            });
            (Some(shell), Some(success))
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
    checked_type_for(typed, types, *target, routine)
}

fn checked_type_for(
    typed: &mut TypedProgram,
    types: &AbiTypeTable,
    type_id: AbiTypeId,
    routine: &ImportedRoutine,
) -> Result<CheckedTypeId, TypecheckError> {
    let abi_type = types.get(type_id).ok_or_else(|| {
        internal(format!(
            "imported routine '{}' references a type the manifest does not define",
            routine.symbol
        ))
    })?;
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
                "imported routine '{}' uses a {} type, which M6 does not surface to FOL",
                routine.symbol,
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
    fn a_status_import_hides_its_out_parameter_and_returns_a_shell() {
        let mut typed = empty_program();
        hydrate_c_import_symbol_types(&mut typed, &[scalar_interface()]).expect("hydration");

        let div = signature(&typed, "checked_div");
        // The caller supplies lhs and rhs; FOL owns the out storage, so the
        // caller must not pass it.
        assert_eq!(div.param_names, vec!["lhs", "rhs"]);

        let inner = match div.return_type.and_then(|id| typed.type_table().get(id)) {
            Some(CheckedType::Error { inner }) => inner.expect("err[T] carries its success type"),
            other => panic!("a status import should return err[T], got {other:?}"),
        };
        assert_eq!(
            typed.type_table().get(inner),
            Some(&CheckedType::Builtin(BuiltinType::Int(
                fol_types::IntWidth::I32
            ))),
            "the success type is the out-parameter's pointee"
        );
        assert!(div.error_type.is_some());
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
