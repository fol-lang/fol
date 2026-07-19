use super::body::lower_body_sequence;
use super::cursor::{DeferScopeKind, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_observed;
use crate::{
    control::{LoweredBinaryOp, LoweredInstrKind, LoweredOperand},
    ids::{LoweredBlockId, LoweredTypeId},
    LoweringError, LoweringErrorKind,
};
use fol_parser::ast::{AstNode, ChannelEndpoint, Literal, LoopCondition, SelectArm};
use fol_resolver::{PackageIdentity, ScopeId, SourceUnitId, SymbolKind};
use std::collections::BTreeMap;

pub(crate) fn when_case_body(case: &fol_parser::ast::WhenCase) -> &[AstNode] {
    match case {
        fol_parser::ast::WhenCase::Case { body, .. }
        | fol_parser::ast::WhenCase::Is { body, .. }
        | fol_parser::ast::WhenCase::In { body, .. }
        | fol_parser::ast::WhenCase::Has { body, .. }
        | fol_parser::ast::WhenCase::On { body, .. }
        | fol_parser::ast::WhenCase::Of { body, .. } => body.as_slice(),
    }
}

pub(crate) fn when_always_terminates(
    cases: &[fol_parser::ast::WhenCase],
    default: Option<&[AstNode]>,
) -> bool {
    let Some(default) = default else {
        return false;
    };
    !cases.is_empty()
        && cases
            .iter()
            .all(|case| body_always_terminates(when_case_body(case)))
        && body_always_terminates(default)
}

/// Whether a bare `when` contains a continuing arm whose final node is a
/// statement. Such a `when` must use statement lowering even when it has an
/// exhaustive default; expression lowering requires every continuing arm to
/// produce a value.
pub(crate) fn when_has_statement_branch(
    typed_package: &fol_typecheck::TypedPackage,
    cases: &[fol_parser::ast::WhenCase],
    default: Option<&[AstNode]>,
) -> bool {
    cases
        .iter()
        .any(|case| body_ends_with_statement(typed_package, when_case_body(case)))
        || default.is_some_and(|body| body_ends_with_statement(typed_package, body))
}

fn body_ends_with_statement(
    typed_package: &fol_typecheck::TypedPackage,
    nodes: &[AstNode],
) -> bool {
    match nodes
        .iter()
        .rev()
        .find(|node| !matches!(node, AstNode::Comment { .. }))
    {
        // An empty continuing arm (e.g. the synthesized default of an
        // else-less `if`) yields no value, so the `when` can only lower as a
        // statement.
        None => true,
        Some(node) => node_is_nonterminating_statement(typed_package, node),
    }
}

fn node_is_nonterminating_statement(
    typed_package: &fol_typecheck::TypedPackage,
    node: &AstNode,
) -> bool {
    match node {
        AstNode::Commented { node, .. } => node_is_nonterminating_statement(typed_package, node),
        AstNode::VarDecl { .. }
        | AstNode::DestructureDecl { .. }
        | AstNode::FunDecl { .. }
        | AstNode::ProDecl { .. }
        | AstNode::LogDecl { .. }
        | AstNode::TypeDecl { .. }
        | AstNode::UseDecl { .. }
        | AstNode::AliasDecl { .. }
        | AstNode::DefDecl { .. }
        | AstNode::SegDecl { .. }
        | AstNode::StdDecl { .. }
        | AstNode::Assignment { .. }
        | AstNode::LabDecl { .. }
        | AstNode::Loop { .. }
        | AstNode::Select { .. }
        | AstNode::Dfr { .. }
        | AstNode::Edf { .. }
        | AstNode::Block { .. }
        | AstNode::Inquiry { .. }
        | AstNode::Program { .. } => true,
        AstNode::When { cases, default, .. } => {
            cases.is_empty()
                || default.is_none()
                || when_has_statement_branch(typed_package, cases, default.as_deref())
        }
        AstNode::FunctionCall {
            surface: fol_parser::ast::CallSurface::DotIntrinsic,
            ..
        }
        | AstNode::Spawn { .. }
        | AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::GiveBack,
            ..
        } => true,
        AstNode::BinaryOp {
            op: fol_parser::ast::BinaryOperator::Pipe,
            right,
            ..
        } if matches!(
            right.as_ref(),
            AstNode::ChannelAccess {
                endpoint: ChannelEndpoint::Tx,
                ..
            }
        ) =>
        {
            true
        }
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            name,
            ..
        } if name != "panic" && name != "report" => {
            typed_package.program.typed_node(*syntax_id).is_none()
        }
        AstNode::QualifiedFunctionCall { path, .. } => path
            .syntax_id()
            .is_some_and(|syntax_id| typed_package.program.typed_node(syntax_id).is_none()),
        AstNode::MethodCall {
            syntax_id: Some(syntax_id),
            ..
        } => typed_package.program.typed_node(*syntax_id).is_none(),
        // Return, break, panic/report, and the expression forms either
        // terminate their arm or are handled as value producers.
        _ => false,
    }
}

fn body_always_terminates(nodes: &[AstNode]) -> bool {
    nodes
        .iter()
        .rev()
        .find(|node| !matches!(node, AstNode::Comment { .. }))
        .is_some_and(node_always_terminates)
}

fn node_always_terminates(node: &AstNode) -> bool {
    match node {
        AstNode::Comment { .. } => false,
        AstNode::Commented { node, .. } => node_always_terminates(node),
        AstNode::Return { .. } => true,
        AstNode::FunctionCall { name, .. } if name == "report" || name == "panic" => true,
        AstNode::Block { statements, .. } => body_always_terminates(statements),
        AstNode::When { cases, default, .. } => when_always_terminates(cases, default.as_deref()),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_when_statement(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expr: &AstNode,
    cases: &[fol_parser::ast::WhenCase],
    default: Option<&[AstNode]>,
) -> Result<(), LoweringError> {
    use super::expressions::lower_expression;
    let subject = lower_expression(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        expr,
    )?;

    if cases.is_empty() {
        let body_block = cursor.create_block();
        let after_block = cursor.create_block();
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: subject.local_id,
            then_block: body_block,
            else_block: after_block,
        })?;
        cursor.switch_block(body_block)?;
        if let Some(default) = default {
            let _ = lower_body_sequence(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                default,
                DeferScopeKind::Ordinary,
            )?;
        }
        if !cursor.current_block_terminated()? {
            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: after_block,
            })?;
        }
        cursor.switch_block(after_block)?;
        return Ok(());
    }

    let mut after_block = None;
    let mut has_fallthrough = false;

    for (index, case) in cases.iter().enumerate() {
        // An `on(v)` branch (V3_MEM §3.3 shell choice) tests whether the shell
        // subject is present and binds its payload in the body block; every other
        // branch tests value equality against the subject.
        let (condition_local, body, on_payload) = if let fol_parser::ast::WhenCase::On {
            channel,
            body,
        } = case
        {
            let bool_type = checked_type_map
                .get(&typed_package.program.builtin_types().bool_)
                .copied()
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "lowered workspace lost builtin bool while lowering an 'on' branch",
                    )
                })?;
            let present = cursor.allocate_local(bool_type, None);
            cursor.push_instr(
                Some(present),
                crate::LoweredInstrKind::OptionalHasValue {
                    operand: subject.local_id,
                },
            )?;
            let payload =
                lower_on_branch_payload(typed_package, checked_type_map, source_unit_id, channel)?;
            (present, body.as_slice(), Some(payload))
        } else {
            let (condition, body) = when_case_condition_and_body(case)?;
            let lowered_condition = lower_when_case_condition(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                &subject,
                condition,
            )?;
            (lowered_condition.local_id, body, None)
        };
        let body_block = cursor.create_block();
        let else_block = if index + 1 < cases.len() || default.is_some() {
            cursor.create_block()
        } else {
            ensure_after_block(cursor, &mut after_block)
        };
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: condition_local,
            then_block: body_block,
            else_block,
        })?;

        cursor.switch_block(body_block)?;
        if let Some((symbol, payload_type)) = on_payload {
            let payload_local = cursor.allocate_local(payload_type, None);
            cursor.push_instr(
                Some(payload_local),
                crate::LoweredInstrKind::UnwrapShell {
                    operand: subject.local_id,
                },
            )?;
            cursor.routine.local_symbols.insert(symbol, payload_local);
        }
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            body,
            DeferScopeKind::Ordinary,
        )?;
        if !cursor.current_block_terminated()? {
            let after_block = ensure_after_block(cursor, &mut after_block);
            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: after_block,
            })?;
            has_fallthrough = true;
        }

        if Some(else_block) != after_block {
            cursor.switch_block(else_block)?;
        }
    }

    if let Some(default) = default {
        has_fallthrough |= lower_default_when_body(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            default,
            &mut after_block,
        )?;
    }

    if let Some(after_block) = after_block.filter(|_| has_fallthrough) {
        cursor.switch_block(after_block)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_loop_statement(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    condition: &LoopCondition,
    body: &[AstNode],
) -> Result<(), LoweringError> {
    use super::expressions::lower_expression;
    let body_scope_id = syntax_id
        .and_then(|syntax_id| typed_package.program.resolved().scope_for_syntax(syntax_id))
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "loop body does not retain its exact resolved scope",
            )
        })?;
    match condition {
        LoopCondition::Condition(condition) => {
            let header_block = cursor.create_block();
            let body_block = cursor.create_block();
            let exit_block = cursor.create_block();

            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: header_block,
            })?;

            cursor.switch_block(header_block)?;
            let lowered_condition = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                condition,
            )?;
            cursor.terminate_current_block(crate::LoweredTerminator::Branch {
                condition: lowered_condition.local_id,
                then_block: body_block,
                else_block: exit_block,
            })?;

            cursor.switch_block(body_block)?;
            cursor.push_loop_exit(exit_block);
            let _ = lower_body_sequence(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                body_scope_id,
                body,
                DeferScopeKind::Loop,
            )?;
            cursor.pop_loop_exit();
            if !cursor.current_block_terminated()? {
                cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                    target: header_block,
                })?;
            }

            cursor.switch_block(exit_block)?;
            Ok(())
        }
        LoopCondition::Iteration {
            var,
            iterable,
            condition,
            ..
        } => {
            if let AstNode::ChannelAccess {
                channel,
                endpoint: ChannelEndpoint::Rx,
            } = iterable.as_ref()
            {
                return lower_channel_iteration(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    body_scope_id,
                    var,
                    channel,
                    condition.as_deref(),
                    body,
                );
            }

            let lowered_iterable = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                iterable,
            )?;

            // Get length of iterable
            let int_type = super::helpers::literal_type_id(
                typed_package,
                checked_type_map,
                &Literal::Integer(0),
            )
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "int type not found for iteration loop index",
                )
            })?;
            let len_local = cursor.allocate_local(int_type, None);
            cursor.push_instr(
                Some(len_local),
                LoweredInstrKind::LengthOf {
                    operand: lowered_iterable.local_id,
                },
            )?;

            // Create index counter initialized to 0
            let index_local = cursor.allocate_local(int_type, None);
            cursor.push_instr(
                Some(index_local),
                LoweredInstrKind::Const(LoweredOperand::Int(0)),
            )?;

            // Find the loop binder symbol and create its local
            let binder_scope_id = body_scope_id;
            let binder_symbol_id = crate::decls::find_symbol_in_scope_or_descendants(
                &typed_package.program,
                source_unit_id,
                binder_scope_id,
                SymbolKind::LoopBinder,
                var,
            )
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("loop binder '{var}' does not retain a lowering symbol"),
                )
            })?;
            let binder_type_id = typed_package
                .program
                .typed_symbol(binder_symbol_id)
                .and_then(|sym| sym.declared_type)
                .and_then(|checked| checked_type_map.get(&checked).copied())
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("loop binder '{var}' does not retain a lowered type"),
                    )
                })?;
            let binder_local = cursor.allocate_local(binder_type_id, Some(var.clone()));
            cursor
                .routine
                .local_symbols
                .insert(binder_symbol_id, binder_local);

            let header_block = cursor.create_block();
            let body_block = cursor.create_block();
            let exit_block = cursor.create_block();

            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: header_block,
            })?;

            // Header: check index < len
            cursor.switch_block(header_block)?;
            let cmp_local = cursor.allocate_local(
                super::helpers::literal_type_id(
                    typed_package,
                    checked_type_map,
                    &Literal::Boolean(true),
                )
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "bool type not found for iteration loop comparison",
                    )
                })?,
                None,
            );
            cursor.push_instr(
                Some(cmp_local),
                LoweredInstrKind::BinaryOp {
                    op: LoweredBinaryOp::Lt,
                    left: index_local,
                    right: len_local,
                },
            )?;
            cursor.terminate_current_block(crate::LoweredTerminator::Branch {
                condition: cmp_local,
                then_block: body_block,
                else_block: exit_block,
            })?;

            // Body: extract element, bind loop variable, run body
            cursor.switch_block(body_block)?;
            cursor.push_loop_exit(exit_block);

            // element = container[index]
            let element_local = cursor.allocate_local(binder_type_id, None);
            cursor.push_instr(
                Some(element_local),
                LoweredInstrKind::IndexAccess {
                    container: lowered_iterable.local_id,
                    index: index_local,
                },
            )?;
            // binder = element
            cursor.push_instr(
                None,
                LoweredInstrKind::StoreLocal {
                    local: binder_local,
                    value: element_local,
                },
            )?;

            // Optional guard condition
            if let Some(guard) = condition.as_deref() {
                let guard_block = cursor.create_block();
                let lowered_guard = lower_expression(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    binder_scope_id,
                    guard,
                )?;
                let increment_block = cursor.create_block();
                cursor.terminate_current_block(crate::LoweredTerminator::Branch {
                    condition: lowered_guard.local_id,
                    then_block: guard_block,
                    else_block: increment_block,
                })?;

                // Guard passed: run body
                cursor.switch_block(guard_block)?;
                let _ = lower_body_sequence(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    binder_scope_id,
                    body,
                    DeferScopeKind::Loop,
                )?;
                if !cursor.current_block_terminated()? {
                    cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                        target: increment_block,
                    })?;
                }

                // Increment index
                cursor.switch_block(increment_block)?;
                let one_local = cursor.allocate_local(int_type, None);
                cursor.push_instr(
                    Some(one_local),
                    LoweredInstrKind::Const(LoweredOperand::Int(1)),
                )?;
                let next_index = cursor.allocate_local(int_type, None);
                cursor.push_instr(
                    Some(next_index),
                    LoweredInstrKind::BinaryOp {
                        op: LoweredBinaryOp::Add,
                        left: index_local,
                        right: one_local,
                    },
                )?;
                cursor.push_instr(
                    None,
                    LoweredInstrKind::StoreLocal {
                        local: index_local,
                        value: next_index,
                    },
                )?;
                cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                    target: header_block,
                })?;
            } else {
                // No guard: run body directly
                let _ = lower_body_sequence(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    binder_scope_id,
                    body,
                    DeferScopeKind::Loop,
                )?;

                // Increment index
                if !cursor.current_block_terminated()? {
                    let one_local = cursor.allocate_local(int_type, None);
                    cursor.push_instr(
                        Some(one_local),
                        LoweredInstrKind::Const(LoweredOperand::Int(1)),
                    )?;
                    let next_index = cursor.allocate_local(int_type, None);
                    cursor.push_instr(
                        Some(next_index),
                        LoweredInstrKind::BinaryOp {
                            op: LoweredBinaryOp::Add,
                            left: index_local,
                            right: one_local,
                        },
                    )?;
                    cursor.push_instr(
                        None,
                        LoweredInstrKind::StoreLocal {
                            local: index_local,
                            value: next_index,
                        },
                    )?;
                    cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                        target: header_block,
                    })?;
                }
            }
            cursor.pop_loop_exit();

            cursor.switch_block(exit_block)?;
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_channel_iteration(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    binder_scope_id: ScopeId,
    var: &str,
    channel: &AstNode,
    condition: Option<&AstNode>,
    body: &[AstNode],
) -> Result<(), LoweringError> {
    let (channel_local, channel_type) =
        super::expressions::channel_binding_local(typed_package, type_table, cursor, channel)?;
    let Some(crate::LoweredType::Channel { element_type }) = type_table.get(channel_type) else {
        unreachable!("channel_binding_local verifies the lowered type")
    };
    let binder_type_id = *element_type;
    let optional_type = type_table
        .find(&crate::LoweredType::Optional {
            inner: binder_type_id,
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "channel iteration optional type was not translated into lowered IR",
            )
        })?;
    let bool_type =
        super::helpers::literal_type_id(typed_package, checked_type_map, &Literal::Boolean(true))
            .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "bool type not found for channel iteration",
            )
        })?;

    let binder_symbol_id = crate::decls::find_symbol_in_scope_or_descendants(
        &typed_package.program,
        source_unit_id,
        binder_scope_id,
        SymbolKind::LoopBinder,
        var,
    )
    .ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("loop binder '{var}' does not retain a lowering symbol"),
        )
    })?;
    let binder_local = cursor.allocate_local(binder_type_id, Some(var.to_string()));
    cursor
        .routine
        .local_symbols
        .insert(binder_symbol_id, binder_local);

    let header_block = cursor.create_block();
    let body_block = cursor.create_block();
    let exit_block = cursor.create_block();
    cursor.terminate_current_block(crate::LoweredTerminator::Jump {
        target: header_block,
    })?;

    cursor.switch_block(header_block)?;
    let optional_local = cursor.allocate_local(optional_type, None);
    cursor.push_instr(
        Some(optional_local),
        LoweredInstrKind::ChannelReceiveOptional {
            channel: channel_local,
        },
    )?;
    let has_value_local = cursor.allocate_local(bool_type, None);
    cursor.push_instr(
        Some(has_value_local),
        LoweredInstrKind::OptionalHasValue {
            operand: optional_local,
        },
    )?;
    cursor.terminate_current_block(crate::LoweredTerminator::Branch {
        condition: has_value_local,
        then_block: body_block,
        else_block: exit_block,
    })?;

    cursor.switch_block(body_block)?;
    cursor.push_loop_exit(exit_block);
    let element_local = cursor.allocate_local(binder_type_id, None);
    cursor.push_instr(
        Some(element_local),
        LoweredInstrKind::UnwrapShell {
            operand: optional_local,
        },
    )?;
    cursor.push_instr(
        None,
        LoweredInstrKind::StoreLocal {
            local: binder_local,
            value: element_local,
        },
    )?;

    if let Some(guard) = condition {
        let guard_body_block = cursor.create_block();
        let continue_block = cursor.create_block();
        let lowered_guard = super::expressions::lower_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            binder_scope_id,
            guard,
        )?;
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: lowered_guard.local_id,
            then_block: guard_body_block,
            else_block: continue_block,
        })?;
        cursor.switch_block(guard_body_block)?;
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            binder_scope_id,
            body,
            DeferScopeKind::Loop,
        )?;
        if !cursor.current_block_terminated()? {
            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: continue_block,
            })?;
        }
        cursor.switch_block(continue_block)?;
    } else {
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            binder_scope_id,
            body,
            DeferScopeKind::Loop,
        )?;
    }
    if !cursor.current_block_terminated()? {
        cursor.terminate_current_block(crate::LoweredTerminator::Jump {
            target: header_block,
        })?;
    }
    cursor.pop_loop_exit();
    cursor.switch_block(exit_block)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_select_statement(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    arms: &[SelectArm],
    default: Option<&[AstNode]>,
) -> Result<(), LoweringError> {
    let bool_type =
        super::helpers::literal_type_id(typed_package, checked_type_map, &Literal::Boolean(true))
            .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "bool type not found for select",
            )
        })?;
    let header_block = cursor.create_block();
    let exit_block = cursor.create_block();
    cursor.terminate_current_block(crate::LoweredTerminator::Jump {
        target: header_block,
    })?;
    cursor.switch_block(header_block)?;

    let mut used_scopes = std::collections::BTreeSet::new();
    let mut channels = Vec::with_capacity(arms.len());
    for arm in arms {
        let channel_node = match &arm.channel {
            AstNode::ChannelAccess {
                channel,
                endpoint: ChannelEndpoint::Rx,
            } => channel.as_ref(),
            other => other,
        };
        let (channel_local, channel_type) = super::expressions::channel_binding_local(
            typed_package,
            type_table,
            cursor,
            channel_node,
        )?;
        let Some(crate::LoweredType::Channel { element_type }) = type_table.get(channel_type)
        else {
            unreachable!("channel_binding_local verifies the lowered type")
        };
        let element_type = *element_type;
        channels.push(channel_local);
        let optional_type = type_table
            .find(&crate::LoweredType::Optional {
                inner: element_type,
            })
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "select optional type was not translated into lowered IR",
                )
            })?;
        let arm_scope = typed_package
            .program
            .resolved()
            .scopes
            .iter_with_ids()
            .find_map(|(candidate, scope)| {
                (scope.parent == Some(scope_id)
                    && scope.source_unit == Some(source_unit_id)
                    && !used_scopes.contains(&candidate)
                    && scope.symbols.iter().any(|symbol_id| {
                        typed_package
                            .program
                            .resolved()
                            .symbol(*symbol_id)
                            .is_some_and(|symbol| {
                                symbol.kind == SymbolKind::ValueBinding
                                    && symbol.name == arm.binding
                            })
                    }))
                .then_some(candidate)
            })
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("select binding '{}' lost its lowering scope", arm.binding),
                )
            })?;
        used_scopes.insert(arm_scope);
        let binder_symbol = crate::decls::find_symbol_in_scope_or_descendants(
            &typed_package.program,
            source_unit_id,
            arm_scope,
            SymbolKind::ValueBinding,
            &arm.binding,
        )
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("select binding '{}' lost its lowering symbol", arm.binding),
            )
        })?;
        let binder_local = cursor.allocate_local(element_type, Some(arm.binding.clone()));
        cursor
            .routine
            .local_symbols
            .insert(binder_symbol, binder_local);

        let optional_local = cursor.allocate_local(optional_type, None);
        cursor.push_instr(
            Some(optional_local),
            LoweredInstrKind::ChannelTryReceive {
                channel: channel_local,
            },
        )?;
        let has_value = cursor.allocate_local(bool_type, None);
        cursor.push_instr(
            Some(has_value),
            LoweredInstrKind::OptionalHasValue {
                operand: optional_local,
            },
        )?;
        let arm_block = cursor.create_block();
        let next_arm_block = cursor.create_block();
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: has_value,
            then_block: arm_block,
            else_block: next_arm_block,
        })?;

        cursor.switch_block(arm_block)?;
        let value_local = cursor.allocate_local(element_type, None);
        cursor.push_instr(
            Some(value_local),
            LoweredInstrKind::UnwrapShell {
                operand: optional_local,
            },
        )?;
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: binder_local,
                value: value_local,
            },
        )?;
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            arm_scope,
            &arm.body,
            DeferScopeKind::Ordinary,
        )?;
        if !cursor.current_block_terminated()? {
            cursor
                .terminate_current_block(crate::LoweredTerminator::Jump { target: exit_block })?;
        }
        cursor.switch_block(next_arm_block)?;
    }

    if let Some(default) = default {
        let default_scope = typed_package
            .program
            .resolved()
            .scopes
            .iter_with_ids()
            .find_map(|(candidate, scope)| {
                (scope.parent == Some(scope_id)
                    && scope.source_unit == Some(source_unit_id)
                    && !used_scopes.contains(&candidate))
                .then_some(candidate)
            })
            .unwrap_or(scope_id);
        let _ = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            default_scope,
            default,
            DeferScopeKind::Ordinary,
        )?;
        if !cursor.current_block_terminated()? {
            cursor
                .terminate_current_block(crate::LoweredTerminator::Jump { target: exit_block })?;
        }
    } else {
        let mut all_closed = None;
        for channel in channels {
            let closed = cursor.allocate_local(bool_type, None);
            cursor.push_instr(Some(closed), LoweredInstrKind::ChannelIsClosed { channel })?;
            all_closed = Some(if let Some(previous) = all_closed {
                let combined = cursor.allocate_local(bool_type, None);
                cursor.push_instr(
                    Some(combined),
                    LoweredInstrKind::BinaryOp {
                        op: LoweredBinaryOp::And,
                        left: previous,
                        right: closed,
                    },
                )?;
                combined
            } else {
                closed
            });
        }
        let wait_block = cursor.create_block();
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: all_closed.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "blocking select requires at least one channel arm",
                )
            })?,
            then_block: exit_block,
            else_block: wait_block,
        })?;
        cursor.switch_block(wait_block)?;
        cursor.push_instr(None, LoweredInstrKind::ProcessorYield)?;
        cursor.terminate_current_block(crate::LoweredTerminator::Jump {
            target: header_block,
        })?;
    }
    cursor.switch_block(exit_block)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_default_when_body(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    default: &[AstNode],
    after_block: &mut Option<LoweredBlockId>,
) -> Result<bool, LoweringError> {
    let _ = lower_body_sequence(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        default,
        DeferScopeKind::Ordinary,
    )?;
    if !cursor.current_block_terminated()? {
        let after_block = ensure_after_block(cursor, after_block);
        cursor.terminate_current_block(crate::LoweredTerminator::Jump {
            target: after_block,
        })?;
        return Ok(true);
    }
    Ok(false)
}

fn ensure_after_block(
    cursor: &mut RoutineCursor<'_>,
    after_block: &mut Option<LoweredBlockId>,
) -> LoweredBlockId {
    *after_block.get_or_insert_with(|| cursor.create_block())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_when_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expr: &AstNode,
    cases: &[fol_parser::ast::WhenCase],
    default: Option<&[AstNode]>,
) -> Result<LoweredValue, LoweringError> {
    use super::expressions::lower_expression;
    let Some(default) = default else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "when expressions require a default branch",
        ));
    };

    let subject = lower_expression(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        expr,
    )?;

    let join_block = cursor.create_block();
    let mut join_local = None;

    for (index, case) in cases.iter().enumerate() {
        // See `lower_when_statement`: `on(v)` tests shell presence and binds the
        // payload in the body block; other branches test value equality.
        let (condition_local, body, on_payload) = if let fol_parser::ast::WhenCase::On {
            channel,
            body,
        } = case
        {
            let bool_type = checked_type_map
                .get(&typed_package.program.builtin_types().bool_)
                .copied()
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "lowered workspace lost builtin bool while lowering an 'on' branch",
                    )
                })?;
            let present = cursor.allocate_local(bool_type, None);
            cursor.push_instr(
                Some(present),
                crate::LoweredInstrKind::OptionalHasValue {
                    operand: subject.local_id,
                },
            )?;
            let payload =
                lower_on_branch_payload(typed_package, checked_type_map, source_unit_id, channel)?;
            (present, body.as_slice(), Some(payload))
        } else {
            let (condition, body) = when_case_condition_and_body(case)?;
            let lowered_condition = lower_when_case_condition(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                &subject,
                condition,
            )?;
            (lowered_condition.local_id, body, None)
        };
        let body_block = cursor.create_block();
        let else_block = if index + 1 < cases.len() || !default.is_empty() {
            cursor.create_block()
        } else {
            join_block
        };
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: condition_local,
            then_block: body_block,
            else_block,
        })?;

        cursor.switch_block(body_block)?;
        if let Some((symbol, payload_type)) = on_payload {
            let payload_local = cursor.allocate_local(payload_type, None);
            cursor.push_instr(
                Some(payload_local),
                crate::LoweredInstrKind::UnwrapShell {
                    operand: subject.local_id,
                },
            )?;
            cursor.routine.local_symbols.insert(symbol, payload_local);
        }
        let branch_value = lower_body_sequence(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            body,
            DeferScopeKind::Ordinary,
        )?;
        lower_when_branch_value(cursor, &mut join_local, branch_value, join_block)?;

        if else_block != join_block {
            cursor.switch_block(else_block)?;
        }
    }

    let default_value = lower_body_sequence(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        default,
        DeferScopeKind::Ordinary,
    )?;
    lower_when_branch_value(cursor, &mut join_local, default_value, join_block)?;

    cursor.switch_block(join_block)?;
    join_local.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "value-producing when did not retain a lowered join value",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_when_case_condition(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    subject: &LoweredValue,
    condition: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let lowered_condition = lower_expression_observed(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        Some(subject.type_id),
        condition,
    )?;
    if subject.type_id != lowered_condition.type_id {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "when case condition type {} does not match subject type {}",
                lowered_condition.type_id.0, subject.type_id.0
            ),
        ));
    }
    let bool_type = checked_type_map
        .get(&typed_package.program.builtin_types().bool_)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "lowered workspace lost builtin bool while lowering when conditions",
            )
        })?;
    let eq_intrinsic = fol_intrinsics::intrinsic_by_canonical_name("eq")
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "intrinsic registry lost '.eq(...)' while lowering when conditions",
            )
        })?
        .id;
    let result_local = cursor.allocate_local(bool_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::IntrinsicCall {
            intrinsic: eq_intrinsic,
            args: vec![subject.local_id, lowered_condition.local_id],
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: bool_type,
        recoverable_error_type: None,
    })
}

fn lower_when_branch_value(
    cursor: &mut RoutineCursor<'_>,
    join_local: &mut Option<LoweredValue>,
    branch_value: Option<LoweredValue>,
    join_block: LoweredBlockId,
) -> Result<(), LoweringError> {
    match branch_value {
        Some(branch_value) => {
            let destination = if let Some(existing) = join_local {
                if existing.type_id != branch_value.type_id {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "value-producing when branches do not agree on one lowered join type",
                    ));
                }
                *existing
            } else {
                let local_id = cursor.allocate_local(branch_value.type_id, None);
                let value = LoweredValue {
                    local_id,
                    type_id: branch_value.type_id,
                    recoverable_error_type: None,
                };
                *join_local = Some(value);
                value
            };
            cursor.push_instr(
                None,
                crate::control::LoweredInstrKind::StoreLocal {
                    local: destination.local_id,
                    value: branch_value.local_id,
                },
            )?;
            if !cursor.current_block_terminated()? {
                cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                    target: join_block,
                })?;
            }
            Ok(())
        }
        None if cursor.current_block_terminated()? => Ok(()),
        None => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "value-producing when branches must yield a value or terminate early",
        )),
    }
}

/// Resolve the payload binding of an `on(v)` shell-choice branch: the value
/// binding declared in the on-branch scope and its lowered payload type.
fn lower_on_branch_payload(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    source_unit_id: SourceUnitId,
    channel: &AstNode,
) -> Result<(fol_resolver::SymbolId, LoweredTypeId), LoweringError> {
    let (name, syntax_id) = match channel {
        AstNode::Identifier {
            name,
            syntax_id: Some(syntax_id),
        } => (name, *syntax_id),
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "an 'on' branch requires a payload binding name, e.g. 'on(value)'",
            ))
        }
    };
    let on_scope = typed_package
        .program
        .resolved()
        .scope_for_syntax(syntax_id)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "'on' branch lost its resolver scope during lowering",
            )
        })?;
    let symbol = crate::decls::find_symbol_in_scope_or_descendants(
        &typed_package.program,
        source_unit_id,
        on_scope,
        SymbolKind::ValueBinding,
        name,
    )
    .ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("'on' binding '{name}' does not retain a lowering symbol"),
        )
    })?;
    let payload_type = typed_package
        .program
        .typed_symbol(symbol)
        .and_then(|sym| sym.declared_type)
        .and_then(|checked| checked_type_map.get(&checked).copied())
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("'on' binding '{name}' does not retain a lowered type"),
            )
        })?;
    Ok((symbol, payload_type))
}

pub(crate) fn when_case_condition_and_body(
    case: &fol_parser::ast::WhenCase,
) -> Result<(&AstNode, &[AstNode]), LoweringError> {
    match case {
        fol_parser::ast::WhenCase::Case { condition, body }
        | fol_parser::ast::WhenCase::Is {
            value: condition,
            body,
        } => Ok((condition, body.as_slice())),
        fol_parser::ast::WhenCase::In { .. }
        | fol_parser::ast::WhenCase::Has { .. }
        | fol_parser::ast::WhenCase::On { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "membership when/in/has branches and channel when/on branches are outside the shipped contract; typecheck should reject them (use select for channels)",
        )),
        fol_parser::ast::WhenCase::Of { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "type-matching when/of branches are not lowered in this slice yet",
        )),
    }
}
