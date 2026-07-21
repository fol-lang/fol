use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::AstNode;
use fol_resolver::{ReferenceKind, ResolvedProgram, SymbolId};

use super::helpers::{
    apparent_type_id, describe_type, ensure_assignable, merge_recoverable_effects, node_origin,
    plain_value_expr, strip_comments, with_node_origin,
};
use super::literals::type_set_index_access;
use super::type_node;
use super::{TypeContext, TypedExpr};

/// Reject reading `object.field` when that exact static field was already
/// moved out of a direct owned binding (Slice C §3.1). Sibling fields stay
/// readable; only the moved place is rejected.
fn reject_moved_field_projection(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    object: &AstNode,
    field: &str,
) -> Result<(), TypecheckError> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        name,
    } = strip_comments(object)
    else {
        return Ok(());
    };
    let Some(symbol) = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
    else {
        return Ok(());
    };
    if let Some(move_origin) = typed.moved_field_origin(symbol, field).cloned() {
        let message = format!("use of moved field '{name}.{field}'");
        let mut error = node_origin(resolved, object).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
            |origin| {
                TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
            },
        );
        error = error.with_related_origin(move_origin, "field moved here");
        return Err(error);
    }
    Ok(())
}

/// Uniform inner-place access `container[]` (V3_MEM §3.3): reads the payload of
/// a pointer, an `opt[T]`, or an `err[T]`. Direct access asserts the payload
/// exists (panics otherwise); the safe form uses `when` choice syntax.
pub(crate) fn type_inner_place_access(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    container: &AstNode,
    patterns: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    if !patterns.is_empty() {
        return Err(with_node_origin(
            resolved,
            container,
            TypecheckErrorKind::Unsupported,
            "pattern-list access 'container[pattern, ...]' is not supported in V3; use the uniform inner access 'container[]'",
        ));
    }
    let container_raw = type_node(typed, resolved, context, container)?;
    let container_expr = plain_value_expr(
        typed,
        context,
        container_raw,
        node_origin(resolved, container),
        "inner-place access '[]' receiver",
    )?;
    let container_type =
        container_expr.required_value("inner-place access '[]' does not have a typed receiver")?;
    let apparent = apparent_type_id(typed, container_type)?;
    match typed.type_table().get(apparent) {
        Some(CheckedType::Pointer { weak: true, .. }) => Err(with_node_origin(
            resolved,
            container,
            TypecheckErrorKind::Ownership,
            "a weak pointer 'ptr[weak, T]' cannot be accessed directly; upgrade it with '[upg]' to 'opt[ptr[shared, T]]' first",
        )),
        Some(CheckedType::Pointer { target, .. }) => Ok(TypedExpr::value(*target)),
        Some(CheckedType::Optional { inner }) => Ok(TypedExpr::value(*inner)),
        Some(CheckedType::Error { inner: Some(inner) }) => Ok(TypedExpr::value(*inner)),
        Some(CheckedType::Error { inner: None }) => Err(with_node_origin(
            resolved,
            container,
            TypecheckErrorKind::InvalidInput,
            "'err[]' has no payload to access",
        )),
        _ => Err(with_node_origin(
            resolved,
            container,
            TypecheckErrorKind::InvalidInput,
            "inner-place access '[]' requires a pointer, 'opt[T]', or 'err[T]' receiver",
        )),
    }
}

pub(crate) fn type_field_access(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    object: &AstNode,
    field: &str,
    expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    let direct_mutex = direct_mutex_identifier(typed, resolved, object);
    if let Some((mutex, name)) = direct_mutex.as_ref() {
        if context.inside_deferred_block {
            return Err(with_node_origin(
                resolved,
                object,
                TypecheckErrorKind::Unsupported,
                format!(
                    "mutex field access through '{name}' is not allowed inside dfr/edf in V3; delayed mutex guard effects are not modeled"
                ),
            ));
        }
        if typed.active_mutex_guard(*mutex).is_none() {
            return Err(with_node_origin(
                resolved,
                object,
                TypecheckErrorKind::InvalidInput,
                format!(
                    "mutex field access through '{name}' requires '{name}.lock()' in the current lexical scope"
                ),
            ));
        }
    }
    // A partially moved aggregate may still be projected for a surviving field,
    // so mark this read as a projection root: the whole-value "partially moved"
    // rejection is suppressed while typing the receiver, and the specific field
    // being read is checked explicitly below (Slice C §3.1).
    reject_moved_field_projection(typed, resolved, object, field)?;
    let object_raw = type_node(
        typed,
        resolved,
        super::TypeContext {
            allow_mutex_handle: direct_mutex.is_some(),
            field_projection_root: true,
            ..context
        },
        object,
    )?;
    let object_expr = plain_value_expr(
        typed,
        context,
        object_raw,
        node_origin(resolved, object),
        format!("field access '.{field}' receiver"),
    )?;
    let object_type = object_expr.required_value(format!(
        "field access '.{field}' does not have a typed receiver"
    ))?;
    if matches!(strip_comments(object), AstNode::FieldAccess { .. })
        && super::bindings::ownership_moves_on_transfer(typed, object_type)
    {
        return Err(with_node_origin(
            resolved,
            object,
            TypecheckErrorKind::Ownership,
            "nested field access through a move-only intermediate is not supported in V3; partial moves are not supported",
        ));
    }
    let resolved_type = apparent_type_id(typed, object_type)?;
    // A record TYPE reference is not a value: `Point.x` must reject at the
    // checker rather than surfacing as a lowering failure. Entries stay
    // accessible as `Type.MEMBER` — their members are the type's constants.
    if matches!(
        typed.type_table().get(resolved_type),
        Some(CheckedType::Record { .. })
    ) {
        if let AstNode::Identifier {
            syntax_id: Some(object_syntax),
            name,
        } = strip_comments(object)
        {
            let names_type_symbol = resolved
                .references
                .iter()
                .find(|reference| reference.syntax_id == Some(*object_syntax))
                .and_then(|reference| reference.resolved)
                .and_then(|symbol| resolved.symbol(symbol))
                .is_some_and(|symbol| {
                    matches!(
                        symbol.kind,
                        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias
                    )
                });
            if names_type_symbol {
                return Err(with_node_origin(
                    resolved,
                    object,
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "'{name}' names a record type, not a value; field access '.{field}' requires an instance"
                    ),
                ));
            }
        }
    }
    match typed.type_table().get(resolved_type) {
        Some(CheckedType::Record { fields }) => fields
            .get(field)
            .copied()
            .map(|type_id| {
                TypedExpr::value(type_id).with_optional_effect(object_expr.recoverable_effect)
            })
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("record receiver does not expose a field named '{field}'"),
                )
            }),
        Some(CheckedType::Entry { variants }) => {
            if !variants.contains_key(field) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("entry receiver does not expose a variant named '{field}'"),
                ));
            }
            // A bare entry-variant access denotes a value of the entry type
            // itself. It may additionally coerce to its stored payload type
            // when an explicit non-entry expectation asks for it (e.g.
            // returning `Color.BLUE` as `str`). Without such an expectation
            // the natural type is the entry, which is what generic argument
            // inference and ordinary assignability both need.
            if let Some(expected_type) = expected_type {
                let expected_apparent = apparent_type_id(typed, expected_type)?;
                if expected_apparent == resolved_type {
                    return Ok(TypedExpr::value(expected_type)
                        .with_optional_effect(object_expr.recoverable_effect));
                }
                if let Some(payload) = variants.get(field).copied().flatten() {
                    return Ok(TypedExpr::value(payload)
                        .with_optional_effect(object_expr.recoverable_effect));
                }
            }
            Ok(TypedExpr::value(object_type).with_optional_effect(object_expr.recoverable_effect))
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "field access '.{field}' requires a record-like or entry-like receiver, got '{}'",
                describe_type(typed, object_type)
            ),
        )),
    }
}

fn direct_mutex_identifier(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    object: &AstNode,
) -> Option<(SymbolId, String)> {
    let AstNode::Identifier {
        name,
        syntax_id: Some(syntax_id),
    } = strip_comments(object)
    else {
        return None;
    };
    let symbol = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })?
        .resolved?;
    typed
        .typed_symbol(symbol)
        .is_some_and(|symbol| symbol.is_mutex)
        .then(|| (symbol, name.clone()))
}

pub(crate) fn type_index_access(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    container: &AstNode,
    index: &AstNode,
) -> Result<TypedExpr, TypecheckError> {
    let container_raw = type_node(typed, resolved, context, container)?;
    let container_expr = plain_value_expr(
        typed,
        context,
        container_raw,
        node_origin(resolved, container),
        "index access receiver",
    )?;
    let container_type =
        container_expr.required_value("index access does not have a typed container")?;
    let resolved_type = apparent_type_id(typed, container_type)?;
    // Type the index against the container's key type so literals settle on
    // the expected shape (a single-character string key must stay str).
    let expected_index_type = match typed.type_table().get(resolved_type) {
        Some(CheckedType::Map { key_type, .. }) => Some(*key_type),
        Some(CheckedType::Array { .. })
        | Some(CheckedType::Vector { .. })
        | Some(CheckedType::Sequence { .. })
        | Some(CheckedType::Set { .. }) => Some(typed.builtin_types().int),
        _ => None,
    };
    let index_raw =
        super::type_node_with_expectation(typed, resolved, context, index, expected_index_type)?;
    let index_expr = plain_value_expr(
        typed,
        context,
        index_raw,
        node_origin(resolved, index),
        "index expression",
    )?;
    let index_type =
        index_expr.required_value("index access does not have a typed index expression")?;
    reject_move_only_index_projection(typed, resolved, container, container_type, "receiver")?;
    reject_move_only_index_projection(typed, resolved, index, index_type, "key")?;
    let merged_effect = merge_recoverable_effects(
        typed,
        node_origin(resolved, container).or_else(|| node_origin(resolved, index)),
        "index access",
        [
            container_expr.recoverable_effect,
            index_expr.recoverable_effect,
        ],
    )?;
    match typed.type_table().get(resolved_type) {
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector { element_type })
        | Some(CheckedType::Sequence { element_type }) => {
            let element_type = *element_type;
            ensure_assignable(
                typed,
                typed.builtin_types().int,
                index_type,
                "container index".to_string(),
                None,
            )?;
            reject_move_only_index_result(typed, resolved, container, index, element_type)?;
            Ok(TypedExpr::value(element_type).with_optional_effect(merged_effect))
        }
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => {
            let key_type = *key_type;
            let value_type = *value_type;
            ensure_assignable(typed, key_type, index_type, "map key".to_string(), None)?;
            reject_move_only_index_result(typed, resolved, container, index, value_type)?;
            Ok(TypedExpr::value(value_type).with_optional_effect(merged_effect))
        }
        Some(CheckedType::Set { member_types }) => {
            ensure_assignable(
                typed,
                typed.builtin_types().int,
                index_type,
                "set index".to_string(),
                None,
            )?;
            let result_type = type_set_index_access(typed, member_types, index)?;
            if let Some(result_type) = result_type {
                reject_move_only_index_result(typed, resolved, container, index, result_type)?;
            }
            Ok(TypedExpr::maybe_value(result_type).with_optional_effect(merged_effect))
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "index access requires an array, vector, sequence, set, or map receiver, got '{}'",
                describe_type(typed, container_type)
            ),
        )),
    }
}

fn reject_move_only_index_projection(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    node: &AstNode,
    type_id: crate::CheckedTypeId,
    role: &str,
) -> Result<(), TypecheckError> {
    if !super::bindings::ownership_moves_on_transfer(typed, type_id)
        || !matches!(strip_comments(node), AstNode::FieldAccess { .. })
    {
        return Ok(());
    }
    Err(with_node_origin(
        resolved,
        node,
        TypecheckErrorKind::Ownership,
        format!(
            "index {role} observation through a move-only field projection is not supported in V3; lookup must not partially move its source"
        ),
    ))
}

fn reject_move_only_index_result(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    container: &AstNode,
    index: &AstNode,
    result_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    if !super::bindings::ownership_moves_on_transfer(typed, result_type) {
        return Ok(());
    }
    let message = "move-only indexed projection cannot be read in V3; partial moves are not supported and clone-based reads would duplicate ownership";
    Err(node_origin(resolved, container)
        .or_else(|| node_origin(resolved, index))
        .map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Ownership, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin),
        ))
}

pub(crate) fn type_slice_access(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    container: &AstNode,
    start: Option<&AstNode>,
    end: Option<&AstNode>,
) -> Result<TypedExpr, TypecheckError> {
    let container_raw = type_node(typed, resolved, context, container)?;
    let container_expr = plain_value_expr(
        typed,
        context,
        container_raw,
        node_origin(resolved, container),
        "slice receiver",
    )?;
    let container_type =
        container_expr.required_value("slice access does not have a typed container")?;
    let mut bound_effects = vec![container_expr.recoverable_effect];
    for bound in [start, end].into_iter().flatten() {
        let bound_raw = type_node(typed, resolved, context, bound)?;
        let bound_expr = plain_value_expr(
            typed,
            context,
            bound_raw,
            node_origin(resolved, bound),
            "slice bound",
        )?;
        let bound_type = bound_expr.required_value("slice bound does not have a type")?;
        bound_effects.push(bound_expr.recoverable_effect);
        ensure_assignable(
            typed,
            typed.builtin_types().int,
            bound_type,
            "slice bound".to_string(),
            None,
        )?;
    }
    let resolved_type = apparent_type_id(typed, container_type)?;
    let merged_effect = merge_recoverable_effects(
        typed,
        node_origin(resolved, container),
        "slice access",
        bound_effects,
    )?;
    match typed.type_table().get(resolved_type) {
        Some(CheckedType::Array { .. }) => Err(with_node_origin(
            resolved,
            container,
            TypecheckErrorKind::Unsupported,
            "fixed-size array slices are not supported; use vec[...] or seq[...] when a runtime-sized slice result is required",
        )),
        Some(CheckedType::Vector { element_type }) => {
            let element_type = *element_type;
            reject_move_only_slice_element(typed, resolved, container, element_type)?;
            Ok(TypedExpr::value(
                typed
                    .type_table_mut()
                    .intern(CheckedType::Vector { element_type }),
            )
            .with_optional_effect(merged_effect))
        }
        Some(CheckedType::Sequence { element_type }) => {
            let element_type = *element_type;
            reject_move_only_slice_element(typed, resolved, container, element_type)?;
            Ok(TypedExpr::value(
                typed
                    .type_table_mut()
                    .intern(CheckedType::Sequence { element_type }),
            )
            .with_optional_effect(merged_effect))
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "slice access requires a vector or sequence receiver, got '{}'",
                describe_type(typed, container_type)
            ),
        )),
    }
}

fn reject_move_only_slice_element(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    container: &AstNode,
    element_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    if !super::bindings::ownership_moves_on_transfer(typed, element_type) {
        return Ok(());
    }
    Err(with_node_origin(
        resolved,
        container,
        TypecheckErrorKind::Ownership,
        "slices of move-only elements are not supported in V3; slice creation would clone unique ownership",
    ))
}
