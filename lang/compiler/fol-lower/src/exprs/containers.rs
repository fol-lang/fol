use super::cursor::{LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_expected;
use crate::{
    control::{LoweredInstrKind, LoweredLinearKind},
    ids::LoweredTypeId,
    LoweringError, LoweringErrorKind,
};
use fol_parser::ast::{AstNode, ContainerType, Literal};
use fol_resolver::{PackageIdentity, ScopeId, SourceUnitId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn lower_record_initializer(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    fields: &[fol_parser::ast::RecordInitField],
) -> Result<LoweredValue, LoweringError> {
    let Some(type_id) = expected_type else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "record initializer lowering requires an expected record type",
        ));
    };
    let (construction_type, expected_fields) = match type_table.get(type_id) {
        Some(crate::LoweredType::Record { fields }) => (type_id, fields.clone()),
        Some(crate::LoweredType::Named {
            package, symbol, ..
        }) => {
            let runtime_type = decl_index
                .record_runtime_type(package, *symbol)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        "named record initializer lost its lowered declaration",
                    )
                })?;
            (runtime_type, decl_index.record_fields(package, *symbol))
        }
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "record initializer does not map to a lowered record runtime type",
            ));
        }
    };
    let mut lowered_fields = Vec::with_capacity(fields.len());
    for field in fields {
        let Some(field_type) = expected_fields.get(&field.name).copied() else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "record initializer field '{}' does not map to a lowered record layout",
                    field.name
                ),
            ));
        };
        let lowered_value = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            Some(field_type),
            &field.value,
        )?;
        lowered_fields.push((field.name.clone(), lowered_value.local_id));
    }

    // Fields omitted from a named initializer are filled from their declared
    // defaults, so the constructed record always carries a complete field set.
    let provided: BTreeSet<&str> = fields.iter().map(|field| field.name.as_str()).collect();
    if let Some(layout) = checked_record_layout(typed_package, checked_type_map, construction_type)
    {
        for field in layout {
            if provided.contains(field.name.as_str()) {
                continue;
            }
            let Some(default_expr) = field.default.clone() else {
                continue;
            };
            let Some(field_type) = expected_fields.get(&field.name).copied() else {
                continue;
            };
            let lowered_value = lower_expression_expected(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                Some(field_type),
                &default_expr,
            )?;
            lowered_fields.push((field.name.clone(), lowered_value.local_id));
        }
    }

    let result_local = cursor.allocate_local(type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ConstructRecord {
            type_id: construction_type,
            fields: lowered_fields,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

/// Recover the ordered record field layout (declaration order plus defaults)
/// for a lowered record type by reversing the checked-to-lowered type map.
fn checked_record_layout<'a>(
    typed_package: &'a fol_typecheck::TypedPackage,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    lowered_type_id: LoweredTypeId,
) -> Option<&'a [fol_typecheck::RecordFieldLayout]> {
    checked_type_map
        .iter()
        .filter(|(_, lowered_id)| **lowered_id == lowered_type_id)
        .find_map(|(checked_id, _)| typed_package.program.record_layout(*checked_id))
}

/// Lower positional (ordered) record initialization `{ v0, v1, ... }`. Values
/// bind to fields in declaration order; fields uncovered by a positional value
/// are filled from their declared defaults. Typecheck has already validated
/// arity and default coverage.
#[allow(clippy::too_many_arguments)]
fn lower_positional_record_literal(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    type_id: LoweredTypeId,
    elements: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let Some(crate::LoweredType::Record {
        fields: expected_fields,
    }) = type_table.get(type_id)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "positional record initializer does not map to a lowered record runtime type",
        ));
    };
    let expected_fields = expected_fields.clone();
    let Some(layout) = checked_record_layout(typed_package, checked_type_map, type_id) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "positional record initialization requires a named record type",
        ));
    };
    let layout = layout.to_vec();

    let element_nodes = container_elements(elements);
    let mut lowered_fields = Vec::with_capacity(layout.len());
    for (index, field) in layout.iter().enumerate() {
        let Some(field_type) = expected_fields.get(&field.name).copied() else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "record field '{}' does not map to a lowered record layout",
                    field.name
                ),
            ));
        };
        let value_node = if let Some(node) = element_nodes.get(index) {
            *node
        } else if let Some(default_expr) = &field.default {
            default_expr
        } else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "positional record initializer is missing a value for field '{}'",
                    field.name
                ),
            ));
        };
        let lowered_value = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            Some(field_type),
            value_node,
        )?;
        lowered_fields.push((field.name.clone(), lowered_value.local_id));
    }

    let result_local = cursor.allocate_local(type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ConstructRecord {
            type_id,
            fields: lowered_fields,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

pub(crate) fn lower_nil_literal(
    type_table: &crate::LoweredTypeTable,
    cursor: &mut RoutineCursor<'_>,
    expected_type: Option<LoweredTypeId>,
) -> Result<LoweredValue, LoweringError> {
    let Some(type_id) = expected_type else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "nil lowering requires an expected opt[...] or err[...] runtime type",
        ));
    };
    let result_local = cursor.allocate_local(type_id, None);
    match type_table.get(type_id) {
        Some(crate::LoweredType::Optional { .. }) => {
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructOptional {
                    type_id,
                    value: None,
                },
            )?;
        }
        Some(crate::LoweredType::Error { .. }) => {
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructError {
                    type_id,
                    value: None,
                },
            )?;
        }
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                "nil lowering requires an expected opt[...] or err[...] runtime type",
            ))
        }
    }
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

pub(crate) fn apply_expected_shell_wrap(
    type_table: &crate::LoweredTypeTable,
    cursor: &mut RoutineCursor<'_>,
    expected_type: Option<LoweredTypeId>,
    value: LoweredValue,
) -> Result<LoweredValue, LoweringError> {
    let Some(expected_type) = expected_type else {
        return Ok(value);
    };
    if expected_type == value.type_id {
        return Ok(value);
    }
    if matches!(
        type_table.get(value.type_id),
        Some(crate::LoweredType::Owned { inner }) if *inner == expected_type
    ) {
        let result_local = cursor.allocate_local(expected_type, None);
        cursor.push_instr(
            Some(result_local),
            LoweredInstrKind::ConsumeOwned {
                value: value.local_id,
            },
        )?;
        return Ok(LoweredValue {
            local_id: result_local,
            type_id: expected_type,
            recoverable_error_type: None,
        });
    }
    if matches!(
        type_table.get(value.type_id),
        Some(crate::LoweredType::Borrowed { inner, .. }) if *inner == expected_type
    ) {
        let result_local = cursor.allocate_local(expected_type, None);
        cursor.push_instr(
            Some(result_local),
            LoweredInstrKind::ReadBorrow {
                borrow: value.local_id,
            },
        )?;
        return Ok(LoweredValue {
            local_id: result_local,
            type_id: expected_type,
            recoverable_error_type: None,
        });
    }
    match type_table.get(expected_type) {
        Some(crate::LoweredType::Owned { inner }) if *inner == value.type_id => {
            let result_local = cursor.allocate_local(expected_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructOwned {
                    type_id: expected_type,
                    value: value.local_id,
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: expected_type,
                recoverable_error_type: None,
            })
        }
        Some(crate::LoweredType::Borrowed { inner, mutable }) if *inner == value.type_id => {
            let result_local = cursor.allocate_local(expected_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructBorrow {
                    type_id: expected_type,
                    owner: value.local_id,
                    mutable: *mutable,
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: expected_type,
                recoverable_error_type: None,
            })
        }
        Some(crate::LoweredType::Optional { inner }) if *inner == value.type_id => {
            let result_local = cursor.allocate_local(expected_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructOptional {
                    type_id: expected_type,
                    value: Some(value.local_id),
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: expected_type,
                recoverable_error_type: None,
            })
        }
        Some(crate::LoweredType::Error { inner: Some(inner) }) if *inner == value.type_id => {
            let result_local = cursor.allocate_local(expected_type, None);
            cursor.push_instr(
                Some(result_local),
                LoweredInstrKind::ConstructError {
                    type_id: expected_type,
                    value: Some(value.local_id),
                },
            )?;
            Ok(LoweredValue {
                local_id: result_local,
                type_id: expected_type,
                recoverable_error_type: None,
            })
        }
        _ => Ok(value),
    }
}

pub(crate) fn lower_container_literal(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    container_type: ContainerType,
    expected_type: Option<LoweredTypeId>,
    elements: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    // A brace literal with an expected record type is positional record
    // initialization; values bind to fields in declaration order.
    if let Some(type_id) = expected_type {
        if matches!(
            type_table.get(type_id),
            Some(crate::LoweredType::Record { .. })
        ) {
            return lower_positional_record_literal(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                type_id,
                elements,
            );
        }
    }

    let container_kind =
        expected_container_kind(type_table, expected_type).unwrap_or(container_type);
    match container_kind {
        ContainerType::Array | ContainerType::Vector | ContainerType::Sequence => {
            lower_linear_container_literal(
                typed_package,
                type_table,
                checked_type_map,
                current_identity,
                decl_index,
                cursor,
                source_unit_id,
                scope_id,
                container_kind,
                expected_type,
                elements,
            )
        }
        ContainerType::Set => lower_set_literal(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_type,
            elements,
        ),
        ContainerType::Map => lower_map_literal(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_type,
            elements,
        ),
    }
}

fn lower_linear_container_literal(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    kind: ContainerType,
    expected_type: Option<LoweredTypeId>,
    elements: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let element_nodes = container_elements(elements);
    let mut lowered_elements = Vec::with_capacity(element_nodes.len());
    let mut element_type = expected_linear_element_type(type_table, expected_type, kind.clone());

    for element in element_nodes {
        let lowered = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            element_type,
            element,
        )?;
        element_type.get_or_insert(lowered.type_id);
        lowered_elements.push(lowered.local_id);
    }

    let Some(type_id) = resolve_linear_container_type(
        type_table,
        kind.clone(),
        expected_type,
        element_type,
        lowered_elements.len(),
    )?
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "empty linear container literals require an expected container type",
        ));
    };

    let result_local = cursor.allocate_local(type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ConstructLinear {
            kind: lowered_linear_kind(kind)?,
            type_id,
            elements: lowered_elements,
        },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

fn lower_set_literal(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    elements: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let element_nodes = container_elements(elements);
    let expected_members = expected_set_member_types(type_table, expected_type);

    if element_nodes.is_empty() {
        let Some(type_id) = expected_type else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                "empty set literals require an expected set type in lowered V1",
            ));
        };
        let result_local = cursor.allocate_local(type_id, None);
        cursor.push_instr(
            Some(result_local),
            LoweredInstrKind::ConstructSet {
                type_id,
                members: Vec::new(),
            },
        )?;
        return Ok(LoweredValue {
            local_id: result_local,
            type_id,
            recoverable_error_type: None,
        });
    }

    let mut member_types = Vec::with_capacity(element_nodes.len());
    let mut members = Vec::with_capacity(element_nodes.len());
    for (index, element) in element_nodes.iter().enumerate() {
        let expected_member = expected_members
            .as_ref()
            .and_then(|member_types| member_types.get(index))
            .copied();
        let lowered = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_member,
            element,
        )?;
        member_types.push(expected_member.unwrap_or(lowered.type_id));
        members.push(lowered.local_id);
    }

    let type_id = match expected_type {
        Some(t) => t,
        None => find_set_type(type_table, &member_types)?,
    };
    let result_local = cursor.allocate_local(type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ConstructSet { type_id, members },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

fn lower_map_literal(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    expected_type: Option<LoweredTypeId>,
    elements: &[AstNode],
) -> Result<LoweredValue, LoweringError> {
    let element_nodes = container_elements(elements);
    let mut expected_key = expected_map_key_type(type_table, expected_type);
    let mut expected_value = expected_map_value_type(type_table, expected_type);

    if element_nodes.is_empty() {
        let Some(type_id) = expected_type else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                "empty map literals require an expected map type in lowered V1",
            ));
        };
        let result_local = cursor.allocate_local(type_id, None);
        cursor.push_instr(
            Some(result_local),
            LoweredInstrKind::ConstructMap {
                type_id,
                entries: Vec::new(),
            },
        )?;
        return Ok(LoweredValue {
            local_id: result_local,
            type_id,
            recoverable_error_type: None,
        });
    }

    let mut entries = Vec::with_capacity(element_nodes.len());
    for pair in element_nodes {
        let (key, value) = map_literal_pair(pair)?;
        let lowered_key = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_key,
            key,
        )?;
        expected_key.get_or_insert(lowered_key.type_id);

        let lowered_value = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            expected_value,
            value,
        )?;
        expected_value.get_or_insert(lowered_value.type_id);
        entries.push((lowered_key.local_id, lowered_value.local_id));
    }

    let Some(type_id) = resolve_map_type(type_table, expected_type, expected_key, expected_value)?
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "map literal could not determine a lowered key/value type",
        ));
    };

    let result_local = cursor.allocate_local(type_id, None);
    cursor.push_instr(
        Some(result_local),
        LoweredInstrKind::ConstructMap { type_id, entries },
    )?;
    Ok(LoweredValue {
        local_id: result_local,
        type_id,
        recoverable_error_type: None,
    })
}

pub(crate) fn container_elements(elements: &[AstNode]) -> Vec<&AstNode> {
    elements
        .iter()
        .filter(|element| !matches!(element, AstNode::Comment { .. }))
        .collect()
}

pub(crate) fn map_literal_pair(pair: &AstNode) -> Result<(&AstNode, &AstNode), LoweringError> {
    match pair {
        AstNode::ContainerLiteral { elements, .. } => {
            let pair_items = container_elements(elements);
            if let [key, value] = pair_items.as_slice() {
                Ok((*key, *value))
            } else {
                Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    "map literals require each element to be a two-value pair",
                ))
            }
        }
        AstNode::Commented { node, .. } => map_literal_pair(node),
        _ => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "map literals require each element to be a two-value pair",
        )),
    }
}

pub(crate) fn literal_index_value(node: &AstNode) -> Option<usize> {
    match node {
        AstNode::Literal(Literal::Integer(value)) => usize::try_from(*value).ok(),
        AstNode::Commented { node, .. } => literal_index_value(node),
        _ => None,
    }
}

pub(crate) fn field_access_type(
    type_table: &crate::LoweredTypeTable,
    decl_index: &WorkspaceDeclIndex,
    object_type: LoweredTypeId,
    field: &str,
) -> Option<LoweredTypeId> {
    match type_table.get(object_type) {
        Some(crate::LoweredType::Record { fields }) => fields.get(field).copied(),
        Some(crate::LoweredType::Entry { variants }) => variants.get(field).copied().flatten(),
        Some(crate::LoweredType::Owned { inner })
        | Some(crate::LoweredType::Borrowed { inner, .. }) => {
            field_access_type(type_table, decl_index, *inner, field)
        }
        Some(crate::LoweredType::Named {
            package, symbol, ..
        }) => decl_index.record_field(package, *symbol, field),
        _ => None,
    }
}

pub(crate) fn slice_access_type(
    type_table: &crate::LoweredTypeTable,
    container_type: LoweredTypeId,
) -> Option<LoweredTypeId> {
    match type_table.get(container_type) {
        Some(crate::LoweredType::Vector { .. }) | Some(crate::LoweredType::Sequence { .. }) => {
            Some(container_type)
        }
        Some(crate::LoweredType::Owned { inner })
        | Some(crate::LoweredType::Borrowed { inner, .. }) => slice_access_type(type_table, *inner),
        _ => None,
    }
}

pub(crate) fn index_access_type(
    type_table: &crate::LoweredTypeTable,
    container_type: LoweredTypeId,
    index: &AstNode,
) -> Option<LoweredTypeId> {
    match type_table.get(container_type) {
        Some(crate::LoweredType::Array { element_type, .. })
        | Some(crate::LoweredType::Vector { element_type })
        | Some(crate::LoweredType::Sequence { element_type }) => Some(*element_type),
        Some(crate::LoweredType::Map { value_type, .. }) => Some(*value_type),
        Some(crate::LoweredType::Set { member_types }) => {
            let index_value = literal_index_value(index)?;
            member_types.get(index_value).copied()
        }
        Some(crate::LoweredType::Owned { inner })
        | Some(crate::LoweredType::Borrowed { inner, .. }) => {
            index_access_type(type_table, *inner, index)
        }
        _ => None,
    }
}

/// Expected type of an index expression for the given container: map keys
/// carry the declared key type; positional containers index by int.
pub(crate) fn index_key_type(
    type_table: &crate::LoweredTypeTable,
    container_type: LoweredTypeId,
) -> Option<LoweredTypeId> {
    match type_table.get(container_type) {
        Some(crate::LoweredType::Map { key_type, .. }) => Some(*key_type),
        Some(crate::LoweredType::Owned { inner })
        | Some(crate::LoweredType::Borrowed { inner, .. }) => index_key_type(type_table, *inner),
        _ => None,
    }
}

fn expected_linear_element_type(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
    kind: ContainerType,
) -> Option<LoweredTypeId> {
    match (
        expected_type.and_then(|type_id| type_table.get(type_id)),
        kind,
    ) {
        (Some(crate::LoweredType::Array { element_type, .. }), ContainerType::Array)
        | (Some(crate::LoweredType::Vector { element_type }), ContainerType::Vector)
        | (Some(crate::LoweredType::Sequence { element_type }), ContainerType::Sequence) => {
            Some(*element_type)
        }
        _ => None,
    }
}

pub(crate) fn expected_container_kind(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
) -> Option<ContainerType> {
    match expected_type.and_then(|type_id| type_table.get(type_id)) {
        Some(crate::LoweredType::Array { .. }) => Some(ContainerType::Array),
        Some(crate::LoweredType::Vector { .. }) => Some(ContainerType::Vector),
        Some(crate::LoweredType::Sequence { .. }) => Some(ContainerType::Sequence),
        Some(crate::LoweredType::Set { .. }) => Some(ContainerType::Set),
        Some(crate::LoweredType::Map { .. }) => Some(ContainerType::Map),
        _ => None,
    }
}

fn resolve_linear_container_type(
    type_table: &crate::LoweredTypeTable,
    kind: ContainerType,
    expected_type: Option<LoweredTypeId>,
    element_type: Option<LoweredTypeId>,
    len: usize,
) -> Result<Option<LoweredTypeId>, LoweringError> {
    if let Some(type_id) = expected_type {
        return Ok(match (type_table.get(type_id), kind) {
            (Some(crate::LoweredType::Array { .. }), ContainerType::Array)
            | (Some(crate::LoweredType::Vector { .. }), ContainerType::Vector)
            | (Some(crate::LoweredType::Sequence { .. }), ContainerType::Sequence) => Some(type_id),
            _ => None,
        });
    }

    let Some(element_type) = element_type else {
        return Ok(None);
    };
    Ok(Some(match kind {
        ContainerType::Array => find_array_type(type_table, element_type, Some(len))?,
        ContainerType::Vector => find_vector_type(type_table, element_type)?,
        ContainerType::Sequence => find_sequence_type(type_table, element_type)?,
        ContainerType::Set | ContainerType::Map => return Ok(None),
    }))
}

fn expected_set_member_types(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
) -> Option<Vec<LoweredTypeId>> {
    match expected_type.and_then(|type_id| type_table.get(type_id)) {
        Some(crate::LoweredType::Set { member_types }) => Some(member_types.clone()),
        _ => None,
    }
}

fn expected_map_key_type(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
) -> Option<LoweredTypeId> {
    match expected_type.and_then(|type_id| type_table.get(type_id)) {
        Some(crate::LoweredType::Map { key_type, .. }) => Some(*key_type),
        _ => None,
    }
}

fn expected_map_value_type(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
) -> Option<LoweredTypeId> {
    match expected_type.and_then(|type_id| type_table.get(type_id)) {
        Some(crate::LoweredType::Map { value_type, .. }) => Some(*value_type),
        _ => None,
    }
}

fn resolve_map_type(
    type_table: &crate::LoweredTypeTable,
    expected_type: Option<LoweredTypeId>,
    key_type: Option<LoweredTypeId>,
    value_type: Option<LoweredTypeId>,
) -> Result<Option<LoweredTypeId>, LoweringError> {
    if let Some(type_id) = expected_type {
        return Ok(matches!(
            type_table.get(type_id),
            Some(crate::LoweredType::Map { .. })
        )
        .then_some(type_id));
    }
    let (Some(key), Some(value)) = (key_type, value_type) else {
        return Ok(None);
    };
    Ok(Some(find_map_type(type_table, key, value)?))
}

fn lowered_linear_kind(kind: ContainerType) -> Result<LoweredLinearKind, LoweringError> {
    match kind {
        ContainerType::Array => Ok(LoweredLinearKind::Array),
        ContainerType::Vector => Ok(LoweredLinearKind::Vector),
        ContainerType::Sequence => Ok(LoweredLinearKind::Sequence),
        ContainerType::Set | ContainerType::Map => Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "set/map container kinds do not lower through linear container instructions",
        )),
    }
}

fn find_array_type(
    type_table: &crate::LoweredTypeTable,
    element_type: LoweredTypeId,
    size: Option<usize>,
) -> Result<LoweredTypeId, LoweringError> {
    (0..type_table.len())
        .map(crate::LoweredTypeId)
        .find(|type_id| {
            matches!(
                type_table.get(*type_id),
                Some(crate::LoweredType::Array {
                    element_type: actual_element,
                    size: actual_size,
                }) if *actual_element == element_type && *actual_size == size
            )
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::Internal,
                format!(
                    "lowered type table lost array shape for element {}",
                    element_type.0
                ),
            )
        })
}

fn find_vector_type(
    type_table: &crate::LoweredTypeTable,
    element_type: LoweredTypeId,
) -> Result<LoweredTypeId, LoweringError> {
    (0..type_table.len())
        .map(crate::LoweredTypeId)
        .find(|type_id| {
            matches!(
                type_table.get(*type_id),
                Some(crate::LoweredType::Vector {
                    element_type: actual_element,
                }) if *actual_element == element_type
            )
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::Internal,
                format!(
                    "lowered type table lost vector shape for element {}",
                    element_type.0
                ),
            )
        })
}

fn find_sequence_type(
    type_table: &crate::LoweredTypeTable,
    element_type: LoweredTypeId,
) -> Result<LoweredTypeId, LoweringError> {
    (0..type_table.len())
        .map(crate::LoweredTypeId)
        .find(|type_id| {
            matches!(
                type_table.get(*type_id),
                Some(crate::LoweredType::Sequence {
                    element_type: actual_element,
                }) if *actual_element == element_type
            )
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::Internal,
                format!(
                    "lowered type table lost sequence shape for element {}",
                    element_type.0
                ),
            )
        })
}

fn find_set_type(
    type_table: &crate::LoweredTypeTable,
    member_types: &[LoweredTypeId],
) -> Result<LoweredTypeId, LoweringError> {
    (0..type_table.len())
        .map(crate::LoweredTypeId)
        .find(|type_id| {
            matches!(
                type_table.get(*type_id),
                Some(crate::LoweredType::Set {
                    member_types: actual_members,
                }) if actual_members == member_types
            )
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::Internal,
                "lowered type table lost set shape",
            )
        })
}

fn find_map_type(
    type_table: &crate::LoweredTypeTable,
    key_type: LoweredTypeId,
    value_type: LoweredTypeId,
) -> Result<LoweredTypeId, LoweringError> {
    (0..type_table.len())
        .map(crate::LoweredTypeId)
        .find(|type_id| {
            matches!(
                type_table.get(*type_id),
                Some(crate::LoweredType::Map {
                    key_type: actual_key,
                    value_type: actual_value,
                }) if *actual_key == key_type && *actual_value == value_type
            )
        })
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::Internal,
                "lowered type table lost map shape",
            )
        })
}
