use super::calls::{resolve_reference_symbol, resolve_reference_type_id};
use super::cursor::{canonical_symbol_key, LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_expected;
use crate::{control::LoweredInstrKind, ids::LoweredTypeId, LoweringError, LoweringErrorKind};
use fol_parser::ast::{AstNode, Literal};
use fol_resolver::{PackageIdentity, ReferenceKind, ScopeId, SourceUnitId};
use std::collections::BTreeMap;

pub(crate) fn literal_type_id(
    typed_package: &fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    literal: &Literal,
) -> Option<LoweredTypeId> {
    let builtin = match literal {
        Literal::Integer(_) => typed_package.program.builtin_types().int,
        Literal::Float(_) => typed_package.program.builtin_types().float,
        Literal::String(_) => typed_package.program.builtin_types().str_,
        Literal::Character(_) => typed_package.program.builtin_types().char_,
        Literal::Boolean(_) => typed_package.program.builtin_types().bool_,
        Literal::Nil => return None,
    };
    checked_type_map.get(&builtin).copied()
}

pub(crate) fn describe_unary_operator(op: &fol_parser::ast::UnaryOperator) -> &'static str {
    match op {
        fol_parser::ast::UnaryOperator::Neg => "neg",
        fol_parser::ast::UnaryOperator::Not => "not",
        fol_parser::ast::UnaryOperator::Ref => "ref",
        fol_parser::ast::UnaryOperator::Deref => "deref",
        fol_parser::ast::UnaryOperator::BorrowFrom => "borrow-from",
        fol_parser::ast::UnaryOperator::GiveBack => "give-back",
        fol_parser::ast::UnaryOperator::Unwrap => "unwrap",
    }
}

pub(crate) fn describe_binary_operator(op: &fol_parser::ast::BinaryOperator) -> &'static str {
    match op {
        fol_parser::ast::BinaryOperator::Add => "add",
        fol_parser::ast::BinaryOperator::Sub => "sub",
        fol_parser::ast::BinaryOperator::Mul => "mul",
        fol_parser::ast::BinaryOperator::Div => "div",
        fol_parser::ast::BinaryOperator::Mod => "mod",
        fol_parser::ast::BinaryOperator::Pow => "pow",
        fol_parser::ast::BinaryOperator::Eq => "eq",
        fol_parser::ast::BinaryOperator::Ne => "ne",
        fol_parser::ast::BinaryOperator::Lt => "lt",
        fol_parser::ast::BinaryOperator::Le => "le",
        fol_parser::ast::BinaryOperator::Gt => "gt",
        fol_parser::ast::BinaryOperator::Ge => "ge",
        fol_parser::ast::BinaryOperator::And => "and",
        fol_parser::ast::BinaryOperator::Or => "or",
        fol_parser::ast::BinaryOperator::Xor => "xor",
        fol_parser::ast::BinaryOperator::In => "in",
        fol_parser::ast::BinaryOperator::Has => "has",
        fol_parser::ast::BinaryOperator::Is => "is",
        fol_parser::ast::BinaryOperator::As => "as",
        fol_parser::ast::BinaryOperator::Cast => "cast",
        fol_parser::ast::BinaryOperator::Pipe => "pipe",
        fol_parser::ast::BinaryOperator::PipeOr => "pipe_or",
    }
}

pub(crate) fn resolve_entry_variant_target(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    current_identity: &PackageIdentity,
    object: &AstNode,
    field: &str,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
) -> Result<Option<(PackageIdentity, fol_resolver::SymbolId, String)>, LoweringError> {
    let (resolved_symbol, checked_type) = match object {
        AstNode::Identifier { syntax_id, name } => (
            resolve_reference_symbol(typed_package, *syntax_id, ReferenceKind::Identifier, name)?,
            resolve_reference_type_id(
                typed_package,
                checked_type_map,
                *syntax_id,
                ReferenceKind::Identifier,
            ),
        ),
        AstNode::QualifiedIdentifier { path } => (
            resolve_reference_symbol(
                typed_package,
                path.syntax_id(),
                ReferenceKind::QualifiedIdentifier,
                &path.joined(),
            )?,
            resolve_reference_type_id(
                typed_package,
                checked_type_map,
                path.syntax_id(),
                ReferenceKind::QualifiedIdentifier,
            ),
        ),
        AstNode::Commented { node, .. } => {
            return resolve_entry_variant_target(
                typed_package,
                type_table,
                current_identity,
                node,
                field,
                checked_type_map,
            );
        }
        _ => return Ok(None),
    };

    if !matches!(
        resolved_symbol.kind,
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias
    ) {
        return Ok(None);
    }
    let lowered_type = checked_type.or_else(|| {
        let typed_symbol = typed_package.program.typed_symbol(resolved_symbol.id)?;
        let declared_type = typed_symbol.declared_type?;
        checked_type_map.get(&declared_type).copied()
    });
    let Some(lowered_type) = lowered_type else {
        return Ok(None);
    };
    if !matches!(type_table_entry_kind(type_table, lowered_type), Some(())) {
        return Ok(None);
    }

    let (owning_identity, owning_symbol_id) = canonical_symbol_key(
        current_identity,
        resolved_symbol.mounted_from.as_ref(),
        resolved_symbol.id,
    );
    Ok(Some((owning_identity, owning_symbol_id, field.to_string())))
}

fn type_table_entry_kind(
    type_table: &crate::LoweredTypeTable,
    lowered_type: LoweredTypeId,
) -> Option<()> {
    matches!(
        type_table.get(lowered_type),
        Some(crate::LoweredType::Entry { .. })
    )
    .then_some(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_unwrap_expression(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    operand: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    use super::expressions::lower_expression;
    let operand = lower_expression(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        operand,
    )?;
    let inner_type = match type_table.get(operand.type_id) {
        Some(crate::LoweredType::Optional { inner }) => Some(*inner),
        Some(crate::LoweredType::Error { inner }) => *inner,
        _ => None,
    }
    .ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "unwrap lowering requires an opt[...] or typed err[...] runtime operand",
        )
    })?;
    let result_local = cursor.allocate_local(inner_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::UnwrapShell {
            operand: operand.local_id,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id: inner_type,
        recoverable_error_type: None,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_entry_variant_access(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    object: &AstNode,
    field: &str,
    expected_type: Option<LoweredTypeId>,
) -> Result<Option<LoweredValue>, LoweringError> {
    let Some((owning_identity, owning_symbol_id, variant)) = resolve_entry_variant_target(
        typed_package,
        type_table,
        current_identity,
        object,
        field,
        checked_type_map,
    )?
    else {
        return Ok(None);
    };
    let Some(entry_variant) =
        decl_index.entry_variant(&owning_identity, owning_symbol_id, &variant)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("entry variant '{variant}' does not retain lowered variant metadata"),
        ));
    };

    // Mirror the typecheck's entry-variant typing: a bare access denotes a
    // value of the entry type and is constructed as such. It only coerces to
    // its stored payload when an explicit *concrete non-entry* expectation
    // asks for it (e.g. returning `Color.BLUE` as `str`). A generic-parameter
    // expectation (or no expectation) leaves the value as the entry, matching
    // how argument-driven generic inference bound the parameter to the entry
    // type.
    let construct_entry = match expected_type {
        Some(expected) if expected == entry_variant.type_id => true,
        None => true,
        Some(expected) => type_table.contains_generic_parameter(expected),
    };
    if construct_entry {
        let payload = match (&entry_variant.payload_type, &entry_variant.default) {
            (Some(payload_type), Some(default)) => Some(
                lower_expression_expected(
                    typed_package,
                    type_table,
                    checked_type_map,
                    current_identity,
                    decl_index,
                    cursor,
                    source_unit_id,
                    scope_id,
                    Some(*payload_type),
                    default,
                )?
                .local_id,
            ),
            (Some(_), None) => {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::Unsupported,
                    format!(
                        "entry construction for variant '{variant}' requires a lowered default payload expression"
                    ),
                ))
            }
            (None, _) => None,
        };
        let result_local = cursor.allocate_local(entry_variant.type_id, None);
        cursor.push_instr(
            Some(result_local),
            LoweredInstrKind::ConstructEntry {
                type_id: entry_variant.type_id,
                variant,
                payload,
            },
        )?;
        return Ok(Some(LoweredValue {
            local_id: result_local,
            type_id: entry_variant.type_id,
            recoverable_error_type: None,
        }));
    }

    let Some(payload_type) = entry_variant.payload_type else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "entry variant access for '{variant}' requires a payload-bearing variant or an expected entry context"
            ),
        ));
    };
    let Some(default) = entry_variant.default.as_ref() else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "entry variant access for '{variant}' requires a lowered default payload expression"
            ),
        ));
    };
    let payload = lower_expression_expected(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        Some(payload_type),
        default,
    )?;
    Ok(Some(payload))
}

pub(crate) fn lower_assignment_target(
    typed_package: &fol_typecheck::TypedPackage,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    target: &AstNode,
    lowered_value: LoweredValue,
) -> Result<LoweredValue, LoweringError> {
    // Field assignment into a mutable record binding, e.g. `counter.total = 5`.
    if let AstNode::FieldAccess { object, field } = target {
        return lower_field_assignment_target(
            typed_package,
            current_identity,
            decl_index,
            cursor,
            object,
            field,
            lowered_value,
        );
    }
    if let AstNode::UnaryOp {
        op: fol_parser::ast::UnaryOperator::Deref,
        operand,
    } = target
    {
        let AstNode::Identifier { syntax_id, name } = operand.as_ref() else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "dereference assignment requires a pointer binding identifier",
            ));
        };
        let resolved =
            resolve_reference_symbol(typed_package, *syntax_id, ReferenceKind::Identifier, name)?;
        let pointer = cursor
            .routine
            .local_symbols
            .get(&resolved.id)
            .copied()
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "dereference assignment pointer is not a lowered local",
                )
            })?;
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreDeref {
                pointer,
                value: lowered_value.local_id,
            },
        )?;
        return Ok(lowered_value);
    }

    let resolved_symbol = match target {
        AstNode::Identifier { syntax_id, name } => {
            resolve_reference_symbol(typed_package, *syntax_id, ReferenceKind::Identifier, name)?
        }
        AstNode::QualifiedIdentifier { path } => resolve_reference_symbol(
            typed_package,
            path.syntax_id(),
            ReferenceKind::QualifiedIdentifier,
            &path.joined(),
        )?,
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "assignment targets must lower from plain or qualified identifiers, \
                 a field of a mutable record binding, or a unique-pointer dereference",
            ))
        }
    };

    if let Some(local_id) = cursor
        .routine
        .local_symbols
        .get(&resolved_symbol.id)
        .copied()
    {
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: local_id,
                value: lowered_value.local_id,
            },
        )?;
        return Ok(lowered_value);
    }

    let (owning_identity, owning_symbol_id) = canonical_symbol_key(
        current_identity,
        resolved_symbol.mounted_from.as_ref(),
        resolved_symbol.id,
    );
    let Some(global_id) = decl_index.global_id_for_symbol(&owning_identity, owning_symbol_id)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "assignment target '{}' does not map to a lowered global definition",
                resolved_symbol.name
            ),
        ));
    };
    cursor.push_instr(
        None,
        LoweredInstrKind::StoreGlobal {
            global: global_id,
            value: lowered_value.local_id,
        },
    )?;
    Ok(lowered_value)
}

/// Lower `<binding>.<field> = <value>` into a `StoreField` against the binding's
/// own local. Typecheck has already verified the binding is a mutable record.
fn lower_field_assignment_target(
    typed_package: &fol_typecheck::TypedPackage,
    _current_identity: &PackageIdentity,
    _decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    object: &AstNode,
    field: &str,
    lowered_value: LoweredValue,
) -> Result<LoweredValue, LoweringError> {
    let resolved_symbol = match object {
        AstNode::Identifier { syntax_id, name } => {
            resolve_reference_symbol(typed_package, *syntax_id, ReferenceKind::Identifier, name)?
        }
        AstNode::QualifiedIdentifier { path } => resolve_reference_symbol(
            typed_package,
            path.syntax_id(),
            ReferenceKind::QualifiedIdentifier,
            &path.joined(),
        )?,
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "nested field assignment targets are not supported",
            ))
        }
    };

    let Some(local_id) = cursor
        .routine
        .local_symbols
        .get(&resolved_symbol.id)
        .copied()
    else {
        // Only local record bindings support field assignment for now; a global
        // record field store would need a distinct lowered lvalue form.
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "field assignment into non-local binding '{}' is not supported",
                resolved_symbol.name
            ),
        ));
    };

    cursor.push_instr(
        None,
        LoweredInstrKind::StoreField {
            base: local_id,
            field: field.to_string(),
            value: lowered_value.local_id,
        },
    )?;
    Ok(lowered_value)
}
