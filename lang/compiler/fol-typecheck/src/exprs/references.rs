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
        if let Some(move_origin) = typed.moved_binding_origin(symbol).cloned() {
            let mut error = origin_for(resolved, syntax_id).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::Ownership,
                        format!("use of moved heap-owned binding '{name}'"),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        format!("use of moved heap-owned binding '{name}'"),
                        origin,
                    )
                },
            );
            error = error.with_related_origin(move_origin, "ownership moved here");
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
