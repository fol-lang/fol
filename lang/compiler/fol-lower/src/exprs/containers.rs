use super::cursor::{LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_expected;
use crate::{
    control::{LoweredInstrKind, LoweredLinearKind},
    ids::LoweredTypeId,
    LoweringError, LoweringErrorKind,
};
use fol_parser::ast::{AstNode, ContainerType, Literal};
use fol_resolver::{PackageIdentity, ScopeId, SourceUnitId};
use std::collections::BTreeMap;

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
    let Some(crate::LoweredType::Record {
        fields: expected_fields,
    }) = type_table.get(type_id)
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            "record initializer does not map to a lowered record runtime type",
        ));
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
    match type_table.get(expected_type) {
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
    )? else {
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
    object_type: LoweredTypeId,
    field: &str,
) -> Option<LoweredTypeId> {
    match type_table.get(object_type) {
        Some(crate::LoweredType::Record { fields }) => fields.get(field).copied(),
        Some(crate::LoweredType::Entry { variants }) => variants.get(field).copied().flatten(),
        _ => None,
    }
}

pub(crate) fn slice_access_type(
    type_table: &crate::LoweredTypeTable,
    container_type: LoweredTypeId,
) -> Option<LoweredTypeId> {
    match type_table.get(container_type) {
        Some(crate::LoweredType::Vector { .. })
        | Some(crate::LoweredType::Sequence { .. }) => Some(container_type),
        Some(crate::LoweredType::Array { .. }) => Some(container_type),
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
