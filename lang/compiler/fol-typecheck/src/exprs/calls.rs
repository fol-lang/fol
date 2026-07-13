use crate::{
    CheckedType, CheckedTypeId, DeclaredTypeKind, RecoverableCallEffect, RoutineType,
    TypecheckError, TypecheckErrorKind, TypedProgram,
};
use fol_intrinsics::{
    boolean_operand_contract, comparison_operand_contract, query_operand_contract,
    select_intrinsic, wrong_arity_message, wrong_type_family_message, BooleanOperandContract,
    ComparisonOperandContract, IntrinsicSelectionErrorKind, IntrinsicSurface, QueryOperandContract,
};
use fol_parser::ast::{AstNode, QualifiedPath, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{ReferenceId, ReferenceKind, ResolvedProgram, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};

use super::helpers::{
    apparent_type_id, describe_type, ensure_assignable, internal_error, is_error_shell_type,
    merge_recoverable_effects, node_origin, observe_context, origin_for, plain_value_expr,
    reject_recoverable_error_shell_conversion, strip_comments, unsupported_node_surface,
};
use super::type_node;
use super::type_node_with_expectation;
use super::{TypeContext, TypedExpr};

pub(crate) fn type_function_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    name: &str,
    type_args: &[fol_parser::ast::FolType],
    args: &[AstNode],
    syntax_id: Option<SyntaxNodeId>,
) -> Result<TypedExpr, TypecheckError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("function call '{name}' does not retain a syntax id"),
        )
    })?;
    let reference_id =
        find_reference_by_syntax(resolved, syntax_id, ReferenceKind::FunctionCall, name)?;
    let signature = routine_signature_for_reference(
        typed,
        resolved,
        reference_id,
        origin_for(resolved, syntax_id),
    )?;
    let signature = if type_args.is_empty() {
        signature
    } else {
        instantiate_signature_with_explicit_type_args(
            typed,
            resolved,
            context,
            &signature,
            name,
            type_args,
            origin_for(resolved, syntax_id),
        )?
    };
    let (signature, arg_effect) = check_call_arguments(
        typed,
        resolved,
        context,
        &signature,
        args,
        name,
        origin_for(resolved, syntax_id),
        true,
        true,
        context.processor_task_call == Some(syntax_id),
    )?;
    let call_effect = merge_recoverable_effects(
        typed,
        origin_for(resolved, syntax_id),
        "function call",
        [
            arg_effect,
            signature
                .error_type
                .map(|error_type| RecoverableCallEffect { error_type }),
        ],
    )?;
    // Keep the post-inference signature at the call site. Processor-boundary
    // validation runs after ordinary call typing and must inspect the concrete
    // parameter types, including parameters supplied by omitted defaults.
    typed.record_call_signature(syntax_id, signature.clone());
    let return_type = signature.return_type;
    if let Some(return_type) = return_type {
        let typed_reference = typed
            .typed_reference_mut(reference_id)
            .ok_or_else(|| internal_error("typed call reference disappeared", None))?;
        typed_reference.resolved_type = Some(return_type);
        typed_reference.recoverable_effect = call_effect;
        typed.record_node_type(syntax_id, context.source_unit_id, return_type)?;
        if let Some(effect) = call_effect {
            typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
            typed.record_reference_recoverable_effect(reference_id, effect)?;
        }
    } else if let Some(effect) = call_effect {
        typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
        typed.record_reference_recoverable_effect(reference_id, effect)?;
    }
    Ok(TypedExpr::maybe_value(return_type).with_optional_effect(call_effect))
}

pub(crate) fn type_dot_intrinsic_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    name: &str,
    args: &[AstNode],
    syntax_id: Option<SyntaxNodeId>,
    expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    let Some(syntax_id) = syntax_id else {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("dot intrinsic '.{name}(...)' does not retain a syntax id"),
        ));
    };
    let origin = origin_for(resolved, syntax_id);
    let entry = select_intrinsic(IntrinsicSurface::DotRootCall, name).map_err(|error| {
        let message = match error.kind {
            IntrinsicSelectionErrorKind::UnknownName => {
                fol_intrinsics::unknown_intrinsic_message(error.surface, name)
            }
            IntrinsicSelectionErrorKind::WrongSurface => {
                format!("'.{name}(...)' is reserved for a different intrinsic surface")
            }
        };
        match origin.clone() {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        }
    })?;

    let typed_expr = match comparison_operand_contract(entry) {
        Some(ComparisonOperandContract::EqualityScalar) => {
            type_comparison_intrinsic(typed, resolved, context, entry, args, syntax_id)?
        }
        Some(ComparisonOperandContract::OrderedScalar) => {
            type_comparison_intrinsic(typed, resolved, context, entry, args, syntax_id)?
        }
        None => match boolean_operand_contract(entry) {
            Some(BooleanOperandContract::BoolScalar) => {
                type_boolean_intrinsic(typed, resolved, context, entry, args, syntax_id)?
            }
            None => match query_operand_contract(entry) {
                Some(QueryOperandContract::LengthQueryable) => {
                    type_query_intrinsic(typed, resolved, context, entry, args, syntax_id)?
                }
                None if entry.name == "echo" => type_echo_intrinsic(
                    typed,
                    resolved,
                    context,
                    entry,
                    args,
                    syntax_id,
                    expected_type,
                )?,
                None => {
                    let message = if entry.availability != fol_intrinsics::IntrinsicAvailability::V1
                    {
                        fol_intrinsics::wrong_version_message(
                            entry,
                            fol_intrinsics::IntrinsicAvailability::V1,
                        )
                    } else {
                        fol_intrinsics::unsupported_intrinsic_message(entry)
                    };
                    return Err(match origin {
                        Some(origin) => TypecheckError::with_origin(
                            TypecheckErrorKind::Unsupported,
                            message,
                            origin,
                        ),
                        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
                    });
                }
            },
        },
    };

    typed.record_node_intrinsic(syntax_id, context.source_unit_id, entry.id)?;
    Ok(typed_expr)
}

fn type_comparison_intrinsic(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: SyntaxNodeId,
) -> Result<TypedExpr, TypecheckError> {
    let origin = origin_for(resolved, syntax_id);
    if args.len() != 2 {
        return Err(match origin {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
            ),
        });
    }

    let left_raw = type_node(typed, resolved, context, &args[0])?;
    let left_expr = plain_value_expr(
        typed,
        context,
        left_raw,
        node_origin(resolved, &args[0]),
        "plain use of an errorful expression",
    )?;
    let right_raw = type_node(typed, resolved, context, &args[1])?;
    let right_expr = plain_value_expr(
        typed,
        context,
        right_raw,
        node_origin(resolved, &args[1]),
        "plain use of an errorful expression",
    )?;

    let left_type = left_expr.required_value("left intrinsic operand does not have a type")?;
    let right_type = right_expr.required_value("right intrinsic operand does not have a type")?;
    let left_apparent = apparent_type_id(typed, left_type)?;
    let right_apparent = apparent_type_id(typed, right_type)?;
    let merged_effect = merge_recoverable_effects(
        typed,
        node_origin(resolved, &args[0]).or_else(|| node_origin(resolved, &args[1])),
        "intrinsic comparison",
        [left_expr.recoverable_effect, right_expr.recoverable_effect],
    )?;

    let valid = left_apparent == right_apparent
        && match comparison_operand_contract(entry) {
            Some(ComparisonOperandContract::EqualityScalar) => {
                super::helpers::is_equality_type(typed, left_apparent)
            }
            Some(ComparisonOperandContract::OrderedScalar) => {
                super::helpers::is_ordered_type(typed, left_apparent)
            }
            None => false,
        };
    if !valid {
        let actual = format!(
            "'{}' and '{}'",
            describe_type(typed, left_type),
            describe_type(typed, right_type)
        );
        let message = wrong_type_family_message(
            entry,
            comparison_operand_contract(entry)
                .expect("comparison intrinsics should retain an operand contract")
                .expected_operands(),
            &actual,
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        });
    }

    typed.record_node_type(
        syntax_id,
        context.source_unit_id,
        typed.builtin_types().bool_,
    )?;
    Ok(TypedExpr::value(typed.builtin_types().bool_).with_optional_effect(merged_effect))
}

fn type_boolean_intrinsic(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: SyntaxNodeId,
) -> Result<TypedExpr, TypecheckError> {
    use crate::CheckedType;
    let origin = origin_for(resolved, syntax_id);
    if args.len() != 1 {
        return Err(match origin {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
            ),
        });
    }

    let operand_raw = type_node(typed, resolved, context, &args[0])?;
    let operand_expr = plain_value_expr(
        typed,
        context,
        operand_raw,
        node_origin(resolved, &args[0]),
        "plain use of an errorful intrinsic operand",
    )?;
    let operand_type = operand_expr.required_value("intrinsic operand does not have a type")?;
    let operand_apparent = apparent_type_id(typed, operand_type)?;

    if !matches!(
        typed.type_table().get(operand_apparent),
        Some(CheckedType::Builtin(crate::BuiltinType::Bool))
    ) {
        let actual = format!("'{}'", describe_type(typed, operand_type));
        let message = wrong_type_family_message(
            entry,
            BooleanOperandContract::BoolScalar.expected_operands(),
            &actual,
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        });
    }

    typed.record_node_type(
        syntax_id,
        context.source_unit_id,
        typed.builtin_types().bool_,
    )?;
    Ok(TypedExpr::value(typed.builtin_types().bool_)
        .with_optional_effect(operand_expr.recoverable_effect))
}

fn type_query_intrinsic(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: SyntaxNodeId,
) -> Result<TypedExpr, TypecheckError> {
    use crate::CheckedType;
    let origin = origin_for(resolved, syntax_id);
    if args.len() != 1 {
        return Err(match origin {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
            ),
        });
    }

    let operand_raw = type_node(typed, resolved, context, &args[0])?;
    let operand_expr = plain_value_expr(
        typed,
        context,
        operand_raw,
        node_origin(resolved, &args[0]),
        "plain use of an errorful intrinsic operand",
    )?;
    let operand_type = operand_expr.required_value("intrinsic operand does not have a type")?;
    let operand_apparent = apparent_type_id(typed, operand_type)?;

    let valid = matches!(
        typed.type_table().get(operand_apparent),
        Some(CheckedType::Builtin(crate::BuiltinType::Str))
            | Some(CheckedType::Array { .. })
            | Some(CheckedType::Vector { .. })
            | Some(CheckedType::Sequence { .. })
            | Some(CheckedType::Set { .. })
            | Some(CheckedType::Map { .. })
    );
    if !valid {
        let actual = format!("'{}'", describe_type(typed, operand_type));
        let message = wrong_type_family_message(
            entry,
            QueryOperandContract::LengthQueryable.expected_operands(),
            &actual,
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        });
    }

    if super::bindings::ownership_moves_on_transfer(typed, operand_type)
        && matches!(strip_comments(&args[0]), AstNode::FieldAccess { .. })
    {
        return Err(node_origin(resolved, &args[0]).map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::Ownership,
                    "'.len(...)' through a move-only field projection is not supported in V3; length observation must not partially move its receiver",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::Ownership,
                    "'.len(...)' through a move-only field projection is not supported in V3; length observation must not partially move its receiver",
                    origin,
                )
            },
        ));
    }

    let core_dynamic_length_query = typed.capability_model()
        == crate::TypecheckCapabilityModel::Core
        && matches!(
            typed.type_table().get(operand_apparent),
            Some(CheckedType::Builtin(crate::BuiltinType::Str))
                | Some(CheckedType::Vector { .. })
                | Some(CheckedType::Sequence { .. })
                | Some(CheckedType::Set { .. })
                | Some(CheckedType::Map { .. })
        );
    if core_dynamic_length_query {
        let message = format!(
            "'.len(...)' over heap-backed strings and containers requires 'fol_model = memo' or 'std'; current artifact model is '{}'",
            typed.capability_model().as_str()
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        });
    }

    typed.record_node_type(syntax_id, context.source_unit_id, typed.builtin_types().int)?;
    Ok(TypedExpr::value(typed.builtin_types().int)
        .with_optional_effect(operand_expr.recoverable_effect))
}

fn type_echo_intrinsic(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: SyntaxNodeId,
    _expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    let origin = origin_for(resolved, syntax_id);
    if typed.capability_model() != crate::TypecheckCapabilityModel::Std {
        let message = format!(
            "'.echo(...)' requires hosted std support; declare build.add_dep({{ alias = \"std\", source = \"internal\", target = \"standard\" }}) and use 'fol_model = memo' (current artifact model is '{}')",
            typed.capability_model().as_str()
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        });
    }
    if args.len() != 1 {
        return Err(match origin {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                wrong_arity_message(entry, args.len()),
            ),
        });
    }

    let operand_raw = type_node(typed, resolved, context, &args[0])?;
    let operand_expr = plain_value_expr(
        typed,
        context,
        operand_raw,
        node_origin(resolved, &args[0]),
        "plain use of an errorful intrinsic operand",
    )?;
    let operand_type = operand_expr.required_value("intrinsic operand does not have a type")?;

    super::bindings::track_value_transfer(typed, resolved, context, Some(&args[0]), operand_type)?;
    typed.record_node_type(syntax_id, context.source_unit_id, operand_type)?;
    Ok(TypedExpr::value(operand_type).with_optional_effect(operand_expr.recoverable_effect))
}

pub(crate) fn type_report_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    args: &[AstNode],
    syntax_id: Option<SyntaxNodeId>,
) -> Result<TypedExpr, TypecheckError> {
    let origin = syntax_id.and_then(|syntax_id| origin_for(resolved, syntax_id));
    let Some(expected) = context.routine_error_type else {
        return Err(TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            "report requires a declared routine error type in V1",
            origin.unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: "report".len(),
            }),
        ));
    };

    if args.len() != 1 {
        return Err(TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            format!(
                "report expects exactly 1 value in V1 but got {}",
                args.len()
            ),
            origin.unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: "report".len(),
            }),
        ));
    }

    let report_context = TypeContext {
        repeating_loop_scope: None,
        ..context
    };
    let report_raw =
        type_node_with_expectation(typed, resolved, report_context, &args[0], Some(expected))?;
    let actual = plain_value_expr(
        typed,
        report_context,
        report_raw,
        node_origin(resolved, &args[0]),
        "report expression",
    )?
    .required_value("report expression does not have a type")
    .map_err(|_| {
        TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            "report expression does not have a type",
            origin.clone().unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: "report".len(),
            }),
        )
    })?;
    ensure_assignable(typed, expected, actual, "report".to_string(), origin)?;
    // Reporting exits the routine through its error path, transferring the
    // reported value just like a return transfers its result.
    super::bindings::track_value_transfer(typed, resolved, report_context, Some(&args[0]), actual)?;
    Ok(TypedExpr::value(typed.builtin_types().never))
}

fn type_panic_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    args: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    let mut arg_effects = Vec::new();
    for arg in args {
        let arg_raw = type_node(typed, resolved, context, arg)?;
        let expr = plain_value_expr(
            typed,
            context,
            arg_raw,
            node_origin(resolved, arg),
            "panic argument",
        )?;
        let _ = expr.value_type;
        arg_effects.push(expr.recoverable_effect);
    }
    let merged = merge_recoverable_effects(typed, None, "panic call", arg_effects)?;
    Ok(TypedExpr::value(typed.builtin_types().never).with_optional_effect(merged))
}

pub(crate) fn type_keyword_intrinsic_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: Option<SyntaxNodeId>,
) -> Result<TypedExpr, TypecheckError> {
    if let Some(syntax_id) = syntax_id {
        typed.record_node_intrinsic(syntax_id, context.source_unit_id, entry.id)?;
    }

    match entry.name {
        "panic" => type_panic_call(typed, resolved, context, args),
        "check" => type_check_call(typed, resolved, context, entry, args, syntax_id),
        other => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("unsupported keyword intrinsic dispatch '{other}(...)'"),
        )),
    }
}

fn type_check_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    entry: &fol_intrinsics::IntrinsicEntry,
    args: &[AstNode],
    syntax_id: Option<SyntaxNodeId>,
) -> Result<TypedExpr, TypecheckError> {
    let origin = syntax_id.and_then(|id| origin_for(resolved, id));
    if args.len() != 1 {
        return Err(origin.clone().map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    wrong_arity_message(entry, args.len()),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    wrong_arity_message(entry, args.len()),
                    origin,
                )
            },
        ));
    }

    let observed = type_node(typed, resolved, observe_context(context), &args[0])?;
    if observed.recoverable_effect.is_none() {
        let message = if observed
            .value_type
            .map(|type_id| is_error_shell_type(typed, type_id))
            .transpose()?
            .unwrap_or(false)
        {
            "check(...) inspects routine call results with '/ ErrorType', not err[...] shell values in V1"
        } else {
            "check(...) requires a routine call result with '/ ErrorType' in V1"
        };
        return Err(node_origin(resolved, &args[0]).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
        ));
    }

    if let Some(syntax_id) = syntax_id {
        typed.record_node_type(
            syntax_id,
            context.source_unit_id,
            typed.builtin_types().bool_,
        )?;
    }
    Ok(TypedExpr::value(typed.builtin_types().bool_))
}

pub(crate) fn type_qualified_function_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    path: &QualifiedPath,
    args: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    let syntax_id = path.syntax_id().ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "qualified function call '{}' does not retain a syntax id",
                path.joined()
            ),
        )
    })?;
    let reference_id = find_reference_by_syntax(
        resolved,
        syntax_id,
        ReferenceKind::QualifiedFunctionCall,
        &path.joined(),
    )?;
    let signature = routine_signature_for_reference(
        typed,
        resolved,
        reference_id,
        origin_for(resolved, syntax_id),
    )?;
    let (signature, arg_effect) = check_call_arguments(
        typed,
        resolved,
        context,
        &signature,
        args,
        &path.joined(),
        origin_for(resolved, syntax_id),
        true,
        true,
        false,
    )?;
    let call_effect = merge_recoverable_effects(
        typed,
        origin_for(resolved, syntax_id),
        "qualified function call",
        [
            arg_effect,
            signature
                .error_type
                .map(|error_type| RecoverableCallEffect { error_type }),
        ],
    )?;
    if let Some(return_type) = signature.return_type {
        let typed_reference = typed
            .typed_reference_mut(reference_id)
            .ok_or_else(|| internal_error("typed qualified call reference disappeared", None))?;
        typed_reference.resolved_type = Some(return_type);
        typed_reference.recoverable_effect = call_effect;
        typed.record_node_type(syntax_id, context.source_unit_id, return_type)?;
        if let Some(effect) = call_effect {
            typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
            typed.record_reference_recoverable_effect(reference_id, effect)?;
        }
    }
    Ok(TypedExpr::maybe_value(signature.return_type).with_optional_effect(call_effect))
}

pub(crate) fn type_method_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    object: &AstNode,
    method: &str,
    args: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    let mutex_receiver = (method == "lock" || method == "unlock")
        .then(|| {
            let AstNode::Identifier {
                name,
                syntax_id: Some(syntax_id),
            } = strip_comments(object)
            else {
                return None;
            };
            let mutex_symbol = resolved
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(*syntax_id)
                        && reference.kind == ReferenceKind::Identifier
                })
                .and_then(|reference| reference.resolved)?;
            typed
                .typed_symbol(mutex_symbol)
                .is_some_and(|symbol| symbol.is_mutex)
                .then_some((name.as_str(), mutex_symbol))
        })
        .flatten();
    if let Some((name, mutex_symbol)) = mutex_receiver {
        if context.inside_deferred_block {
            return Err(unsupported_node_surface(
                resolved,
                node,
                format!(
                    "mutex .{method}() is not allowed inside dfr/edf in V3; delayed mutex guard effects are not modeled"
                ),
            ));
        }
        if !typed.capability_model().supports_processor() {
            return Err(unsupported_node_surface(
                resolved,
                node,
                "mutex operations require hosted std support; declare the bundled internal standard dependency",
            ));
        }
        if !args.is_empty() {
            return Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!("mutex .{method}() does not accept arguments"),
            ));
        }
        let _ = type_node(
            typed,
            resolved,
            TypeContext {
                allow_mutex_handle: true,
                ..context
            },
            object,
        )?;
        let origin = node_origin(resolved, node)
            .or_else(|| node_origin(resolved, object))
            .ok_or_else(|| internal_error("mutex operation lost its syntax origin", None))?;
        if method == "lock" {
            let guard = crate::ActiveMutexGuard {
                scope: context.scope_id,
                origin: origin.clone(),
            };
            if let Some(active) = typed.register_mutex_guard(mutex_symbol, guard) {
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    format!("mutex parameter '{name}' is already locked"),
                    origin,
                )
                .with_related_origin(active.origin, "mutex lock acquired here"));
            }
        } else if typed
            .release_mutex_guard(mutex_symbol, context.scope_id)
            .is_none()
        {
            let message = if typed.active_mutex_guard(mutex_symbol).is_some() {
                format!(
                    "mutex parameter '{name}' must be unlocked in the lexical scope that acquired it"
                )
            } else {
                format!("mutex parameter '{name}' is not locked")
            };
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                message,
                origin,
            ));
        }
        return Ok(TypedExpr::none());
    }
    let receiver_raw = type_node(typed, resolved, context, object)?;
    let receiver_expr = plain_value_expr(
        typed,
        context,
        receiver_raw,
        node_origin(resolved, object),
        format!("method receiver for '{method}'"),
    )?;
    let object_type = receiver_expr.required_value(format!(
        "method receiver for '{method}' does not have a type"
    ))?;
    let origin = node_origin(resolved, node).or_else(|| node_origin(resolved, object));
    let signature =
        routine_signature_for_method(typed, method, object_type, origin.clone(), node.syntax_id())?;
    // Method syntax is value-receiver sugar: the receiver is the first value
    // passed to the routine. Record that transfer before checking the explicit
    // arguments so the same move-only binding cannot be used twice in one call.
    super::bindings::track_value_transfer(typed, resolved, context, Some(object), object_type)?;
    let (signature, arg_effect) = check_call_arguments(
        typed,
        resolved,
        context,
        &signature,
        args,
        method,
        origin.clone(),
        true,
        true,
        false,
    )?;
    let merged = merge_recoverable_effects(
        typed,
        origin,
        "method call",
        [
            receiver_expr.recoverable_effect,
            arg_effect,
            signature
                .error_type
                .map(|error_type| RecoverableCallEffect { error_type }),
        ],
    )?;
    if let Some(syntax_id) = node.syntax_id() {
        if let Some(return_type) = signature.return_type {
            typed.record_node_type(syntax_id, context.source_unit_id, return_type)?;
        }
        if let Some(effect) = merged {
            typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
        }
    }
    Ok(TypedExpr::maybe_value(signature.return_type).with_optional_effect(merged))
}

pub(crate) fn find_reference_by_syntax(
    resolved: &ResolvedProgram,
    syntax_id: SyntaxNodeId,
    kind: ReferenceKind,
    display_name: &str,
) -> Result<ReferenceId, TypecheckError> {
    resolved
        .references
        .iter_with_ids()
        .find(|(_, reference)| reference.syntax_id == Some(syntax_id) && reference.kind == kind)
        .map(|(reference_id, _)| reference_id)
        .ok_or_else(|| {
            TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                format!("reference '{display_name}' is missing from resolver output"),
                origin_for(resolved, syntax_id).unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: display_name.len(),
                }),
            )
        })
}

pub(crate) fn routine_signature_for_reference(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    reference_id: ReferenceId,
    origin: Option<SyntaxOrigin>,
) -> Result<RoutineType, TypecheckError> {
    let symbol_id = resolved
        .reference(reference_id)
        .and_then(|reference| reference.resolved)
        .ok_or_else(|| {
            TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                "call reference lost its resolved routine symbol",
                origin.clone().unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: 1,
                }),
            )
        })?;
    routine_signature_for_symbol(typed, resolved, symbol_id, origin)
}

fn routine_signature_for_symbol(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    symbol_id: SymbolId,
    origin: Option<SyntaxOrigin>,
) -> Result<RoutineType, TypecheckError> {
    use crate::CheckedType;
    let type_id = symbol_type(typed, resolved, symbol_id, origin.clone())?;
    match typed.type_table().get(type_id) {
        Some(CheckedType::Routine(signature)) => Ok(signature.clone()),
        _ => Err(TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            format!("resolved routine symbol {} is not callable", symbol_id.0),
            origin.unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: 1,
            }),
        )),
    }
}

fn routine_signature_for_method(
    typed: &mut TypedProgram,
    method: &str,
    object_type: crate::CheckedTypeId,
    origin: Option<SyntaxOrigin>,
    call_syntax_id: Option<SyntaxNodeId>,
) -> Result<RoutineType, TypecheckError> {
    let mut matches: Vec<(SymbolId, RoutineType)> = Vec::new();
    // Candidates matched by unifying a *generic* receiver template against the
    // concrete object. Two distinct generic types with the same structure
    // (`Box(T){value:T}` and `Cup(T){value:T}`) instantiate to the identical
    // structural record, so a value of that shape genuinely cannot say which
    // base it came from — nominal identity of generic instantiations is not yet
    // tracked. We detect that case below and gate it with an honest diagnostic
    // rather than silently choosing one conformer.
    let mut generic_receiver_matches = 0usize;

    let candidate_ids = typed
        .resolved()
        .symbols
        .iter_with_ids()
        .filter_map(|(symbol_id, symbol)| {
            (symbol.kind == SymbolKind::Routine && symbol.name == method).then_some(symbol_id)
        })
        .collect::<Vec<_>>();

    for symbol_id in candidate_ids {
        let receiver_type = typed
            .typed_symbol(symbol_id)
            .and_then(|symbol| symbol.receiver_type);
        let Some(receiver_type) = receiver_type else {
            continue;
        };
        if receiver_type == object_type {
            matches.push((
                symbol_id,
                routine_signature_for_symbol(typed, typed.resolved(), symbol_id, origin.clone())?,
            ));
            continue;
        }
        // Try to unify the routine's generic receiver template against the
        // concrete object type. If it unifies, bind the routine generics and
        // monomorphize the signature here so downstream argument checking
        // sees a fully-concrete signature.
        let signature =
            routine_signature_for_symbol(typed, typed.resolved(), symbol_id, origin.clone())?;
        if signature.generic_params.is_empty() {
            continue;
        }
        let Some(bindings) = crate::decls::unify_receiver_with_object(
            typed,
            receiver_type,
            object_type,
            &signature.generic_params,
        ) else {
            continue;
        };
        let instantiated =
            instantiate_generic_signature(typed, &signature, &bindings, method, origin.clone())?;
        matches.push((symbol_id, instantiated));
        generic_receiver_matches += 1;
    }

    // Constrained generic receivers: `fun measure(T: sized)(thing: T)` may
    // call any routine the constraint standard requires. The concrete callee
    // is only known after monomorphization, so record the call site as a
    // deferred constraint call and type it from the requirement signature.
    if matches.is_empty() {
        if let Some(CheckedType::Declared {
            kind: crate::DeclaredTypeKind::GenericParameter,
            symbol: param_symbol,
            ..
        }) = typed.type_table().get(object_type).cloned()
        {
            // A generic parameter's bound can be recorded on more than one
            // typed symbol (the declaring routine/type and the parameter's own
            // self-entry), so dedupe before turning each constraint into a
            // constraint-call candidate; otherwise the same requirement would
            // register twice and read as ambiguous. Each constraint carries the
            // standard's type arguments so a generic standard's own parameters
            // (`fun fetch(): Item` on `Holder[int]`) are substituted to the
            // instantiation type before the call site is typed.
            let constraints = typed
                .all_typed_symbols()
                .flat_map(|typed_symbol| typed_symbol.generic_constraints.get(&param_symbol))
                .flatten()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>();
            for constraint in constraints {
                let Some(standard) = typed.typed_standard(constraint.standard).cloned() else {
                    continue;
                };
                // Bind the standard's own generic parameters to the constraint
                // arguments; empty for a non-generic standard.
                let bindings: BTreeMap<SymbolId, CheckedTypeId> =
                    if standard.generic_params.len() == constraint.args.len() {
                        standard
                            .generic_params
                            .iter()
                            .copied()
                            .zip(constraint.args.iter().copied())
                            .collect()
                    } else {
                        BTreeMap::new()
                    };
                for requirement in &standard.required_routines {
                    if requirement.name != method {
                        continue;
                    }
                    let params = requirement
                        .params
                        .iter()
                        .map(|type_id| {
                            crate::decls::substitute_generic_checked_type(
                                typed, *type_id, &bindings, None,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let return_type = requirement
                        .return_type
                        .map(|type_id| {
                            crate::decls::substitute_generic_checked_type(
                                typed, type_id, &bindings, None,
                            )
                        })
                        .transpose()?;
                    let error_type = requirement
                        .error_type
                        .map(|type_id| {
                            crate::decls::substitute_generic_checked_type(
                                typed, type_id, &bindings, None,
                            )
                        })
                        .transpose()?;
                    let param_names = (0..params.len())
                        .map(|index| format!("arg{index}"))
                        .collect::<Vec<_>>();
                    let param_defaults = vec![None; params.len()];
                    let signature = RoutineType {
                        generic_params: Vec::new(),
                        generic_constraints: BTreeMap::new(),
                        param_names,
                        param_defaults,
                        variadic_index: None,
                        mutex_params: BTreeSet::new(),
                        params,
                        return_type,
                        error_type,
                    };
                    if let Some(syntax_id) = call_syntax_id {
                        typed.record_constraint_call_site(syntax_id);
                    }
                    matches.push((requirement.symbol_id, signature));
                }
            }
        }
    }

    // Fallback: the conformer may inherit this method from a standard
    // default body. We only look for defaults when no direct receiver
    // routine was found, so explicit conformer overrides always win.
    if matches.is_empty() {
        if let Some(type_symbol_id) = crate::decls::conformance_subject_symbol(typed, object_type) {
            if let Some(conformance) = typed.typed_conformance(type_symbol_id).cloned() {
                for standard_symbol_id in conformance.standard_symbol_ids {
                    let Some(standard) = typed.typed_standard(standard_symbol_id).cloned() else {
                        continue;
                    };
                    for requirement in &standard.required_routines {
                        if !requirement.has_default_body || requirement.name != method {
                            continue;
                        }
                        // Surface the default body as a method signature.
                        // FOL receiver routines keep the receiver separate
                        // from the explicit `params` list, so we mirror
                        // that shape: `params` stays exactly as declared
                        // in the standard and the call-site receiver is
                        // implicit.
                        let param_names = (0..requirement.params.len())
                            .map(|index| format!("arg{index}"))
                            .collect::<Vec<_>>();
                        let param_defaults = vec![None; requirement.params.len()];
                        let signature = RoutineType {
                            generic_params: Vec::new(),
                            generic_constraints: BTreeMap::new(),
                            param_names,
                            param_defaults,
                            variadic_index: None,
                            mutex_params: BTreeSet::new(),
                            params: requirement.params.clone(),
                            return_type: requirement.return_type,
                            error_type: requirement.error_type,
                        };
                        matches.push((requirement.symbol_id, signature));
                    }
                }
            }
        }
    }

    match matches.len() {
        1 => {
            let (chosen_symbol, signature) = matches.remove(0);
            if let Some(syntax_id) = call_syntax_id {
                typed.record_method_call_target(syntax_id, chosen_symbol);
            }
            Ok(signature)
        }
        0 => Err(origin.clone().map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("method '{method}' is not available for the receiver type in V1"),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    format!("method '{method}' is not available for the receiver type in V1"),
                    origin,
                )
            },
        )),
        _ => {
            let _ = generic_receiver_matches;
            let message = format!("method '{method}' is ambiguous for the receiver type");
            Err(match origin {
                Some(origin) => {
                    TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
                }
                None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
            })
        }
    }
}

pub(crate) fn check_call_arguments(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    signature: &RoutineType,
    args: &[AstNode],
    callee: &str,
    origin: Option<SyntaxOrigin>,
    allow_named: bool,
    allow_defaults: bool,
    processor_task_target: bool,
) -> Result<(RoutineType, Option<RecoverableCallEffect>), TypecheckError> {
    let ordered_args = bind_call_arguments(
        signature,
        args,
        callee,
        origin.clone(),
        allow_named,
        allow_defaults,
    )?;
    validate_deferred_mutex_argument_forwarding(
        typed,
        resolved,
        context,
        signature,
        &ordered_args,
        callee,
    )?;
    validate_mutex_argument_forwarding(
        typed,
        resolved,
        signature,
        &ordered_args,
        callee,
        origin.clone(),
        processor_task_target,
    )?;
    validate_call_site_borrows(
        typed,
        resolved,
        signature,
        &ordered_args,
        callee,
        origin.clone(),
    )?;

    let mut generic_bindings = BTreeMap::new();
    let mut arg_effects = Vec::new();
    for (param_index, (expected, arg)) in
        signature.params.iter().zip(ordered_args.iter()).enumerate()
    {
        match arg {
            BoundCallArg::Explicit(arg) | BoundCallArg::VariadicUnpack(arg) => {
                let contains_generics =
                    crate::decls::checked_type_contains_generic_param(typed, *expected);
                let forwards_mutex_handle = signature.mutex_params.contains(&param_index)
                    && argument_is_direct_mutex_handle(typed, resolved, arg);
                let argument_context = TypeContext {
                    allow_mutex_handle: forwards_mutex_handle,
                    ..context
                };
                let actual_expr = if contains_generics {
                    type_node(typed, resolved, argument_context, arg)
                } else {
                    type_node_with_expectation(
                        typed,
                        resolved,
                        argument_context,
                        arg,
                        Some(*expected),
                    )
                }
                .map_err(|error| {
                    origin
                        .clone()
                        .or_else(|| node_origin(resolved, arg))
                        .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                })?;
                if !contains_generics {
                    reject_recoverable_error_shell_conversion(
                        typed,
                        *expected,
                        &actual_expr,
                        origin.clone().or_else(|| node_origin(resolved, arg)),
                        format!("call to '{callee}'"),
                    )?;
                }
                let actual_expr = plain_value_expr(
                    typed,
                    context,
                    actual_expr,
                    origin.clone().or_else(|| node_origin(resolved, arg)),
                    format!("call to '{callee}'"),
                )?;
                let actual = actual_expr
                    .required_value(format!("argument for '{callee}' does not have a type"))?;
                arg_effects.push(actual_expr.recoverable_effect);
                validate_eventual_call_argument(
                    typed,
                    resolved,
                    arg,
                    actual,
                    contains_generics,
                    callee,
                    origin.clone(),
                )?;
                if contains_generics {
                    infer_generic_bindings_from_argument(
                        typed,
                        *expected,
                        actual,
                        &mut generic_bindings,
                        format!("call to '{callee}'"),
                        origin.clone().or_else(|| node_origin(resolved, arg)),
                    )?;
                } else {
                    ensure_assignable(
                        typed,
                        *expected,
                        apparent_type_id(typed, actual)?,
                        format!("call to '{callee}'"),
                        origin.clone(),
                    )?;
                }
                let extracts_sender = matches!(
                    (
                        typed.type_table().get(*expected),
                        typed.type_table().get(actual),
                    ),
                    (
                        Some(CheckedType::ChannelSender {
                            element_type: expected,
                        }),
                        Some(CheckedType::Channel {
                            element_type: actual,
                        }),
                    ) if expected == actual
                );
                if !extracts_sender && !forwards_mutex_handle {
                    super::bindings::track_value_transfer(
                        typed,
                        resolved,
                        context,
                        Some(arg),
                        actual,
                    )?;
                }
            }
            BoundCallArg::VariadicPack(args) => {
                let element_type = match typed.type_table().get(*expected) {
                    Some(crate::CheckedType::Sequence { element_type }) => *element_type,
                    _ => {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::Internal,
                            format!("variadic call to '{callee}' lost its sequence parameter type"),
                        ))
                    }
                };
                for arg in args {
                    let contains_generics =
                        crate::decls::checked_type_contains_generic_param(typed, element_type);
                    let actual_expr = if contains_generics {
                        type_node(typed, resolved, context, arg)
                    } else {
                        type_node_with_expectation(
                            typed,
                            resolved,
                            context,
                            arg,
                            Some(element_type),
                        )
                    }
                    .map_err(|error| {
                        origin
                            .clone()
                            .or_else(|| node_origin(resolved, arg))
                            .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                    })?;
                    if !contains_generics {
                        reject_recoverable_error_shell_conversion(
                            typed,
                            element_type,
                            &actual_expr,
                            origin.clone().or_else(|| node_origin(resolved, arg)),
                            format!("call to '{callee}'"),
                        )?;
                    }
                    let actual_expr = plain_value_expr(
                        typed,
                        context,
                        actual_expr,
                        origin.clone().or_else(|| node_origin(resolved, arg)),
                        format!("call to '{callee}'"),
                    )?;
                    let actual = actual_expr.required_value(format!(
                        "variadic argument for '{callee}' does not have a type"
                    ))?;
                    arg_effects.push(actual_expr.recoverable_effect);
                    validate_eventual_call_argument(
                        typed,
                        resolved,
                        arg,
                        actual,
                        contains_generics,
                        callee,
                        origin.clone(),
                    )?;
                    if contains_generics {
                        infer_generic_bindings_from_argument(
                            typed,
                            element_type,
                            actual,
                            &mut generic_bindings,
                            format!("call to '{callee}'"),
                            origin.clone().or_else(|| node_origin(resolved, arg)),
                        )?;
                    } else {
                        ensure_assignable(
                            typed,
                            element_type,
                            apparent_type_id(typed, actual)?,
                            format!("call to '{callee}'"),
                            origin.clone(),
                        )?;
                    }
                    super::bindings::track_value_transfer(
                        typed,
                        resolved,
                        context,
                        Some(arg),
                        actual,
                    )?;
                }
            }
            BoundCallArg::Default => {}
        }
    }

    let instantiated =
        instantiate_generic_signature(typed, signature, &generic_bindings, callee, origin.clone())?;
    Ok((
        instantiated,
        merge_recoverable_effects(typed, origin, "call arguments", arg_effects)?,
    ))
}

fn validate_eventual_call_argument(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    arg: &AstNode,
    actual: CheckedTypeId,
    generic_parameter: bool,
    callee: &str,
    call_origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let origin = node_origin(resolved, arg).or(call_origin);
    super::bindings::reject_nested_eventual_value(
        typed,
        actual,
        origin.clone(),
        format!("argument to '{callee}'"),
    )?;
    if generic_parameter
        && matches!(
            typed.type_table().get(actual),
            Some(CheckedType::Eventual { .. })
        )
    {
        let message = format!(
            "call to '{callee}' cannot pass an internal eventual through a generic parameter in V3; await it before the call"
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::Ownership, message),
        });
    }
    Ok(())
}

fn validate_call_site_borrows(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    signature: &RoutineType,
    args: &[BoundCallArg<'_>],
    callee: &str,
    call_origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let mut transient_sources = vec![None; args.len()];

    for (index, (expected, bound)) in signature.params.iter().zip(args.iter()).enumerate() {
        let Some(CheckedType::Borrowed { mutable, .. }) = typed.type_table().get(*expected) else {
            continue;
        };
        let Some(arg) = explicit_bound_arg(bound) else {
            continue;
        };
        if let Some(owner) = borrow_from_owner(resolved, arg) {
            let borrow_origin = node_origin(resolved, arg)
                .or_else(|| call_origin.clone())
                .unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: 1,
                });
            if let Some(conflict) = typed.active_borrow_for_owner(owner) {
                if *mutable || conflict.mutable {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::BorrowConflict,
                        format!(
                            "call to '{callee}' borrows an owner that already has an incompatible active borrow"
                        ),
                        borrow_origin,
                    )
                    .with_related_origin(conflict.origin.clone(), "conflicting borrow created here"));
                }
            }
            transient_sources[index] = Some((owner, borrow_origin, *mutable));
            continue;
        }
        if argument_is_borrow_binding(typed, resolved, arg) {
            continue;
        }
        return Err(node_origin(resolved, arg).or(call_origin.clone()).map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::BorrowConflict,
                    format!(
                        "call to '{callee}' must pass '#owner' or an existing borrow binding to a [bor] parameter"
                    ),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::BorrowConflict,
                    format!(
                        "call to '{callee}' must pass '#owner' or an existing borrow binding to a [bor] parameter"
                    ),
                    origin,
                )
            },
        ));
    }

    for (borrow_index, source) in transient_sources.iter().enumerate() {
        let Some((owner, borrow_origin, _mutable)) = source else {
            continue;
        };
        for (arg_index, bound) in args.iter().enumerate() {
            if arg_index == borrow_index
                || transient_sources[arg_index]
                    .as_ref()
                    .is_some_and(|(other_owner, _, _)| other_owner == owner)
            {
                continue;
            }
            if bound_arg_references_symbol(resolved, bound, *owner) {
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::BorrowConflict,
                    format!(
                        "call to '{callee}' accesses an owner in another argument while it is borrowed for a [bor] parameter"
                    ),
                    node_origin_for_bound_arg(resolved, bound)
                        .or_else(|| call_origin.clone())
                        .unwrap_or_else(|| borrow_origin.clone()),
                )
                .with_related_origin(borrow_origin.clone(), "call-site borrow created here"));
            }
        }
    }

    Ok(())
}

fn validate_deferred_mutex_argument_forwarding(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    signature: &RoutineType,
    args: &[BoundCallArg<'_>],
    callee: &str,
) -> Result<(), TypecheckError> {
    if !context.inside_deferred_block {
        return Ok(());
    }

    for param_index in &signature.mutex_params {
        let Some(arg) = args.get(*param_index).and_then(explicit_bound_arg) else {
            continue;
        };
        if argument_is_direct_mutex_handle(typed, resolved, arg) {
            return Err(unsupported_node_surface(
                resolved,
                arg,
                format!(
                    "mutex handles cannot be forwarded to [mux] parameter {param_index} of '{callee}' inside dfr/edf in V3; delayed mutex guard effects are not modeled"
                ),
            ));
        }
    }

    Ok(())
}

fn validate_mutex_argument_forwarding(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    signature: &RoutineType,
    args: &[BoundCallArg<'_>],
    callee: &str,
    call_origin: Option<SyntaxOrigin>,
    processor_task_target: bool,
) -> Result<(), TypecheckError> {
    let mut forwarded = BTreeMap::new();

    for param_index in &signature.mutex_params {
        let Some(arg) = args.get(*param_index).and_then(explicit_bound_arg) else {
            continue;
        };
        let Some(symbol) = direct_mutex_handle_symbol(typed, resolved, arg) else {
            continue;
        };
        let name = resolved
            .symbol(symbol)
            .map(|symbol| symbol.name.as_str())
            .unwrap_or("<unknown>");
        let argument_origin = node_origin(resolved, arg)
            .or_else(|| call_origin.clone())
            .unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: 1,
            });

        if let Some((first_param, first_origin)) = forwarded.insert(
            symbol,
            (*param_index, argument_origin.clone()),
        ) {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "call to '{callee}' cannot forward mutex handle '{name}' to both [mux] parameter {first_param} and [mux] parameter {param_index}; aliased mutex parameters can self-deadlock"
                ),
                argument_origin,
            )
            .with_related_origin(first_origin, "same mutex handle first forwarded here"));
        }

        if !processor_task_target {
            if let Some(active) = typed.active_mutex_guard(symbol) {
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "call to '{callee}' cannot synchronously forward mutex handle '{name}' while its lock is active; unlock it before the call"
                    ),
                    argument_origin,
                )
                .with_related_origin(active.origin.clone(), "mutex lock acquired here"));
            }
        }
    }

    Ok(())
}

fn explicit_bound_arg<'a>(arg: &'a BoundCallArg<'a>) -> Option<&'a AstNode> {
    match arg {
        BoundCallArg::Explicit(arg) | BoundCallArg::VariadicUnpack(arg) => Some(arg),
        BoundCallArg::Default | BoundCallArg::VariadicPack(_) => None,
    }
}

fn borrow_from_owner(resolved: &ResolvedProgram, arg: &AstNode) -> Option<SymbolId> {
    matches!(
        strip_comments(arg),
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::BorrowFrom,
            ..
        }
    )
    .then(|| super::bindings::borrow_source_symbol(resolved, strip_comments(arg)))?
}

fn argument_is_borrow_binding(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    arg: &AstNode,
) -> bool {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(arg)
    else {
        return false;
    };
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
        .and_then(|symbol| typed.typed_symbol(symbol))
        .and_then(|symbol| symbol.declared_type)
        .and_then(|type_id| typed.type_table().get(type_id))
        .is_some_and(|typ| matches!(typ, CheckedType::Borrowed { .. }))
}

fn argument_is_direct_mutex_handle(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    arg: &AstNode,
) -> bool {
    direct_mutex_handle_symbol(typed, resolved, arg).is_some()
}

fn direct_mutex_handle_symbol(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    arg: &AstNode,
) -> Option<SymbolId> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(arg)
    else {
        return None;
    };
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
        .filter(|symbol| {
            typed
                .typed_symbol(*symbol)
                .is_some_and(|symbol| symbol.is_mutex)
        })
}

fn bound_arg_references_symbol(
    resolved: &ResolvedProgram,
    arg: &BoundCallArg<'_>,
    symbol: SymbolId,
) -> bool {
    match arg {
        BoundCallArg::Explicit(arg) | BoundCallArg::VariadicUnpack(arg) => {
            node_references_symbol(resolved, arg, symbol)
        }
        BoundCallArg::VariadicPack(args) => args
            .iter()
            .any(|arg| node_references_symbol(resolved, arg, symbol)),
        BoundCallArg::Default => false,
    }
}

fn node_references_symbol(resolved: &ResolvedProgram, node: &AstNode, symbol: SymbolId) -> bool {
    let mut syntax_ids = BTreeSet::new();
    super::helpers::collect_syntax_ids(node, &mut syntax_ids);
    resolved.references.iter().any(|reference| {
        reference.resolved == Some(symbol)
            && reference
                .syntax_id
                .is_some_and(|syntax_id| syntax_ids.contains(&syntax_id))
    })
}

fn node_origin_for_bound_arg(
    resolved: &ResolvedProgram,
    arg: &BoundCallArg<'_>,
) -> Option<SyntaxOrigin> {
    match arg {
        BoundCallArg::Explicit(arg) | BoundCallArg::VariadicUnpack(arg) => {
            node_origin(resolved, arg)
        }
        BoundCallArg::VariadicPack(args) => args.iter().find_map(|arg| node_origin(resolved, arg)),
        BoundCallArg::Default => None,
    }
}

fn infer_generic_bindings_from_argument(
    typed: &TypedProgram,
    expected: CheckedTypeId,
    actual: CheckedTypeId,
    bindings: &mut BTreeMap<SymbolId, CheckedTypeId>,
    surface: String,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    // Nominal fast-path: if the parameter is a generic instantiation like
    // `Box[T]` and the argument is `Box[Rect]` (same base declaration), bind
    // the type parameters by unifying the arguments pairwise, BEFORE stripping
    // to the structural record (which would hide the nominal args).
    if let (
        Some(CheckedType::Declared {
            symbol: expected_symbol,
            args: expected_args,
            ..
        }),
        Some(CheckedType::Declared {
            symbol: actual_symbol,
            args: actual_args,
            ..
        }),
    ) = (
        typed.type_table().get(expected).cloned(),
        typed.type_table().get(actual).cloned(),
    ) {
        if !expected_args.is_empty()
            && expected_symbol == actual_symbol
            && expected_args.len() == actual_args.len()
        {
            for (expected_arg, actual_arg) in expected_args.iter().zip(actual_args.iter()) {
                infer_generic_bindings_from_argument(
                    typed,
                    *expected_arg,
                    *actual_arg,
                    bindings,
                    surface.clone(),
                    origin.clone(),
                )?;
            }
            return Ok(());
        }
    }

    let expected = apparent_type_id(typed, expected)?;
    let actual_apparent = apparent_type_id(typed, actual)?;

    match (
        typed.type_table().get(expected),
        typed.type_table().get(actual_apparent),
    ) {
        (
            Some(CheckedType::Declared {
                symbol,
                kind: DeclaredTypeKind::GenericParameter,
                ..
            }),
            _,
        ) => bind_generic_parameter(typed, *symbol, actual, bindings, surface, origin),
        (
            Some(CheckedType::Array {
                element_type: expected_element,
                size: expected_size,
            }),
            Some(CheckedType::Array {
                element_type: actual_element,
                size: actual_size,
            }),
        ) if expected_size == actual_size => infer_generic_bindings_from_argument(
            typed,
            *expected_element,
            *actual_element,
            bindings,
            surface,
            origin,
        ),
        (
            Some(CheckedType::Vector {
                element_type: expected_element,
            }),
            Some(CheckedType::Vector {
                element_type: actual_element,
            }),
        )
        | (
            Some(CheckedType::Sequence {
                element_type: expected_element,
            }),
            Some(CheckedType::Sequence {
                element_type: actual_element,
            }),
        )
        | (
            Some(CheckedType::Optional {
                inner: expected_element,
            }),
            Some(CheckedType::Optional {
                inner: actual_element,
            }),
        ) => infer_generic_bindings_from_argument(
            typed,
            *expected_element,
            *actual_element,
            bindings,
            surface,
            origin,
        ),
        (
            Some(CheckedType::Error {
                inner: expected_inner,
            }),
            Some(CheckedType::Error {
                inner: actual_inner,
            }),
        ) => match (expected_inner, actual_inner) {
            (Some(expected_inner), Some(actual_inner)) => infer_generic_bindings_from_argument(
                typed,
                *expected_inner,
                *actual_inner,
                bindings,
                surface,
                origin,
            ),
            (None, None) => Ok(()),
            _ => ensure_assignable(typed, expected, actual_apparent, surface, origin),
        },
        // An instantiated generic named type (`Box[T]`) is a structural
        // record/entry: recurse field-by-field so the concrete argument
        // (`Box[Rect]`) binds the routine's generic parameters. Without this the
        // parameter type keeps its raw `T` and the call fails to type-check.
        (
            Some(CheckedType::Record {
                fields: expected_fields,
            }),
            Some(CheckedType::Record {
                fields: actual_fields,
            }),
        ) if expected_fields.len() == actual_fields.len() => {
            let pairs = expected_fields
                .iter()
                .map(|(name, expected_field)| {
                    actual_fields
                        .get(name)
                        .map(|actual_field| (*expected_field, *actual_field))
                })
                .collect::<Option<Vec<_>>>();
            match pairs {
                Some(pairs) => {
                    for (expected_field, actual_field) in pairs {
                        infer_generic_bindings_from_argument(
                            typed,
                            expected_field,
                            actual_field,
                            bindings,
                            surface.clone(),
                            origin.clone(),
                        )?;
                    }
                    Ok(())
                }
                None => ensure_assignable(typed, expected, actual_apparent, surface, origin),
            }
        }
        (
            Some(CheckedType::Entry {
                variants: expected_variants,
            }),
            Some(CheckedType::Entry {
                variants: actual_variants,
            }),
        ) if expected_variants.len() == actual_variants.len() => {
            let pairs = expected_variants
                .iter()
                .map(|(name, expected_payload)| {
                    actual_variants
                        .get(name)
                        .map(|actual_payload| (*expected_payload, *actual_payload))
                })
                .collect::<Option<Vec<_>>>();
            match pairs {
                Some(pairs) => {
                    for (expected_payload, actual_payload) in pairs {
                        if let (Some(expected_payload), Some(actual_payload)) =
                            (expected_payload, actual_payload)
                        {
                            infer_generic_bindings_from_argument(
                                typed,
                                expected_payload,
                                actual_payload,
                                bindings,
                                surface.clone(),
                                origin.clone(),
                            )?;
                        }
                    }
                    Ok(())
                }
                None => ensure_assignable(typed, expected, actual_apparent, surface, origin),
            }
        }
        (
            Some(CheckedType::Map {
                key_type: expected_key,
                value_type: expected_value,
            }),
            Some(CheckedType::Map {
                key_type: actual_key,
                value_type: actual_value,
            }),
        ) => {
            let (expected_key, expected_value) = (*expected_key, *expected_value);
            let (actual_key, actual_value) = (*actual_key, *actual_value);
            infer_generic_bindings_from_argument(
                typed,
                expected_key,
                actual_key,
                bindings,
                surface.clone(),
                origin.clone(),
            )?;
            infer_generic_bindings_from_argument(
                typed,
                expected_value,
                actual_value,
                bindings,
                surface,
                origin,
            )
        }
        _ => ensure_assignable(typed, expected, actual_apparent, surface, origin),
    }
}

fn bind_generic_parameter(
    typed: &TypedProgram,
    symbol: SymbolId,
    actual: CheckedTypeId,
    bindings: &mut BTreeMap<SymbolId, CheckedTypeId>,
    surface: String,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    if let Some(bound) = bindings.get(&symbol).copied() {
        ensure_assignable(typed, bound, actual, surface, origin)
    } else {
        bindings.insert(symbol, actual);
        Ok(())
    }
}

/// Substitute a generic routine signature with the explicit type
/// arguments supplied at the call site via turbofish syntax
/// (`pick::[int](x)`). Runs constraint validation in the same way as
/// argument-driven inference.
fn instantiate_signature_with_explicit_type_args(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    signature: &RoutineType,
    callee: &str,
    type_args: &[fol_parser::ast::FolType],
    origin: Option<SyntaxOrigin>,
) -> Result<RoutineType, TypecheckError> {
    if signature.generic_params.is_empty() {
        return Err(match origin.clone() {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "explicit generic type arguments supplied to '{callee}', which is not a generic routine"
                ),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "explicit generic type arguments supplied to '{callee}', which is not a generic routine"
                ),
            ),
        });
    }
    if type_args.len() != signature.generic_params.len() {
        let message = format!(
            "explicit generic call to '{callee}' expects {} type argument(s) but got {}",
            signature.generic_params.len(),
            type_args.len()
        );
        return Err(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        });
    }
    let mut bindings: BTreeMap<SymbolId, CheckedTypeId> = BTreeMap::new();
    for (symbol_id, type_arg) in signature.generic_params.iter().zip(type_args.iter()) {
        let lowered = crate::decls::lower_type(typed, resolved, context.scope_id, type_arg)?;
        bindings.insert(*symbol_id, lowered);
    }
    instantiate_generic_signature(typed, signature, &bindings, callee, origin)
}

fn instantiate_generic_signature(
    typed: &mut TypedProgram,
    signature: &RoutineType,
    bindings: &BTreeMap<SymbolId, CheckedTypeId>,
    callee: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<RoutineType, TypecheckError> {
    if signature.generic_params.is_empty() {
        return Ok(signature.clone());
    }

    let params = signature
        .params
        .iter()
        .map(|param| substitute_generic_type(typed, *param, bindings, callee, origin.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = signature
        .return_type
        .map(|ty| substitute_generic_type(typed, ty, bindings, callee, origin.clone()))
        .transpose()?;
    let error_type = signature
        .error_type
        .map(|ty| substitute_generic_type(typed, ty, bindings, callee, origin.clone()))
        .transpose()?;
    crate::decls::validate_generic_bindings_against_constraints(
        typed,
        bindings,
        &signature.generic_constraints,
        format!("call to '{callee}'"),
        origin.clone(),
    )?;

    Ok(RoutineType {
        generic_params: Vec::new(),
        generic_constraints: BTreeMap::new(),
        param_names: signature.param_names.clone(),
        param_defaults: signature.param_defaults.clone(),
        variadic_index: signature.variadic_index,
        mutex_params: signature.mutex_params.clone(),
        params,
        return_type,
        error_type,
    })
}

fn substitute_generic_type(
    typed: &mut TypedProgram,
    type_id: CheckedTypeId,
    bindings: &BTreeMap<SymbolId, CheckedTypeId>,
    callee: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<CheckedTypeId, TypecheckError> {
    let Some(checked) = typed.type_table().get(type_id).cloned() else {
        return Err(internal_error(
            "generic substitution lost a checked type",
            origin,
        ));
    };

    match checked {
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::GenericParameter,
            ..
        } => bindings.get(&symbol).copied().ok_or_else(|| {
            let generic_name = typed
                .resolved()
                .symbol(symbol)
                .map(|symbol| symbol.name.as_str())
                .unwrap_or("?");
            TypecheckError::with_origin(
                TypecheckErrorKind::Unsupported,
                format!(
                    "call to '{callee}' leaves generic parameter '{generic_name}' underconstrained in V2 Milestone 1; inference only uses call arguments, so add an argument whose type mentions '{generic_name}' or make the routine stop depending on '{generic_name}' outside the argument list"
                ),
                origin.unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: callee.len(),
                }),
            )
        }),
        CheckedType::Array { element_type, size } => {
            let element_type =
                substitute_generic_type(typed, element_type, bindings, callee, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Array { element_type, size }))
        }
        CheckedType::Vector { element_type } => {
            let element_type =
                substitute_generic_type(typed, element_type, bindings, callee, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Vector { element_type }))
        }
        CheckedType::Sequence { element_type } => {
            let element_type =
                substitute_generic_type(typed, element_type, bindings, callee, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Sequence { element_type }))
        }
        CheckedType::Optional { inner } => {
            let inner = substitute_generic_type(typed, inner, bindings, callee, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Optional { inner }))
        }
        CheckedType::Error { inner } => {
            let inner = inner
                .map(|inner| substitute_generic_type(typed, inner, bindings, callee, origin))
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Error { inner }))
        }
        CheckedType::Set { member_types } => {
            let member_types = member_types
                .into_iter()
                .map(|member_type| {
                    substitute_generic_type(typed, member_type, bindings, callee, origin.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Set { member_types }))
        }
        CheckedType::Map {
            key_type,
            value_type,
        } => {
            let key_type =
                substitute_generic_type(typed, key_type, bindings, callee, origin.clone())?;
            let value_type =
                substitute_generic_type(typed, value_type, bindings, callee, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Map {
                key_type,
                value_type,
            }))
        }
        // An instantiated generic named type (e.g. `Box[T]`) is a structural
        // record/entry whose fields still mention the routine's generic
        // parameters. Recurse so the concrete call binding replaces them; a
        // missed field is exactly the P3 leak where a call to `wrap` returned
        // `Box[T]` instead of `Box[int]`.
        CheckedType::Record { fields } => {
            let fields = fields
                .into_iter()
                .map(|(name, field_type)| {
                    substitute_generic_type(typed, field_type, bindings, callee, origin.clone())
                        .map(|field_type| (name, field_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Record { fields }))
        }
        CheckedType::Entry { variants } => {
            let variants = variants
                .into_iter()
                .map(|(name, variant_type)| {
                    variant_type
                        .map(|variant_type| {
                            substitute_generic_type(
                                typed,
                                variant_type,
                                bindings,
                                callee,
                                origin.clone(),
                            )
                        })
                        .transpose()
                        .map(|variant_type| (name, variant_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Entry { variants }))
        }
        // A generic instantiation carries its type args nominally
        // (`Box[T]` = Declared{Box, args:[T]}). Substitute the args and
        // re-instantiate so the node keeps nominal identity AND its apparent
        // structural shape is recomputed for the concrete args — otherwise a
        // call to `wrap` returns `Box[T]` instead of `Box[int]` (the P3 leak).
        CheckedType::Declared {
            symbol,
            name,
            kind,
            args,
        } if !args.is_empty() => {
            let substituted_args = args
                .iter()
                .map(|arg| substitute_generic_type(typed, *arg, bindings, callee, origin.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            let template = typed
                .typed_symbol(symbol)
                .and_then(|s| s.declared_type);
            let generic_params = typed
                .typed_symbol(symbol)
                .map(|s| s.generic_params.clone())
                .unwrap_or_default();
            let instance = typed.type_table_mut().intern(CheckedType::Declared {
                symbol,
                name,
                kind,
                args: substituted_args.clone(),
            });
            if let Some(template) = template {
                if generic_params.len() == substituted_args.len() {
                    let inner_bindings: BTreeMap<SymbolId, CheckedTypeId> = generic_params
                        .into_iter()
                        .zip(substituted_args)
                        .collect();
                    let structural = crate::decls::substitute_generic_checked_type(
                        typed,
                        template,
                        &inner_bindings,
                        origin.clone(),
                    )?;
                    if instance != structural {
                        typed.record_apparent_type_override(instance, structural);
                    }
                }
            }
            Ok(instance)
        }
        other => Ok(typed.type_table_mut().intern(other)),
    }
}

pub(super) enum BoundCallArg<'a> {
    Explicit(&'a AstNode),
    Default,
    VariadicPack(Vec<&'a AstNode>),
    VariadicUnpack(&'a AstNode),
}

pub(super) fn bind_call_arguments<'a>(
    signature: &RoutineType,
    args: &'a [AstNode],
    callee: &str,
    origin: Option<SyntaxOrigin>,
    allow_named: bool,
    allow_defaults: bool,
) -> Result<Vec<BoundCallArg<'a>>, TypecheckError> {
    let has_named_args = args
        .iter()
        .any(|arg| matches!(arg, AstNode::NamedArgument { .. }));
    if signature.params.len() != args.len() && !has_named_args && !allow_defaults {
        return Err(call_arity_error(
            signature.params.len(),
            args.len(),
            callee,
            origin,
        ));
    }
    if args.len() < signature.params.len()
        && !has_named_args
        && signature.variadic_index.is_none()
        && signature
            .param_defaults
            .iter()
            .skip(args.len())
            .any(Option::is_none)
    {
        return Err(call_arity_error(
            signature.params.len(),
            args.len(),
            callee,
            origin,
        ));
    }

    let mut ordered_args = vec![None; signature.params.len()];
    let mut variadic_trailing = Vec::new();
    let mut next_positional = 0usize;
    let mut seen_named = false;
    let variadic_index = signature.variadic_index;

    for arg in args {
        match arg {
            AstNode::NamedArgument { name, value } => {
                if !allow_named {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!("named arguments are not supported for call to '{callee}'"),
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: name.len(),
                        }),
                    ));
                }
                seen_named = true;
                let Some(index) = signature.param_names.iter().position(|param| param == name)
                else {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!("call to '{callee}' does not have a parameter named '{name}'"),
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: name.len(),
                        }),
                    ));
                };
                if ordered_args[index].is_some() {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!("call to '{callee}' supplies parameter '{name}' more than once"),
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: name.len(),
                        }),
                    ));
                }
                ordered_args[index] = Some(value.as_ref());
            }
            AstNode::Unpack { .. } => {
                let Some(index) = variadic_index else {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        "call-site unpack is only supported for variadic calls in V1",
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: 3,
                        }),
                    ));
                };
                if index + 1 != signature.params.len()
                    || ordered_args[index].is_some()
                    || !variadic_trailing.is_empty()
                {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        "call-site unpack cannot be combined with other variadic arguments in V1",
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: 3,
                        }),
                    ));
                }
                ordered_args[index] = Some(arg);
            }
            _ => {
                if seen_named {
                    return Err(TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!("call to '{callee}' cannot place positional arguments after named arguments"),
                        origin.clone().unwrap_or(SyntaxOrigin {
                            file: None,
                            line: 1,
                            column: 1,
                            length: callee.len(),
                        }),
                    ));
                }
                if variadic_index.is_some_and(|index| next_positional >= index) {
                    variadic_trailing.push(arg);
                    continue;
                }
                if next_positional >= ordered_args.len() {
                    return Err(call_arity_error(
                        signature.params.len(),
                        args.len(),
                        callee,
                        origin,
                    ));
                }
                ordered_args[next_positional] = Some(arg);
                next_positional += 1;
            }
        }
    }

    if let Some(index) = variadic_index {
        if ordered_args[index].is_none() && !variadic_trailing.is_empty() {
            ordered_args[index] = Some(variadic_trailing[0]);
        }
    }

    let mut bound_args = Vec::with_capacity(ordered_args.len());
    for (index, arg) in ordered_args.into_iter().enumerate() {
        match arg {
            Some(AstNode::Unpack { value }) if variadic_index == Some(index) => {
                bound_args.push(BoundCallArg::VariadicUnpack(value.as_ref()));
            }
            Some(arg) if variadic_index == Some(index) && !variadic_trailing.is_empty() => {
                let mut packed = vec![arg];
                packed.extend(variadic_trailing.iter().skip(1).copied());
                bound_args.push(BoundCallArg::VariadicPack(packed));
            }
            Some(arg) => bound_args.push(BoundCallArg::Explicit(arg)),
            None if variadic_index == Some(index) => {
                bound_args.push(BoundCallArg::VariadicPack(Vec::new()));
            }
            None if allow_defaults
                && matches!(signature.param_defaults.get(index), Some(Some(_))) =>
            {
                bound_args.push(BoundCallArg::Default);
            }
            None => {
                let missing_name = signature
                    .param_names
                    .get(index)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("#{index}"));
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    format!("call to '{callee}' is missing required argument '{missing_name}'"),
                    origin.unwrap_or(SyntaxOrigin {
                        file: None,
                        line: 1,
                        column: 1,
                        length: callee.len(),
                    }),
                ));
            }
        }
    }

    Ok(bound_args)
}

fn call_arity_error(
    expected: usize,
    actual: usize,
    callee: &str,
    origin: Option<SyntaxOrigin>,
) -> TypecheckError {
    TypecheckError::with_origin(
        TypecheckErrorKind::InvalidInput,
        format!("call to '{callee}' expects {expected} args but got {actual}"),
        origin.unwrap_or(SyntaxOrigin {
            file: None,
            line: 1,
            column: 1,
            length: callee.len(),
        }),
    )
}

pub(crate) fn type_for_reference(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    reference_id: ReferenceId,
    origin: Option<SyntaxOrigin>,
) -> Result<TypedExpr, TypecheckError> {
    let reference = resolved.reference(reference_id).ok_or_else(|| {
        TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            "resolved reference disappeared before typechecking",
            origin.clone().unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: 1,
            }),
        )
    })?;
    let symbol_id = reference.resolved.ok_or_else(|| {
        TypecheckError::with_origin(
            TypecheckErrorKind::InvalidInput,
            "resolved reference lost its target symbol",
            origin.clone().unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: 1,
            }),
        )
    })?;
    let type_id = symbol_type(typed, resolved, symbol_id, origin.clone())?;
    if let Some(CheckedType::Routine(signature)) = typed.type_table().get(type_id) {
        let symbol_name = resolved
            .symbol(symbol_id)
            .map(|symbol| symbol.name.as_str())
            .unwrap_or("<routine>");
        if !signature.generic_params.is_empty() {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::Unsupported,
                format!(
                    "generic routine '{symbol_name}' cannot be used as a plain routine value in V2 Milestone 1; call it directly instead"
                ),
                origin.clone().unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: 1,
                }),
            ));
        }
        if !signature.mutex_params.is_empty() {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::Unsupported,
                format!(
                    "routine '{symbol_name}' with [mux] parameters cannot be used as a plain routine value in V3; call it directly instead"
                ),
                origin.clone().unwrap_or(SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: 1,
                }),
            ));
        }
    }
    let typed_reference = typed.typed_reference_mut(reference_id).ok_or_else(|| {
        TypecheckError::with_origin(
            TypecheckErrorKind::Internal,
            "typed reference table lost a resolved reference",
            origin.unwrap_or(SyntaxOrigin {
                file: None,
                line: 1,
                column: 1,
                length: 1,
            }),
        )
    })?;
    typed_reference.resolved_type = Some(type_id);
    Ok(TypedExpr::value(type_id))
}

pub(crate) fn symbol_type(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    symbol_id: SymbolId,
    origin: Option<SyntaxOrigin>,
) -> Result<crate::CheckedTypeId, TypecheckError> {
    if let Some(type_id) = typed
        .typed_symbol(symbol_id)
        .and_then(|symbol| symbol.declared_type)
    {
        return Ok(type_id);
    }

    let fallback_origin = origin.unwrap_or(SyntaxOrigin {
        file: None,
        line: 1,
        column: 1,
        length: 1,
    });
    if let Some(symbol) = resolved.symbol(symbol_id) {
        if symbol.mounted_from.is_some() {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::Unsupported,
                format!(
                    "imported symbol '{}' requires workspace-aware typechecking in V1; the legacy single-package path is not sufficient",
                    symbol.name
                ),
                fallback_origin,
            ));
        }
        if matches!(
            symbol.kind,
            SymbolKind::ValueBinding | SymbolKind::LabelBinding | SymbolKind::DestructureBinding
        ) {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "binding '{}' needs a declared type or an inferable initializer in V1",
                    symbol.name
                ),
                symbol.origin.clone().unwrap_or(fallback_origin),
            ));
        }
    }

    Err(TypecheckError::with_origin(
        TypecheckErrorKind::InvalidInput,
        format!(
            "resolved symbol {} does not have a lowered type yet",
            symbol_id.0
        ),
        fallback_origin,
    ))
}
