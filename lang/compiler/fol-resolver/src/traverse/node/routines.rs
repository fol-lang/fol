use crate::{
    model::{ResolvedProgram, ScopeKind, SymbolKind},
    ResolverError, ResolverSession, ScopeId, SourceUnitId,
};
use fol_parser::ast::{AstNode, FolType, Generic, Parameter, SyntaxNodeId};

use super::super::scope::{insert_generic_symbols, insert_local_symbol, insert_local_symbol_with_origin};
use super::types::resolve_type_reference;
use super::RoutineContext;

pub fn traverse_named_routine(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: &Option<SyntaxNodeId>,
    generics: &[Generic],
    receiver_type: &Option<FolType>,
    captures: &[String],
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
            resolve_type_reference(
                session,
                program,
                source_unit_id,
                routine_scope,
                constraint,
            )?;
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
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            return_type,
        )?;
    }
    if let Some(error_type) = error_type {
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            error_type,
        )?;
    }

    for capture in captures {
        insert_local_symbol(
            program,
            source_unit_id,
            routine_scope,
            capture,
            SymbolKind::Capture,
            format!("symbol#{}", fol_types::canonical_identifier_key(capture)),
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
            header_origin.clone(),
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

pub fn traverse_anonymous_routine(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: &Option<SyntaxNodeId>,
    captures: &[String],
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
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            return_type,
        )?;
    }
    if let Some(error_type) = error_type {
        resolve_type_reference(
            session,
            program,
            source_unit_id,
            routine_scope,
            error_type,
        )?;
    }

    for capture in captures {
        insert_local_symbol(
            program,
            source_unit_id,
            routine_scope,
            capture,
            SymbolKind::Capture,
            format!("symbol#{}", fol_types::canonical_identifier_key(capture)),
        )?;
    }

    for param in params {
        insert_local_symbol(
            program,
            source_unit_id,
            routine_scope,
            &param.name,
            SymbolKind::Parameter,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&param.name)
            ),
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
