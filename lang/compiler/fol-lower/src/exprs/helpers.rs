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
        fol_parser::ast::BinaryOperator::Pipe => "pipe",
        fol_parser::ast::BinaryOperator::PipeOr => "pipe_or",
    }
}

/// The entry-declaring symbol an identifier names when a value binding shadows
/// it. Matches only when that entry actually exposes `field`, so an ordinary
/// field read on an instance is never redirected.
fn shadowed_entry_symbol(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    canonical_name: &str,
    field: &str,
) -> Option<fol_resolver::ResolvedSymbol> {
    typed_package
        .program
        .resolved()
        .symbols
        .iter()
        .find(|symbol| {
            matches!(
                symbol.kind,
                fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias
            ) && symbol.canonical_name == canonical_name
                && typed_package
                    .program
                    .typed_symbol(symbol.id)
                    .and_then(|typed| typed.declared_type)
                    .and_then(|declared| checked_type_map.get(&declared).copied())
                    .is_some_and(|lowered| entry_declares_variant(type_table, lowered, field))
        })
        .cloned()
}

/// Whether the lowered entry type carries a variant of this name.
fn entry_declares_variant(
    type_table: &crate::LoweredTypeTable,
    lowered_type: LoweredTypeId,
    field: &str,
) -> bool {
    matches!(
        type_table.get(lowered_type),
        Some(crate::LoweredType::Entry { variants }) if variants.contains_key(field)
    )
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

    // Name lookup folds case and underscores, so a `var node` binding can win
    // over a `typ Node` and the reference resolves to the value. Typecheck
    // falls back to the type when the value has no such field; lowering has to
    // agree or the read reaches here as a record-field access that does not
    // exist.
    let owned;
    // Substituting the type also discards the reference's recorded type: that
    // belongs to the shadowing VALUE, and using it would look up the variant on
    // an `int`.
    let (resolved_symbol, checked_type) = if matches!(
        resolved_symbol.kind,
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias
    ) {
        (resolved_symbol, checked_type)
    } else {
        let Some(shadowed) = shadowed_entry_symbol(
            typed_package,
            type_table,
            checked_type_map,
            &resolved_symbol.canonical_name,
            field,
        ) else {
            return Ok(None);
        };
        owned = shadowed;
        (&owned, None)
    };
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
        // A guard binding aliases the mutex local, so this store cannot be told
        // apart from initialising the mutex by its shape -- only the symbol
        // knows. Writing through the held guard is what assigning to a guard
        // means; re-taking the lock here would deadlock against itself.
        let through_guard = typed_package
            .program
            .typed_symbol(resolved_symbol.id)
            .is_some_and(|symbol| symbol.is_mutex_guard);
        let kind = if through_guard {
            LoweredInstrKind::StoreMutexValue {
                mutex: local_id,
                value: lowered_value.local_id,
            }
        } else {
            LoweredInstrKind::StoreLocal {
                local: local_id,
                value: lowered_value.local_id,
            }
        };
        cursor.push_instr(None, kind)?;
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

/// What a method call turned out to be, so both lowering copies can dispatch on
/// one result.
pub(crate) enum ContainerCallOutcome {
    /// Not a container method — ordinary method lowering should proceed.
    NotContainer,
    /// Lowered, and yields no value.
    Statement,
    /// Lowered, and yields the displaced element.
    Value(LoweredValue),
}

/// The receiver's family decides which op a method name means, so `clear` is
/// `VecClear` on a vector and `MapClear` on a map. Typecheck has already refused
/// a name the family does not own.
fn container_mutate_op(
    method: &str,
    container: &crate::types::LoweredType,
) -> Option<crate::control::ContainerMutateOp> {
    use crate::control::ContainerMutateOp;
    use crate::types::LoweredType;
    match container {
        LoweredType::Vector { .. } => match method {
            "push" => Some(ContainerMutateOp::VecPush),
            "pop" => Some(ContainerMutateOp::VecPop),
            "insert_at" => Some(ContainerMutateOp::VecInsertAt),
            "remove_at" => Some(ContainerMutateOp::VecRemoveAt),
            "clear" => Some(ContainerMutateOp::VecClear),
            "truncate" => Some(ContainerMutateOp::VecTruncate),
            "sort" => Some(ContainerMutateOp::VecSort),
            "swap" => Some(ContainerMutateOp::VecSwap),
            "reserve" => Some(ContainerMutateOp::VecReserve),
            _ => None,
        },
        LoweredType::Map { .. } => match method {
            "insert" => Some(ContainerMutateOp::MapInsert),
            "get" => Some(ContainerMutateOp::MapGet),
            "remove" => Some(ContainerMutateOp::MapRemove),
            "contains" => Some(ContainerMutateOp::MapContains),
            "clear" => Some(ContainerMutateOp::MapClear),
            "keys" => Some(ContainerMutateOp::MapKeys),
            "values" => Some(ContainerMutateOp::MapValues),
            _ => None,
        },
        _ => None,
    }
}

/// Every method name either container family owns, used as a cheap gate before
/// the receiver is resolved.
fn is_container_method_name(method: &str) -> bool {
    matches!(
        method,
        "push"
            | "pop"
            | "insert_at"
            | "remove_at"
            | "clear"
            | "truncate"
            | "sort"
            | "swap"
            | "reserve"
            | "insert"
            | "get"
            | "remove"
            | "contains"
            | "keys"
            | "values"
    )
}

/// Lower a growable-container method (`values.push(x)`) into a
/// `ContainerMutate`.
///
/// Called from BOTH copies of the method-call lowering — expression position and
/// statement position. Wiring only one has silently done nothing before.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_container_method_call(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    object: &AstNode,
    method: &str,
    args: &[AstNode],
) -> Result<ContainerCallOutcome, LoweringError> {
    if !is_container_method_name(method) {
        return Ok(ContainerCallOutcome::NotContainer);
    }
    // The binding's OWN local, not a loaded copy -- mutating a copy would be
    // observed by nobody. The indexed-store path takes the same shortcut.
    let (base_node, field) = match object {
        AstNode::FieldAccess { object, field } => (object.as_ref(), Some(field.clone())),
        other => (other, None),
    };
    let Some(base) =
        super::expressions::direct_local_identifier_value(typed_package, cursor, base_node)
    else {
        return Ok(ContainerCallOutcome::NotContainer);
    };
    let container_type = match &field {
        None => base.type_id,
        Some(field) => {
            match super::containers::field_access_type(type_table, decl_index, base.type_id, field)
            {
                Some(type_id) => type_id,
                None => return Ok(ContainerCallOutcome::NotContainer),
            }
        }
    };
    let Some(container) = type_table.get(container_type) else {
        return Ok(ContainerCallOutcome::NotContainer);
    };
    let Some(op) = container_mutate_op(method, container) else {
        return Ok(ContainerCallOutcome::NotContainer);
    };
    // Element for a vector; key and value for a map. The int is the index type
    // the positional vector operations take.
    let (element_type, key_type) = match container {
        crate::types::LoweredType::Vector { element_type } => (*element_type, None),
        crate::types::LoweredType::Map {
            key_type,
            value_type,
        } => (*value_type, Some(*key_type)),
        _ => return Ok(ContainerCallOutcome::NotContainer),
    };
    if args.len() != op.arity() {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "'.{method}' expects exactly {} argument(s), got {}",
                op.arity(),
                args.len()
            ),
        ));
    }
    // An index argument lowers against `int`; an element argument lowers against
    // the ELEMENT type, so a bare literal picks up the right one -- a `vec[flt]`
    // taking `1` would otherwise lower as an int.
    let int_type = typed_package.program.builtin_types().int;
    let int_type = checked_type_map.get(&int_type).copied();
    let mut lowered_args = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        use crate::control::ContainerMutateOp;
        let expected = match (op, index) {
            (ContainerMutateOp::VecInsertAt, 0)
            | (ContainerMutateOp::VecRemoveAt, _)
            | (ContainerMutateOp::VecTruncate, _)
            | (ContainerMutateOp::VecSwap, _)
            | (ContainerMutateOp::VecReserve, _) => int_type,
            (
                ContainerMutateOp::MapInsert
                | ContainerMutateOp::MapGet
                | ContainerMutateOp::MapRemove
                | ContainerMutateOp::MapContains,
                0,
            ) => key_type,
            _ => Some(element_type),
        };
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
    if !op.yields_value() {
        cursor.push_instr(
            None,
            LoweredInstrKind::ContainerMutate {
                base: base.local_id,
                field,
                op,
                args: lowered_args,
            },
        )?;
        return Ok(ContainerCallOutcome::Statement);
    }
    // The `opt[T]` result type is whatever typecheck recorded for this call site.
    let result_type = syntax_id
        .and_then(|syntax_id| typed_package.program.typed_node(syntax_id))
        .and_then(|node| node.inferred_type)
        .and_then(|checked| checked_type_map.get(&checked).copied())
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("'.{method}' did not retain a lowered result type"),
            )
        })?;
    let result_local = cursor.allocate_local(result_type, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ContainerMutate {
            base: base.local_id,
            field,
            op,
            args: lowered_args,
        },
    )?;
    Ok(ContainerCallOutcome::Value(LoweredValue {
        local_id: result_local,
        type_id: result_type,
        recoverable_error_type: None,
    }))
}

/// Lower `<binding>.<field> = <value>` into a `StoreField` against the binding's
/// own local. Typecheck has already verified the binding is a mutable record.
/// Lower `container[index] = value` into a `StoreIndex`.
///
/// Kept apart from the identifier/field path because it needs two things that
/// path cannot give: the index has to be lowered as its own operand, and the
/// value has to be lowered against the ELEMENT type so a bare literal picks up
/// the right one (a `vec[flt]` taking `1` would otherwise lower as an int).
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_index_assignment_target(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    container: &AstNode,
    index: &AstNode,
    value: &AstNode,
) -> Result<LoweredValue, LoweringError> {
    // The binding's OWN local, not a loaded copy -- a store into a copy would
    // be observed by nobody. The read path takes the same shortcut.
    let (base_node, field) = match container {
        AstNode::FieldAccess { object, field } => (object.as_ref(), Some(field.clone())),
        other => (other, None),
    };
    let Some(base) =
        super::expressions::direct_local_identifier_value(typed_package, cursor, base_node)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "indexed assignment requires a local container binding",
        ));
    };
    // Through a field, the container is the field's type, not the record's.
    let container_type = match &field {
        None => base.type_id,
        Some(field) => {
            super::containers::field_access_type(type_table, decl_index, base.type_id, field)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!("indexed assignment names unknown field '{field}'"),
                    )
                })?
        }
    };
    let element_type = super::containers::index_access_type(type_table, container_type, index)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "indexed assignment target does not resolve to a container element type",
            )
        })?;
    let index_expected = super::containers::index_key_type(type_table, container_type);
    let index_value = lower_expression_expected(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        index_expected,
        index,
    )?;
    let lowered_value = lower_expression_expected(
        typed_package,
        type_table,
        checked_type_map,
        current_identity,
        decl_index,
        cursor,
        source_unit_id,
        scope_id,
        Some(element_type),
        value,
    )?;
    cursor.push_instr(
        None,
        LoweredInstrKind::StoreIndex {
            base: base.local_id,
            field,
            index: index_value.local_id,
            value: lowered_value.local_id,
        },
    )?;
    Ok(lowered_value)
}

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
