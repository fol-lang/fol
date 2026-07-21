use crate::{decls, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{
    AstNode, BinaryOperator, ChannelEndpoint, LoopCondition, SelectArm, WhenCase,
};
use fol_resolver::{ResolvedProgram, SymbolKind};

use super::helpers::{
    apparent_type_id, ensure_assignable, invalid_binary_operator_error, is_equality_type,
    loop_body_scope, merge_recoverable_effects, node_origin, plain_value_expr, record_symbol_type,
    reject_recoverable_error_shell_conversion, unsupported_node_surface, with_node_origin,
};
use super::{type_body, type_body_transferring_value, type_node, type_node_with_expectation};
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
    let selector_type = selector_expr.required_value("when selector does not have a type")?;
    let selector_apparent = apparent_type_id(typed, selector_type)?;
    if cases.is_empty() {
        ensure_assignable(
            typed,
            typed.builtin_types().bool_,
            selector_type,
            "case-less when condition".to_string(),
            node_origin(resolved, expr),
        )?;
        let Some(default) = default else {
            return Err(unsupported_node_surface(
                resolved,
                expr,
                "when statements require a default branch",
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
        let entry_flow = typed.ownership_flow_state();
        let default_expr = type_body_transferring_value(typed, resolved, default_context, default)?;
        let mut branch_flows = vec![entry_flow.clone()];
        if !default_expr.is_never(typed) {
            branch_flows.push(typed.ownership_flow_state());
        }
        typed.merge_ownership_flows(&entry_flow, &branch_flows);
        let merged = merge_recoverable_effects(
            typed,
            node_origin(resolved, expr),
            "case-less when statement",
            [
                selector_expr.recoverable_effect,
                default_expr.recoverable_effect,
            ],
        )?;
        return Ok(TypedExpr::none().with_optional_effect(merged));
    }
    let mut case_types = Vec::new();
    let entry_flow = typed.ownership_flow_state();
    let mut branch_flows = Vec::new();

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
            WhenCase::On { channel, body } => {
                // Safe shell choice (V3_MEM §3.3): the scrutinee must be a
                // nil-able shell, and `on(v)` binds its present payload `T`. Both
                // `opt[T]` (present branch = some) and `err[T]` (present branch =
                // a stored error payload; `nil` = no error) are nil-able shells.
                let payload_type = match typed.type_table().get(selector_apparent) {
                    Some(crate::CheckedType::Optional { inner }) => *inner,
                    Some(crate::CheckedType::Error { inner: Some(inner) }) => *inner,
                    _ => {
                        return Err(unsupported_node_surface(
                            resolved,
                            expr,
                            "an 'on' branch requires an 'opt[T]' or 'err[T]' scrutinee; channel 'on' is not supported (use select)",
                        ));
                    }
                };
                let (on_scope, symbol) = match channel {
                    AstNode::Identifier {
                        name,
                        syntax_id: Some(syntax_id),
                    } => {
                        let scope = resolved.scope_for_syntax(*syntax_id).ok_or_else(|| {
                            TypecheckError::new(
                                TypecheckErrorKind::Internal,
                                "'on' branch lost its resolver scope",
                            )
                        })?;
                        let symbol = decls::find_symbol_id_in_scope(
                            resolved,
                            context.source_unit_id,
                            scope,
                            &[SymbolKind::ValueBinding],
                            name,
                        )?;
                        (scope, symbol)
                    }
                    _ => {
                        return Err(node_origin(resolved, expr).map_or_else(
                            || {
                                TypecheckError::new(
                                    TypecheckErrorKind::InvalidInput,
                                    "an 'on' branch requires a payload binding name, e.g. 'on(value)'",
                                )
                            },
                            |origin| {
                                TypecheckError::with_origin(
                                    TypecheckErrorKind::InvalidInput,
                                    "an 'on' branch requires a payload binding name, e.g. 'on(value)'",
                                    origin,
                                )
                            },
                        ));
                    }
                };
                decls::record_symbol_type(typed, symbol, payload_type)?;
                let body_context = TypeContext {
                    scope_id: on_scope,
                    ..context
                };
                let body_entry_flow = typed.ownership_flow_state();
                let body_type = type_body_transferring_value(typed, resolved, body_context, body)?;
                if !body_type.is_never(typed) {
                    branch_flows.push(typed.ownership_flow_state());
                }
                typed.restore_ownership_flow(&body_entry_flow);
                case_types.push(body_type);
            }
            WhenCase::Case { condition, body }
            | WhenCase::Is {
                value: condition,
                body,
            } => {
                let condition_raw = type_node(typed, resolved, context, condition)?;
                let condition_expr = plain_value_expr(
                    typed,
                    context,
                    condition_raw,
                    node_origin(resolved, condition),
                    "when condition",
                )?;
                let condition_type =
                    condition_expr.required_value("when condition does not have a type")?;
                ensure_assignable(
                    typed,
                    selector_type,
                    condition_type,
                    "when condition".to_string(),
                    node_origin(resolved, condition),
                )?;
                let condition_apparent = apparent_type_id(typed, condition_type)?;
                if selector_apparent != condition_apparent
                    || !is_equality_type(typed, selector_apparent)
                {
                    let error = invalid_binary_operator_error(
                        typed,
                        &BinaryOperator::Eq,
                        selector_type,
                        condition_type,
                    );
                    return Err(node_origin(resolved, condition)
                        .map_or(error.clone(), |origin| error.with_fallback_origin(origin)));
                }
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
                let body_entry_flow = typed.ownership_flow_state();
                let body_type = type_body_transferring_value(typed, resolved, body_context, body)?;
                if !body_type.is_never(typed) {
                    branch_flows.push(typed.ownership_flow_state());
                }
                typed.restore_ownership_flow(&body_entry_flow);
                case_types.push(body_type);
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
    let default_expr = type_body_transferring_value(typed, resolved, default_context, default)?;
    if !default_expr.is_never(typed) {
        branch_flows.push(typed.ownership_flow_state());
    }
    typed.merge_ownership_flows(&entry_flow, &branch_flows);
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
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    condition: &LoopCondition,
    body: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    let body_scope = loop_body_scope(resolved, syntax_id)?;
    match condition {
        LoopCondition::Condition(condition) => {
            let loop_context = TypeContext {
                scope_id: body_scope,
                repeating_loop_scope: Some(body_scope),
                ..context
            };
            let condition_context = TypeContext {
                scope_id: context.scope_id,
                ..loop_context
            };
            let condition_raw = type_node(typed, resolved, condition_context, condition)?;
            let condition_type = plain_value_expr(
                typed,
                condition_context,
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
            let body_entry_flow = typed.ownership_flow_state();
            let body_expr = type_body(typed, resolved, loop_context, body)?;
            let mut continuation_flows = vec![body_entry_flow.clone()];
            if !body_expr.is_never(typed) {
                continuation_flows.push(typed.ownership_flow_state());
            }
            typed.merge_ownership_flows(&body_entry_flow, &continuation_flows);
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
                super::helpers::require_direct_channel_binding(
                    resolved,
                    context.scope_id,
                    channel,
                )?;
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
                let item_type = super::helpers::channel_receiver_element_type(typed, channel_type)?;
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
                let element_type = iterable_element_type(typed, iterable_type)?;
                if super::bindings::ownership_moves_on_transfer(typed, iterable_type) {
                    return Err(with_node_origin(
                        resolved,
                        iterable,
                        TypecheckErrorKind::Ownership,
                        "iteration over a move-only collection is not supported in V3; index-based iteration would clone move-only elements",
                    ));
                }
                element_type
            };
            let binder_scope = body_scope;
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
                processor_task_call: context.processor_task_call,
                allow_mutex_handle: false,
                repeating_loop_scope: Some(binder_scope),
                inside_deferred_block: context.inside_deferred_block,
                inside_error_deferred_block: context.inside_error_deferred_block,
                field_projection_root: false,
                direct_spawn_anonymous: false,
                direct_binding_anonymous: false,
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
            let body_entry_flow = typed.ownership_flow_state();
            let body_expr = type_body(typed, resolved, loop_context, body)?;
            let mut continuation_flows = vec![body_entry_flow.clone()];
            if !body_expr.is_never(typed) {
                continuation_flows.push(typed.ownership_flow_state());
            }
            typed.merge_ownership_flows(&body_entry_flow, &continuation_flows);
        }
    }

    Ok(TypedExpr::none())
}

pub(crate) fn type_select(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    arms: &[SelectArm],
    default: Option<&[AstNode]>,
) -> Result<TypedExpr, TypecheckError> {
    if !typed.capability_model().supports_processor() {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::Unsupported,
            "select requires hosted std support; declare the bundled internal standard dependency",
        ));
    }
    if arms.is_empty() && default.is_none() {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::InvalidInput,
            "blocking select requires at least one channel arm",
        ));
    }
    super::helpers::reject_bound_guard_boundary(
        typed,
        "blocking select",
        node_origin(resolved, node),
    )?;
    let entry_flow = typed.ownership_flow_state();
    let mut branch_flows = Vec::new();
    let mut used_scopes = std::collections::BTreeSet::new();
    for arm in arms {
        // Select arms are mutually exclusive. Type every arm from the same
        // ownership state, then conservatively merge the continuing paths.
        // Otherwise a reinitialization in a later arm can erase a move from
        // an earlier arm even though that earlier runtime path never executes
        // the reinitialization.
        typed.restore_ownership_flow(&entry_flow);
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
        super::helpers::require_direct_channel_binding(resolved, context.scope_id, channel_node)?;
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
        let item_type = super::helpers::channel_receiver_element_type(typed, channel_type)?;
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
        let arm_expr = type_body(typed, resolved, arm_context, &arm.body)?;
        if !arm_expr.is_never(typed) {
            branch_flows.push(typed.ownership_flow_state());
        }
    }
    if let Some(default) = default {
        typed.restore_ownership_flow(&entry_flow);
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
        let default_expr = type_body(
            typed,
            resolved,
            TypeContext {
                scope_id: default_scope,
                ..context
            },
            default,
        )?;
        if !default_expr.is_never(typed) {
            branch_flows.push(typed.ownership_flow_state());
        }
    } else {
        // A blocking select can still continue without choosing an arm when
        // every receiver is closed, so preserve the unchanged entry path.
        branch_flows.push(entry_flow.clone());
    }
    typed.merge_ownership_flows(&entry_flow, &branch_flows);
    Ok(TypedExpr::none())
}

pub(crate) fn type_return(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: Option<&AstNode>,
    exit_origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<TypedExpr, TypecheckError> {
    let Some(expected) = context.routine_return_type else {
        return match value {
            Some(_) => Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "return with a value requires a declared routine return type in V1",
            )),
            None => {
                super::helpers::reject_all_recoverable_eventuals(
                    typed,
                    resolved,
                    context.scope_id,
                    exit_origin,
                    "returning from the routine",
                )?;
                Ok(TypedExpr::value(typed.builtin_types().never))
            }
        };
    };

    let Some(value) = value else {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "return requires a value for routines with a declared return type",
        ));
    };
    // Returning exits the routine, so every transfer nested in the expression
    // is single-shot even when the return itself is inside a repeating loop.
    let return_context = TypeContext {
        repeating_loop_scope: None,
        ..context
    };
    let actual = type_node_with_expectation(typed, resolved, return_context, value, Some(expected))
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
        return_context,
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
    // A returned borrow must originate outside the routine. Returning a borrow
    // of an owned local or by-value parameter leaves the borrow dangling after
    // the routine returns, so it is rejected. Borrows of already-borrowed
    // sources are handled by the reborrow rule elsewhere.
    if matches!(
        typed.type_table().get(actual),
        Some(crate::CheckedType::Borrowed { .. })
    ) {
        // A borrow of a temporary has no owner to trace; it dangles the moment
        // the enclosing expression ends, so it can never be returned.
        if super::calls::is_borrow_of_temporary(value) {
            let message =
                "cannot return a borrow of a temporary value; the borrow would dangle after the routine returns"
                    .to_string();
            return Err(match node_origin(resolved, value) {
                Some(origin) => {
                    TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin)
                }
                None => TypecheckError::new(TypecheckErrorKind::Ownership, message),
            });
        }
        if let Some(source) = super::bindings::borrow_source_symbol(resolved, value) {
            // Trace the borrow chain to its root owner. A reborrow of a borrow
            // that ultimately roots in an owned routine-local still dangles; only
            // a chain that roots in a borrowed input (whose value is Borrowed and
            // is not itself a loan of a local) escapes soundly.
            let mut root = source;
            while let Some(borrow) = typed.borrow_for_binding(root) {
                if borrow.owner == root {
                    break;
                }
                root = borrow.owner;
            }
            let root_is_borrowed_input = typed
                .typed_symbol(root)
                .and_then(|symbol| symbol.declared_type)
                .and_then(|type_id| typed.type_table().get(type_id))
                .is_some_and(|typ| matches!(typ, crate::CheckedType::Borrowed { .. }));
            if !root_is_borrowed_input {
                let name = resolved
                    .symbol(root)
                    .map(|symbol| symbol.name.clone())
                    .unwrap_or_else(|| "value".to_string());
                let message = format!(
                    "cannot return a borrow of the owned local '{name}'; the borrow would dangle after the routine returns"
                );
                return Err(match node_origin(resolved, value) {
                    Some(origin) => {
                        TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin)
                    }
                    None => TypecheckError::new(TypecheckErrorKind::Ownership, message),
                });
            }
        }
    }
    // §2.2: returning an existing owned value is a transfer boundary and must
    // state its operation explicitly.
    super::bindings::reject_untagged_owned_transfer(typed, resolved, value, actual, "returned")?;
    super::bindings::track_value_transfer(typed, resolved, return_context, Some(value), actual)?;
    super::helpers::reject_all_recoverable_eventuals(
        typed,
        resolved,
        context.scope_id,
        exit_origin,
        "returning from the routine",
    )?;
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
