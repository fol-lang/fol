use super::body::{lower_panic_terminator, lower_report_terminator, routine_error_type};
use super::cursor::{canonical_symbol_key, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::{lower_expression, lower_expression_expected, lower_expression_observed};
use crate::{
    control::LoweredInstrKind,
    ids::LoweredTypeId,
    LoweringError, LoweringErrorKind,
};
use fol_intrinsics::{select_intrinsic, IntrinsicEntry, IntrinsicSurface};
use fol_parser::ast::{AstNode, ContainerType};
use fol_resolver::{PackageIdentity, ReferenceKind, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::BTreeMap;

pub(crate) enum BoundLoweredCallArg<'a> {
    Explicit(&'a AstNode),
    Default(usize),
    VariadicPack(Vec<&'a AstNode>),
    VariadicUnpack(&'a AstNode),
}

/// The variadic pack is built at the call site from the concrete argument
/// expressions. When the callee is generic its declared variadic parameter
/// type still mentions the routine's generic parameter (e.g. `seq[T]`). Using
/// that as the pack's expected type would leak `T` into the caller's scope
/// (`FolSeq<t>` with `t` unbound), which the backend cannot emit. When there
/// are concrete elements to infer from, drop the generic expected type so the
/// pack is typed from its actual elements instead.
fn variadic_pack_expected(
    type_table: &crate::LoweredTypeTable,
    expected: Option<LoweredTypeId>,
    has_elements: bool,
) -> Option<LoweredTypeId> {
    match expected {
        Some(type_id)
            if has_elements && type_table.contains_generic_parameter(type_id) =>
        {
            None
        }
        other => other,
    }
}

pub(crate) fn bind_lowered_call_arguments<'a>(
    args: &'a [AstNode],
    param_names: &[String],
    param_defaults: &[Option<AstNode>],
    variadic_index: Option<usize>,
    display_name: &str,
) -> Result<Vec<BoundLoweredCallArg<'a>>, LoweringError> {
    let mut ordered_args = vec![None; param_names.len()];
    let mut variadic_trailing = Vec::new();
    let mut next_positional = 0usize;
    let mut seen_named = false;

    for arg in args {
        match arg {
            AstNode::NamedArgument { name, value } => {
                seen_named = true;
                let Some(index) = param_names.iter().position(|param| param == name) else {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("call to '{display_name}' does not have a parameter named '{name}'"),
                    ));
                };
                if ordered_args[index].is_some() {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("call to '{display_name}' supplies parameter '{name}' more than once"),
                    ));
                }
                ordered_args[index] = Some(value.as_ref());
            }
            AstNode::Unpack { .. } => {
                let Some(index) = variadic_index else {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "call-site unpack is only supported for variadic calls in V1",
                    ));
                };
                if index + 1 != param_names.len()
                    || ordered_args[index].is_some()
                    || !variadic_trailing.is_empty()
                {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "call-site unpack cannot be combined with other variadic arguments in V1",
                    ));
                }
                ordered_args[index] = Some(arg);
            }
            _ => {
                if seen_named {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("call to '{display_name}' cannot place positional arguments after named arguments"),
                    ));
                }
                if variadic_index.is_some_and(|index| next_positional >= index) {
                    variadic_trailing.push(arg);
                    continue;
                }
                if next_positional >= ordered_args.len() {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "call to '{display_name}' expects {} args but got {}",
                            param_names.len(),
                            args.len()
                        ),
                    ));
                }
                ordered_args[next_positional] = Some(arg);
                next_positional += 1;
            }
        }
    }

    if let Some(index) = variadic_index {
        if ordered_args[index].is_none() && !variadic_trailing.is_empty() {
            ordered_args[index] = Some(variadic_trailing[0]);
        }
    }

    let mut bound_args = Vec::with_capacity(ordered_args.len());
    for (index, arg) in ordered_args.into_iter().enumerate() {
        match arg {
            Some(AstNode::Unpack { value }) if variadic_index == Some(index) => {
                bound_args.push(BoundLoweredCallArg::VariadicUnpack(value.as_ref()));
            }
            Some(arg) if variadic_index == Some(index) && !variadic_trailing.is_empty() => {
                let mut packed = vec![arg];
                packed.extend(variadic_trailing.iter().skip(1).copied());
                bound_args.push(BoundLoweredCallArg::VariadicPack(packed));
            }
            Some(arg) => bound_args.push(BoundLoweredCallArg::Explicit(arg)),
            None if variadic_index == Some(index) => {
                bound_args.push(BoundLoweredCallArg::VariadicPack(Vec::new()));
            }
            None if matches!(param_defaults.get(index), Some(Some(_))) => {
                bound_args.push(BoundLoweredCallArg::Default(index));
            }
            None => {
                let missing_name = param_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("#{index}"));
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("call to '{display_name}' is missing required argument '{missing_name}'"),
                ));
            }
        }
    }

    Ok(bound_args)
}

pub(crate) fn lower_default_call_argument(
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    callee: crate::LoweredRoutineId,
    param_index: usize,
    expected: Option<LoweredTypeId>,
) -> Result<LoweredValue, LoweringError> {
    let default_info = decl_index.routine_param_defaults(callee).ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("call target {} does not retain lowered default arguments", callee.0),
        )
    })?;
    let default_expr = default_info
        .defaults
        .get(param_index)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "call target {} does not retain a default for parameter {}",
                    callee.0, param_index
                ),
            )
        })?;
    let typed_package = decl_index
        .typed_package(&default_info.package_identity)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "default argument lowering lost typed package '{}'",
                    default_info.package_identity.canonical_root
                ),
            )
        })?;
    lower_expression_expected(
        typed_package,
        type_table,
        checked_type_map,
        &default_info.package_identity,
        decl_index,
        cursor,
        default_info.source_unit_id,
        default_info.scope_id,
        expected,
        default_expr,
    )
}

pub(crate) fn lower_dot_intrinsic_call(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    name: &str,
    args: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("dot intrinsic '.{name}(...)' does not retain a syntax id"),
        )
    })?;
    let typed_node = typed_package.program.typed_node(syntax_id).ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("dot intrinsic '.{name}(...)' does not retain typed node facts"),
        )
    })?;
    let intrinsic_id = typed_node.intrinsic_id.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("dot intrinsic '.{name}(...)' does not retain a selected intrinsic id"),
        )
    })?;
    let checked_result = typed_node.inferred_type.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("dot intrinsic '.{name}(...)' does not retain a checked result type"),
        )
    })?;
    let result_type = checked_type_map
        .get(&checked_result)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("dot intrinsic '.{name}(...)' does not retain a lowered result type"),
            )
        })?;
    let lowered_args = args
        .iter()
        .map(|arg| {
            lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                arg,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let kind = match fol_intrinsics::lowering_mode_for_intrinsic(intrinsic_id) {
        Some(fol_intrinsics::IntrinsicLoweringMode::DedicatedIr) if name == "len" => {
            let [operand] = lowered_args.as_slice() else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("dot intrinsic '.{name}(...)' expected exactly 1 lowered operand"),
                ));
            };
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::LengthOf {
                    operand: operand.local_id,
                },
            )?;
            return Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            });
        }
        Some(fol_intrinsics::IntrinsicLoweringMode::RuntimeHook) if name == "echo" => {
            let [operand] = lowered_args.as_slice() else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("dot intrinsic '.{name}(...)' expected exactly 1 lowered operand"),
                ));
            };
            cursor.push_instr(
                None,
                LoweredInstrKind::RuntimeHook {
                    intrinsic: intrinsic_id,
                    args: vec![operand.local_id],
                },
            )?;
            return Ok(*operand);
        }
        _ => LoweredInstrKind::IntrinsicCall {
            intrinsic: intrinsic_id,
            args: lowered_args.iter().map(|value| value.local_id).collect(),
        },
    };
    let result_local = cursor.allocate_local(result_type, None);
    cursor.push_instr(Some(result_local), kind)?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: result_type,
        recoverable_error_type: None,
    })
}

pub(crate) fn lower_pipe_or_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    left: &AstNode,
    right: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let observed_left = lower_expression_observed(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        None,
        left,
    )?;
    if observed_left.recoverable_error_type.is_none() {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "'||' lowering requires a recoverable expression on the left in V1",
        ));
    }
    let bool_type = checked_type_map
        .get(&typed_package.program.builtin_types().bool_)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "lowered workspace lost builtin bool while lowering '||'",
            )
        })?;
    let condition_local = cursor.allocate_local(bool_type, None);
    cursor.push_instr(
        Some(condition_local),
        LoweredInstrKind::CheckRecoverable {
            operand: observed_left.local_id,
        },
    )?;

    let error_block = cursor.create_block();
    let success_block = cursor.create_block();
    let join_block = cursor.create_block();

    cursor.terminate_current_block(crate::LoweredTerminator::Branch {
        condition: condition_local,
        then_block: error_block,
        else_block: success_block,
    })?;

    // Lower the fallback arm first: a recoverable arm (chained `a || b || c`)
    // re-wraps the whole expression as recoverable, which changes the join
    // carrier from the plain value to the still-wrapped recoverable.
    cursor.switch_block(error_block)?;
    let fallback_value = lower_pipe_or_fallback(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        observed_left.type_id,
        right,
    )?;

    let arm_recover = fallback_value
        .as_ref()
        .and_then(|value| value.recoverable_error_type);
    let result_local = if arm_recover.is_some() {
        let carrier = fallback_value
            .as_ref()
            .and_then(|value| cursor.routine.locals.get(value.local_id))
            .and_then(|local| local.type_id)
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::Internal,
                    "recoverable fallback arm lost its lowered carrier type",
                )
            })?;
        cursor.allocate_local(carrier, None)
    } else {
        cursor.allocate_local(observed_left.type_id, None)
    };

    if let Some(fallback_value) = &fallback_value {
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: result_local,
                value: fallback_value.local_id,
            },
        )?;
        cursor.terminate_current_block(crate::LoweredTerminator::Jump { target: join_block })?;
    }

    cursor.switch_block(success_block)?;
    if arm_recover.is_some() {
        // Keep the left side still wrapped; the outer handler unwraps it.
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: result_local,
                value: observed_left.local_id,
            },
        )?;
    } else {
        let success_value = cursor.allocate_local(observed_left.type_id, None);
        cursor.push_instr(
            Some(success_value),
            LoweredInstrKind::UnwrapRecoverable {
                operand: observed_left.local_id,
            },
        )?;
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: result_local,
                value: success_value,
            },
        )?;
    }
    cursor.terminate_current_block(crate::LoweredTerminator::Jump { target: join_block })?;

    cursor.switch_block(join_block)?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: observed_left.type_id,
        recoverable_error_type: arm_recover,
    })
}

fn lower_pipe_or_fallback(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: LoweredTypeId,
    right: &AstNode,
) -> Result<Option<LoweredValue>, LoweringError> {
    match right {
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
                        format!(
                            "report expects exactly 1 value in lowered V1 bodies, got {}",
                            args.len()
                        ),
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
        AstNode::FunctionCall { name, args, .. } if name == "panic" => {
            let entry = select_intrinsic(IntrinsicSurface::KeywordCall, "panic").map_err(|_| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "panic should remain a keyword intrinsic in lowered V1",
                )
            })?;
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
                None,
                args,
            )
        }
        AstNode::Return { .. } => {
            use super::body::lower_body_node;
            let _ = lower_body_node(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                right,
            )?;
            Ok(None)
        }
        // A fallback arm may itself be recoverable (chained `a || b || c`);
        // the caller decides how to join it, so lower it observed.
        _ => lower_expression_observed(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            Some(expected_type),
            right,
        )
        .map(Some),
    }
}

pub(crate) fn lower_check_call(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    _syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    args: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let [operand] = args else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "check expects exactly 1 value in lowered V1, got {}",
                args.len()
            ),
        ));
    };
    let observed = lower_expression_observed(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        None,
        operand,
    )?;
    if observed.recoverable_error_type.is_none() {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "check lowering requires a recoverable routine result operand in V1",
        ));
    }
    let bool_type = checked_type_map
        .get(&typed_package.program.builtin_types().bool_)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "lowered workspace lost builtin bool while lowering check(...)",
            )
        })?;
    let result_local = cursor.allocate_local(bool_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::CheckRecoverable {
            operand: observed.local_id,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: bool_type,
        recoverable_error_type: None,
    })
}

pub(crate) fn lower_keyword_intrinsic_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    entry: &IntrinsicEntry,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    args: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    match entry.name {
        "check" => lower_check_call(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            syntax_id,
            args,
        ),
        "panic" => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "panic lowering requires a statement or control-flow context in V1",
        )),
        other => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("unsupported keyword intrinsic expression '{other}(...)'"),
        )),
    }
}

pub(crate) fn lower_keyword_intrinsic_statement(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    entry: &IntrinsicEntry,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    args: &[AstNode],
) -> Result<Option<LoweredValue>, LoweringError> {
    match entry.name {
        "panic" => {
            lower_keyword_panic_terminator(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                args,
            )?;
            Ok(None)
        }
        "check" => lower_keyword_intrinsic_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            entry,
            syntax_id,
            args,
        )
        .map(Some),
        other => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("unsupported keyword intrinsic statement '{other}(...)'"),
        )),
    }
}

fn lower_keyword_panic_terminator(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    args: &[AstNode],
) -> Result<(), LoweringError> {
    let lowered = match args {
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
                None,
                value,
            )?
            .local_id,
        ),
        [] => None,
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "panic expects at most 1 value in lowered V1, got {}",
                    args.len()
                ),
            ))
        }
    };
    lower_panic_terminator(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        lowered,
    )?;
    Ok(())
}

pub(crate) fn lower_function_call(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    kind: ReferenceKind,
    display_name: &str,
    args: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let resolved_symbol = resolve_reference_symbol(typed_package, syntax_id, kind, display_name)?;
    let (owning_identity, owning_symbol_id) = canonical_symbol_key(
        current_identity,
        resolved_symbol.mounted_from.as_ref(),
        resolved_symbol.id,
    );
    let callee_opt = decl_index.routine_id_for_symbol(&owning_identity, owning_symbol_id);

    // If the symbol is not a top-level routine, check if it's a function-typed local/parameter
    if callee_opt.is_none() {
        if let Some(local_id) = cursor.routine.local_symbols.get(&resolved_symbol.id).copied() {
            let local = cursor.routine.locals.get(local_id).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("call target '{display_name}' maps to missing local"),
                )
            })?;
            let local_type_id = local.type_id.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("call target '{display_name}' local does not have a type"),
                )
            })?;
            let local_type = type_table.get(local_type_id).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("call target '{display_name}' local type not found in type table"),
                )
            })?;
            if let crate::LoweredType::Routine(signature) = local_type {
                let signature = signature.clone();
                let callee_local = cursor.allocate_local(local_type_id, None);
                cursor.push_instr(
                    Some(callee_local),
                    LoweredInstrKind::LoadLocal { local: local_id },
                )?;
                let mut lowered_args = Vec::with_capacity(args.len());
                for (index, arg) in args.iter().enumerate() {
                    let expected = signature.params.get(index).copied();
                    let lowered = lower_expression_expected(
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
                    )?;
                    lowered_args.push(lowered.local_id);
                }
                return match signature.return_type {
                    Some(result_type) => {
                        let result_local = cursor.allocate_local(result_type, None);
                        cursor.push_instr(
                            Some(result_local),
                            LoweredInstrKind::CallIndirect {
                                callee: callee_local,
                                args: lowered_args,
                                error_type: signature.error_type,
                            },
                        )?;
                        Ok(LoweredValue {
                            local_id: result_local,
                            type_id: result_type,
                            recoverable_error_type: signature.error_type,
                        })
                    }
                    None => {
                        cursor.push_instr(
                            None,
                            LoweredInstrKind::CallIndirect {
                                callee: callee_local,
                                args: lowered_args,
                                error_type: signature.error_type,
                            },
                        )?;
                        Err(LoweringError::with_kind(
                            LoweringErrorKind::Unsupported,
                            format!("call to function-typed '{display_name}' with no return type cannot be used as a value"),
                        ))
                    }
                };
            }
        }
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("call target '{display_name}' does not map to a lowered routine definition"),
        ));
    }
    let callee = callee_opt.unwrap();
    let Some(result_type) =
        resolve_reference_type_id(typed_package, checked_type_map, syntax_id, kind)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "procedure-style calls without a value result are not lowered in this slice yet: '{display_name}'"
            ),
        ));
    };
    let param_types = decl_index
        .routine_param_types(callee)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered parameter types"),
            )
        })?
        .to_vec();
    let param_names = decl_index
        .routine_param_names(callee)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered parameter names"),
            )
        })?;
    let param_defaults = decl_index
        .routine_param_defaults(callee)
        .cloned()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered default arguments"),
            )
        })?;
    let ordered_args =
        bind_lowered_call_arguments(
            args,
            param_names,
            param_defaults.defaults.as_slice(),
            param_defaults.variadic_index,
            display_name,
        )?;
    let lowered_args = ordered_args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let expected = param_types.get(index).copied();
            match arg {
                BoundLoweredCallArg::Explicit(arg) => lower_expression_expected(
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
                ),
                BoundLoweredCallArg::Default(param_index) => lower_default_call_argument(
                    type_table,
                    checked_type_map,
                    decl_index,
                    cursor,
                    callee,
                    *param_index,
                    expected,
                ),
                BoundLoweredCallArg::VariadicUnpack(arg) => lower_expression_expected(
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
                ),
                BoundLoweredCallArg::VariadicPack(args) => {
                    let packed = AstNode::ContainerLiteral {
                        container_type: ContainerType::Sequence,
                        elements: args.iter().map(|arg| (*arg).clone()).collect(),
                    };
                    let pack_expected =
                        variadic_pack_expected(type_table, expected, !args.is_empty());
                    lower_expression_expected(
                        typed_package,
                        type_table,
                        checked_type_map,
                        current_identity,
                        decl_index,
                        cursor,
                        source_unit_id,
                        scope_id,
                        pack_expected,
                        &packed,
                    )
                }
            }
            .map(|value| value.local_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Prefer the call-site's recorded recoverable effect — this is the
    // monomorphized error type, which matters when the routine has a
    // generic error (e.g., `fun echo(T)(...): T / T`). Fall back to the
    // declared error type when no effect is recorded.
    let call_error_type = syntax_id
        .and_then(|syntax_id| typed_package.program.typed_node(syntax_id))
        .and_then(|node| node.recoverable_effect)
        .and_then(|effect| checked_type_map.get(&effect.error_type).copied())
        .or_else(|| lowered_symbol_error_type(typed_package, checked_type_map, resolved_symbol.id));
    let result_local = cursor.allocate_local(result_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::Call {
            callee,
            args: lowered_args,
            error_type: call_error_type,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: result_type,
        recoverable_error_type: call_error_type,
    })
}

pub(crate) fn lower_statement_free_call(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    kind: ReferenceKind,
    display_name: &str,
    args: &[AstNode],
) -> Result<Option<LoweredValue>, LoweringError> {
    let resolved_symbol = resolve_reference_symbol(typed_package, syntax_id, kind, display_name)?;
    let (owning_identity, owning_symbol_id) = canonical_symbol_key(
        current_identity,
        resolved_symbol.mounted_from.as_ref(),
        resolved_symbol.id,
    );
    let Some(callee) = decl_index.routine_id_for_symbol(&owning_identity, owning_symbol_id) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("call target '{display_name}' does not map to a lowered routine definition"),
        ));
    };
    let result_type =
        resolve_reference_type_id(typed_package, checked_type_map, syntax_id, kind);
    let param_types = decl_index
        .routine_param_types(callee)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered parameter types"),
            )
        })?
        .to_vec();
    let param_names = decl_index
        .routine_param_names(callee)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered parameter names"),
            )
        })?;
    let param_defaults = decl_index
        .routine_param_defaults(callee)
        .cloned()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("call target '{display_name}' does not retain lowered default arguments"),
            )
        })?;
    let ordered_args =
        bind_lowered_call_arguments(
            args,
            param_names,
            param_defaults.defaults.as_slice(),
            param_defaults.variadic_index,
            display_name,
        )?;
    let lowered_args = ordered_args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let expected = param_types.get(index).copied();
            match arg {
                BoundLoweredCallArg::Explicit(arg) => lower_expression_expected(
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
                ),
                BoundLoweredCallArg::Default(param_index) => lower_default_call_argument(
                    type_table,
                    checked_type_map,
                    decl_index,
                    cursor,
                    callee,
                    *param_index,
                    expected,
                ),
                BoundLoweredCallArg::VariadicUnpack(arg) => lower_expression_expected(
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
                ),
                BoundLoweredCallArg::VariadicPack(args) => {
                    let packed = AstNode::ContainerLiteral {
                        container_type: ContainerType::Sequence,
                        elements: args.iter().map(|arg| (*arg).clone()).collect(),
                    };
                    let pack_expected =
                        variadic_pack_expected(type_table, expected, !args.is_empty());
                    lower_expression_expected(
                        typed_package,
                        type_table,
                        checked_type_map,
                        current_identity,
                        decl_index,
                        cursor,
                        source_unit_id,
                        scope_id,
                        pack_expected,
                        &packed,
                    )
                }
            }
            .map(|value| value.local_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let call_error_type =
        lowered_symbol_error_type(typed_package, checked_type_map, resolved_symbol.id);
    match result_type {
        Some(result_type) => {
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::Call {
                    callee,
                    args: lowered_args,
                    error_type: call_error_type,
                },
            )?;
            Ok(Some(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: call_error_type,
            }))
        }
        None => {
            cursor.push_instr(
                None,
                LoweredInstrKind::Call {
                    callee,
                    args: lowered_args,
                    error_type: call_error_type,
                },
            )?;
            Ok(None)
        }
    }
}

pub(crate) fn resolve_reference_type_id(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    kind: ReferenceKind,
) -> Option<LoweredTypeId> {
    let syntax_id = syntax_id?;
    let reference = typed_package
        .program
        .resolved()
        .references
        .iter()
        .find(|reference| reference.syntax_id == Some(syntax_id) && reference.kind == kind)?;
    let checked_type = reference_type_id(typed_package, reference.id)?;
    checked_type_map.get(&checked_type).copied()
}

pub(crate) fn reference_type_id(
    typed_package: &fol_typecheck::TypedPackage,
    reference_id: fol_resolver::ReferenceId,
) -> Option<fol_typecheck::CheckedTypeId> {
    typed_package
        .program
        .typed_reference(reference_id)?
        .resolved_type
}

fn lowered_symbol_error_type(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    symbol_id: SymbolId,
) -> Option<LoweredTypeId> {
    let declared_type = typed_package
        .program
        .typed_symbol(symbol_id)?
        .declared_type?;
    let fol_typecheck::CheckedType::Routine(signature) =
        typed_package.program.type_table().get(declared_type)?
    else {
        return None;
    };
    signature
        .error_type
        .and_then(|error_type| checked_type_map.get(&error_type).copied())
}

pub(crate) fn resolve_method_target(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    method: &str,
    receiver_type: LoweredTypeId,
    call_syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
) -> Result<crate::LoweredRoutineId, LoweringError>
{
    // Fast path: typecheck already resolved the method symbol for this call
    // site (including generic receiver unification). Prefer that when present.
    if let Some(syntax_id) = call_syntax_id {
        if let Some(symbol_id) = typed_package.program.method_call_target(syntax_id) {
            let symbol = typed_package
                .program
                .resolved()
                .symbol(symbol_id)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("method '{method}' resolved to an unknown symbol"),
                    )
                })?;
            let (owning_identity, owning_symbol_id) =
                canonical_symbol_key(current_identity, symbol.mounted_from.as_ref(), symbol_id);
            if let Some(routine_id) =
                decl_index.routine_id_for_symbol(&owning_identity, owning_symbol_id)
            {
                return Ok(routine_id);
            }
        }
    }

    let mut matches = Vec::new();

    for (symbol_id, symbol) in typed_package.program.resolved().symbols.iter_with_ids() {
        if symbol.kind != SymbolKind::Routine || symbol.name != method {
            continue;
        }
        let Some(typed_symbol) = typed_package.program.typed_symbol(symbol_id) else {
            continue;
        };
        let Some(receiver_checked_type) = typed_symbol.receiver_type else {
            continue;
        };
        let Some(lowered_receiver_type) = checked_type_map.get(&receiver_checked_type).copied()
        else {
            continue;
        };
        if lowered_receiver_type != receiver_type {
            continue;
        }

        let (owning_identity, owning_symbol_id) =
            canonical_symbol_key(current_identity, symbol.mounted_from.as_ref(), symbol_id);
        let Some(routine_id) = decl_index.routine_id_for_symbol(&owning_identity, owning_symbol_id)
        else {
            continue;
        };
        matches.push(routine_id);
    }

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("method '{method}' is not available for the lowered receiver type"),
        )),
        _ => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("method '{method}' is ambiguous for the lowered receiver type"),
        )),
    }
}

pub(crate) fn resolve_reference_symbol<'a>(
    typed_package: &'a fol_typecheck::TypedPackage,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    kind: ReferenceKind,
    display_name: &str,
) -> Result<&'a fol_resolver::ResolvedSymbol, LoweringError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("reference '{display_name}' does not retain a syntax id"),
        )
    })?;
    let Some(reference) = typed_package
        .program
        .resolved()
        .references
        .iter()
        .find(|reference| reference.syntax_id == Some(syntax_id) && reference.kind == kind)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("reference '{display_name}' is missing from resolver output"),
        ));
    };
    let Some(symbol_id) = reference.resolved else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("reference '{display_name}' does not resolve to a lowered symbol"),
        ));
    };
    typed_package
        .program
        .resolved()
        .symbol(symbol_id)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("reference '{display_name}' lost its resolved symbol"),
            )
        })
}
