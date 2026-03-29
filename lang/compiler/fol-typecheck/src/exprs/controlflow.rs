use crate::{decls, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, LoopCondition, WhenCase};
use fol_resolver::{ResolvedProgram, SymbolKind};

use super::helpers::{
    ensure_assignable, loop_binder_scope, merge_recoverable_effects, node_origin, plain_value_expr,
    record_symbol_type, reject_recoverable_error_shell_conversion, unsupported_node_surface,
};
use super::{TypeContext, TypedExpr};
use super::{type_body, type_node, type_node_with_expectation};

pub(crate) fn type_when(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    expr: &AstNode,
    cases: &[WhenCase],
    default: Option<&[AstNode]>,
) -> Result<TypedExpr, TypecheckError> {
    let selector_raw = type_node(typed, resolved, context, expr)?;
    let selector_expr = plain_value_expr(
        typed,
        context,
        selector_raw,
        node_origin(resolved, expr),
        "when selector",
    )?;
    let mut case_types = Vec::new();

    for case in cases {
        match case {
            WhenCase::Case { condition, body }
            | WhenCase::Is {
                value: condition,
                body,
            }
            | WhenCase::In {
                range: condition,
                body,
            }
            | WhenCase::Has {
                member: condition,
                body,
            }
            | WhenCase::On {
                channel: condition,
                body,
            } => {
                let condition_raw = type_node(typed, resolved, context, condition)?;
                let _ = plain_value_expr(
                    typed,
                    context,
                    condition_raw,
                    node_origin(resolved, condition),
                    "when condition",
                )?;
                case_types.push(type_body(typed, resolved, context, body)?);
            }
            WhenCase::Of { type_match, body } => {
                let _ = decls::lower_type(typed, resolved, context.scope_id, type_match)?;
                case_types.push(type_body(typed, resolved, context, body)?);
            }
        }
    }

    let Some(default) = default else {
        return Err(unsupported_node_surface(
            resolved,
            expr,
            "when expressions require a default branch",
        ));
    };
    let default_expr = type_body(typed, resolved, context, default)?;
    let Some(expected) = default_expr.value_type else {
        return Ok(TypedExpr::none());
    };

    for case_type in &case_types {
        let Some(actual) = case_type.value_type else {
            return Ok(TypedExpr::none());
        };
        ensure_assignable(typed, expected, actual, "when branch".to_string(), None)?;
    }
    let branch_effects = case_types
        .iter()
        .map(|case| case.recoverable_effect)
        .chain(std::iter::once(default_expr.recoverable_effect))
        .chain(std::iter::once(selector_expr.recoverable_effect))
        .collect::<Vec<_>>();
    let merged = merge_recoverable_effects(
        typed,
        node_origin(resolved, expr),
        "when expression",
        branch_effects,
    )?;
    Ok(TypedExpr::value(expected).with_optional_effect(merged))
}

pub(crate) fn type_loop(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    condition: &LoopCondition,
    body: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    match condition {
        LoopCondition::Condition(condition) => {
            let condition_raw = type_node(typed, resolved, context, condition)?;
            let condition_type = plain_value_expr(
                typed,
                context,
                condition_raw,
                node_origin(resolved, condition),
                "loop condition",
            )?
            .required_value("loop condition does not have a type")?;
            ensure_assignable(
                typed,
                typed.builtin_types().bool_,
                condition_type,
                "loop condition".to_string(),
                None,
            )?;
            let _ = type_body(typed, resolved, context, body)?;
        }
        LoopCondition::Iteration {
            var,
            type_hint,
            iterable,
            condition,
        } => {
            let iterable_raw = type_node(typed, resolved, context, iterable)?;
            let iterable_type = plain_value_expr(
                typed,
                context,
                iterable_raw,
                node_origin(resolved, iterable),
                "loop iterable",
            )?
            .required_value("loop iterable does not have a type")?;
            let item_type = iterable_element_type(typed, iterable_type)?;
            let binder_scope = loop_binder_scope(
                resolved,
                context.source_unit_id,
                context.scope_id,
                var,
                condition,
                body,
            )?;
            if let Some(type_hint) = type_hint {
                let hinted = decls::lower_type(typed, resolved, binder_scope, type_hint)?;
                ensure_assignable(
                    typed,
                    hinted,
                    item_type,
                    format!("loop binder '{var}'"),
                    None,
                )?;
                record_symbol_type(
                    typed,
                    resolved,
                    context.source_unit_id,
                    binder_scope,
                    var,
                    SymbolKind::LoopBinder,
                    hinted,
                )?;
            } else {
                record_symbol_type(
                    typed,
                    resolved,
                    context.source_unit_id,
                    binder_scope,
                    var,
                    SymbolKind::LoopBinder,
                    item_type,
                )?;
            }
            let loop_context = TypeContext {
                source_unit_id: context.source_unit_id,
                scope_id: binder_scope,
                routine_return_type: context.routine_return_type,
                routine_error_type: context.routine_error_type,
                error_call_mode: context.error_call_mode,
            };
            if let Some(condition) = condition.as_deref() {
                let guard_raw = type_node(typed, resolved, loop_context, condition)?;
                let condition_type = plain_value_expr(
                    typed,
                    loop_context,
                    guard_raw,
                    node_origin(resolved, condition),
                    "loop guard",
                )?
                .required_value("loop guard does not have a type")?;
                ensure_assignable(
                    typed,
                    typed.builtin_types().bool_,
                    condition_type,
                    "loop guard".to_string(),
                    None,
                )?;
            }
            let _ = type_body(typed, resolved, loop_context, body)?;
        }
    }

    Ok(TypedExpr::none())
}

pub(crate) fn type_return(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: Option<&AstNode>,
) -> Result<TypedExpr, TypecheckError> {
    let Some(expected) = context.routine_return_type else {
        return match value {
            Some(_) => Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "return with a value requires a declared routine return type in V1",
            )),
            None => Ok(TypedExpr::none()),
        };
    };

    let Some(value) = value else {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "return requires a value for routines with a declared return type",
        ));
    };
    let actual = type_node_with_expectation(typed, resolved, context, value, Some(expected))
        .map_err(|error| {
            node_origin(resolved, value)
                .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
        })?;
    reject_recoverable_error_shell_conversion(
        typed,
        expected,
        &actual,
        node_origin(resolved, value),
        "return",
    )?;
    let actual = plain_value_expr(
        typed,
        context,
        actual,
        node_origin(resolved, value),
        "return expression",
    )?
    .required_value("return expression does not have a type")?;
    ensure_assignable(
        typed,
        expected,
        actual,
        "return".to_string(),
        node_origin(resolved, value),
    )?;
    Ok(TypedExpr::value(typed.builtin_types().never))
}

fn iterable_element_type(
    typed: &TypedProgram,
    iterable_type: crate::CheckedTypeId,
) -> Result<crate::CheckedTypeId, TypecheckError> {
    use crate::CheckedType;
    use super::helpers::{apparent_type_id, describe_type};

    let apparent = apparent_type_id(typed, iterable_type)?;
    match typed.type_table().get(apparent) {
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector { element_type })
        | Some(CheckedType::Sequence { element_type }) => Ok(*element_type),
        Some(CheckedType::Set { member_types }) => {
            let Some(first) = member_types.first().copied() else {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "cannot infer an iteration element type from an empty set",
                ));
            };
            if member_types.iter().all(|member| *member == first) {
                Ok(first)
            } else {
                Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "heterogeneous set iteration is not yet supported",
                ))
            }
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "loop iteration requires an array, vector, sequence, or homogeneous set receiver, got '{}'",
                describe_type(typed, iterable_type)
            ),
        )),
    }
}
