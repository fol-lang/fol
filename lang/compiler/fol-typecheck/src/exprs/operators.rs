use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, BinaryOperator, ChannelEndpoint, OwnershipOption, UnaryOperator};
use fol_resolver::{ReferenceKind, ResolvedProgram};

use super::helpers::{
    apparent_type_id, invalid_binary_operator_error, invalid_unary_operator_error,
    is_equality_type, is_error_shell_type, is_ordered_type, merge_recoverable_effects, node_origin,
    observe_context, plain_value_expr, unsupported_binary_surface,
    unsupported_conversion_intrinsic, unwrap_shell_result_type, with_node_origin,
};
use super::type_node;
use super::type_node_with_expectation;
use super::{TypeContext, TypedExpr};

pub(crate) fn type_binary_op(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    op: &BinaryOperator,
    left: &AstNode,
    right: &AstNode,
) -> Result<TypedExpr, TypecheckError> {
    match op {
        BinaryOperator::As => {
            return Err(unsupported_conversion_intrinsic(
                resolved, left, right, "as",
            ));
        }
        BinaryOperator::Cast => {
            return Err(unsupported_conversion_intrinsic(
                resolved, left, right, "cast",
            ));
        }
        BinaryOperator::Pipe
            if matches!(
                super::helpers::strip_comments(right),
                AstNode::ChannelAccess {
                    endpoint: ChannelEndpoint::Tx,
                    ..
                }
            ) =>
        {
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "channel send requires hosted std support; declare the bundled internal standard dependency",
                ));
            }
            let AstNode::ChannelAccess { channel, .. } = super::helpers::strip_comments(right)
            else {
                unreachable!("channel-send guard preserves the endpoint shape")
            };
            super::helpers::require_direct_channel_binding(resolved, context.scope_id, channel)?;
            let value_raw = type_node(typed, resolved, context, left)?;
            let value_type = plain_value_expr(
                typed,
                context,
                value_raw,
                node_origin(resolved, left),
                "channel send value",
            )?
            .required_value("channel send value does not have a type")?;
            let channel_raw = type_node(typed, resolved, context, channel)?;
            let channel_type = plain_value_expr(
                typed,
                context,
                channel_raw,
                node_origin(resolved, channel),
                "channel send target",
            )?
            .required_value("channel send target does not have a type")?;
            let element_type = super::helpers::channel_element_type(typed, channel_type)?;
            super::helpers::ensure_assignable(
                typed,
                element_type,
                value_type,
                "channel send value".to_string(),
                node_origin(resolved, left),
            )?;
            super::bindings::reject_untagged_owned_transfer(
                typed,
                resolved,
                left,
                value_type,
                "sent on a channel",
            )?;
            super::bindings::track_value_transfer(
                typed,
                resolved,
                context,
                Some(left),
                value_type,
            )?;
            // A send produces a must-handle `err[T]` result (V3_MEM §8.2): `nil`
            // means the payload was delivered; the present branch owns the unsent
            // payload when the receiver has closed. A bare send that discards this
            // result is rejected as a statement (see reject_discarded_body_expr).
            let send_result = typed.type_table_mut().intern(CheckedType::Error {
                inner: Some(element_type),
            });
            return Ok(TypedExpr::value(send_result));
        }
        BinaryOperator::PipeOr
            if matches!(super::helpers::strip_comments(right), AstNode::AsyncStage) =>
        {
            return Err(unsupported_binary_surface(
                resolved,
                left,
                right,
                "async is only a '|' pipe stage; '|| async' is not part of V3",
            ));
        }
        BinaryOperator::Pipe
            if matches!(super::helpers::strip_comments(right), AstNode::AsyncStage) =>
        {
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "async pipe stages require hosted std support; declare the bundled internal standard dependency",
                ));
            }
            if !matches!(
                super::helpers::strip_comments(left),
                AstNode::FunctionCall { .. } | AstNode::QualifiedFunctionCall { .. }
            ) {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "| async requires a direct named routine call on its left side",
                ));
            }
            super::require_named_processor_call_target(resolved, left, "| async")?;
            super::reject_direct_spawn_channel_receiver(typed, resolved, left)?;
            let observed = type_node(
                typed,
                resolved,
                TypeContext {
                    error_call_mode: super::ErrorCallMode::Observe,
                    processor_task_call: super::processor_call_syntax_id(left),
                    ..context
                },
                left,
            )?;
            // A `| async` stage produces a scoped eventual; it is never detached.
            super::apply_spawn_argument_boundary(typed, resolved, left, false)?;
            let value_type = observed.value_type.ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "| async requires a call that yields a value",
                )
            })?;
            let error_type = observed.recoverable_effect.map(|effect| effect.error_type);
            if super::helpers::type_contains_shared_pointer(typed, value_type)
                || error_type
                    .is_some_and(|error| super::helpers::type_contains_shared_pointer(typed, error))
            {
                let message =
                    "an async result containing shared Rc pointers cannot cross the task boundary";
                return Err(node_origin(resolved, left).map_or_else(
                    || TypecheckError::new(TypecheckErrorKind::Ownership, message),
                    |origin| {
                        TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin)
                    },
                ));
            }
            let eventual = typed.type_table_mut().intern(CheckedType::Eventual {
                value_type,
                error_type,
            });
            return Ok(TypedExpr::value(eventual));
        }
        BinaryOperator::PipeOr
            if matches!(super::helpers::strip_comments(right), AstNode::AwaitStage) =>
        {
            return Err(unsupported_binary_surface(
                resolved,
                left,
                right,
                "await is only a '|' pipe stage; '|| await' is not part of V3",
            ));
        }
        BinaryOperator::Pipe
            if matches!(super::helpers::strip_comments(right), AstNode::AwaitStage) =>
        {
            if context.inside_error_deferred_block {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "await is not allowed inside edf in V3; error-only deferred cleanup cannot discharge eventual ownership on normal exits",
                ));
            }
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "await pipe stages require hosted std support; declare the bundled internal standard dependency",
                ));
            }
            let eventual_raw = type_node(typed, resolved, context, left)?;
            let eventual_type = eventual_raw
                .required_value("| await requires an eventual value on its left side")?;
            let Some(CheckedType::Eventual {
                value_type,
                error_type,
            }) = typed.type_table().get(eventual_type).cloned()
            else {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "| await requires the internal eventual produced by | async",
                ));
            };
            super::helpers::reject_bound_guard_boundary(
                typed,
                "await",
                node_origin(resolved, left),
            )?;
            mark_awaited_eventual_binding(typed, resolved, context, left)?;
            return Ok(TypedExpr::value(value_type).with_optional_effect(
                error_type.map(|error_type| crate::RecoverableCallEffect { error_type }),
            ));
        }
        BinaryOperator::PipeOr => return type_pipe_or(typed, resolved, context, left, right),
        _ => {}
    }

    let left_raw = type_node(typed, resolved, context, left)?;
    let left_expr = plain_value_expr(
        typed,
        context,
        left_raw,
        node_origin(resolved, left),
        "plain use of an errorful expression",
    )?;
    let right_raw = type_node(typed, resolved, context, right)?;
    let right_expr = plain_value_expr(
        typed,
        context,
        right_raw,
        node_origin(resolved, right),
        "plain use of an errorful expression",
    )?;
    let left_type =
        left_expr.required_value("binary operator left operand does not have a type")?;
    let right_type =
        right_expr.required_value("binary operator right operand does not have a type")?;
    let left_apparent = apparent_type_id(typed, left_type)?;
    let right_apparent = apparent_type_id(typed, right_type)?;
    let merged_effect = merge_recoverable_effects(
        typed,
        node_origin(resolved, left).or_else(|| node_origin(resolved, right)),
        "binary expression",
        [left_expr.recoverable_effect, right_expr.recoverable_effect],
    )?;

    match op {
        BinaryOperator::Add => {
            match (
                typed.type_table().get(left_apparent),
                typed.type_table().get(right_apparent),
            ) {
                (
                    Some(CheckedType::Builtin(crate::BuiltinType::Int)),
                    Some(CheckedType::Builtin(crate::BuiltinType::Int)),
                ) => {
                    Ok(TypedExpr::value(typed.builtin_types().int)
                        .with_optional_effect(merged_effect))
                }
                (
                    Some(CheckedType::Builtin(crate::BuiltinType::Float)),
                    Some(CheckedType::Builtin(crate::BuiltinType::Float)),
                ) => Ok(TypedExpr::value(typed.builtin_types().float)
                    .with_optional_effect(merged_effect)),
                (
                    Some(CheckedType::Builtin(crate::BuiltinType::Str)),
                    Some(CheckedType::Builtin(crate::BuiltinType::Str)),
                ) => Ok(TypedExpr::value(typed.builtin_types().str_)
                    .with_optional_effect(merged_effect)),
                _ => Err(invalid_binary_operator_error(
                    typed, op, left_type, right_type,
                )),
            }
        }
        BinaryOperator::Sub
        | BinaryOperator::Mul
        | BinaryOperator::Div
        | BinaryOperator::Mod
        | BinaryOperator::Pow => match (
            typed.type_table().get(left_apparent),
            typed.type_table().get(right_apparent),
        ) {
            (
                Some(CheckedType::Builtin(crate::BuiltinType::Int)),
                Some(CheckedType::Builtin(crate::BuiltinType::Int)),
            ) => {
                Ok(TypedExpr::value(typed.builtin_types().int).with_optional_effect(merged_effect))
            }
            (
                Some(CheckedType::Builtin(crate::BuiltinType::Float)),
                Some(CheckedType::Builtin(crate::BuiltinType::Float)),
            ) => {
                Ok(TypedExpr::value(typed.builtin_types().float)
                    .with_optional_effect(merged_effect))
            }
            _ => Err(invalid_binary_operator_error(
                typed, op, left_type, right_type,
            )),
        },
        BinaryOperator::Eq | BinaryOperator::Ne => {
            if left_apparent == right_apparent && is_equality_type(typed, left_apparent) {
                Ok(TypedExpr::value(typed.builtin_types().bool_)
                    .with_optional_effect(merged_effect))
            } else {
                Err(invalid_binary_operator_error(
                    typed, op, left_type, right_type,
                ))
            }
        }
        BinaryOperator::Lt | BinaryOperator::Le | BinaryOperator::Gt | BinaryOperator::Ge => {
            if left_apparent == right_apparent && is_ordered_type(typed, left_apparent) {
                Ok(TypedExpr::value(typed.builtin_types().bool_)
                    .with_optional_effect(merged_effect))
            } else {
                Err(invalid_binary_operator_error(
                    typed, op, left_type, right_type,
                ))
            }
        }
        BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor => {
            if left_apparent == typed.builtin_types().bool_
                && right_apparent == typed.builtin_types().bool_
            {
                Ok(TypedExpr::value(typed.builtin_types().bool_)
                    .with_optional_effect(merged_effect))
            } else {
                Err(invalid_binary_operator_error(
                    typed, op, left_type, right_type,
                ))
            }
        }
        BinaryOperator::In | BinaryOperator::Has => Err(unsupported_binary_surface(
            resolved,
            left,
            right,
            "membership operators 'in' and 'has' are not yet supported",
        )),
        BinaryOperator::Is => Err(unsupported_binary_surface(
            resolved,
            left,
            right,
            "type testing operator 'is' is not yet supported",
        )),
        BinaryOperator::Pipe => {
            // A send through a first-class sender value: `value | sender`, where
            // `sender` is a `chn[tx, T]` binding, parameter, or clone (V3_MEM
            // §8.2). Senders are clone-capable, so — unlike the unique receiver —
            // they need no direct-binding restriction. The result is the same
            // must-handle `err[T]` a `value | channel[tx]` send produces.
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "channel send requires hosted std support; declare the bundled internal standard dependency",
                ));
            }
            let sender_raw = type_node(typed, resolved, context, right)?;
            let sender_type = plain_value_expr(
                typed,
                context,
                sender_raw,
                node_origin(resolved, right),
                "channel send target",
            )?
            .required_value("channel send target does not have a type")?;
            let sender_apparent = super::helpers::apparent_type_id(typed, sender_type)?;
            let element_type = match typed.type_table().get(sender_apparent) {
                Some(CheckedType::ChannelSender { element_type }) => *element_type,
                _ => {
                    return Err(unsupported_binary_surface(
                        resolved,
                        left,
                        right,
                        "the pipe target must be a channel transmitter ('channel[tx]' or a 'chn[tx, T]' sender value)",
                    ));
                }
            };
            let value_raw = type_node(typed, resolved, context, left)?;
            let value_type = plain_value_expr(
                typed,
                context,
                value_raw,
                node_origin(resolved, left),
                "channel send value",
            )?
            .required_value("channel send value does not have a type")?;
            super::helpers::ensure_assignable(
                typed,
                element_type,
                value_type,
                "channel send value".to_string(),
                node_origin(resolved, left),
            )?;
            super::bindings::reject_untagged_owned_transfer(
                typed,
                resolved,
                left,
                value_type,
                "sent on a channel",
            )?;
            super::bindings::track_value_transfer(
                typed,
                resolved,
                context,
                Some(left),
                value_type,
            )?;
            let send_result = typed.type_table_mut().intern(CheckedType::Error {
                inner: Some(element_type),
            });
            Ok(TypedExpr::value(send_result))
        }
        BinaryOperator::As | BinaryOperator::Cast | BinaryOperator::PipeOr => {
            unreachable!("handled before plain binary typing")
        }
    }
}

pub(crate) fn type_pipe_or(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    left: &AstNode,
    right: &AstNode,
) -> Result<TypedExpr, TypecheckError> {
    let observed_left = type_node(typed, resolved, observe_context(context), left)?;
    let Some(success_type) = observed_left.value_type else {
        return Err(node_origin(resolved, left).map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "left side of '||' must produce a value result in V1",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    "left side of '||' must produce a value result in V1",
                    origin,
                )
            },
        ));
    };
    if observed_left.recoverable_effect.is_none() {
        let message = if observed_left
            .value_type
            .map(|type_id| is_error_shell_type(typed, type_id))
            .transpose()?
            .unwrap_or(false)
        {
            "'||' handles recoverable '/ ErrorType' expressions (direct calls or awaited recoverable eventuals), not err[...] shell values"
        } else {
            "'||' requires a recoverable expression with '/ ErrorType' on the left (a direct call or awaited recoverable eventual)"
        };
        return Err(node_origin(resolved, left).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
        ));
    }

    // The fallback executes only when the observed call reports an error.
    // Preserve the post-call success flow as its own continuation so a
    // fallback-only reinitialization cannot erase a move that remains visible
    // when the call succeeds.
    let success_flow = typed.ownership_flow_state();
    let fallback = type_node_with_expectation(typed, resolved, context, right, Some(success_type))?;
    let fallback = plain_value_expr(
        typed,
        context,
        fallback,
        node_origin(resolved, right),
        "recoverable-error fallback",
    )?;

    let result = match fallback.value_type {
        Some(actual) if actual == typed.builtin_types().never => {
            Ok(TypedExpr::value(success_type).with_optional_effect(fallback.recoverable_effect))
        }
        Some(actual) => {
            super::helpers::ensure_assignable(
                typed,
                success_type,
                actual,
                "recoverable-error fallback".to_string(),
                node_origin(resolved, right),
            )?;
            Ok(TypedExpr::value(success_type).with_optional_effect(fallback.recoverable_effect))
        }
        None => Err(node_origin(resolved, right).map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "right side of '||' must produce a value or early-exit in V1",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    "right side of '||' must produce a value or early-exit in V1",
                    origin,
                )
            },
        )),
    }?;

    let mut continuation_flows = vec![success_flow.clone()];
    if !fallback.is_never(typed) {
        continuation_flows.push(typed.ownership_flow_state());
    }
    typed.merge_ownership_flows(&success_flow, &continuation_flows);
    Ok(result)
}

fn mark_awaited_eventual_binding(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
) -> Result<(), TypecheckError> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        name,
    } = super::helpers::strip_comments(node)
    else {
        return Ok(());
    };
    let Some(symbol) = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
    else {
        return Ok(());
    };
    super::bindings::reject_repeated_outer_move(resolved, context, node, symbol, name)?;
    if let Some(origin) = node_origin(resolved, node) {
        typed.mark_eventual_awaited(symbol, origin);
    }
    Ok(())
}

/// Type the canonical V3 prefix ownership operation `[opt, ...]operand`. This is
/// the explicit spelling of the transfer/borrow/allocation operations. It is
/// wired additively alongside the legacy `#`/`@`/implicit forms: each option
/// combination reuses the same checked machinery the legacy forms do, so the two
/// spellings stay semantically identical until the legacy forms are retired.
pub(crate) fn type_ownership_op(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    options: &[OwnershipOption],
    operand: &AstNode,
    expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    let has = |option: OwnershipOption| options.contains(&option);
    let borrow = has(OwnershipOption::Borrow);
    let mutable = has(OwnershipOption::Mutable);
    let new = has(OwnershipOption::New);
    let mov = has(OwnershipOption::Move);
    let copy = has(OwnershipOption::Copy);
    let clone = has(OwnershipOption::Clone);
    let weak = has(OwnershipOption::Weak);
    let upgrade = has(OwnershipOption::Upgrade);
    let finalize = has(OwnershipOption::Finalize);

    // Reject nonsensical combinations up front.
    let source_ops = [mov, copy, clone, borrow, weak, upgrade, finalize]
        .iter()
        .filter(|present| **present)
        .count();
    if borrow {
        if source_ops > 1 {
            return Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::InvalidInput,
                "'[bor]' cannot be combined with another source operation; use '[bor]' or '[mut, bor]'",
            ));
        }
    } else if mutable {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::InvalidInput,
            "'[mut]' is only valid combined with '[bor]' as '[mut, bor]'",
        ));
    } else if source_ops != 1 {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::InvalidInput,
            "an ownership operation needs exactly one source: '[mov]', '[cpy]', '[cln]', '[bor]', '[weak]', '[upg]', or '[fin]'",
        ));
    }

    // A lifetime-scoped mutex guard value cannot be moved, copied, or cloned as
    // a whole (V3_MEM §8.3). Duplicating or transferring the guard would alias
    // or leak the held lock; read its fields (`guard.value`) to snapshot the
    // protected data instead.
    if mov || copy || clone {
        if let AstNode::Identifier {
            syntax_id: Some(syntax_id),
            ..
        } = super::helpers::strip_comments(operand)
        {
            let operand_symbol = resolved
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(*syntax_id)
                        && reference.kind == ReferenceKind::Identifier
                })
                .and_then(|reference| reference.resolved);
            if operand_symbol
                .and_then(|symbol| typed.typed_symbol(symbol))
                .is_some_and(|symbol| symbol.is_mutex_guard)
            {
                let op = if mov {
                    "[mov]"
                } else if copy {
                    "[cpy]"
                } else {
                    "[cln]"
                };
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::Ownership,
                    format!(
                        "a mutex guard cannot be moved, copied, or cloned as a whole; read its fields (e.g. 'guard.value') instead of '{op}guard'"
                    ),
                ));
            }
        }
    }

    // Managed pointer family: `[weak]shared` downgrades a shared pointer to a
    // weak handle; `[upg]weak` upgrades a weak handle back to an optional shared
    // pointer. Neither consumes its operand (both observe it, like `[cpy]`).
    if weak || upgrade {
        let operand_raw = type_node(typed, resolved, context, operand)?;
        let operand_expr = plain_value_expr(
            typed,
            context,
            operand_raw,
            node_origin(resolved, operand),
            "operand of a managed pointer operation",
        )?;
        let operand_type = operand_expr
            .required_value("managed pointer operation operand does not have a type")?;
        let apparent = super::helpers::apparent_type_id(typed, operand_type)?;
        let pointer = match typed.type_table().get(apparent) {
            Some(CheckedType::Pointer {
                target,
                shared,
                weak,
                sync,
            }) => Some((*target, *shared, *weak, *sync)),
            _ => None,
        };
        if weak {
            let Some((target, true, false, sync)) = pointer else {
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::InvalidInput,
                    "'[weak]' requires a 'ptr[shared, T]' operand to downgrade",
                ));
            };
            // A weak handle keeps its source's thread-safety (Arc→sync Weak).
            let weak_ptr = typed.type_table_mut().intern(CheckedType::Pointer {
                target,
                shared: false,
                weak: true,
                sync,
            });
            return Ok(TypedExpr::value(weak_ptr));
        }
        // upgrade
        let Some((target, _, true, sync)) = pointer else {
            return Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::InvalidInput,
                "'[upg]' requires a 'ptr[weak, T]' operand to upgrade",
            ));
        };
        let shared_ptr = typed.type_table_mut().intern(CheckedType::Pointer {
            target,
            shared: true,
            weak: false,
            sync,
        });
        let optional = typed
            .type_table_mut()
            .intern(CheckedType::Optional { inner: shared_ptr });
        return Ok(TypedExpr::value(optional));
    }

    // Borrow family: `[bor]owner` / `[mut, bor]owner`.
    if borrow {
        let Some(symbol) = super::bindings::borrow_source_symbol(resolved, operand) else {
            // Borrowing a temporary (a non-place value such as a call result):
            // valid for the enclosing expression only, with no owner to lock. It
            // cannot be stored (rejected at the binding site) or returned
            // (rejected at the return site) because it would dangle.
            let operand_raw = type_node(typed, resolved, context, operand)?;
            let operand_expr = plain_value_expr(
                typed,
                context,
                operand_raw,
                node_origin(resolved, operand),
                "operand of a borrow operation",
            )?;
            let operand_type =
                operand_expr.required_value("borrow operation operand does not have a type")?;
            let inner = super::bindings::owned_or_borrowed_inner(typed, operand_type);
            let borrowed = typed
                .type_table_mut()
                .intern(CheckedType::Borrowed { inner, mutable });
            return Ok(TypedExpr::value(borrowed));
        };
        let origin = node_origin(resolved, node)
            .or_else(|| node_origin(resolved, operand))
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::Internal,
                    "borrow operation lost its syntax origin",
                )
            })?;
        if let Some(move_origin) = typed.moved_binding_origin(symbol).cloned() {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::Ownership,
                "cannot borrow from an owner whose value was already moved",
                origin,
            )
            .with_related_origin(move_origin, "ownership moved here"));
        }
        if let Some(conflict) = typed.active_borrow_for_owner(symbol).cloned() {
            if conflict.mutable || mutable {
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::BorrowConflict,
                    "borrow conflicts with an active borrow of the same owner",
                    origin,
                )
                .with_related_origin(conflict.origin, "conflicting borrow created here"));
            }
        }
        if mutable
            && !typed
                .typed_symbol(symbol)
                .is_some_and(|symbol| symbol.is_mutable)
        {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::BorrowMutability,
                "mutable borrow requires an owner declared with 'var[mut]'",
                origin,
            ));
        }
        super::bindings::reject_reborrow_source(typed, symbol, origin)?;
        let owner_type = typed
            .typed_symbol(symbol)
            .and_then(|symbol| symbol.declared_type)
            .ok_or_else(|| {
                with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::Internal,
                    "borrow operation owner lost its checked type",
                )
            })?;
        // For a bare identifier owner the borrowed type is the owner's value
        // type. For a place-borrow through an accessor (`obj.field`, `slot[]`,
        // `arr[i]`) the borrowed type is the accessed place's type instead.
        let inner = if matches!(
            super::helpers::strip_comments(operand),
            AstNode::Identifier { .. }
        ) {
            // Type the operand identifier so its reference retains a resolved
            // type for lowering. Without this, an explicit borrow used as a
            // postfix receiver (`([bor]x).method()` / `([bor]x).field`, a
            // canonical V3_MEM §2.2 form) fails lowering with L1002 even though
            // passing `[bor]x` as an argument works. The borrowed inner type is
            // still the owner's value type, not the plain read result.
            let _ = type_node(typed, resolved, context, operand)?;
            super::bindings::owned_or_borrowed_inner(typed, owner_type)
        } else {
            let accessed_raw = type_node(typed, resolved, context, operand)?;
            let accessed = plain_value_expr(
                typed,
                context,
                accessed_raw,
                node_origin(resolved, operand),
                "operand of a borrow operation",
            )?;
            accessed.required_value("borrow operation operand does not have a type")?
        };
        let borrowed = typed
            .type_table_mut()
            .intern(CheckedType::Borrowed { inner, mutable });
        return Ok(TypedExpr::value(borrowed));
    }

    // Value-source family: `[mov]`, `[cpy]`, `[cln]`, `[new, mov]`, `[new, cln]`,
    // `[fin]`. Type the operand once, then apply the transfer/allocation rule.
    let operand_raw = type_node(typed, resolved, context, operand)?;
    let operand_expr = plain_value_expr(
        typed,
        context,
        operand_raw,
        node_origin(resolved, operand),
        "operand of an ownership operation",
    )?;
    let operand_type =
        operand_expr.required_value("ownership operation operand does not have a type")?;

    if new && typed.capability_model() == crate::TypecheckCapabilityModel::Core {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::Unsupported,
            "heap allocation requires heap support; choose fol_model = 'memo'",
        ));
    }

    // `[mov]` and `[fin]` consume the source; `[cpy]`/`[cln]` leave it usable.
    if mov || finalize {
        super::bindings::track_value_transfer(
            typed,
            resolved,
            context,
            Some(operand),
            operand_type,
        )?;
    }

    // §2.1: an explicit `[mov]` invalidates the source even when the type is
    // copy-safe — the compiler must not silently turn `[mov]` into `[cpy]`. The
    // transfer tracking above marks only move-only types moved, so mark an
    // explicit `[mov]` of a bare binding (whole-value) or a direct field
    // (partial move) here so a later use is rejected. A sibling field stays
    // readable.
    if mov {
        let resolve_symbol = |syntax_id: fol_parser::ast::SyntaxNodeId| {
            resolved
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(syntax_id)
                        && reference.kind == fol_resolver::ReferenceKind::Identifier
                })
                .and_then(|reference| reference.resolved)
        };
        match super::helpers::strip_comments(operand) {
            AstNode::Identifier {
                syntax_id: Some(syntax_id),
                ..
            } => {
                if let (Some(symbol), Some(origin)) =
                    (resolve_symbol(*syntax_id), node_origin(resolved, operand))
                {
                    typed.mark_binding_moved(symbol, origin);
                }
            }
            AstNode::FieldAccess { object, field } => {
                if let AstNode::Identifier {
                    syntax_id: Some(syntax_id),
                    ..
                } = super::helpers::strip_comments(object)
                {
                    if let (Some(symbol), Some(origin)) =
                        (resolve_symbol(*syntax_id), node_origin(resolved, operand))
                    {
                        typed.mark_field_moved(symbol, field, origin);
                    }
                }
            }
            _ => {}
        }
    }

    let _ = expected_type;

    // Capability conformance (V3_MEM §4.1): `[cpy]` requires `copy`, `[cln]`
    // requires `clone`. Enforcement is conservative — only types that are
    // definitively non-copy / non-clone are rejected here; the full structural
    // and capability-declared conformance model is a later Slice B step.
    if copy && crate::decls::type_lacks_copy(typed, operand_type)? {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::Ownership,
            format!(
                "'[cpy]' requires the 'copy' capability, but '{}' is not copy-safe; use '[mov]' to transfer it or '[cln]' if it supports 'clone'",
                super::helpers::describe_type(typed, operand_type)
            ),
        ));
    }
    // A nominal user aggregate must DECLARE `copy` — copy has no structural
    // default (§4.1). An all-copy-safe record without a `(copy)` header is
    // move/clone-only, so `[cpy]record` on it is rejected.
    if copy && crate::decls::type_is_nominal_aggregate_lacking_copy(typed, operand_type)? {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::Ownership,
            format!(
                "'[cpy]' requires the 'copy' capability, but '{}' does not claim 'copy'; add a '(copy)' conformance header or use '[cln]'/'[mov]'",
                super::helpers::describe_type(typed, operand_type)
            ),
        ));
    }
    if clone && crate::decls::type_lacks_clone(typed, operand_type)? {
        return Err(with_node_origin(
            resolved,
            node,
            TypecheckErrorKind::Ownership,
            format!(
                "'[cln]' requires the 'clone' capability, but '{}' cannot be cloned; use '[mov]' to transfer it",
                super::helpers::describe_type(typed, operand_type)
            ),
        ));
    }

    if finalize {
        return Ok(TypedExpr::none());
    }

    let inner_view = super::bindings::owned_or_borrowed_inner(typed, operand_type);
    let result = if new {
        typed
            .type_table_mut()
            .intern(CheckedType::Owned { inner: inner_view })
    } else {
        operand_type
    };
    Ok(TypedExpr::value(result))
}

pub(crate) fn type_unary_op(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    op: &UnaryOperator,
    operand: &AstNode,
    expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    if matches!(op, UnaryOperator::GiveBack) {
        let AstNode::Identifier {
            syntax_id: Some(syntax_id),
            name,
        } = operand
        else {
            return Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::InvalidInput,
                "give-back requires a borrow binding identifier",
            ));
        };
        let symbol = resolved
            .references
            .iter()
            .find(|reference| {
                reference.syntax_id == Some(*syntax_id)
                    && reference.kind == ReferenceKind::Identifier
            })
            .and_then(|reference| reference.resolved)
            .ok_or_else(|| {
                with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::InvalidInput,
                    format!("give-back target '{name}' does not resolve to a borrow binding"),
                )
            })?;
        let origin = node_origin(resolved, node)
            .or_else(|| node_origin(resolved, operand))
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::Internal,
                    "give-back expression lost its syntax origin",
                )
            })?;
        // A deferred (`dfr`/`edf`) block that reads this borrower re-extends its
        // loan to the scope exit where the block runs, so the loan cannot end
        // early: emitted Rust keeps the reference alive across the give-back and
        // rejects any interleaving owner access. Reject the early give-back at
        // the source instead of silently disagreeing with the backend.
        if let Some(deferred_use) = typed.first_deferred_binding_use(symbol) {
            let deferred_origin = deferred_use.origin.clone();
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::BorrowReturned,
                format!(
                    "borrow '{name}' is captured by a deferred (dfr/edf) block and cannot be given back early; its loan lives until the block runs at scope exit"
                ),
                origin,
            )
            .with_related_origin(deferred_origin, "borrow captured by deferred block here"));
        }
        // A guard-VALUE binding (`var[mut, bor] guard = ([bor]mux).lock()`)
        // guards its mutex owner; giving the guard back releases the lock early
        // (V3_MEM §8.3: "'[end]guard' or NLL unlocks it"). Lowering emits a
        // matching `MutexUnlock` (drops the Rust guard now), so a subsequent
        // spawn/await/blocking-receive/blocking-select is no longer gated and
        // the emitted Rust still borrow-checks.
        let released_mutex = typed
            .typed_symbol(symbol)
            .is_some_and(|guard_symbol| guard_symbol.is_mutex_guard)
            .then(|| {
                typed
                    .active_borrow_binding(symbol)
                    .map(|borrow| borrow.owner)
            })
            .flatten();
        if !typed.give_back_borrow(symbol, origin.clone()) {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::BorrowReturned,
                format!("'{name}' is not an active borrow binding"),
                origin,
            ));
        }
        if let Some(mutex) = released_mutex {
            typed.release_mutex_guard(mutex, context.scope_id);
        }
        return Ok(TypedExpr::none());
    }

    if matches!(op, UnaryOperator::Unwrap) {
        let operand_expr = type_node(typed, resolved, observe_context(context), operand)?;
        if operand_expr.recoverable_effect.is_some() {
            return Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::Unsupported,
                "postfix '!' unwrap applies to opt[...] and err[...] shell values, not to routine call results with '/ ErrorType' in V1",
            ));
        }
        let operand_type =
            operand_expr.required_value("unary operator operand does not have a type")?;
        return if let Some(inner) = unwrap_shell_result_type(typed, operand_type)? {
            // Extracting a move-only shell payload consumes the shell even
            // when the unwrap appears as a standalone expression.
            super::bindings::track_value_transfer(
                typed,
                resolved,
                context,
                Some(operand),
                operand_type,
            )?;
            Ok(TypedExpr::value(inner))
        } else {
            Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::InvalidInput,
                "unwrap requires an opt[...] or err[...] shell with a value type in V1",
            ))
        };
    }

    if matches!(op, UnaryOperator::BorrowFrom) {
        let symbol = super::bindings::borrow_source_symbol(resolved, operand).ok_or_else(|| {
            with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::InvalidInput,
                "borrow-from requires an identifier owner",
            )
        })?;
        if let Some(move_origin) = typed.moved_binding_origin(symbol).cloned() {
            return Err(with_node_origin(
                resolved,
                node,
                TypecheckErrorKind::Ownership,
                "cannot borrow from an owner whose value was already moved",
            )
            .with_related_origin(move_origin, "ownership moved here"));
        }
        if let Some(conflict) = typed.active_borrow_for_owner(symbol).cloned() {
            if conflict.mutable {
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::BorrowConflict,
                    "shared borrow conflicts with an active mutable borrow of the same owner",
                )
                .with_related_origin(conflict.origin, "mutable borrow created here"));
            }
        }
        let owner_type = typed
            .typed_symbol(symbol)
            .and_then(|symbol| symbol.declared_type)
            .ok_or_else(|| {
                with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::Internal,
                    "borrow-from owner lost its checked type",
                )
            })?;
        super::bindings::reject_reborrow_source(
            typed,
            symbol,
            node_origin(resolved, node)
                .or_else(|| node_origin(resolved, operand))
                .ok_or_else(|| {
                    TypecheckError::new(
                        TypecheckErrorKind::Internal,
                        "borrow-from expression lost its syntax origin",
                    )
                })?,
        )?;
        let inner = super::bindings::owned_or_borrowed_inner(typed, owner_type);
        let borrowed = typed.type_table_mut().intern(CheckedType::Borrowed {
            inner,
            mutable: false,
        });
        return Ok(TypedExpr::value(borrowed));
    }

    let operand_raw = type_node(typed, resolved, context, operand)?;
    let operand_expr = plain_value_expr(
        typed,
        context,
        operand_raw,
        node_origin(resolved, operand),
        "plain use of an errorful expression",
    )?;
    let operand_type =
        operand_expr.required_value("unary operator operand does not have a type")?;
    let apparent = apparent_type_id(typed, operand_type)?;

    match op {
        UnaryOperator::Neg => match typed.type_table().get(apparent) {
            Some(CheckedType::Builtin(crate::BuiltinType::Int)) => {
                Ok(TypedExpr::value(typed.builtin_types().int)
                    .with_optional_effect(operand_expr.recoverable_effect))
            }
            Some(CheckedType::Builtin(crate::BuiltinType::Float)) => {
                Ok(TypedExpr::value(typed.builtin_types().float)
                    .with_optional_effect(operand_expr.recoverable_effect))
            }
            _ => Err(invalid_unary_operator_error(typed, op, operand_type)),
        },
        UnaryOperator::Not => {
            if apparent == typed.builtin_types().bool_ {
                Ok(TypedExpr::value(typed.builtin_types().bool_)
                    .with_optional_effect(operand_expr.recoverable_effect))
            } else {
                Err(invalid_unary_operator_error(typed, op, operand_type))
            }
        }
        UnaryOperator::Ref => {
            if typed.capability_model() == crate::TypecheckCapabilityModel::Core {
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::Unsupported,
                    "pointer construction requires heap support; choose fol_model = 'memo'",
                ));
            }
            // Pointer construction stores the operand by value. Unique values
            // therefore leave their source binding at this point rather than
            // being silently cloned into the new allocation.
            super::bindings::track_value_transfer(
                typed,
                resolved,
                context,
                Some(operand),
                operand_type,
            )?;
            let (shared, sync) = expected_type
                .and_then(|expected| typed.type_table().get(expected))
                .and_then(|expected| match expected {
                    CheckedType::Pointer { shared, sync, .. } => Some((*shared, *sync)),
                    _ => None,
                })
                .unwrap_or((false, false));
            let pointer = typed.type_table_mut().intern(CheckedType::Pointer {
                target: operand_type,
                shared,
                weak: false,
                sync,
            });
            Ok(TypedExpr::value(pointer))
        }
        UnaryOperator::Deref => match typed.type_table().get(apparent) {
            Some(CheckedType::Pointer {
                target,
                shared,
                weak,
                ..
            }) => {
                if *weak {
                    return Err(with_node_origin(
                        resolved,
                        operand,
                        TypecheckErrorKind::Ownership,
                        "a weak pointer 'ptr[weak, T]' cannot be dereferenced directly; upgrade it with '[upg]' to 'opt[ptr[shared, T]]' first",
                    ));
                }
                let target = *target;
                let shared = *shared;
                if super::bindings::ownership_moves_on_transfer(typed, operand_type)
                    && matches!(
                        super::helpers::strip_comments(operand),
                        AstNode::FieldAccess { .. }
                    )
                {
                    return Err(with_node_origin(
                        resolved,
                        operand,
                        TypecheckErrorKind::Ownership,
                        "dereferencing through a move-only field projection is not supported in V3; pointer observation must not partially move its source",
                    ));
                }
                if super::bindings::ownership_moves_on_transfer(typed, target) {
                    if matches!(
                        typed.type_table().get(operand_type),
                        Some(CheckedType::Borrowed { .. })
                    ) {
                        return Err(with_node_origin(
                            resolved,
                            node,
                            TypecheckErrorKind::Ownership,
                            "cannot move a move-only pointee through a borrowed pointer; borrowed pointer dereference is read-only",
                        ));
                    }
                    if shared {
                        return Err(with_node_origin(
                            resolved,
                            node,
                            TypecheckErrorKind::Ownership,
                            "cannot move a move-only pointee out of ptr[shared, T]; shared pointer dereference is read-only and requires a clone-safe pointee",
                        ));
                    }
                    // A unique pointer is the sole owner of its pointee. A
                    // by-value dereference of a move-only T therefore consumes
                    // the pointer and transfers T out of its allocation, just
                    // as moving out of a Box<T> does. Clone-safe pointees keep
                    // the existing observational dereference behavior.
                    super::bindings::track_value_transfer(
                        typed,
                        resolved,
                        context,
                        Some(operand),
                        operand_type,
                    )?;
                }
                Ok(TypedExpr::value(target))
            }
            _ => Err(invalid_unary_operator_error(typed, op, operand_type)),
        },
        UnaryOperator::BorrowFrom => unreachable!("borrow-from is handled before operand typing"),
        UnaryOperator::GiveBack => unreachable!("give-back is handled before unary typing"),
        UnaryOperator::Unwrap => unreachable!("unwrap is handled before plain unary typing"),
    }
}
