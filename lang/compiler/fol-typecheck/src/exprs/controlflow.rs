use crate::{decls, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, ChannelEndpoint, LoopCondition, SelectArm, WhenCase};
use fol_resolver::{ResolvedProgram, SymbolKind};

use super::helpers::{
    ensure_assignable, loop_binder_scope, merge_recoverable_effects, node_origin, plain_value_expr,
    record_symbol_type, reject_recoverable_error_shell_conversion, unsupported_node_surface,
};
use super::{type_body, type_node, type_node_with_expectation};
use super::{TypeContext, TypedExpr};

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
            // Membership and channel arms are declared syntax, but their
            // semantics (container membership, range matching, channels)
            // are later-milestone surfaces; keep the boundary explicit
            // instead of silently lowering them as equality checks.
            WhenCase::Has { .. } => {
                return Err(unsupported_node_surface(
                    resolved,
                    expr,
                    "membership when/has branches are not yet supported in V1",
                ));
            }
            WhenCase::In { .. } => {
                return Err(unsupported_node_surface(
                    resolved,
                    expr,
                    "range/set when/in branches are not yet supported in V1",
                ));
            }
            WhenCase::On { .. } => {
                return Err(unsupported_node_surface(
                    resolved,
                    expr,
                    "channel when/on branches are not part of the shipped channel surface; use select",
                ));
            }
            WhenCase::Case { condition, body }
            | WhenCase::Is {
                value: condition,
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
                // Case bodies live in their own resolver Block scope.
                let body_context = match super::helpers::inline_body_block_scope(
                    resolved,
                    context.source_unit_id,
                    context.scope_id,
                    body,
                ) {
                    Some(scope_id) => TypeContext {
                        scope_id,
                        ..context
                    },
                    None => context,
                };
                case_types.push(type_body(typed, resolved, body_context, body)?);
            }
            WhenCase::Of { .. } => {
                return Err(unsupported_node_surface(
                    resolved,
                    expr,
                    "type-matching when/of branches are not yet supported in V1",
                ));
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
    let default_context = match super::helpers::inline_body_block_scope(
        resolved,
        context.source_unit_id,
        context.scope_id,
        default,
    ) {
        Some(scope_id) => TypeContext {
            scope_id,
            ..context
        },
        None => context,
    };
    let default_expr = type_body(typed, resolved, default_context, default)?;
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
            let item_type = if let AstNode::ChannelAccess {
                channel,
                endpoint: ChannelEndpoint::Rx,
            } = super::helpers::strip_comments(iterable)
            {
                if !typed.capability_model().supports_processor() {
                    return Err(unsupported_node_surface(
                        resolved,
                        iterable,
                        "channel iteration requires hosted std support; declare the bundled internal standard dependency",
                    ));
                }
                super::reject_sender_capture_receive(typed, resolved, channel)?;
                let channel_raw = type_node(typed, resolved, context, channel)?;
                let channel_type = plain_value_expr(
                    typed,
                    context,
                    channel_raw,
                    node_origin(resolved, channel),
                    "channel loop receiver",
                )?
                .required_value("channel loop receiver does not have a type")?;
                let item_type = super::helpers::channel_element_type(typed, channel_type)?;
                // Channel iteration lowers each blocking receive through an
                // optional shell so channel closure can terminate the loop.
                // Retain that synthetic shell type in compiler truth even
                // though it is not written explicitly in the source.
                typed
                    .type_table_mut()
                    .intern(crate::CheckedType::Optional { inner: item_type });
                item_type
            } else {
                let iterable_raw = type_node(typed, resolved, context, iterable)?;
                let iterable_type = plain_value_expr(
                    typed,
                    context,
                    iterable_raw,
                    node_origin(resolved, iterable),
                    "loop iterable",
                )?
                .required_value("loop iterable does not have a type")?;
                iterable_element_type(typed, iterable_type)?
            };
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

pub(crate) fn type_select(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    arms: &[SelectArm],
    default: Option<&[AstNode]>,
) -> Result<TypedExpr, TypecheckError> {
    if !typed.capability_model().supports_processor() {
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "select requires hosted std support; declare the bundled internal standard dependency",
        ));
    }
    let mut used_scopes = std::collections::BTreeSet::new();
    for arm in arms {
        let channel_node = match &arm.channel {
            AstNode::ChannelAccess {
                channel,
                endpoint: ChannelEndpoint::Rx,
            } => channel.as_ref(),
            AstNode::ChannelAccess { .. } => {
                return Err(unsupported_node_surface(
                    resolved,
                    &arm.channel,
                    "select arms wait on receivers; use a channel binding or c[rx]",
                ));
            }
            channel => channel,
        };
        super::reject_sender_capture_receive(typed, resolved, channel_node)?;
        let channel_raw = type_node(typed, resolved, context, channel_node)?;
        let channel_type = plain_value_expr(
            typed,
            context,
            channel_raw,
            node_origin(resolved, channel_node),
            "select receiver",
        )?
        .required_value("select receiver does not have a type")?;
        let item_type = super::helpers::channel_element_type(typed, channel_type)?;
        typed
            .type_table_mut()
            .intern(crate::CheckedType::Optional { inner: item_type });

        let arm_scope = resolved
            .scopes
            .iter_with_ids()
            .find_map(|(scope_id, scope)| {
                (scope.parent == Some(context.scope_id)
                    && scope.source_unit == Some(context.source_unit_id)
                    && !used_scopes.contains(&scope_id)
                    && scope.symbols.iter().any(|symbol_id| {
                        resolved.symbol(*symbol_id).is_some_and(|symbol| {
                            symbol.kind == SymbolKind::ValueBinding && symbol.name == arm.binding
                        })
                    }))
                .then_some(scope_id)
            })
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "select arm binding '{}' lost its resolver scope",
                        arm.binding
                    ),
                )
            })?;
        used_scopes.insert(arm_scope);
        record_symbol_type(
            typed,
            resolved,
            context.source_unit_id,
            arm_scope,
            &arm.binding,
            SymbolKind::ValueBinding,
            item_type,
        )?;
        let arm_context = TypeContext {
            scope_id: arm_scope,
            ..context
        };
        let _ = type_body(typed, resolved, arm_context, &arm.body)?;
    }
    if let Some(default) = default {
        let default_scope = resolved
            .scopes
            .iter_with_ids()
            .find_map(|(scope_id, scope)| {
                (scope.parent == Some(context.scope_id)
                    && scope.source_unit == Some(context.source_unit_id)
                    && !used_scopes.contains(&scope_id))
                .then_some(scope_id)
            })
            .unwrap_or(context.scope_id);
        let _ = type_body(
            typed,
            resolved,
            TypeContext {
                scope_id: default_scope,
                ..context
            },
            default,
        )?;
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
    super::bindings::mark_plain_identifier_move(typed, resolved, Some(value), actual)?;
    Ok(TypedExpr::value(typed.builtin_types().never))
}

fn iterable_element_type(
    typed: &TypedProgram,
    iterable_type: crate::CheckedTypeId,
) -> Result<crate::CheckedTypeId, TypecheckError> {
    use super::helpers::{apparent_type_id, describe_type};
    use crate::CheckedType;

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
