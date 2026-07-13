use super::body::lower_body_sequence;
use super::calls::{
    lower_dot_intrinsic_call, lower_function_call, lower_keyword_intrinsic_expression,
    lower_pipe_or_expression, reference_type_id, resolve_method_target,
};
use super::containers::{
    apply_expected_shell_wrap, field_access_type, index_access_type, lower_container_literal,
    lower_nil_literal, lower_record_initializer, slice_access_type,
};
use super::cursor::{DeferScopeKind, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::flow::lower_when_expression;
use super::helpers::{
    describe_binary_operator, describe_unary_operator,
    literal_type_id, lower_assignment_target, lower_entry_variant_access, lower_unwrap_expression,
};
use crate::{
    control::{LoweredInstrKind, LoweredBinaryOp, LoweredUnaryOp},
    ids::LoweredTypeId,
    types::{LoweredRoutineType, LoweredType},
    LoweredBlock, LoweredLocal, LoweredRoutine, LoweringError, LoweringErrorKind,
};
use fol_intrinsics::{select_intrinsic, IntrinsicSurface};
use fol_parser::ast::{AstNode, CallSurface, FolType, Literal};
use fol_resolver::{PackageIdentity, ReferenceKind, ScopeId, SourceUnitId, SymbolKind};
use std::collections::BTreeMap;

pub(crate) fn lower_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    node: &AstNode,
) -> Result<LoweredValue, LoweringError> {
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
        node,
    )
}

pub(crate) fn lower_expression_expected(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    node: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let lowered = lower_expression_observed(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        expected_type,
        node,
    )?;
    if let Some(error_type) = lowered.recoverable_error_type {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "recoverable value with lowered error type {} cannot enter plain expected lowering; handle it with '||' or check(...)",
                error_type.0
            ),
        ));
    }
    apply_expected_shell_wrap(type_table, cursor, expected_type, lowered)
}

pub(crate) fn lower_expression_observed(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    node: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    // Expression lowering recurses with the AST; grow the stack in segments
    // so deep (but legal) nesting cannot overflow worker-thread stacks.
    stacker::maybe_grow(256 * 1024, 4 * 1024 * 1024, || {
        lower_expression_observed_inner(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_type,
            node,
        )
    })
}

fn lower_expression_observed_inner(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    node: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let lowered = match node {
        AstNode::Literal(Literal::Nil) => lower_nil_literal(type_table, cursor, expected_type),
        AstNode::Literal(literal) => {
            let type_id =
                literal_type_id(typed_package, checked_type_map, literal).ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "literal expression does not retain a lowering-owned type",
                    )
                })?;
            {
                // A width-classified character literal may have been typed as
                // a single-element string by the expected-type rule; lower it
                // as the string it denotes.
                let expected_is_str = expected_type.is_some_and(|expected| {
                    type_table.get(expected).is_some_and(|ty| {
                        matches!(
                            ty,
                            crate::LoweredType::Builtin(crate::LoweredBuiltinType::Str)
                        )
                    })
                });
                match literal {
                    Literal::Character(value) if expected_is_str => cursor.lower_literal(
                        &Literal::String(value.to_string()),
                        expected_type.expect("expected str type is present"),
                    ),
                    _ => cursor.lower_literal(literal, type_id),
                }
            }
        }
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::Unwrap,
            operand,
        } => lower_unwrap_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            operand,
        ),
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::Neg,
            operand,
        } => lower_unary_op(
            typed_package, type_table, checked_type_map, current_identity,
            decl_index, cursor, source_unit_id, scope_id, LoweredUnaryOp::Neg, operand,
        ),
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::Not,
            operand,
        } => lower_unary_op(
            typed_package, type_table, checked_type_map, current_identity,
            decl_index, cursor, source_unit_id, scope_id, LoweredUnaryOp::Not, operand,
        ),
        AstNode::UnaryOp { op, .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "unary operator lowering for '{}' is not yet supported",
                describe_unary_operator(op)
            ),
        )),
        AstNode::BinaryOp {
            op: fol_parser::ast::BinaryOperator::PipeOr,
            left,
            right,
        } => lower_pipe_or_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            left,
            right,
        ),
        AstNode::BinaryOp { op, left, right } => {
            let lowered_op = match op {
                fol_parser::ast::BinaryOperator::Add => LoweredBinaryOp::Add,
                fol_parser::ast::BinaryOperator::Sub => LoweredBinaryOp::Sub,
                fol_parser::ast::BinaryOperator::Mul => LoweredBinaryOp::Mul,
                fol_parser::ast::BinaryOperator::Div => LoweredBinaryOp::Div,
                fol_parser::ast::BinaryOperator::Mod => LoweredBinaryOp::Mod,
                fol_parser::ast::BinaryOperator::Pow => LoweredBinaryOp::Pow,
                fol_parser::ast::BinaryOperator::Eq => LoweredBinaryOp::Eq,
                fol_parser::ast::BinaryOperator::Ne => LoweredBinaryOp::Ne,
                fol_parser::ast::BinaryOperator::Lt => LoweredBinaryOp::Lt,
                fol_parser::ast::BinaryOperator::Le => LoweredBinaryOp::Le,
                fol_parser::ast::BinaryOperator::Gt => LoweredBinaryOp::Gt,
                fol_parser::ast::BinaryOperator::Ge => LoweredBinaryOp::Ge,
                fol_parser::ast::BinaryOperator::And => LoweredBinaryOp::And,
                fol_parser::ast::BinaryOperator::Or => LoweredBinaryOp::Or,
                fol_parser::ast::BinaryOperator::Xor => LoweredBinaryOp::Xor,
                other => {
                    return Err(LoweringError::with_kind(
                        LoweringErrorKind::Unsupported,
                        format!(
                            "binary operator lowering for '{}' is not yet supported",
                            describe_binary_operator(other)
                        ),
                    ));
                }
            };
            lower_binary_op(
                typed_package, type_table, checked_type_map, current_identity,
                decl_index, cursor, source_unit_id, scope_id, lowered_op, left, right,
            )
        }
        AstNode::RecordInit { fields, .. } => lower_record_initializer(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_type,
            fields,
        ),
        AstNode::ContainerLiteral {
            container_type,
            elements,
        } => lower_container_literal(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            container_type.clone(),
            expected_type,
            elements,
        ),
        AstNode::Assignment { target, value } => {
            let lowered_value = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                value,
            )?;
            lower_assignment_target(
                typed_package,
                current_identity,
                decl_index,
                cursor,
                target,
                lowered_value,
            )
        }
        AstNode::FunctionCall {
            surface: CallSurface::DotIntrinsic,
            syntax_id,
            name,
            args,
            ..
        } => lower_dot_intrinsic_call(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            *syntax_id,
            name,
            args,
        ),
        AstNode::FunctionCall {
            syntax_id,
            name,
            args,
            ..
        } => {
            if let Ok(entry) = select_intrinsic(IntrinsicSurface::KeywordCall, name) {
                lower_keyword_intrinsic_expression(
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
                lower_function_call(
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
        AstNode::QualifiedFunctionCall { path, args } => lower_function_call(
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
        AstNode::MethodCall {
            syntax_id,
            object,
            method,
            args,
        } => {
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
            // to monomorphization.
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
                let result_type = result_type.ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::Unsupported,
                        format!(
                            "constraint call '{method}' cannot be used as an expression without a result type"
                        ),
                    )
                })?;
                let result_local = cursor.allocate_local(result_type, None);
                cursor.push_instr(
                    Some(result_local),
                    crate::control::LoweredInstrKind::ConstraintCall {
                        method: method.clone(),
                        args: lowered_args,
                        error_type,
                    },
                )?;
                return Ok(LoweredValue {
                    local_id: result_local,
                    type_id: result_type,
                    recoverable_error_type: error_type,
                });
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
            let typed_node = syntax_id.and_then(|syntax_id| typed_package.program.typed_node(syntax_id));
            let result_type = typed_node
                .and_then(|node| node.inferred_type)
                .and_then(|checked_type| checked_type_map.get(&checked_type).copied())
                .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::Unsupported,
                    format!(
                        "procedure-style method call '{method}' cannot be used as an expression value"
                    ),
                )
            })?;
            let error_type = typed_node
                .and_then(|node| node.recoverable_effect)
                .and_then(|effect| checked_type_map.get(&effect.error_type).copied());
            let callee_has_receiver = decl_index.routine_has_receiver(callee);
            let mut lowered_args = if callee_has_receiver {
                vec![receiver.local_id]
            } else {
                Vec::new()
            };
            let receiver_skip: usize = if callee_has_receiver { 1 } else { 0 };
            let param_types = decl_index
                .routine_param_types(callee)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("method '{method}' does not retain lowered parameter types"),
                    )
                })?
                .to_vec();
            let param_names = decl_index
                .routine_param_names(callee)
                .ok_or_else(|| {
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
                param_names.get(receiver_skip..).unwrap_or(&[]),
                param_defaults.defaults.get(receiver_skip..).unwrap_or(&[]),
                param_defaults
                    .variadic_index
                    .map(|index| index.saturating_sub(receiver_skip)),
                method,
            )?;
            lowered_args.extend(
                ordered_args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        let expected = param_types.get(index + receiver_skip).copied();
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
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::Call {
                    callee,
                    args: lowered_args,
                    error_type,
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: error_type,
            })
        }
        AstNode::FieldAccess { object, field } => {
            if let Some(entry_value) = lower_entry_variant_access(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                object,
                field,
                expected_type,
            )? {
                return apply_expected_shell_wrap(type_table, cursor, expected_type, entry_value);
            }
            let base = lower_expression(
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
            let Some(result_type) = field_access_type(type_table, base.type_id, field) else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("field access '.{field}' does not map to a lowered record field"),
                ));
            };
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::FieldAccess {
                    base: base.local_id,
                    field: field.clone(),
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            })
        }
        AstNode::IndexAccess { container, index } => {
            let lowered_container = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                container,
            )?;
            let expected_index_type =
                super::containers::index_key_type(type_table, lowered_container.type_id);
            let lowered_index = lower_expression_observed(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                expected_index_type,
                index,
            )?;
            let Some(result_type) = index_access_type(type_table, lowered_container.type_id, index)
            else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "index access does not map to a lowered container element type",
                ));
            };
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::IndexAccess {
                    container: lowered_container.local_id,
                    index: lowered_index.local_id,
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            })
        }
        AstNode::When {
            expr,
            cases,
            default,
        } => lower_when_expression(
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
        ),
        AstNode::Identifier { syntax_id, name } => {
            let syntax_id = syntax_id.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("identifier '{name}' does not retain a syntax id"),
                )
            })?;
            let Some(reference) =
                typed_package
                    .program
                    .resolved()
                    .references
                    .iter()
                    .find(|reference| {
                        reference.syntax_id == Some(syntax_id)
                            && reference.kind == ReferenceKind::Identifier
                    })
            else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("identifier '{name}' is missing from resolver output"),
                ));
            };
            let Some(symbol_id) = reference.resolved else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("identifier '{name}' does not resolve to a lowered symbol"),
                ));
            };
            let resolved_symbol = typed_package
                .program
                .resolved()
                .symbol(symbol_id)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("identifier '{name}' lost its resolved symbol"),
                    )
                })?;
            let result_type = reference_type_id(typed_package, reference.id).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("identifier '{name}' does not retain a lowered reference type"),
                )
            })?;
            let result_type = checked_type_map.get(&result_type).copied().ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("identifier '{name}' does not retain a lowered reference type"),
                )
            })?;
            cursor.lower_identifier_reference(
                current_identity,
                decl_index,
                resolved_symbol,
                result_type,
            )
        }
        AstNode::QualifiedIdentifier { path } => {
            let syntax_id = path.syntax_id().ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified identifier '{}' does not retain a syntax id",
                        path.joined()
                    ),
                )
            })?;
            let Some(reference) =
                typed_package
                    .program
                    .resolved()
                    .references
                    .iter()
                    .find(|reference| {
                        reference.syntax_id == Some(syntax_id)
                            && reference.kind == ReferenceKind::QualifiedIdentifier
                    })
            else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified identifier '{}' is missing from resolver output",
                        path.joined()
                    ),
                ));
            };
            let Some(symbol_id) = reference.resolved else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified identifier '{}' does not resolve to a lowered symbol",
                        path.joined()
                    ),
                ));
            };
            let resolved_symbol = typed_package
                .program
                .resolved()
                .symbol(symbol_id)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "qualified identifier '{}' lost its resolved symbol",
                            path.joined()
                        ),
                    )
                })?;
            let result_type = reference_type_id(typed_package, reference.id).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified identifier '{}' does not retain a lowered reference type",
                        path.joined()
                    ),
                )
            })?;
            let result_type = checked_type_map.get(&result_type).copied().ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified identifier '{}' does not retain a lowered reference type",
                        path.joined()
                    ),
                )
            })?;
            cursor.lower_identifier_reference(
                current_identity,
                decl_index,
                resolved_symbol,
                result_type,
            )
        }
        AstNode::Commented { node, .. } => lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_type,
            node,
        ),
        AstNode::Invoke { callee, args } => lower_invoke_expression(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            callee,
            args,
        ),
        AstNode::AnonymousFun {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            ..
        }
        | AstNode::AnonymousPro {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            ..
        }
        | AstNode::AnonymousLog {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            ..
        } => lower_anonymous_routine(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            *syntax_id,
            captures,
            params,
            return_type.as_ref(),
            error_type.as_ref(),
            body,
        ),
        // V1 pipeline gaps (Phase 3)
        AstNode::TemplateCall { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "template call lowering is not yet implemented",
        )),
        AstNode::AvailabilityAccess { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "availability access lowering is not yet implemented",
        )),
        AstNode::SliceAccess {
            container,
            start,
            end,
            ..
        } => {
            let lowered_container = lower_expression(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                container,
            )?;
            let lowered_start = if let Some(start) = start {
                lower_expression(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    start,
                )?
            } else {
                let int_type = literal_type_id(typed_package, checked_type_map, &Literal::Integer(0))
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            "int type not found for slice default start bound",
                        )
                    })?;
                let zero_local = cursor.allocate_local(int_type, None);
                cursor.push_instr(
                    Some(zero_local),
                    LoweredInstrKind::Const(crate::control::LoweredOperand::Int(0)),
                )?;
                LoweredValue {
                    local_id: zero_local,
                    type_id: int_type,
                    recoverable_error_type: None,
                }
            };
            let lowered_end = if let Some(end) = end {
                lower_expression(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    end,
                )?
            } else {
                let int_type = literal_type_id(typed_package, checked_type_map, &Literal::Integer(0))
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            "int type not found for slice default end bound",
                        )
                    })?;
                let len_local = cursor.allocate_local(int_type, None);
                cursor.push_instr(
                    Some(len_local),
                    LoweredInstrKind::LengthOf {
                        operand: lowered_container.local_id,
                    },
                )?;
                LoweredValue {
                    local_id: len_local,
                    type_id: int_type,
                    recoverable_error_type: None,
                }
            };
            let Some(result_type) =
                slice_access_type(type_table, lowered_container.type_id)
            else {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "slice access does not map to a lowered container type",
                ));
            };
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::SliceAccess {
                    container: lowered_container.local_id,
                    start: lowered_start.local_id,
                    end: lowered_end.local_id,
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            })
        }
        AstNode::Loop { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "loop lowering is not yet implemented",
        )),
        AstNode::Block { statements: _, .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "block expression lowering is not yet implemented",
        )),
        // Beyond V1 — deferred
        AstNode::AsyncStage
        | AstNode::AwaitStage
        | AstNode::Spawn { .. }
        | AstNode::ChannelAccess { .. }
        | AstNode::Select { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "concurrency primitives (async, await, spawn, channels, select) are not yet supported",
        )),
        AstNode::Rolling { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "rolling/comprehension expressions are not yet supported",
        )),
        AstNode::Range { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "range expressions are not yet supported",
        )),
        AstNode::Yield { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "yield expressions are not yet supported",
        )),
        AstNode::PatternAccess { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "pattern access is not yet supported",
        )),
        // Structural nodes consumed by parent lowering
        AstNode::NamedArgument { .. } | AstNode::Unpack { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "named arguments and unpacks should be consumed by call-site lowering",
        )),
        AstNode::PatternWildcard | AstNode::PatternCapture { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "pattern elements should be consumed by pattern matching lowering",
        )),
        // Statement nodes in expression position
        AstNode::Return { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "return statement should not appear in expression lowering",
        )),
        AstNode::Break => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "break statement should not appear in expression lowering",
        )),
        AstNode::Dfr { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "dfr statement should not appear in expression lowering",
        )),
        AstNode::Inquiry { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "inquiry clause should not appear in expression lowering",
        )),
        // Declaration nodes should never appear in expression position
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
        | AstNode::LabDecl { .. }
        | AstNode::Comment { .. }
        | AstNode::Program { .. } => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "declaration node should not appear in expression lowering",
        )),
    }?;
    apply_expected_shell_wrap(type_table, cursor, expected_type, lowered)
}

fn resolve_fol_type_to_lowered(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    fol_type: &FolType,
) -> Result<LoweredTypeId, LoweringError> {
    let builtins = typed_package.program.builtin_types();
    let checked_id = match fol_type {
        FolType::Int { .. } => builtins.int,
        FolType::Float { .. } => builtins.float,
        FolType::Bool => builtins.bool_,
        FolType::Char { .. } => builtins.char_,
        ty if ty.is_builtin_str() => builtins.str_,
        FolType::Never => builtins.never,
        FolType::Named { name, syntax_id } => {
            let syntax_id = syntax_id.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("type annotation '{name}' does not retain a syntax id"),
                )
            })?;
            let reference = typed_package
                .program
                .resolved()
                .references
                .iter()
                .find(|r| {
                    r.syntax_id == Some(syntax_id) && r.kind == ReferenceKind::TypeName
                })
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("type annotation '{name}' is missing from resolver output"),
                    )
                })?;
            let symbol_id = reference.resolved.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("type annotation '{name}' does not resolve to a symbol"),
                )
            })?;
            let typed_symbol =
                typed_package
                    .program
                    .typed_symbol(symbol_id)
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!("type annotation '{name}' lost its typed symbol"),
                        )
                    })?;
            return typed_symbol
                .declared_type
                .and_then(|id| checked_type_map.get(&id).copied())
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("type annotation '{name}' does not map to a lowered type"),
                    )
                });
        }
        FolType::QualifiedNamed { path } => {
            let syntax_id = path.syntax_id().ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified type '{}' does not retain a syntax id",
                        path.joined()
                    ),
                )
            })?;
            let reference = typed_package
                .program
                .resolved()
                .references
                .iter()
                .find(|r| {
                    r.syntax_id == Some(syntax_id)
                        && r.kind == ReferenceKind::QualifiedTypeName
                })
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "qualified type '{}' is missing from resolver output",
                            path.joined()
                        ),
                    )
                })?;
            let symbol_id = reference.resolved.ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "qualified type '{}' does not resolve to a symbol",
                        path.joined()
                    ),
                )
            })?;
            let typed_symbol =
                typed_package
                    .program
                    .typed_symbol(symbol_id)
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "qualified type '{}' lost its typed symbol",
                                path.joined()
                            ),
                        )
                    })?;
            return typed_symbol
                .declared_type
                .and_then(|id| checked_type_map.get(&id).copied())
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "qualified type '{}' does not map to a lowered type",
                            path.joined()
                        ),
                    )
                });
        }
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                "complex type annotation in anonymous routine is not yet supported",
            ));
        }
    };
    checked_type_map.get(&checked_id).copied().ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "type annotation does not map to a lowered type",
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_anonymous_routine(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    _scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    captures: &[String],
    params: &[fol_parser::ast::Parameter],
    return_type: Option<&FolType>,
    error_type: Option<&FolType>,
    body: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    if !captures.is_empty() {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "anonymous routines with captures are not yet supported",
        ));
    }

    // Resolve parameter types
    let mut param_lowered_types = Vec::with_capacity(params.len());
    for param in params {
        param_lowered_types.push(resolve_fol_type_to_lowered(
            typed_package,
            checked_type_map,
            &param.param_type,
        )?);
    }

    // Resolve return and error types
    let lowered_return_type = match return_type {
        None | Some(FolType::None) => None,
        Some(ty) => Some(resolve_fol_type_to_lowered(typed_package, checked_type_map, ty)?),
    };
    let lowered_error_type = match error_type {
        None | Some(FolType::None) => None,
        Some(ty) => Some(resolve_fol_type_to_lowered(typed_package, checked_type_map, ty)?),
    };

    // Find the signature type in the lowered type table
    let signature_type = LoweredType::Routine(LoweredRoutineType {
        params: param_lowered_types.clone(),
        return_type: lowered_return_type,
        error_type: lowered_error_type,
    });
    let signature_type_id = type_table.find(&signature_type).ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "anonymous routine signature type is not present in the lowered type table",
        )
    })?;

    // Create anonymous routine
    let routine_id = crate::LoweredRoutineId(cursor.next_routine_index);
    cursor.next_routine_index += 1;
    let anon_name = format!("__anon_{}", routine_id.0);
    let mut anon_routine = LoweredRoutine::new(routine_id, &anon_name, crate::LoweredBlockId(0));
    anon_routine.source_unit_id = Some(source_unit_id);
    anon_routine.signature = Some(signature_type_id);
    let entry_block = anon_routine.blocks.push(LoweredBlock {
        id: crate::LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: None,
    });
    anon_routine.entry_block = entry_block;

    // Set up parameters
    let routine_scope_id = syntax_id
        .and_then(|sid| typed_package.program.resolved().scope_for_syntax(sid));
    let mut next_local_index = 0;
    for (param, &param_type) in params.iter().zip(param_lowered_types.iter()) {
        let local_id = anon_routine.locals.push(LoweredLocal {
            id: crate::LoweredLocalId(next_local_index),
            type_id: Some(param_type),
            name: Some(param.name.clone()),
        });
        if let Some(scope_id) = routine_scope_id {
            if let Some(param_symbol_id) = crate::decls::find_symbol_in_scope_or_descendants(
                &typed_package.program,
                source_unit_id,
                scope_id,
                SymbolKind::Parameter,
                &param.name,
            ) {
                anon_routine.local_symbols.insert(param_symbol_id, local_id);
            }
        }
        anon_routine.params.push(local_id);
        next_local_index += 1;
    }

    // Lower body into the anonymous routine
    let scope_id = routine_scope_id.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "anonymous routine does not retain a scope for body lowering",
        )
    })?;
    let mut anon_cursor = RoutineCursor::new(&mut anon_routine, entry_block);
    anon_cursor.next_routine_index = cursor.next_routine_index;
    anon_cursor.routine.body_result = lower_body_sequence(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        &mut anon_cursor,
        source_unit_id,
        scope_id,
        body,
        DeferScopeKind::Ordinary,
    )?
    .map(|value| value.local_id);
    cursor.next_routine_index = anon_cursor.next_routine_index;
    let nested_anon = std::mem::take(&mut anon_cursor.anonymous_routines);
    drop(anon_cursor);

    cursor.anonymous_routines.extend(nested_anon);
    cursor.anonymous_routines.push(anon_routine);

    // Emit RoutineRef instruction in the current routine
    let result_local = cursor.allocate_local(signature_type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::RoutineRef { routine: routine_id },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: signature_type_id,
        recoverable_error_type: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_invoke_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    callee: &AstNode,
    args: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let callee_value = lower_expression(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        callee,
    )?;

    // Extract the routine signature from the callee's type
    let callee_type = type_table.get(callee_value.type_id).ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "invoke callee does not retain a lowered type",
        )
    })?;
    let LoweredType::Routine(signature) = callee_type else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "invoke callee is not a routine type",
        ));
    };
    let signature = signature.clone();

    // Lower arguments with expected param types
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

    // Emit CallIndirect instruction
    match signature.return_type {
        Some(result_type) => {
            let result_local = cursor.allocate_local(result_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::CallIndirect {
                    callee: callee_value.local_id,
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
                    callee: callee_value.local_id,
                    args: lowered_args,
                    error_type: signature.error_type,
                },
            )?;
            Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                "invoke expression with void callee cannot be used as a value",
            ))
        }
    }
}

fn binary_op_result_type(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    op: LoweredBinaryOp,
    left_type: LoweredTypeId,
) -> Option<LoweredTypeId> {
    match op {
        LoweredBinaryOp::Add
        | LoweredBinaryOp::Sub
        | LoweredBinaryOp::Mul
        | LoweredBinaryOp::Div
        | LoweredBinaryOp::Mod
        | LoweredBinaryOp::Pow => Some(left_type),
        LoweredBinaryOp::Eq
        | LoweredBinaryOp::Ne
        | LoweredBinaryOp::Lt
        | LoweredBinaryOp::Le
        | LoweredBinaryOp::Gt
        | LoweredBinaryOp::Ge
        | LoweredBinaryOp::And
        | LoweredBinaryOp::Or
        | LoweredBinaryOp::Xor => {
            checked_type_map.get(&typed_package.program.builtin_types().bool_).copied()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_binary_op(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    op: LoweredBinaryOp,
    left: &AstNode,
    right: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let left_val = lower_expression(
        typed_package, type_table, checked_type_map, current_identity,
        decl_index, cursor, source_unit_id, scope_id, left,
    )?;
    let right_val = lower_expression(
        typed_package, type_table, checked_type_map, current_identity,
        decl_index, cursor, source_unit_id, scope_id, right,
    )?;
    let result_type = binary_op_result_type(typed_package, checked_type_map, op, left_val.type_id)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "binary operator result type could not be resolved in the lowered type table",
            )
        })?;
    let result_local = cursor.allocate_local(result_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::BinaryOp {
            op,
            left: left_val.local_id,
            right: right_val.local_id,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: result_type,
        recoverable_error_type: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_unary_op(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    op: LoweredUnaryOp,
    operand: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    let operand_val = lower_expression(
        typed_package, type_table, checked_type_map, current_identity,
        decl_index, cursor, source_unit_id, scope_id, operand,
    )?;
    let result_type = match op {
        LoweredUnaryOp::Neg => operand_val.type_id,
        LoweredUnaryOp::Not => {
            checked_type_map
                .get(&typed_package.program.builtin_types().bool_)
                .copied()
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "boolean result type could not be resolved in the lowered type table",
                    )
                })?
        }
    };
    let result_local = cursor.allocate_local(result_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::UnaryOp {
            op,
            operand: operand_val.local_id,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: result_type,
        recoverable_error_type: None,
    })
}
