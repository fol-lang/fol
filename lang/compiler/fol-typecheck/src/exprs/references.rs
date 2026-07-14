use crate::{TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{QualifiedPath, SyntaxNodeId};
use fol_resolver::{ReferenceKind, ResolvedProgram};

use super::calls::{find_reference_by_syntax, type_for_reference};
use super::helpers::origin_for;
use super::{TypeContext, TypedExpr};

pub(crate) fn type_identifier_reference(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    name: &str,
    syntax_id: Option<SyntaxNodeId>,
) -> Result<TypedExpr, TypecheckError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("identifier '{name}' does not retain a syntax id"),
        )
    })?;
    let reference_id =
        find_reference_by_syntax(resolved, syntax_id, ReferenceKind::Identifier, name)?;
    if let Some(symbol) = resolved
        .reference(reference_id)
        .and_then(|reference| reference.resolved)
    {
        if context.inside_error_deferred_block
            && typed
                .typed_symbol(symbol)
                .and_then(|symbol| symbol.declared_type)
                .and_then(|type_id| typed.type_table().get(type_id))
                .is_some_and(|typ| matches!(typ, crate::CheckedType::Eventual { .. }))
        {
            return Err(origin_for(resolved, syntax_id).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        format!(
                            "eventual binding '{name}' cannot be accessed inside edf in V3; await or transfer it in ordinary control flow"
                        ),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        format!(
                            "eventual binding '{name}' cannot be accessed inside edf in V3; await or transfer it in ordinary control flow"
                        ),
                        origin,
                    )
                },
            ));
        }
        if typed
            .typed_symbol(symbol)
            .is_some_and(|symbol| symbol.is_mutex)
            && !context.allow_mutex_handle
        {
            return Err(origin_for(resolved, syntax_id).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!(
                            "mutex parameter '{name}' cannot be used as an unguarded whole value; access fields after '{name}.lock()' or pass the handle to another [mux] parameter"
                        ),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!(
                            "mutex parameter '{name}' cannot be used as an unguarded whole value; access fields after '{name}.lock()' or pass the handle to another [mux] parameter"
                        ),
                        origin,
                    )
                },
            ));
        }
        if let Some(move_origin) = typed.moved_binding_origin(symbol).cloned() {
            let eventual = typed
                .typed_symbol(symbol)
                .and_then(|symbol| symbol.declared_type)
                .and_then(|type_id| typed.type_table().get(type_id))
                .is_some_and(|typ| matches!(typ, crate::CheckedType::Eventual { .. }));
            let channel = typed
                .typed_symbol(symbol)
                .and_then(|symbol| symbol.declared_type)
                .and_then(|type_id| typed.type_table().get(type_id))
                .is_some_and(|typ| matches!(typ, crate::CheckedType::Channel { .. }));
            let eventual_move = typed.eventual_move_kind(symbol);
            let message = match eventual_move {
                Some(crate::model::EventualMoveKind::Await) => {
                    format!("use of consumed eventual binding '{name}'")
                }
                Some(crate::model::EventualMoveKind::Transfer) => {
                    format!("use of moved eventual binding '{name}'")
                }
                None if eventual => format!("use of moved eventual binding '{name}'"),
                None if channel => format!("use of moved channel receiver binding '{name}'"),
                None => format!("use of moved heap-owned binding '{name}'"),
            };
            let mut error = origin_for(resolved, syntax_id).map_or_else(
                || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        message.clone(),
                        origin,
                    )
                },
            );
            error = error.with_related_origin(
                move_origin,
                match eventual_move {
                    Some(crate::model::EventualMoveKind::Await) => {
                        "eventual consumed by await here"
                    }
                    Some(crate::model::EventualMoveKind::Transfer) => {
                        "eventual ownership transferred here"
                    }
                    None => "ownership moved here",
                },
            );
            return Err(error);
        }
        if let Some(borrow) = typed.active_borrow_for_owner(symbol).cloned() {
            let mut error = origin_for(resolved, syntax_id).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::OwnerBorrowed,
                        format!("owner '{name}' is inaccessible while borrowed"),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::OwnerBorrowed,
                        format!("owner '{name}' is inaccessible while borrowed"),
                        origin,
                    )
                },
            );
            error = error.with_related_origin(borrow.origin, "borrow created here");
            return Err(error);
        }
        if let Some(return_origin) = typed.returned_borrow_origin(symbol).cloned() {
            let mut error = origin_for(resolved, syntax_id).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::BorrowReturned,
                        format!("borrow binding '{name}' was already returned"),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::BorrowReturned,
                        format!("borrow binding '{name}' was already returned"),
                        origin,
                    )
                },
            );
            error = error.with_related_origin(return_origin, "borrow returned here");
            return Err(error);
        }
    }
    let typed_expr = type_for_reference(
        typed,
        resolved,
        reference_id,
        origin_for(resolved, syntax_id),
    )?;
    if let Some(type_id) = typed_expr.value_type {
        typed.record_node_type(syntax_id, context.source_unit_id, type_id)?;
    }
    if let Some(effect) = typed_expr.recoverable_effect {
        typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
    }
    Ok(typed_expr)
}

pub(crate) fn type_qualified_identifier_reference(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    path: &QualifiedPath,
) -> Result<TypedExpr, TypecheckError> {
    let syntax_id = path.syntax_id().ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "qualified identifier '{}' does not retain a syntax id",
                path.joined()
            ),
        )
    })?;
    let reference_id = find_reference_by_syntax(
        resolved,
        syntax_id,
        ReferenceKind::QualifiedIdentifier,
        &path.joined(),
    )?;
    let typed_expr = type_for_reference(
        typed,
        resolved,
        reference_id,
        origin_for(resolved, syntax_id),
    )?;
    if let Some(type_id) = typed_expr.value_type {
        typed.record_node_type(syntax_id, context.source_unit_id, type_id)?;
    }
    if let Some(effect) = typed_expr.recoverable_effect {
        typed.record_node_recoverable_effect(syntax_id, context.source_unit_id, effect)?;
    }
    Ok(typed_expr)
}
