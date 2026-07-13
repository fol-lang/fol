use super::bindings::lower_local_binding;
use super::calls::{
    lower_keyword_intrinsic_statement, lower_spawn_call, lower_statement_free_call,
    resolve_method_target,
};
use super::cursor::{DeferScopeKind, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::{
    direct_local_identifier_value, lower_channel_send, lower_expression, lower_expression_expected,
};
use super::flow::{lower_loop_statement, lower_when_statement, when_always_terminates};
use crate::{ids::LoweredTypeId, LoweredPackage, LoweredRoutine, LoweringError, LoweringErrorKind};
use fol_intrinsics::{select_intrinsic, IntrinsicSurface};
use fol_parser::ast::{AstNode, BinaryOperator, ChannelEndpoint};
use fol_resolver::{PackageIdentity, ReferenceKind, ScopeId, SourceUnitId, SymbolKind};
use std::collections::BTreeMap;

pub(crate) fn lower_routine_bodies(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    decl_index: &WorkspaceDeclIndex,
    lowered_package: &mut LoweredPackage,
    next_routine_index: &mut usize,
) -> Result<(), Vec<LoweringError>> {
    let mut errors = Vec::new();

    for (source_unit_index, source_unit) in typed_package
        .program
        .resolved()
        .syntax()
        .source_units
        .iter()
        .enumerate()
    {
        if source_unit.kind == fol_parser::ast::ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        for item in &source_unit.items {
            // Collect routine bodies to lower. Most come from top-level
            // routine decls, but standard default bodies are nested inside
            // `std` decls and need the same treatment so method resolution
            // can reach their bodies at call sites.
            let member_iter: Vec<&AstNode> = if let AstNode::StdDecl { body, .. } = &item.node {
                body.iter().collect()
            } else {
                vec![&item.node]
            };
            for member in member_iter {
                let (name, syntax_id, body) = match member {
                    AstNode::FunDecl {
                        name,
                        syntax_id,
                        body,
                        ..
                    }
                    | AstNode::ProDecl {
                        name,
                        syntax_id,
                        body,
                        ..
                    }
                    | AstNode::LogDecl {
                        name,
                        syntax_id,
                        body,
                        ..
                    } => (name.as_str(), *syntax_id, body.as_slice()),
                    AstNode::Commented { node, .. } => match node.as_ref() {
                        AstNode::FunDecl {
                            name,
                            syntax_id,
                            body,
                            ..
                        }
                        | AstNode::ProDecl {
                            name,
                            syntax_id,
                            body,
                            ..
                        }
                        | AstNode::LogDecl {
                            name,
                            syntax_id,
                            body,
                            ..
                        } => (name.as_str(), *syntax_id, body.as_slice()),
                        _ => continue,
                    },
                    _ => continue,
                };
                if body.is_empty() {
                    // Signature-only members (e.g., bare protocol standard
                    // requirements with no default body) have nothing to
                    // lower into a routine body.
                    continue;
                }
                let Some(symbol_id) = crate::decls::find_routine_symbol_for_item(
                    &typed_package.program,
                    source_unit_id,
                    name,
                    syntax_id,
                ) else {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("routine '{name}' does not retain a local lowering symbol"),
                    ));
                    continue;
                };
                let Some(scope_id) = syntax_id.and_then(|syntax_id| {
                    typed_package.program.resolved().scope_for_syntax(syntax_id)
                }) else {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("routine '{name}' does not retain typed scope information"),
                    ));
                    continue;
                };
                let Some(routine_id) =
                    lowered_package
                        .routine_decls
                        .iter()
                        .find_map(|(routine_id, routine)| {
                            (routine.symbol_id == Some(symbol_id)).then_some(*routine_id)
                        })
                else {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("routine '{name}' does not map to a lowered routine shell"),
                    ));
                    continue;
                };
                let Some(routine) = lowered_package.routine_decls.get_mut(&routine_id) else {
                    continue;
                };
                match lower_body_nodes(
                    typed_package,
                    type_table,
                    &lowered_package.checked_type_map,
                    lowered_package.identity.clone(),
                    decl_index,
                    routine,
                    source_unit_id,
                    scope_id,
                    body,
                    next_routine_index,
                ) {
                    Ok(anonymous_routines) => {
                        for anon in anonymous_routines {
                            lowered_package.routines.push(anon.id);
                            lowered_package.routine_decls.insert(anon.id, anon);
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn lower_body_nodes(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    routine: &mut LoweredRoutine,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    nodes: &[AstNode],
    next_routine_index: &mut usize,
) -> Result<Vec<LoweredRoutine>, LoweringError> {
    let entry_block = routine.entry_block;
    let mut cursor = RoutineCursor::new(routine, entry_block);
    cursor.next_routine_index = *next_routine_index;
    cursor.routine.body_result = lower_body_sequence(
        typed_package,
        type_table,
        checked_type_map,
        &current_identity,
        decl_index,
        &mut cursor,
        source_unit_id,
        scope_id,
        nodes,
        DeferScopeKind::Ordinary,
    )?
    .map(|value| value.local_id);

    *next_routine_index = cursor.next_routine_index;
    Ok(cursor.anonymous_routines)
}

pub(crate) fn lower_body_sequence(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    nodes: &[AstNode],
    defer_scope_kind: DeferScopeKind,
) -> Result<Option<super::cursor::LoweredValue>, LoweringError> {
    let entry_depth = cursor.defer_scope_depth();
    cursor.push_defer_scope(defer_scope_kind);
    let mut final_value = None;

    for node in nodes {
        if let Some(value) = lower_body_node(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            node,
        )? {
            final_value = Some(value);
        }
        if cursor.current_block_terminated()? {
            break;
        }
    }

    if cursor.defer_scope_depth() > entry_depth {
        let deferred = cursor.pop_defer_scope().ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "active dfr scope disappeared before body finished lowering",
            )
        })?;
        if !cursor.current_block_terminated()? {
            lower_deferred_entries(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                deferred.entries,
                false,
            )?;
            lower_mutex_unlocks(cursor, deferred.mutex_guards)?;
            lower_lexical_drops(cursor, deferred.lexical_drops)?;
        }
    }

    Ok(final_value)
}

fn lower_lexical_drops(
    cursor: &mut RoutineCursor<'_>,
    locals: Vec<crate::LoweredLocalId>,
) -> Result<(), LoweringError> {
    if cursor.current_block_terminated()? {
        return Ok(());
    }
    for local in locals.into_iter().rev() {
        cursor.push_instr(None, crate::LoweredInstrKind::DropLocal { local })?;
    }
    Ok(())
}

fn lower_mutex_unlocks(
    cursor: &mut RoutineCursor<'_>,
    mutexes: Vec<crate::LoweredLocalId>,
) -> Result<(), LoweringError> {
    if cursor.current_block_terminated()? {
        return Ok(());
    }
    for mutex in mutexes.into_iter().rev() {
        cursor.push_instr(None, crate::LoweredInstrKind::MutexUnlock { mutex })?;
    }
    Ok(())
}

fn lower_deferred_entries(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    entries: Vec<super::cursor::DeferredBody>,
    error_exit: bool,
) -> Result<(), LoweringError> {
    for deferred in entries
        .into_iter()
        .rev()
        .filter(|deferred| error_exit || !deferred.error_only)
    {
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            deferred.source_unit_id,
            deferred.scope_id,
            &deferred.body,
            DeferScopeKind::Ordinary,
        )?;
        if cursor.current_block_terminated()? {
            break;
        }
    }
    Ok(())
}

fn nested_scope_for_syntax(
    typed_package: &fol_typecheck::TypedPackage,
    syntax_id: Option<fol_parser::ast::syntax::SyntaxNodeId>,
    parent_scope_id: ScopeId,
    construct_name: &str,
) -> Result<ScopeId, LoweringError> {
    let Some(syntax_id) = syntax_id else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("{construct_name} is missing syntax identity for lowering"),
        ));
    };
    let Some(scope_id) = typed_package.program.resolved().scope_for_syntax(syntax_id) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("{construct_name} is missing a resolved block scope for lowering"),
        ));
    };
    let Some(scope) = typed_package.program.resolved().scope(scope_id) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("{construct_name} resolved to unknown scope {}", scope_id.0),
        ));
    };
    if scope.parent != Some(parent_scope_id) {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "{construct_name} resolved scope {} does not belong to parent scope {}",
                scope_id.0, parent_scope_id.0
            ),
        ));
    }
    Ok(scope_id)
}

fn lower_all_active_defers(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    error_exit: bool,
) -> Result<(), LoweringError> {
    for scope in cursor.defer_scopes_snapshot().into_iter().rev() {
        lower_deferred_entries(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            scope.entries,
            error_exit,
        )?;
        if cursor.current_block_terminated()? {
            break;
        }
        lower_mutex_unlocks(cursor, scope.mutex_guards)?;
        lower_lexical_drops(cursor, scope.lexical_drops)?;
    }
    Ok(())
}

fn lower_defers_until_loop_exit(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
) -> Result<(), LoweringError> {
    let Some(loop_depth) = cursor.nearest_loop_defer_depth() else {
        return Ok(());
    };
    let scopes = cursor.defer_scopes_snapshot();
    for scope in scopes.into_iter().skip(loop_depth).rev() {
        lower_deferred_entries(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            scope.entries,
            false,
        )?;
        if cursor.current_block_terminated()? {
            return Ok(());
        }
        lower_mutex_unlocks(cursor, scope.mutex_guards)?;
        lower_lexical_drops(cursor, scope.lexical_drops)?;
    }
    Ok(())
}

pub(crate) fn lower_report_terminator(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    lowered: Option<crate::ids::LoweredLocalId>,
) -> Result<(), LoweringError> {
    lower_all_active_defers(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        true,
    )?;
    cursor.terminate_current_block(crate::LoweredTerminator::Report { value: lowered })?;
    Ok(())
}

pub(crate) fn lower_panic_terminator(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    lowered: Option<crate::ids::LoweredLocalId>,
) -> Result<(), LoweringError> {
    lower_all_active_defers(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        false,
    )?;
    cursor.terminate_current_block(crate::LoweredTerminator::Panic { value: lowered })?;
    Ok(())
}

pub(crate) fn lower_body_node(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    node: &AstNode,
) -> Result<Option<super::cursor::LoweredValue>, LoweringError> {
    match node {
        AstNode::Comment { .. } => Ok(None),
        AstNode::Commented { node, .. } => lower_body_node(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            node,
        ),
        AstNode::VarDecl { name, value, .. } => lower_local_binding(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            name,
            SymbolKind::ValueBinding,
            value.as_deref(),
        ),
        AstNode::LabDecl { name, value, .. } => lower_local_binding(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            name,
            SymbolKind::LabelBinding,
            value.as_deref(),
        ),
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::GiveBack,
            operand,
        } => {
            let AstNode::Identifier {
                syntax_id: Some(syntax_id),
                name,
            } = operand.as_ref()
            else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "give-back requires a borrow binding identifier",
                ));
            };
            let symbol = typed_package
                .program
                .resolved()
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(*syntax_id)
                        && reference.kind == ReferenceKind::Identifier
                })
                .and_then(|reference| reference.resolved)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("give-back target '{name}' does not resolve"),
                    )
                })?;
            let borrow = cursor
                .routine
                .local_symbols
                .get(&symbol)
                .copied()
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("give-back target '{name}' is not a lowered local"),
                    )
                })?;
            cursor.push_instr(None, crate::LoweredInstrKind::GiveBackBorrow { borrow })?;
            Ok(None)
        }
        AstNode::Return { value } => match value.as_deref() {
            Some(value) => {
                let lowered = lower_expression_expected(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    routine_return_type(cursor, type_table),
                    value,
                )?;
                lower_all_active_defers(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    false,
                )?;
                cursor.terminate_current_block(crate::LoweredTerminator::Return {
                    value: Some(lowered.local_id),
                })?;
                Ok(None)
            }
            None => {
                lower_all_active_defers(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    false,
                )?;
                cursor.terminate_current_block(crate::LoweredTerminator::Return { value: None })?;
                Ok(None)
            }
        },
        AstNode::FunctionCall { name, args, .. } if name == "report" => {
            let lowered = match args.as_slice() {
                [value] => Some(
                    lower_expression_expected(
                        typed_package,
                        type_table,
                        checked_type_map,
                        current_identity,
                        decl_index,
                        cursor,
                        source_unit_id,
                        scope_id,
                        routine_error_type(cursor, type_table),
                        value,
                    )?
                    .local_id,
                ),
                [] => None,
                _ => {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("report expects exactly 1 value, got {}", args.len()),
                    ))
                }
            };
            lower_report_terminator(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                lowered,
            )?;
            Ok(None)
        }
        AstNode::FunctionCall {
            surface: fol_parser::ast::CallSurface::DotIntrinsic,
            ..
        } => {
            // Statement-position dot intrinsics (`.echo(x);`) lower through
            // the expression path — they never carry resolver references.
            let _ = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                node,
            )?;
            Ok(None)
        }
        AstNode::Spawn { task } => {
            lower_spawn_call(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                task,
            )?;
            Ok(None)
        }
        AstNode::Select { arms, default } => {
            super::flow::lower_select_statement(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                arms,
                default.as_deref(),
            )?;
            Ok(None)
        }
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            left,
            right,
        } if matches!(
            right.as_ref(),
            AstNode::ChannelAccess {
                endpoint: ChannelEndpoint::Tx,
                ..
            }
        ) =>
        {
            let AstNode::ChannelAccess { channel, .. } = right.as_ref() else {
                unreachable!("channel-send guard preserves the endpoint shape")
            };
            lower_channel_send(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                left,
                channel,
            )?;
            Ok(None)
        }
        AstNode::FunctionCall {
            syntax_id,
            name,
            args,
            ..
        } => {
            if let Ok(entry) = select_intrinsic(IntrinsicSurface::KeywordCall, name) {
                lower_keyword_intrinsic_statement(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    entry,
                    *syntax_id,
                    args,
                )
            } else {
                lower_statement_free_call(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    *syntax_id,
                    ReferenceKind::FunctionCall,
                    name,
                    args,
                )
            }
        }
        AstNode::QualifiedFunctionCall { path, args } => lower_statement_free_call(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            path.syntax_id(),
            ReferenceKind::QualifiedFunctionCall,
            &path.joined(),
            args,
        ),
        AstNode::When {
            expr,
            cases,
            default,
        } if default.is_none() || when_always_terminates(cases, default.as_deref()) => {
            lower_when_statement(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                expr,
                cases,
                default.as_deref(),
            )?;
            Ok(None)
        }
        AstNode::Loop { condition, body } => {
            lower_loop_statement(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                condition,
                body,
            )?;
            Ok(None)
        }
        AstNode::Break => {
            let Some(exit_block) = cursor.current_loop_exit() else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "break lowering requires an active loop exit block",
                ));
            };
            lower_defers_until_loop_exit(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
            )?;
            cursor
                .terminate_current_block(crate::LoweredTerminator::Jump { target: exit_block })?;
            Ok(None)
        }
        // Routine-local imports bind an alias during resolution; they have
        // no runtime effect, so lowering skips them.
        AstNode::UseDecl { .. } => Ok(None),
        AstNode::Dfr { syntax_id, body } | AstNode::Edf { syntax_id, body } => {
            let error_only = matches!(node, AstNode::Edf { .. });
            let construct = if error_only { "edf block" } else { "dfr block" };
            let deferred_scope_id =
                nested_scope_for_syntax(typed_package, *syntax_id, scope_id, construct)?;
            cursor.register_defer(source_unit_id, deferred_scope_id, body, error_only)?;
            Ok(None)
        }
        AstNode::Block {
            syntax_id,
            statements,
        } => {
            let block_scope_id =
                nested_scope_for_syntax(typed_package, *syntax_id, scope_id, "block")?;
            let _ = lower_body_sequence(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                block_scope_id,
                statements,
                DeferScopeKind::Ordinary,
            )?;
            Ok(None)
        }
        AstNode::MethodCall {
            syntax_id,
            object,
            method,
            args,
        } => {
            if method == "lock" || method == "unlock" {
                if !args.is_empty() {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("mutex .{method}() does not accept arguments"),
                    ));
                }
                let mutex = direct_local_identifier_value(typed_package, cursor, object)
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!("mutex .{method}() requires a local [mux] parameter"),
                        )
                    })?;
                if !cursor.routine.mutex_params.contains(&mutex.local_id) {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(".{method}() requires a [mux] parameter"),
                    ));
                }
                if method == "lock" {
                    cursor.register_mutex_guard(mutex.local_id)?;
                } else if !cursor.release_mutex_guard(mutex.local_id) {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "mutex .unlock() requires a guard acquired in the current lexical scope",
                    ));
                }
                cursor.push_instr(
                    None,
                    if method == "lock" {
                        crate::control::LoweredInstrKind::MutexLock {
                            mutex: mutex.local_id,
                        }
                    } else {
                        crate::control::LoweredInstrKind::MutexUnlock {
                            mutex: mutex.local_id,
                        }
                    },
                )?;
                return Ok(None);
            }
            let receiver = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                object,
            )?;
            // Constraint calls on generic parameters defer callee resolution
            // to monomorphization; emit the placeholder instruction instead
            // of resolving a concrete routine here.
            if syntax_id
                .is_some_and(|syntax_id| typed_package.program.is_constraint_call_site(syntax_id))
            {
                let typed_node =
                    syntax_id.and_then(|syntax_id| typed_package.program.typed_node(syntax_id));
                let result_type = typed_node
                    .and_then(|node| node.inferred_type)
                    .and_then(|checked_type| checked_type_map.get(&checked_type).copied());
                let error_type = typed_node
                    .and_then(|node| node.recoverable_effect)
                    .and_then(|effect| checked_type_map.get(&effect.error_type).copied());
                let mut lowered_args = vec![receiver.local_id];
                for arg in args {
                    let value = lower_expression(
                        typed_package,
                        type_table,
                        checked_type_map,
                        current_identity,
                        decl_index,
                        cursor,
                        source_unit_id,
                        scope_id,
                        arg,
                    )?;
                    lowered_args.push(value.local_id);
                }
                let result_local =
                    result_type.map(|result_type| cursor.allocate_local(result_type, None));
                cursor.push_instr(
                    result_local,
                    crate::control::LoweredInstrKind::ConstraintCall {
                        method: method.clone(),
                        args: lowered_args,
                        error_type,
                    },
                )?;
                return Ok(result_local
                    .zip(result_type)
                    .map(|(local_id, type_id)| LoweredValue {
                        local_id,
                        type_id,
                        recoverable_error_type: error_type,
                    }));
            }
            let callee = resolve_method_target(
                typed_package,
                checked_type_map,
                current_identity,
                decl_index,
                method,
                receiver.type_id,
                *syntax_id,
            )?;
            let typed_node =
                syntax_id.and_then(|syntax_id| typed_package.program.typed_node(syntax_id));
            let result_type = typed_node
                .and_then(|node| node.inferred_type)
                .and_then(|checked_type| checked_type_map.get(&checked_type).copied());
            let error_type = typed_node
                .and_then(|node| node.recoverable_effect)
                .and_then(|effect| checked_type_map.get(&effect.error_type).copied());
            let mut lowered_args = vec![receiver.local_id];
            let param_types = decl_index
                .routine_param_types(callee)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("method '{method}' does not retain lowered parameter types"),
                    )
                })?
                .to_vec();
            let param_names = decl_index.routine_param_names(callee).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("method '{method}' does not retain lowered parameter names"),
                )
            })?;
            let param_defaults = decl_index
                .routine_param_defaults(callee)
                .cloned()
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("method '{method}' does not retain lowered default arguments"),
                    )
                })?;
            let ordered_args = super::calls::bind_lowered_call_arguments(
                args,
                param_names.get(1..).unwrap_or(&[]),
                param_defaults.defaults.get(1..).unwrap_or(&[]),
                param_defaults
                    .variadic_index
                    .map(|index| index.saturating_sub(1)),
                method,
            )?;
            lowered_args.extend(
                ordered_args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        let expected = param_types.get(index + 1).copied();
                        match arg {
                            super::calls::BoundLoweredCallArg::Explicit(arg) => {
                                lower_expression_expected(
                                    typed_package,
                                    type_table,
                                    checked_type_map,
                                    current_identity,
                                    decl_index,
                                    cursor,
                                    source_unit_id,
                                    scope_id,
                                    expected,
                                    arg,
                                )
                            }
                            super::calls::BoundLoweredCallArg::Default(param_index) => {
                                super::calls::lower_default_call_argument(
                                    type_table,
                                    checked_type_map,
                                    decl_index,
                                    cursor,
                                    callee,
                                    param_index + 1,
                                    expected,
                                )
                            }
                            super::calls::BoundLoweredCallArg::VariadicUnpack(arg) => {
                                lower_expression_expected(
                                    typed_package,
                                    type_table,
                                    checked_type_map,
                                    current_identity,
                                    decl_index,
                                    cursor,
                                    source_unit_id,
                                    scope_id,
                                    expected,
                                    arg,
                                )
                            }
                            super::calls::BoundLoweredCallArg::VariadicPack(args) => {
                                let packed = AstNode::ContainerLiteral {
                                    container_type: fol_parser::ast::ContainerType::Sequence,
                                    elements: args.iter().map(|arg| (*arg).clone()).collect(),
                                };
                                lower_expression_expected(
                                    typed_package,
                                    type_table,
                                    checked_type_map,
                                    current_identity,
                                    decl_index,
                                    cursor,
                                    source_unit_id,
                                    scope_id,
                                    expected,
                                    &packed,
                                )
                            }
                        }
                        .map(|value| value.local_id)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            match result_type {
                Some(result_type) => {
                    let result_local = cursor.allocate_local(result_type, None);
                    cursor.push_instr(
                        Some(result_local),
                        crate::control::LoweredInstrKind::Call {
                            callee,
                            args: lowered_args,
                            error_type,
                        },
                    )?;
                    Ok(Some(LoweredValue {
                        local_id: result_local,
                        type_id: result_type,
                        recoverable_error_type: error_type,
                    }))
                }
                None => {
                    cursor.push_instr(
                        None,
                        crate::control::LoweredInstrKind::Call {
                            callee,
                            args: lowered_args,
                            error_type,
                        },
                    )?;
                    Ok(None)
                }
            }
        }
        AstNode::Yield { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "yield lowering is not yet supported",
        )),
        _ => lower_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            node,
        )
        .map(Some),
    }
}

pub(crate) fn routine_return_type(
    cursor: &RoutineCursor<'_>,
    type_table: &crate::LoweredTypeTable,
) -> Option<LoweredTypeId> {
    let signature_id = cursor.routine.signature?;
    match type_table.get(signature_id) {
        Some(crate::LoweredType::Routine(signature)) => signature.return_type,
        _ => None,
    }
}

pub(crate) fn routine_error_type(
    cursor: &RoutineCursor<'_>,
    type_table: &crate::LoweredTypeTable,
) -> Option<LoweredTypeId> {
    let signature_id = cursor.routine.signature?;
    match type_table.get(signature_id) {
        Some(crate::LoweredType::Routine(signature)) => signature.error_type,
        _ => None,
    }
}
