use crate::{
    model::{ResolvedProgram, ScopeKind, SymbolKind},
    ResolverError, ResolverSession, ScopeId, SourceUnitId,
};
use fol_parser::ast::{AstNode, FolType, Generic, Parameter, RoutineCapture, SyntaxNodeId};

use super::super::scope::{insert_generic_symbols, insert_local_symbol_with_origin};
use super::types::resolve_type_reference;
use super::RoutineContext;

// Traversal parameters mirror the routine AST fields plus resolver context;
// grouping them would add a second, duplicative routine representation.
#[allow(clippy::too_many_arguments)]
pub fn traverse_named_routine(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: &Option<SyntaxNodeId>,
    generics: &[Generic],
    receiver_type: &Option<FolType>,
    captures: &[RoutineCapture],
    params: &[Parameter],
    return_type: &Option<FolType>,
    error_type: &Option<FolType>,
    body: &[AstNode],
    inquiries: &[AstNode],
) -> Result<(), ResolverError> {
    let routine_scope = program.add_scope(ScopeKind::Routine, scope_id, source_unit_id);
    let nested_routine_context = Some(RoutineContext {
        this_available: return_type.is_some(),
    });
    program.record_scope_for_syntax(*syntax_id, routine_scope);
    // Parameters and the receiver binding are declared by the routine header,
    // so they borrow its origin for editor navigation.
    let header_origin = syntax_id
        .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
        .cloned();

    insert_generic_symbols(program, source_unit_id, routine_scope, generics)?;
    for generic in generics {
        for constraint in &generic.constraints {
            if super::types::constraint_is_capability_standard(constraint) {
                continue;
            }
            resolve_type_reference(session, program, source_unit_id, routine_scope, constraint)?;
        }
    }
    if let Some(receiver_type) = receiver_type {
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            receiver_type,
        )?;
    }
    for param in params {
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            &param.param_type,
        )?;
    }
    if let Some(return_type) = return_type {
        resolve_type_reference(session, program, source_unit_id, routine_scope, return_type)?;
    }
    if let Some(error_type) = error_type {
        resolve_type_reference(session, program, source_unit_id, routine_scope, error_type)?;
    }

    for capture in captures {
        let capture_origin = capture
            .syntax_id
            .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
            .cloned();
        record_capture_source_reference(program, source_unit_id, scope_id, capture);
        insert_local_symbol_with_origin(
            program,
            source_unit_id,
            routine_scope,
            &capture.name,
            SymbolKind::Capture,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&capture.name)
            ),
            capture_origin,
        )?;
    }
    if receiver_type.is_some() {
        insert_local_symbol_with_origin(
            program,
            source_unit_id,
            routine_scope,
            "self",
            SymbolKind::Parameter,
            format!("symbol#{}", fol_types::canonical_identifier_key("self")),
            header_origin.clone(),
        )?;
    }
    for param in params {
        // A parameter now carries its own NAME syntax id, so its symbol origin
        // points at the parameter declaration span (not the routine header).
        // The synthesized `self` receiver above keeps the header origin because
        // it has no source token of its own.
        let param_origin = param
            .syntax_id
            .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
            .cloned()
            .or_else(|| header_origin.clone());
        insert_local_symbol_with_origin(
            program,
            source_unit_id,
            routine_scope,
            &param.name,
            SymbolKind::Parameter,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&param.name)
            ),
            param_origin,
        )?;
    }

    for statement in body {
        super::traverse_node(
            session,
            program,
            source_unit_id,
            routine_scope,
            statement,
            false,
            nested_routine_context,
        )?;
    }
    for inquiry in inquiries {
        super::traverse_node(
            session,
            program,
            source_unit_id,
            routine_scope,
            inquiry,
            false,
            nested_routine_context,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn traverse_anonymous_routine(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: &Option<SyntaxNodeId>,
    captures: &[RoutineCapture],
    params: &[Parameter],
    return_type: &Option<FolType>,
    error_type: &Option<FolType>,
    body: &[AstNode],
    inquiries: &[AstNode],
) -> Result<(), ResolverError> {
    let routine_scope = program.add_scope(ScopeKind::Routine, scope_id, source_unit_id);
    let nested_routine_context = Some(RoutineContext {
        this_available: return_type.is_some(),
    });
    program.record_scope_for_syntax(*syntax_id, routine_scope);

    for param in params {
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            &param.param_type,
        )?;
    }
    if let Some(return_type) = return_type {
        resolve_type_reference(session, program, source_unit_id, routine_scope, return_type)?;
    }
    if let Some(error_type) = error_type {
        resolve_type_reference(session, program, source_unit_id, routine_scope, error_type)?;
    }

    for capture in captures {
        let capture_origin = capture
            .syntax_id
            .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
            .cloned();
        record_capture_source_reference(program, source_unit_id, scope_id, capture);
        insert_local_symbol_with_origin(
            program,
            source_unit_id,
            routine_scope,
            &capture.name,
            SymbolKind::Capture,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&capture.name)
            ),
            capture_origin,
        )?;
    }

    for param in params {
        // Anonymous routine parameters carry their own NAME syntax id too, so
        // give the parameter symbol its true declaration origin.
        let param_origin = param
            .syntax_id
            .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
            .cloned();
        insert_local_symbol_with_origin(
            program,
            source_unit_id,
            routine_scope,
            &param.name,
            SymbolKind::Parameter,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&param.name)
            ),
            param_origin,
        )?;
    }

    for statement in body {
        super::traverse_node(
            session,
            program,
            source_unit_id,
            routine_scope,
            statement,
            false,
            nested_routine_context,
        )?;
    }
    for inquiry in inquiries {
        super::traverse_node(
            session,
            program,
            source_unit_id,
            routine_scope,
            inquiry,
            false,
            nested_routine_context,
        )?;
    }

    Ok(())
}

/// Link a capture-list entry back to the binding it captures: record an
/// identifier reference at the capture site resolved against the ENCLOSING
/// scope. The capture's own `SymbolKind::Capture` binding (inserted right
/// after) shadows the outer name inside the routine body, so without this
/// reference the outer binding and its captures form disconnected symbol
/// clusters and editor rename/references miss the capture site entirely.
/// Resolution failures are left for typecheck to report; the reference is
/// purely navigational.
fn record_capture_source_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    enclosing_scope: ScopeId,
    capture: &RoutineCapture,
) {
    let Some(syntax_id) = capture.syntax_id else {
        return;
    };
    let origin = program.syntax_index().origin(syntax_id).cloned();
    let Ok(outer_symbol) = super::super::resolve::resolve_visible_symbol_of_kinds(
        program,
        enclosing_scope,
        &capture.name,
        &[
            SymbolKind::ValueBinding,
            SymbolKind::Parameter,
            SymbolKind::Capture,
        ],
        Some("captured binding"),
        origin,
    ) else {
        return;
    };
    let reference_id = program.references.push(crate::model::ResolvedReference {
        id: crate::ReferenceId(0),
        kind: crate::model::ReferenceKind::Identifier,
        syntax_id: Some(syntax_id),
        anchor_syntax_id: None,
        name: capture.name.clone(),
        scope: enclosing_scope,
        source_unit: source_unit_id,
        resolved: Some(outer_symbol),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
}
