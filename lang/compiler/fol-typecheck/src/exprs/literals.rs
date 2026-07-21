use crate::{CheckedType, CheckedTypeId, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, ContainerType, Literal};
use fol_resolver::ResolvedProgram;

use super::helpers::{
    apparent_type_id, ensure_assignable, expected_nil_shell_type, merge_recoverable_effects,
    node_origin, plain_value_expr, strip_comments, with_node_origin,
};
use super::type_node_with_expectation;
use super::{TypeContext, TypedExpr};

pub(crate) fn type_literal(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    node: &AstNode,
    literal: &Literal,
    expected_type: Option<CheckedTypeId>,
) -> Result<CheckedTypeId, TypecheckError> {
    Ok(match literal {
        Literal::Integer(_) => typed.builtin_types().int,
        Literal::Float(_) => typed.builtin_types().float,
        Literal::String(_) => {
            reject_heap_backed_literal_in_core(typed, resolved, node, "string literals")?;
            typed.builtin_types().str_
        }
        Literal::Character(_) => {
            // A double-quoted single element is width-classified as a
            // character by the parser, but the book allows it as a
            // single-element string too — the expected type decides.
            if expected_type == Some(typed.builtin_types().str_) {
                reject_heap_backed_literal_in_core(typed, resolved, node, "string literals")?;
                typed.builtin_types().str_
            } else {
                typed.builtin_types().char_
            }
        }
        Literal::Boolean(_) => typed.builtin_types().bool_,
        Literal::Nil => {
            if let Some(shell_type) = expected_nil_shell_type(typed, expected_type)? {
                shell_type
            } else {
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::InvalidInput,
                    "nil literals require an expected opt[...] or err[...] shell type in V1",
                ));
            }
        }
    })
}

#[derive(Debug, Clone)]
pub(crate) enum ExpectedContainerShape {
    Array {
        element_type: CheckedTypeId,
        size: Option<usize>,
    },
    Vector {
        element_type: CheckedTypeId,
    },
    Sequence {
        element_type: CheckedTypeId,
    },
    Set {
        member_types: Vec<CheckedTypeId>,
    },
    Map {
        key_type: CheckedTypeId,
        value_type: CheckedTypeId,
    },
}

impl ExpectedContainerShape {
    pub(crate) fn kind(&self) -> ContainerType {
        match self {
            Self::Array { .. } => ContainerType::Array,
            Self::Vector { .. } => ContainerType::Vector,
            Self::Sequence { .. } => ContainerType::Sequence,
            Self::Set { .. } => ContainerType::Set,
            Self::Map { .. } => ContainerType::Map,
        }
    }
}

pub(crate) fn expected_container_shape(
    typed: &TypedProgram,
    expected_type: CheckedTypeId,
) -> Result<Option<ExpectedContainerShape>, TypecheckError> {
    let apparent = apparent_type_id(typed, expected_type)?;
    Ok(match typed.type_table().get(apparent) {
        Some(CheckedType::Array { element_type, size }) => Some(ExpectedContainerShape::Array {
            element_type: *element_type,
            size: *size,
        }),
        Some(CheckedType::Vector { element_type }) => Some(ExpectedContainerShape::Vector {
            element_type: *element_type,
        }),
        Some(CheckedType::Sequence { element_type }) => Some(ExpectedContainerShape::Sequence {
            element_type: *element_type,
        }),
        Some(CheckedType::Set { member_types }) => Some(ExpectedContainerShape::Set {
            member_types: member_types.clone(),
        }),
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => Some(ExpectedContainerShape::Map {
            key_type: *key_type,
            value_type: *value_type,
        }),
        _ => None,
    })
}

pub(crate) fn type_container_literal(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    container_type: ContainerType,
    elements: &[AstNode],
    expected_type: Option<CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    // A brace literal whose expected type is a record is positional (ordered)
    // record initialization: `{ v0, v1, ... }` binds values to fields in
    // declaration order. This is distinct from named `{ field = value }`
    // record initializers, which the parser routes to `RecordInit`.
    if let Some(expected) = expected_type {
        let apparent = apparent_type_id(typed, expected)?;
        if matches!(
            typed.type_table().get(apparent),
            Some(CheckedType::Record { .. })
        ) {
            return type_positional_record_init(
                typed, resolved, context, elements, expected, apparent,
            );
        }
    }

    let expected_container = expected_type
        .map(|expected| expected_container_shape(typed, expected))
        .transpose()?
        .flatten();
    let container_kind = expected_container
        .as_ref()
        .map(ExpectedContainerShape::kind)
        .unwrap_or(container_type);
    match container_kind {
        ContainerType::Array | ContainerType::Vector | ContainerType::Sequence => {
            reject_heap_backed_container_kind_in_core(typed, resolved, elements, &container_kind)?;
            Ok(TypedExpr::maybe_value(type_linear_container_literal(
                typed,
                resolved,
                context,
                container_kind,
                elements,
                expected_container.as_ref(),
            )?))
        }
        ContainerType::Set => {
            reject_heap_backed_container_kind_in_core(typed, resolved, elements, &container_kind)?;
            Ok(TypedExpr::maybe_value(type_set_literal(
                typed,
                resolved,
                context,
                elements,
                expected_container.as_ref(),
            )?))
        }
        ContainerType::Map => {
            reject_heap_backed_container_kind_in_core(typed, resolved, elements, &container_kind)?;
            Ok(TypedExpr::maybe_value(type_map_literal(
                typed,
                resolved,
                context,
                elements,
                expected_container.as_ref(),
            )?))
        }
    }
}

/// Positional (ordered) record initialization: `{ v0, v1, ... }` where the
/// expected type is a record. Values bind to fields in declaration order.
/// Fields left uncovered by positional values must carry a declared default,
/// otherwise they are reported as missing.
fn type_positional_record_init(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    elements: &[AstNode],
    expected_type: CheckedTypeId,
    apparent: CheckedTypeId,
) -> Result<TypedExpr, TypecheckError> {
    let element_nodes = container_elements(elements);
    let origin = element_nodes
        .first()
        .and_then(|node| node_origin(resolved, node));

    let Some(layout) = typed.record_layout(apparent) else {
        return Err(origin.map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "positional record initialization requires a named record type",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    "positional record initialization requires a named record type",
                    origin,
                )
            },
        ));
    };
    let layout = layout.to_vec();

    if element_nodes.len() > layout.len() {
        return Err(origin.map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::IncompatibleType,
                    format!(
                        "positional record initializer has {} value(s) but the record has {} field(s)",
                        element_nodes.len(),
                        layout.len()
                    ),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::IncompatibleType,
                    format!(
                        "positional record initializer has {} value(s) but the record has {} field(s)",
                        element_nodes.len(),
                        layout.len()
                    ),
                    origin,
                )
            },
        ));
    }

    let mut field_effects = Vec::new();
    for (element, field) in element_nodes.iter().zip(layout.iter()) {
        let field_origin = node_origin(resolved, element);
        let actual_expr =
            type_node_with_expectation(typed, resolved, context, element, Some(field.type_id))
                .map_err(|error| {
                    field_origin
                        .clone()
                        .map_or(error.clone(), |o| error.with_fallback_origin(o))
                })?;
        let actual_expr = plain_value_expr(
            typed,
            context,
            actual_expr,
            field_origin.clone(),
            format!("record field '{}'", field.name),
        )?;
        field_effects.push(actual_expr.recoverable_effect);
        let actual = actual_expr.required_value(format!(
            "positional record field '{}' does not have a type",
            field.name
        ))?;
        ensure_assignable(
            typed,
            field.type_id,
            actual,
            format!("record field '{}'", field.name),
            field_origin,
        )?;
        super::bindings::reject_untagged_owned_transfer(
            typed,
            resolved,
            element,
            actual,
            "inserted into a container",
        )?;
        super::bindings::track_value_transfer(typed, resolved, context, Some(element), actual)?;
    }

    // Fields not covered by a positional value must have a default.
    let missing = layout
        .iter()
        .skip(element_nodes.len())
        .filter(|field| field.default.is_none())
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(origin.clone().map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::IncompatibleType,
                    format!(
                        "record initializer is missing required fields: {}",
                        missing.join(", ")
                    ),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::IncompatibleType,
                    format!(
                        "record initializer is missing required fields: {}",
                        missing.join(", ")
                    ),
                    origin,
                )
            },
        ));
    }

    let merged = merge_recoverable_effects(typed, origin, "record initializer", field_effects)?;
    Ok(TypedExpr::value(expected_type).with_optional_effect(merged))
}

fn reject_heap_backed_literal_in_core(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    node: &AstNode,
    label: &str,
) -> Result<(), TypecheckError> {
    if typed.capability_model() != crate::TypecheckCapabilityModel::Core {
        return Ok(());
    }

    Err(with_node_origin(
        resolved,
        node,
        TypecheckErrorKind::Unsupported,
        format!("{label} require heap support and are unavailable in 'fol_model = core'"),
    ))
}

fn reject_heap_backed_container_kind_in_core(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    elements: &[AstNode],
    kind: &ContainerType,
) -> Result<(), TypecheckError> {
    if typed.capability_model() != crate::TypecheckCapabilityModel::Core {
        return Ok(());
    }

    let label = match kind {
        ContainerType::Array => return Ok(()),
        ContainerType::Vector => "vec[...] literals",
        ContainerType::Sequence => "seq[...] literals",
        ContainerType::Set => "set[...] literals",
        ContainerType::Map => "map[...] literals",
    };
    let message = format!("{label} require heap support and are unavailable in 'fol_model = core'");
    if let Some(origin) = elements
        .first()
        .and_then(|node| node_origin(resolved, node))
    {
        Err(TypecheckError::with_origin(
            TypecheckErrorKind::Unsupported,
            message,
            origin,
        ))
    } else {
        Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            message,
        ))
    }
}

pub(crate) fn type_linear_container_literal(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    kind: ContainerType,
    elements: &[AstNode],
    expected_container: Option<&ExpectedContainerShape>,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let mut inferred_element = expected_container.and_then(|shape| match shape {
        ExpectedContainerShape::Array { element_type, .. }
        | ExpectedContainerShape::Vector { element_type }
        | ExpectedContainerShape::Sequence { element_type } => Some(*element_type),
        _ => None,
    });
    let element_nodes = container_elements(elements);
    // Fixed-size arrays must match their declared size exactly; letting a
    // mismatch through leaks a raw rustc failure at emission.
    if let Some(ExpectedContainerShape::Array {
        size: Some(expected_size),
        ..
    }) = expected_container
    {
        if kind == ContainerType::Array && element_nodes.len() != *expected_size {
            return Err(TypecheckError::new(
                TypecheckErrorKind::IncompatibleType,
                format!(
                    "array literal has {} element(s) but the expected array size is {}",
                    element_nodes.len(),
                    expected_size
                ),
            ));
        }
    }
    if element_nodes.is_empty() {
        let Some(expected_container) = expected_container else {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                "empty container literals require an expected container type in V1",
            ));
        };
        return Ok(Some(intern_linear_container_shape(
            typed,
            kind,
            inferred_element.expect("linear expected containers should carry an element type"),
            match expected_container {
                ExpectedContainerShape::Array { size, .. } => *size,
                _ => None,
            },
        )));
    }

    let element_count = element_nodes.len();
    for element in element_nodes {
        let actual_raw =
            type_node_with_expectation(typed, resolved, context, element, inferred_element)?;
        let actual = plain_value_expr(
            typed,
            context,
            actual_raw,
            node_origin(resolved, element),
            "container element",
        )?
        .required_value("container element does not have a type")
        .map_err(|_| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "container element does not have a type",
            )
        })?;
        if let Some(expected) = inferred_element {
            ensure_assignable(
                typed,
                expected,
                actual,
                "container element".to_string(),
                None,
            )?;
        } else {
            inferred_element = Some(actual);
        }
        super::bindings::reject_untagged_owned_transfer(
            typed,
            resolved,
            element,
            actual,
            "inserted into a container",
        )?;
        super::bindings::track_value_transfer(typed, resolved, context, Some(element), actual)?;
    }

    let element_type = inferred_element.ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "container literal could not infer an element type",
        )
    })?;
    let array_size = match expected_container {
        Some(ExpectedContainerShape::Array { size, .. }) => *size,
        // Bare array literals carry their own length; lowering resolves the
        // container against the sized shape, so intern it sized here.
        _ if kind == ContainerType::Array => Some(element_count),
        _ => None,
    };
    Ok(Some(intern_linear_container_shape(
        typed,
        kind,
        element_type,
        array_size,
    )))
}

pub(crate) fn type_set_literal(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    elements: &[AstNode],
    expected_container: Option<&ExpectedContainerShape>,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let element_nodes = container_elements(elements);
    let mut member_types = Vec::new();

    if element_nodes.is_empty() {
        let Some(ExpectedContainerShape::Set { member_types }) = expected_container else {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                "empty container literals require an expected container type in V1",
            ));
        };
        return Ok(Some(typed.type_table_mut().intern(CheckedType::Set {
            member_types: member_types.clone(),
        })));
    }

    let expected_members = match expected_container {
        Some(ExpectedContainerShape::Set { member_types }) => Some(member_types.as_slice()),
        _ => None,
    };
    if let Some(expected_members) = expected_members {
        if expected_members.len() != element_nodes.len() {
            return Err(TypecheckError::new(
                TypecheckErrorKind::IncompatibleType,
                format!(
                    "set literal expects {} elements but got {}",
                    expected_members.len(),
                    element_nodes.len()
                ),
            ));
        }
    }

    for (index, element) in element_nodes.iter().enumerate() {
        let expected = expected_members
            .and_then(|members| members.get(index))
            .copied();
        let actual_raw = type_node_with_expectation(typed, resolved, context, element, expected)?;
        let actual = plain_value_expr(
            typed,
            context,
            actual_raw,
            node_origin(resolved, element),
            format!("set member {}", index),
        )?
        .required_value("set member does not have a type")
        .map_err(|_| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "set member does not have a type",
            )
        })?;
        if let Some(expected) = expected {
            ensure_assignable(
                typed,
                expected,
                actual,
                format!("set member {}", index),
                None,
            )?;
            member_types.push(expected);
        } else {
            member_types.push(actual);
        }
        super::bindings::reject_untagged_owned_transfer(
            typed,
            resolved,
            element,
            actual,
            "inserted into a container",
        )?;
        super::bindings::track_value_transfer(typed, resolved, context, Some(element), actual)?;
    }

    Ok(Some(
        typed
            .type_table_mut()
            .intern(CheckedType::Set { member_types }),
    ))
}

pub(crate) fn type_map_literal(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    elements: &[AstNode],
    expected_container: Option<&ExpectedContainerShape>,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let element_nodes = container_elements(elements);
    let mut inferred_key_type = match expected_container {
        Some(ExpectedContainerShape::Map { key_type, .. }) => Some(*key_type),
        _ => None,
    };
    let mut inferred_value_type = match expected_container {
        Some(ExpectedContainerShape::Map { value_type, .. }) => Some(*value_type),
        _ => None,
    };

    if element_nodes.is_empty() {
        let Some(ExpectedContainerShape::Map {
            key_type,
            value_type,
        }) = expected_container
        else {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                "empty container literals require an expected container type in V1",
            ));
        };
        return Ok(Some(typed.type_table_mut().intern(CheckedType::Map {
            key_type: *key_type,
            value_type: *value_type,
        })));
    }

    for pair in element_nodes {
        let (key, value) = map_literal_pair(pair)?;
        let actual_key_raw =
            type_node_with_expectation(typed, resolved, context, key, inferred_key_type)?;
        let actual_key = plain_value_expr(
            typed,
            context,
            actual_key_raw,
            node_origin(resolved, key),
            "map key",
        )?
        .required_value("map key does not have a type")
        .map_err(|_| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "map key does not have a type",
            )
        })?;
        let actual_value_raw =
            type_node_with_expectation(typed, resolved, context, value, inferred_value_type)?;
        let actual_value = plain_value_expr(
            typed,
            context,
            actual_value_raw,
            node_origin(resolved, value),
            "map value",
        )?
        .required_value("map value does not have a type")
        .map_err(|_| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "map value does not have a type",
            )
        })?;
        if let Some(expected_key) = inferred_key_type {
            ensure_assignable(typed, expected_key, actual_key, "map key".to_string(), None)?;
        } else {
            inferred_key_type = Some(actual_key);
        }
        if let Some(expected_value) = inferred_value_type {
            ensure_assignable(
                typed,
                expected_value,
                actual_value,
                "map value".to_string(),
                None,
            )?;
        } else {
            inferred_value_type = Some(actual_value);
        }
        super::bindings::reject_untagged_owned_transfer(
            typed,
            resolved,
            key,
            actual_key,
            "used as a container key",
        )?;
        super::bindings::reject_untagged_owned_transfer(
            typed,
            resolved,
            value,
            actual_value,
            "inserted into a container",
        )?;
        super::bindings::track_value_transfer(typed, resolved, context, Some(key), actual_key)?;
        super::bindings::track_value_transfer(typed, resolved, context, Some(value), actual_value)?;
    }

    Ok(Some(typed.type_table_mut().intern(CheckedType::Map {
        key_type: inferred_key_type.ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "map literal could not infer a key type",
            )
        })?,
        value_type: inferred_value_type.ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "map literal could not infer a value type",
            )
        })?,
    })))
}

pub(crate) fn intern_linear_container_shape(
    typed: &mut TypedProgram,
    kind: ContainerType,
    element_type: CheckedTypeId,
    array_size: Option<usize>,
) -> CheckedTypeId {
    match kind {
        ContainerType::Array => typed.type_table_mut().intern(CheckedType::Array {
            element_type,
            size: array_size,
        }),
        ContainerType::Vector => typed
            .type_table_mut()
            .intern(CheckedType::Vector { element_type }),
        ContainerType::Sequence => typed
            .type_table_mut()
            .intern(CheckedType::Sequence { element_type }),
        ContainerType::Set | ContainerType::Map => {
            unreachable!("set/map shapes must be interned through dedicated container helpers")
        }
    }
}

pub(crate) fn container_elements(elements: &[AstNode]) -> Vec<&AstNode> {
    elements
        .iter()
        .filter(|element| !matches!(element, AstNode::Comment { .. }))
        .collect()
}

pub(crate) fn map_literal_pair(pair: &AstNode) -> Result<(&AstNode, &AstNode), TypecheckError> {
    match strip_comments(pair) {
        AstNode::ContainerLiteral { elements, .. } => {
            let pair_items = container_elements(elements);
            if let [key, value] = pair_items.as_slice() {
                Ok((*key, *value))
            } else {
                Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "map literals require each element to be a two-value pair",
                ))
            }
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "map literals require each element to be a two-value pair",
        )),
    }
}

pub(crate) fn type_set_index_access(
    _typed: &TypedProgram,
    member_types: &[CheckedTypeId],
    index: &AstNode,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let Some(index_value) = literal_integer_value(index) else {
        let Some(first) = member_types.first().copied() else {
            return Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                "cannot index an empty set value in V1",
            ));
        };
        if member_types.iter().all(|member| *member == first) {
            return Ok(Some(first));
        }
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "non-literal indexing into heterogeneous sets is not yet supported",
        ));
    };

    if index_value < 0 {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "set index literals must be non-negative",
        ));
    }

    member_types
        .get(index_value as usize)
        .copied()
        .map(Some)
        .ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "set index {} is out of bounds for a {}-member set",
                    index_value,
                    member_types.len()
                ),
            )
        })
}

pub(crate) fn literal_integer_value(node: &AstNode) -> Option<i64> {
    use fol_parser::ast::Literal;
    match strip_comments(node) {
        AstNode::Literal(Literal::Integer(value)) => Some(*value),
        _ => None,
    }
}

// Expose type_literal with just typed/literal/expected for test compatibility
#[cfg(test)]
pub(crate) fn type_literal_simple(
    typed: &mut TypedProgram,
    literal: &Literal,
    expected_type: Option<CheckedTypeId>,
) -> Result<CheckedTypeId, TypecheckError> {
    Ok(match literal {
        Literal::Integer(_) => typed.builtin_types().int,
        Literal::Float(_) => typed.builtin_types().float,
        Literal::String(_) => typed.builtin_types().str_,
        Literal::Character(_) => typed.builtin_types().char_,
        Literal::Boolean(_) => typed.builtin_types().bool_,
        Literal::Nil => {
            if let Some(shell_type) = expected_nil_shell_type(typed, expected_type)? {
                shell_type
            } else {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "nil literals require an expected opt[...] or err[...] shell type in V1",
                ));
            }
        }
    })
}
