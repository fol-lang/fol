use super::body::lower_body_sequence;
use super::cursor::{DeferScopeKind, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_observed;
use crate::{
    control::{LoweredBinaryOp, LoweredInstrKind, LoweredOperand},
    ids::{LoweredBlockId, LoweredTypeId},
    LoweringError, LoweringErrorKind,
};
use fol_parser::ast::{AstNode, Literal, LoopCondition};
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

    let mut after_block = None;
    let mut has_fallthrough = false;

    for (index, case) in cases.iter().enumerate() {
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
        let body_block = cursor.create_block();
        let else_block = if index + 1 < cases.len() || default.is_some() {
            cursor.create_block()
        } else {
            ensure_after_block(cursor, &mut after_block)
        };
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: lowered_condition.local_id,
            then_block: body_block,
            else_block,
        })?;

        cursor.switch_block(body_block)?;
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

pub(crate) fn lower_loop_statement(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    condition: &LoopCondition,
    body: &[AstNode],
) -> Result<(), LoweringError> {
    use super::expressions::lower_expression;
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
                scope_id,
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
            let int_type =
                super::helpers::literal_type_id(typed_package, checked_type_map, &Literal::Integer(0))
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
            let binder_scope_id = typed_package
                .program
                .resolved()
                .scopes
                .iter_with_ids()
                .find_map(|(sid, s)| {
                    (s.kind == fol_resolver::ScopeKind::LoopBinder
                        && s.parent == Some(scope_id)
                        && s.source_unit == Some(source_unit_id))
                    .then_some(sid)
                })
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "iteration loop binder scope not found",
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
            cursor.routine.local_symbols.insert(binder_symbol_id, binder_local);

            let header_block = cursor.create_block();
            let body_block = cursor.create_block();
            let exit_block = cursor.create_block();

            cursor.terminate_current_block(crate::LoweredTerminator::Jump {
                target: header_block,
            })?;

            // Header: check index < len
            cursor.switch_block(header_block)?;
            let cmp_local = cursor.allocate_local(
                super::helpers::literal_type_id(typed_package, checked_type_map, &Literal::Boolean(true))
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
        let body_block = cursor.create_block();
        let else_block = if index + 1 < cases.len() || !default.is_empty() {
            cursor.create_block()
        } else {
            join_block
        };
        cursor.terminate_current_block(crate::LoweredTerminator::Branch {
            condition: lowered_condition.local_id,
            then_block: body_block,
            else_block,
        })?;

        cursor.switch_block(body_block)?;
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
            "membership and channel when branches are not lowered in V1; typecheck should have rejected them",
        )),
        fol_parser::ast::WhenCase::Of { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "type-matching when/of branches are not lowered in this slice yet",
        )),
    }
}
