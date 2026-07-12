mod inquiry;
mod routines;
mod statements;
mod types;

use crate::{
    collect::{binding_names, insert_import_record, semantic_node},
    imports,
    model::{ResolvedProgram, ScopeKind, SymbolKind},
    ResolverError, ResolverErrorKind, ResolverSession, ScopeId, SourceUnitId,
};
use fol_parser::ast::{AstNode, CallSurface, ParsedTopLevel};

use super::references::{
    is_builtin_diagnostic_call, record_function_call_reference, record_identifier_reference,
    record_qualified_function_call_reference, record_qualified_identifier_reference,
};
use super::scope::{insert_generic_symbols, insert_local_named_symbol, insert_local_symbol};

#[derive(Debug, Clone, Copy)]
pub struct RoutineContext {
    pub this_available: bool,
}

pub fn traverse_top_level_item(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    _scope_id: ScopeId,
    item: &ParsedTopLevel,
) -> Result<(), ResolverError> {
    let Some(source_unit) = program.source_unit(source_unit_id) else {
        return Err(ResolverError::new(
            ResolverErrorKind::Internal,
            format!(
                "source unit {:?} not found during top-level traversal",
                source_unit_id
            ),
        ));
    };
    let traversal_scope = source_unit.scope_id;
    traverse_node(
        session,
        program,
        source_unit_id,
        traversal_scope,
        &item.node,
        true,
        None,
    )
}

pub fn traverse_node(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    node: &AstNode,
    is_top_level_node: bool,
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    // Traversal recurses with the AST; grow the stack in segments so deep
    // (but legal) nesting cannot overflow small worker-thread stacks.
    stacker::maybe_grow(256 * 1024, 4 * 1024 * 1024, || {
        traverse_node_inner(
            session,
            program,
            source_unit_id,
            scope_id,
            node,
            is_top_level_node,
            routine_context,
        )
    })
}

fn traverse_node_inner(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    node: &AstNode,
    is_top_level_node: bool,
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    match semantic_node(node) {
        AstNode::FunDecl {
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            if !is_top_level_node {
                insert_local_named_symbol(
                    program,
                    source_unit_id,
                    scope_id,
                    node,
                    SymbolKind::Routine,
                )?;
            }
            routines::traverse_named_routine(
                session,
                program,
                source_unit_id,
                scope_id,
                syntax_id,
                generics,
                receiver_type,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            )?;
        }
        AstNode::AnonymousFun {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousPro {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousLog {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            routines::traverse_anonymous_routine(
                session,
                program,
                source_unit_id,
                scope_id,
                syntax_id,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            )?;
        }
        AstNode::Comment { .. } | AstNode::Commented { .. } => {}
        AstNode::Identifier { name, syntax_id } => {
            record_identifier_reference(
                program,
                source_unit_id,
                scope_id,
                name,
                *syntax_id,
                syntax_id
                    .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
                    .cloned(),
            )?;
        }
        AstNode::QualifiedIdentifier { path } => {
            record_qualified_identifier_reference(program, source_unit_id, scope_id, path)?;
        }
        AstNode::BinaryOp { op, left, right } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                left,
                false,
                routine_context,
            )?;
            // `as` / `cast` right-hand sides are type positions, not value
            // references; typecheck owns the explicit unsupported-casting
            // boundary, so the resolver must not misreport them as
            // unresolved value names.
            if !matches!(
                op,
                fol_parser::ast::BinaryOperator::As | fol_parser::ast::BinaryOperator::Cast
            ) {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    right,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::UnaryOp { operand, .. } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                operand,
                false,
                routine_context,
            )?;
        }
        AstNode::FunctionCall {
            surface: CallSurface::DotIntrinsic,
            args,
            ..
        } => {
            for arg in args {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    arg,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::FunctionCall {
            name,
            args,
            syntax_id,
            ..
        } => {
            if !is_builtin_diagnostic_call(name) {
                record_function_call_reference(
                    program,
                    source_unit_id,
                    scope_id,
                    name,
                    *syntax_id,
                    syntax_id
                        .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
                        .cloned(),
                )?;
            }
            for arg in args {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    arg,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::QualifiedFunctionCall { path, args } => {
            record_qualified_function_call_reference(program, source_unit_id, scope_id, path)?;
            for arg in args {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    arg,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::Invoke { callee, args } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                callee,
                false,
                routine_context,
            )?;
            for arg in args {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    arg,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::NamedArgument { value, .. }
        | AstNode::Unpack { value }
        | AstNode::Spawn { task: value }
        | AstNode::Return { value: Some(value) } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                value,
                false,
                routine_context,
            )?;
        }
        AstNode::Assignment { target, value } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                target,
                false,
                routine_context,
            )?;
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                value,
                false,
                routine_context,
            )?;
        }
        AstNode::Return { value: None }
        | AstNode::Break
        | AstNode::AsyncStage
        | AstNode::AwaitStage => {}
        AstNode::Defer { syntax_id, body } => {
            traverse_block_body(
                session,
                program,
                source_unit_id,
                scope_id,
                *syntax_id,
                body,
                routine_context,
            )?;
        }
        AstNode::MethodCall { object, args, .. } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                object,
                false,
                routine_context,
            )?;
            for arg in args {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    arg,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::TemplateCall { object, .. }
        | AstNode::FieldAccess { object, .. }
        | AstNode::AvailabilityAccess { target: object } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                object,
                false,
                routine_context,
            )?;
        }
        AstNode::IndexAccess { container, index } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                container,
                false,
                routine_context,
            )?;
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                index,
                false,
                routine_context,
            )?;
        }
        AstNode::ChannelAccess { channel, .. } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                channel,
                false,
                routine_context,
            )?;
        }
        AstNode::SliceAccess {
            container,
            start,
            end,
            ..
        } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                container,
                false,
                routine_context,
            )?;
            if let Some(start) = start {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    start,
                    false,
                    routine_context,
                )?;
            }
            if let Some(end) = end {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    end,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::PatternAccess {
            container,
            patterns,
        } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                container,
                false,
                routine_context,
            )?;
            for pattern in patterns {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    pattern,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::PatternCapture { pattern, .. } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                pattern,
                false,
                routine_context,
            )?;
        }
        AstNode::ContainerLiteral { elements, .. } => {
            for element in elements {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    element,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::RecordInit { fields, .. } => {
            for field in fields {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    &field.value,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::Rolling {
            expr,
            bindings,
            condition,
        } => {
            let rolling_scope =
                program.add_scope(ScopeKind::RollingBinder, scope_id, source_unit_id);
            for binding in bindings {
                if let Some(type_hint) = &binding.type_hint {
                    types::resolve_type_reference(
                        session,
                        program,
                        source_unit_id,
                        rolling_scope,
                        type_hint,
                    )?;
                }
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    rolling_scope,
                    &binding.iterable,
                    false,
                    routine_context,
                )?;
                insert_local_symbol(
                    program,
                    source_unit_id,
                    rolling_scope,
                    &binding.name,
                    SymbolKind::RollingBinder,
                    format!(
                        "symbol#{}",
                        fol_types::canonical_identifier_key(&binding.name)
                    ),
                )?;
            }
            traverse_node(
                session,
                program,
                source_unit_id,
                rolling_scope,
                expr,
                false,
                routine_context,
            )?;
            if let Some(condition) = condition {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    rolling_scope,
                    condition,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::Range { start, end, .. } => {
            if let Some(start) = start {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    start,
                    false,
                    routine_context,
                )?;
            }
            if let Some(end) = end {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    end,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::VarDecl { name, value, .. } => {
            if let AstNode::VarDecl {
                type_hint: Some(type_hint),
                ..
            } = semantic_node(node)
            {
                types::resolve_type_reference(session, program, source_unit_id, scope_id, type_hint)?;
            }
            if let Some(value) = value {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    value,
                    false,
                    routine_context,
                )?;
            }
            if !is_top_level_node {
                let symbol_id = insert_local_symbol(
                    program,
                    source_unit_id,
                    scope_id,
                    name,
                    SymbolKind::ValueBinding,
                    format!("symbol#{}", fol_types::canonical_identifier_key(name)),
                )?;
                if let AstNode::VarDecl { options, .. } = semantic_node(node) {
                    if fol_parser::ast::binding_is_mutable(options) {
                        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
                            symbol.is_mutable = true;
                        }
                    }
                }
            }
        }
        AstNode::LabDecl { name, value, .. } => {
            if let AstNode::LabDecl {
                type_hint: Some(type_hint),
                ..
            } = semantic_node(node)
            {
                types::resolve_type_reference(session, program, source_unit_id, scope_id, type_hint)?;
            }
            if let Some(value) = value {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    value,
                    false,
                    routine_context,
                )?;
            }
            if !is_top_level_node {
                let symbol_id = insert_local_symbol(
                    program,
                    source_unit_id,
                    scope_id,
                    name,
                    SymbolKind::LabelBinding,
                    format!("symbol#{}", fol_types::canonical_identifier_key(name)),
                )?;
                if let AstNode::LabDecl { options, .. } = semantic_node(node) {
                    if fol_parser::ast::binding_is_mutable(options) {
                        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
                            symbol.is_mutable = true;
                        }
                    }
                }
            }
        }
        AstNode::DestructureDecl { pattern, value, .. } => {
            if let AstNode::DestructureDecl {
                type_hint: Some(type_hint),
                ..
            } = semantic_node(node)
            {
                types::resolve_type_reference(session, program, source_unit_id, scope_id, type_hint)?;
            }
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                value,
                false,
                routine_context,
            )?;
            if !is_top_level_node {
                for name in binding_names(pattern) {
                    insert_local_symbol(
                        program,
                        source_unit_id,
                        scope_id,
                        &name,
                        SymbolKind::DestructureBinding,
                        format!("symbol#{}", fol_types::canonical_identifier_key(&name)),
                    )?;
                }
            }
        }
        AstNode::Yield { value } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                value,
                false,
                routine_context,
            )?;
        }
        AstNode::When {
            expr,
            cases,
            default,
        } => {
            statements::traverse_when_node(
                session,
                program,
                source_unit_id,
                scope_id,
                expr,
                cases,
                default,
                routine_context,
            )?;
        }
        AstNode::Loop { condition, body } => {
            statements::traverse_loop_node(
                session,
                program,
                source_unit_id,
                scope_id,
                condition.as_ref(),
                body,
                routine_context,
            )?;
        }
        AstNode::Select {
            channel,
            binding,
            body,
        } => {
            traverse_node(
                session,
                program,
                source_unit_id,
                scope_id,
                channel,
                false,
                routine_context,
            )?;
            let select_scope = program.add_scope(ScopeKind::Block, scope_id, source_unit_id);
            if let Some(binding) = binding {
                insert_local_symbol(
                    program,
                    source_unit_id,
                    select_scope,
                    binding,
                    SymbolKind::ValueBinding,
                    format!("symbol#{}", fol_types::canonical_identifier_key(binding)),
                )?;
            }
            for statement in body {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    select_scope,
                    statement,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::Block {
            syntax_id,
            statements,
        } => {
            traverse_block_body(
                session,
                program,
                source_unit_id,
                scope_id,
                *syntax_id,
                statements,
                routine_context,
            )?;
        }
        AstNode::Program {
            declarations: statements,
        } => {
            for statement in statements {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    statement,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::Inquiry { target, body } => {
            inquiry::resolve_inquiry_target(program, source_unit_id, scope_id, target, routine_context)?;
            for statement in body {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    statement,
                    false,
                    routine_context,
                )?;
            }
        }
        AstNode::TypeDecl {
            generics,
            explicit_contracts,
            type_def,
            ..
        } => {
            let type_scope =
                program.add_scope(ScopeKind::TypeDeclaration, scope_id, source_unit_id);
            insert_generic_symbols(program, source_unit_id, type_scope, generics)?;
            for generic in generics {
                for constraint in &generic.constraints {
                    types::resolve_type_reference(
                        session,
                        program,
                        source_unit_id,
                        type_scope,
                        constraint,
                    )?;
                }
            }
            for contract in explicit_contracts {
                types::resolve_contract_reference(
                    session,
                    program,
                    source_unit_id,
                    type_scope,
                    contract,
                )?;
            }
            types::resolve_type_definition(session, program, source_unit_id, type_scope, type_def)?;
        }
        AstNode::StdDecl {
            syntax_id,
            generics,
            body,
            ..
        } => {
            let standard_scope =
                program.add_scope(ScopeKind::StandardDeclaration, scope_id, source_unit_id);
            program.record_scope_for_syntax(*syntax_id, standard_scope);
            insert_generic_symbols(program, source_unit_id, standard_scope, generics)?;
            for generic in generics {
                for constraint in &generic.constraints {
                    types::resolve_type_reference(
                        session,
                        program,
                        source_unit_id,
                        standard_scope,
                        constraint,
                    )?;
                }
            }
            for member in body {
                traverse_node(
                    session,
                    program,
                    source_unit_id,
                    standard_scope,
                    member,
                    false,
                    None,
                )?;
            }
        }
        AstNode::AliasDecl { target, .. } => {
            types::resolve_type_reference(session, program, source_unit_id, scope_id, target)?;
        }
        AstNode::DefDecl {
            params, def_type, ..
        } => {
            for param in params {
                types::resolve_type_reference(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    &param.param_type,
                )?;
            }
            types::resolve_type_reference(session, program, source_unit_id, scope_id, def_type)?;
        }
        AstNode::SegDecl { seg_type, .. } => {
            types::resolve_type_reference(session, program, source_unit_id, scope_id, seg_type)?;
        }
        AstNode::Literal(_) | AstNode::PatternWildcard => {}
        AstNode::UseDecl {
            name,
            path_type,
            import_target,
            path_segments,
            ..
        } => {
            if !is_top_level_node {
                let symbol_id = insert_local_symbol(
                    program,
                    source_unit_id,
                    scope_id,
                    name,
                    SymbolKind::ImportAlias,
                    format!("symbol#{}", fol_types::canonical_identifier_key(name)),
                )?;
                let import_id = insert_import_record(
                    program,
                    source_unit_id,
                    scope_id,
                    symbol_id,
                    name,
                    path_type.clone(),
                    import_target.clone(),
                    path_segments.clone(),
                );
                imports::resolve_import_target_with_session(session, program, import_id)?;
            }
        }
    }

    Ok(())
}

pub fn traverse_block_body(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    parent_scope: ScopeId,
    syntax_id: Option<fol_parser::ast::syntax::SyntaxNodeId>,
    statements: &[AstNode],
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    let block_scope = program.add_scope(ScopeKind::Block, parent_scope, source_unit_id);
    program.record_scope_for_syntax(syntax_id, block_scope);
    for statement in statements {
        traverse_node(
            session,
            program,
            source_unit_id,
            block_scope,
            statement,
            false,
            routine_context,
        )?;
    }
    Ok(())
}
