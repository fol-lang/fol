use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, BinaryOperator, ChannelEndpoint, UnaryOperator};
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
            super::bindings::mark_plain_identifier_move(
                typed,
                resolved,
                context,
                Some(left),
                value_type,
            )?;
            return Ok(TypedExpr::none());
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
                AstNode::FunctionCall { .. }
            ) {
                return Err(unsupported_binary_surface(
                    resolved,
                    left,
                    right,
                    "| async currently requires a direct routine call on its left side",
                ));
            }
            super::reject_direct_spawn_channel_receiver(typed, resolved, left)?;
            let observed = type_node(
                typed,
                resolved,
                TypeContext {
                    error_call_mode: super::ErrorCallMode::Observe,
                    ..context
                },
                left,
            )?;
            super::apply_spawn_argument_boundary(typed, resolved, left)?;
            let value_type = observed.value_type.ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "| async requires a call that yields a value",
                )
            })?;
            let error_type = observed.recoverable_effect.map(|effect| effect.error_type);
            if super::helpers::type_contains_shared_pointer(typed, value_type)
                || error_type.is_some_and(|error| {
                    super::helpers::type_contains_shared_pointer(typed, error)
                })
            {
                let message =
                    "an async result containing shared Rc pointers cannot cross the task boundary";
                return Err(node_origin(resolved, left).map_or_else(
                    || TypecheckError::new(TypecheckErrorKind::Ownership, message),
                    |origin| {
                        TypecheckError::with_origin(
                            TypecheckErrorKind::Ownership,
                            message,
                            origin,
                        )
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
        BinaryOperator::Pipe => Err(unsupported_binary_surface(
            resolved,
            left,
            right,
            "pipe operator '|>' is not yet supported",
        )),
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
            "'||' handles routine call results with '/ ErrorType', not err[...] shell values in V1"
        } else {
            "'||' requires a routine call result with '/ ErrorType' on the left in V1"
        };
        return Err(node_origin(resolved, left).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
        ));
    }

    let fallback = type_node_with_expectation(typed, resolved, context, right, Some(success_type))?;
    let fallback = plain_value_expr(
        typed,
        context,
        fallback,
        node_origin(resolved, right),
        "recoverable-error fallback",
    )?;

    match fallback.value_type {
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
    }
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
        if !typed.give_back_borrow(symbol, origin.clone()) {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::BorrowReturned,
                format!("'{name}' is not an active borrow binding"),
                origin,
            ));
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
                    "pointer construction requires heap support; choose fol_model = 'memo' or add bundled std",
                ));
            }
            let shared = expected_type
                .and_then(|expected| typed.type_table().get(expected))
                .is_some_and(|expected| {
                    matches!(expected, CheckedType::Pointer { shared: true, .. })
                });
            let pointer = typed.type_table_mut().intern(CheckedType::Pointer {
                target: operand_type,
                shared,
            });
            Ok(TypedExpr::value(pointer))
        }
        UnaryOperator::Deref => match typed.type_table().get(operand_type) {
            Some(CheckedType::Pointer { target, .. }) => Ok(TypedExpr::value(*target)),
            _ => Err(invalid_unary_operator_error(typed, op, operand_type)),
        },
        UnaryOperator::BorrowFrom => unreachable!("borrow-from is handled before operand typing"),
        UnaryOperator::GiveBack => unreachable!("give-back is handled before unary typing"),
        UnaryOperator::Unwrap => unreachable!("unwrap is handled before plain unary typing"),
    }
}
