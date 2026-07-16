use crate::{
    model::{ResolvedProgram, ScopeKind, SymbolKind},
    ResolverError, ResolverSession, ScopeId, SourceUnitId,
};
use fol_parser::ast::{AstNode, LoopCondition, WhenCase};

use super::super::scope::insert_local_symbol;
use super::RoutineContext;

// These parameters mirror the statement AST fields plus resolver context.
#[allow(clippy::too_many_arguments)]
pub fn traverse_when_node(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expr: &AstNode,
    cases: &[WhenCase],
    default: &Option<Vec<AstNode>>,
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    super::traverse_node(
        session,
        program,
        source_unit_id,
        scope_id,
        expr,
        false,
        routine_context,
    )?;
    for case in cases {
        match case {
            WhenCase::Case { condition, body } => {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    condition,
                    false,
                    routine_context,
                )?;
                super::traverse_block_body(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    None,
                    body,
                    routine_context,
                )?;
            }
            WhenCase::Is { value, body }
            | WhenCase::In { range: value, body }
            | WhenCase::Has {
                member: value,
                body,
            }
            | WhenCase::On {
                channel: value,
                body,
            } => {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    value,
                    false,
                    routine_context,
                )?;
                super::traverse_block_body(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    None,
                    body,
                    routine_context,
                )?;
            }
            WhenCase::Of { type_match, body } => {
                super::types::resolve_type_reference(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    type_match,
                )?;
                super::traverse_block_body(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    None,
                    body,
                    routine_context,
                )?;
            }
        }
    }
    if let Some(default_body) = default {
        super::traverse_block_body(
            session,
            program,
            source_unit_id,
            scope_id,
            None,
            default_body,
            routine_context,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn traverse_loop_node(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    condition: &LoopCondition,
    body: &[AstNode],
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    match condition {
        LoopCondition::Condition(cond) => {
            super::traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                cond,
                false,
                routine_context,
            )?;
            let body_scope = program.add_scope(ScopeKind::Block, scope_id, source_unit_id);
            program.record_scope_for_syntax(syntax_id, body_scope);
            for statement in body {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    body_scope,
                    statement,
                    false,
                    routine_context,
                )?;
            }
        }
        LoopCondition::Iteration {
            var,
            type_hint,
            iterable,
            condition,
            ..
        } => {
            if let Some(type_hint) = type_hint {
                super::types::resolve_type_reference(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    type_hint,
                )?;
            }
            super::traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                iterable,
                false,
                routine_context,
            )?;
            let binder_scope = program.add_scope(ScopeKind::LoopBinder, scope_id, source_unit_id);
            program.record_scope_for_syntax(syntax_id, binder_scope);
            insert_local_symbol(
                program,
                source_unit_id,
                binder_scope,
                var,
                SymbolKind::LoopBinder,
                format!("symbol#{}", fol_types::canonical_identifier_key(var)),
            )?;
            if let Some(condition) = condition {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    binder_scope,
                    condition,
                    false,
                    routine_context,
                )?;
            }
            for statement in body {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    binder_scope,
                    statement,
                    false,
                    routine_context,
                )?;
            }
        }
    }
    Ok(())
}
