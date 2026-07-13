use crate::model::ActiveBorrow;
use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, UnaryOperator};
use fol_resolver::{ReferenceKind, ResolvedProgram, SymbolId};
use std::collections::BTreeSet;

use super::helpers::{
    ensure_assignable, find_symbol_in_scope_chain, internal_error, merge_recoverable_effects,
    node_origin, plain_value_expr, reject_recoverable_error_shell_conversion,
    reject_recoverable_plain_use,
};
use super::type_node_with_expectation;
use super::{TypeContext, TypedExpr};

pub(crate) fn type_binding_initializer(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    name: &str,
    value: Option<&AstNode>,
    symbol_kind: fol_resolver::SymbolKind,
    heap_owned: bool,
    borrowing: bool,
    mutable_borrow: bool,
) -> Result<TypedExpr, TypecheckError> {
    let binding_origin = find_symbol_in_scope_chain(
        resolved,
        context.source_unit_id,
        context.scope_id,
        name,
        symbol_kind,
    )
    .and_then(|symbol_id| resolved.symbol(symbol_id))
    .and_then(|symbol| symbol.origin.clone());

    if heap_owned && typed.capability_model() == crate::TypecheckCapabilityModel::Core {
        let message =
            "heap allocation binding requires heap support and is unavailable in 'fol_model = core'";
        return Err(binding_origin.clone().map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
        ));
    }

    let Some(symbol_id) = find_symbol_in_scope_chain(
        resolved,
        context.source_unit_id,
        context.scope_id,
        name,
        symbol_kind,
    ) else {
        let initializer_expr = value
            .map(|value| {
                type_node_with_expectation(typed, resolved, context, value, None).map_err(|error| {
                    node_origin(resolved, value)
                        .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                })
            })
            .transpose()?;
        return Ok(initializer_expr.unwrap_or_else(TypedExpr::none));
    };
    let declared_type = typed
        .typed_symbol(symbol_id)
        .and_then(|symbol| symbol.declared_type);
    if let Some(declared_type) = declared_type {
        reject_top_level_channel_binding(
            typed,
            resolved,
            symbol_id,
            declared_type,
            binding_origin.clone(),
        )?;
    }
    let initializer_expr = value
        .map(|value| {
            let typed_value = if borrowing {
                type_borrow_source(typed, resolved, value)
            } else {
                None
            };
            typed_value
                .unwrap_or_else(|| {
                    type_node_with_expectation(typed, resolved, context, value, declared_type)
                })
                .map_err(|error| {
                    binding_origin
                        .clone()
                        .or_else(|| node_origin(resolved, value))
                        .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                })
        })
        .transpose()?;

    match (declared_type, initializer_expr) {
        (Some(expected), Some(actual_expr)) => {
            reject_recoverable_error_shell_conversion(
                typed,
                expected,
                &actual_expr,
                value.and_then(|node| node_origin(resolved, node)),
                format!("initializer for '{name}'"),
            )?;
            let actual_expr = plain_value_expr(
                typed,
                context,
                actual_expr,
                value.and_then(|node| node_origin(resolved, node)),
                format!("initializer for '{name}'"),
            )?;
            let actual = actual_expr
                .required_value(format!("initializer for '{name}' does not have a type"))?;
            reject_nested_eventual_value(
                typed,
                actual,
                value.and_then(|node| node_origin(resolved, node)),
                format!("initializer for '{name}'"),
            )?;
            let actual_view = owned_or_borrowed_inner(typed, actual);
            ensure_assignable(
                typed,
                expected,
                actual_view,
                format!("initializer for '{name}'"),
                value.and_then(|node| node_origin(resolved, node)),
            )?;
            if borrowing {
                let borrowed = register_borrow_binding(
                    typed,
                    resolved,
                    context,
                    symbol_id,
                    value,
                    expected,
                    mutable_borrow,
                )?;
                Ok(TypedExpr::value(borrowed))
            } else {
                track_value_transfer(typed, resolved, context, value, actual)?;
                Ok(TypedExpr::value(expected))
            }
        }
        (None, Some(inferred_expr)) => {
            if inferred_expr.recoverable_effect.is_some() {
                let error = reject_recoverable_plain_use(
                    value.and_then(|node| node_origin(resolved, node)),
                    format!("initializer for '{name}'"),
                )
                .expect_err("recoverable plain-use rejection should always return an error");
                return Err(error);
            }
            let inferred = inferred_expr
                .required_value(format!("initializer for '{name}' does not have a type"))?;
            if (heap_owned || borrowing) && type_contains_eventual(typed, inferred) {
                return Err(value
                    .and_then(|node| node_origin(resolved, node))
                    .map_or_else(
                        || {
                            TypecheckError::new(
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "initializer for '{name}' cannot wrap the internal eventual type in an ownership or borrow shell in V3"
                                ),
                            )
                        },
                        |origin| {
                            TypecheckError::with_origin(
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "initializer for '{name}' cannot wrap the internal eventual type in an ownership or borrow shell in V3"
                                ),
                                origin,
                            )
                        },
                    ));
            }
            let inferred_view = owned_or_borrowed_inner(typed, inferred);
            let inferred_borrow_from = !heap_owned
                && !borrowing
                && value.is_some_and(is_borrow_from_expression)
                && matches!(
                    typed.type_table().get(inferred),
                    Some(CheckedType::Borrowed { .. })
                );
            let inferred = if borrowing || inferred_borrow_from {
                register_borrow_binding(
                    typed,
                    resolved,
                    context,
                    symbol_id,
                    value,
                    inferred_view,
                    borrowing && mutable_borrow,
                )?
            } else if heap_owned {
                typed
                    .type_table_mut()
                    .intern(CheckedType::Owned { inner: inferred })
            } else {
                inferred
            };
            reject_top_level_channel_binding(
                typed,
                resolved,
                symbol_id,
                inferred,
                binding_origin.clone(),
            )?;
            reject_nested_eventual_value(
                typed,
                inferred,
                value.and_then(|node| node_origin(resolved, node)),
                format!("initializer for '{name}'"),
            )?;
            let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
                internal_error("typed symbol table lost an inferred binding", None)
            })?;
            symbol.declared_type = Some(inferred);
            if !borrowing && !inferred_borrow_from {
                track_value_transfer(
                    typed,
                    resolved,
                    context,
                    value,
                    inferred_expr.value_type.unwrap(),
                )?;
            }
            Ok(TypedExpr::value(inferred))
        }
        (Some(expected), None) => Ok(TypedExpr::value(expected)),
        (None, None) => Ok(TypedExpr::none()),
    }
}

fn reject_top_level_channel_binding(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    symbol: SymbolId,
    type_id: crate::CheckedTypeId,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let mut scope = resolved.symbol(symbol).map(|symbol| symbol.scope);
    while let Some(scope_id) = scope {
        let Some(resolved_scope) = resolved.scope(scope_id) else {
            break;
        };
        if matches!(resolved_scope.kind, fol_resolver::ScopeKind::Routine) {
            return Ok(());
        }
        scope = resolved_scope.parent;
    }
    if !matches!(
        typed
            .type_table()
            .get(super::helpers::apparent_type_id(typed, type_id)?),
        Some(CheckedType::Channel { .. })
    ) {
        return Ok(());
    }
    let message =
        "top-level channel bindings are not supported in V3; declare the channel inside its receiving routine";
    Err(origin.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
    ))
}

fn is_borrow_from_expression(node: &AstNode) -> bool {
    matches!(
        super::helpers::strip_comments(node),
        AstNode::UnaryOp {
            op: UnaryOperator::BorrowFrom,
            ..
        }
    )
}

fn borrow_source_identifier(node: &AstNode) -> Option<(&AstNode, fol_parser::ast::SyntaxNodeId)> {
    let node = match node {
        AstNode::UnaryOp {
            op: UnaryOperator::BorrowFrom,
            operand,
        } => operand.as_ref(),
        other => other,
    };
    match node {
        AstNode::Identifier {
            syntax_id: Some(syntax_id),
            ..
        } => Some((node, *syntax_id)),
        _ => None,
    }
}

pub(crate) fn borrow_source_symbol(resolved: &ResolvedProgram, node: &AstNode) -> Option<SymbolId> {
    let (_, syntax_id) = borrow_source_identifier(node)?;
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id) && reference.kind == ReferenceKind::Identifier
        })?
        .resolved
}

pub(crate) fn reject_reborrow_source(
    typed: &TypedProgram,
    symbol: SymbolId,
    origin: fol_parser::ast::SyntaxOrigin,
) -> Result<(), TypecheckError> {
    let borrowed = typed
        .typed_symbol(symbol)
        .and_then(|symbol| symbol.declared_type)
        .and_then(|type_id| typed.type_table().get(type_id))
        .is_some_and(|typ| matches!(typ, CheckedType::Borrowed { .. }));
    if !borrowed {
        return Ok(());
    }

    let error = TypecheckError::with_origin(
        TypecheckErrorKind::BorrowConflict,
        "reborrowing a borrow binding is not supported in V3; borrow from the original owner",
        origin,
    );
    Err(typed
        .borrow_for_binding(symbol)
        .map(|borrow| {
            error
                .clone()
                .with_related_origin(borrow.origin.clone(), "original borrow created here")
        })
        .unwrap_or(error))
}

pub(crate) fn owned_or_borrowed_inner(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
) -> crate::CheckedTypeId {
    match typed.type_table().get(type_id) {
        Some(CheckedType::Owned { inner }) | Some(CheckedType::Borrowed { inner, .. }) => *inner,
        _ => type_id,
    }
}

pub(crate) fn type_contains_eventual(typed: &TypedProgram, type_id: crate::CheckedTypeId) -> bool {
    fn contains(
        typed: &TypedProgram,
        type_id: crate::CheckedTypeId,
        visiting: &mut BTreeSet<crate::CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let result = if let Some(apparent) = typed.apparent_type_override(type_id) {
            contains(typed, apparent, visiting)
        } else {
            match typed.type_table().get(type_id) {
                Some(CheckedType::Eventual { .. }) => true,
                Some(CheckedType::Declared { args, .. }) => {
                    args.iter().any(|arg| contains(typed, *arg, visiting))
                }
                Some(CheckedType::Array { element_type, .. })
                | Some(CheckedType::Vector { element_type })
                | Some(CheckedType::Sequence { element_type })
                | Some(CheckedType::Channel { element_type })
                | Some(CheckedType::ChannelSender { element_type }) => {
                    contains(typed, *element_type, visiting)
                }
                Some(CheckedType::Optional { inner })
                | Some(CheckedType::Owned { inner })
                | Some(CheckedType::Borrowed { inner, .. }) => contains(typed, *inner, visiting),
                Some(CheckedType::Pointer { target, .. }) => contains(typed, *target, visiting),
                Some(CheckedType::Error { inner }) => {
                    inner.is_some_and(|inner| contains(typed, inner, visiting))
                }
                Some(CheckedType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| contains(typed, *member, visiting)),
                Some(CheckedType::Map {
                    key_type,
                    value_type,
                }) => {
                    contains(typed, *key_type, visiting) || contains(typed, *value_type, visiting)
                }
                Some(CheckedType::Record { fields }) => fields
                    .values()
                    .any(|field| contains(typed, *field, visiting)),
                Some(CheckedType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| contains(typed, *variant, visiting)),
                Some(CheckedType::Routine(signature)) => {
                    signature
                        .params
                        .iter()
                        .any(|param| contains(typed, *param, visiting))
                        || signature
                            .return_type
                            .is_some_and(|ret| contains(typed, ret, visiting))
                        || signature
                            .error_type
                            .is_some_and(|error| contains(typed, error, visiting))
                }
                Some(CheckedType::Builtin(_)) | None => false,
            }
        };
        visiting.remove(&type_id);
        result
    }

    contains(typed, type_id, &mut BTreeSet::new())
}

pub(crate) fn type_has_nested_eventual(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
) -> bool {
    match typed.type_table().get(type_id) {
        Some(CheckedType::Eventual {
            value_type,
            error_type,
        }) => {
            type_contains_eventual(typed, *value_type)
                || error_type.is_some_and(|error| type_contains_eventual(typed, error))
        }
        _ => type_contains_eventual(typed, type_id),
    }
}

pub(crate) fn reject_nested_eventual_value(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
    surface: impl Into<String>,
) -> Result<(), TypecheckError> {
    if !type_has_nested_eventual(typed, type_id) {
        return Ok(());
    }
    let message = format!(
        "{} cannot embed the internal eventual type in a composite value in V3; transfer or await the eventual directly",
        surface.into()
    );
    Err(match origin {
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin),
        None => TypecheckError::new(TypecheckErrorKind::Ownership, message),
    })
}

fn type_borrow_source(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    value: &AstNode,
) -> Option<Result<TypedExpr, TypecheckError>> {
    let symbol = borrow_source_symbol(resolved, value)?;
    let type_id = typed.typed_symbol(symbol)?.declared_type?;
    Some(Ok(TypedExpr::value(owned_or_borrowed_inner(
        typed, type_id,
    ))))
}

fn register_borrow_binding(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    binding: SymbolId,
    value: Option<&AstNode>,
    inner: crate::CheckedTypeId,
    mutable: bool,
) -> Result<crate::CheckedTypeId, TypecheckError> {
    let value = value.ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "borrow bindings require an owner initializer",
        )
    })?;
    let owner = borrow_source_symbol(resolved, value).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "borrow bindings require an identifier owner or '#owner'",
        )
    })?;
    let origin = node_origin(resolved, value)
        .or_else(|| {
            resolved
                .symbol(binding)
                .and_then(|symbol| symbol.origin.clone())
        })
        .ok_or_else(|| internal_error("borrow binding lost its syntax origin", None))?;
    if let Some(move_origin) = typed.moved_binding_origin(owner).cloned() {
        return Err(TypecheckError::with_origin(
            TypecheckErrorKind::Ownership,
            "cannot borrow from an owner whose value was already moved",
            origin,
        )
        .with_related_origin(move_origin, "ownership moved here"));
    }
    reject_reborrow_source(typed, owner, origin.clone())?;

    if mutable
        && !typed
            .typed_symbol(owner)
            .is_some_and(|symbol| symbol.is_mutable)
    {
        return Err(TypecheckError::with_origin(
            TypecheckErrorKind::BorrowMutability,
            "mutable borrow requires an owner declared with 'var[mut]'",
            origin,
        ));
    }

    let borrow = ActiveBorrow {
        owner,
        binding,
        scope: context.scope_id,
        mutable,
        origin: origin.clone(),
    };
    if let Some(conflict) = typed.register_borrow(borrow) {
        return Err(TypecheckError::with_origin(
            TypecheckErrorKind::BorrowConflict,
            "borrow conflicts with an active mutable borrow of the same owner",
            origin,
        )
        .with_related_origin(conflict.origin, "conflicting borrow created here"));
    }

    let borrowed = typed
        .type_table_mut()
        .intern(CheckedType::Borrowed { inner, mutable });
    let symbol = typed
        .typed_symbol_mut(binding)
        .ok_or_else(|| internal_error("typed borrow binding disappeared", None))?;
    symbol.declared_type = Some(borrowed);
    Ok(borrowed)
}

pub(crate) fn track_value_transfer(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: Option<&AstNode>,
    actual_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    let Some(value) = value else {
        return Ok(());
    };
    match super::helpers::strip_comments(value) {
        AstNode::Identifier {
            syntax_id: Some(syntax_id),
            name,
        } => track_identifier_transfer(
            typed,
            resolved,
            context,
            value,
            *syntax_id,
            name,
            actual_type,
        ),
        AstNode::FieldAccess { object, field }
            if ownership_moves_on_transfer(typed, actual_type) =>
        {
            track_move_only_field_transfer(
                typed,
                resolved,
                context,
                value,
                object,
                field,
                actual_type,
            )
        }
        AstNode::IndexAccess { .. } if ownership_moves_on_transfer(typed, actual_type) => {
            reject_move_only_projection_transfer(resolved, value, "indexed projection")
        }
        _ => Ok(()),
    }
}

fn track_identifier_transfer(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: &AstNode,
    syntax_id: fol_parser::ast::SyntaxNodeId,
    name: &str,
    actual_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    if let Some(CheckedType::Borrowed { inner, .. }) = typed.type_table().get(actual_type) {
        if ownership_moves_on_transfer(typed, *inner) {
            let message = format!(
                "move-only value cannot be transferred out of borrow binding '{name}'"
            );
            return Err(node_origin(resolved, value).map_or_else(
                || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        message.clone(),
                        origin,
                    )
                },
            ));
        }
    }
    let eventual = matches!(
        typed.type_table().get(actual_type),
        Some(CheckedType::Eventual { .. })
    );
    let ownership_move = ownership_moves_on_transfer(typed, actual_type);
    if !eventual && !ownership_move {
        return Ok(());
    }
    let Some(reference) = resolved.references.iter().find(|reference| {
        reference.syntax_id == Some(syntax_id)
            && reference.kind == fol_resolver::ReferenceKind::Identifier
    }) else {
        return Ok(());
    };
    let Some(symbol) = reference.resolved else {
        return Ok(());
    };
    reject_repeated_outer_move(resolved, context, value, symbol, name)?;
    if let Some(origin) = node_origin(resolved, value) {
        if eventual {
            typed.mark_eventual_transferred(symbol, origin);
        } else {
            typed.mark_binding_moved(symbol, origin);
        }
    }
    Ok(())
}

fn track_move_only_field_transfer(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: &AstNode,
    object: &AstNode,
    field: &str,
    actual_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    let Some((root, syntax_id, name)) = projection_root_identifier(object) else {
        // A temporary record can surrender a move-only field directly; there
        // is no source binding that remains available afterward.
        return Ok(());
    };
    let symbol = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id)
                && reference.kind == fol_resolver::ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved);
    let root_symbol = symbol.and_then(|symbol| typed.typed_symbol(symbol));
    let borrowed_root = root_symbol
        .and_then(|symbol| symbol.declared_type)
        .is_some_and(|type_id| {
            matches!(
                typed.type_table().get(type_id),
                Some(CheckedType::Borrowed { .. })
            )
        });
    let mutex_root = root_symbol.is_some_and(|symbol| symbol.is_mutex);
    if borrowed_root || mutex_root {
        let surface = if borrowed_root {
            "borrowed"
        } else {
            "mutex-guarded"
        };
        let message = format!(
            "move-only field projection '.{field}' cannot be transferred from a {surface} value"
        );
        return Err(node_origin(resolved, value).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
            |origin| {
                TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
            },
        ));
    }

    // FOL does not expose a partially moved base in V3. Moving one field is
    // supported by consuming the entire root binding, which maps directly to
    // Rust field-move emission and keeps the remaining fields inaccessible.
    track_identifier_transfer(
        typed,
        resolved,
        context,
        root,
        syntax_id,
        name,
        actual_type,
    )
}

fn projection_root_identifier(node: &AstNode) -> Option<(&AstNode, fol_parser::ast::SyntaxNodeId, &str)> {
    match super::helpers::strip_comments(node) {
        AstNode::Identifier {
            syntax_id: Some(syntax_id),
            name,
        } => Some((node, *syntax_id, name.as_str())),
        AstNode::FieldAccess { object, .. } => projection_root_identifier(object),
        _ => None,
    }
}

fn reject_move_only_projection_transfer(
    resolved: &ResolvedProgram,
    value: &AstNode,
    surface: impl std::fmt::Display,
) -> Result<(), TypecheckError> {
    let message = format!(
        "move-only {surface} cannot be transferred in V3; partial moves are not supported"
    );
    Err(node_origin(resolved, value).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
        },
    ))
}

pub(crate) fn reject_repeated_outer_move(
    resolved: &ResolvedProgram,
    context: TypeContext,
    value: &AstNode,
    symbol: SymbolId,
    name: &str,
) -> Result<(), TypecheckError> {
    let Some(loop_scope) = context.repeating_loop_scope else {
        return Ok(());
    };
    let declaration = resolved.symbol(symbol).ok_or_else(|| {
        internal_error("resolved move-only binding disappeared before move checking", None)
    })?;
    let declared_inside_loop = std::iter::successors(Some(declaration.scope), |scope_id| {
        resolved.scope(*scope_id).and_then(|scope| scope.parent)
    })
    .any(|scope_id| scope_id == loop_scope);
    if declared_inside_loop {
        return Ok(());
    }

    let message = format!(
        "move-only binding '{name}' declared outside a repeating loop cannot be transferred from the loop body"
    );
    let error = node_origin(resolved, value).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
        },
    );
    Err(declaration.origin.clone().map_or(error.clone(), |origin| {
        error.with_related_origin(origin, "move-only binding declared here")
    }))
}

pub(crate) fn ownership_moves_on_transfer(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
) -> bool {
    ownership_moves_on_transfer_inner(typed, type_id, &mut BTreeSet::new())
}

fn ownership_moves_on_transfer_inner(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
    visiting: &mut BTreeSet<crate::CheckedTypeId>,
) -> bool {
    if !visiting.insert(type_id) {
        return false;
    }
    let moves = if let Some(apparent) = typed.apparent_type_override(type_id) {
        ownership_moves_on_transfer_inner(typed, apparent, visiting)
    } else {
        match typed.type_table().get(type_id) {
            Some(CheckedType::Owned { .. })
            | Some(CheckedType::Pointer { shared: false, .. })
            | Some(CheckedType::Eventual { .. })
            | Some(CheckedType::Channel { .. }) => true,
            Some(CheckedType::Declared {
                kind: crate::DeclaredTypeKind::GenericParameter,
                ..
            }) => {
                // The generic body cannot know whether a future actual is
                // copy-safe. Consume transfers conservatively here; call-site
                // checking still classifies the concrete argument type.
                true
            }
            Some(CheckedType::Declared { symbol, args, .. }) => {
                args.iter().any(|arg| {
                    ownership_moves_on_transfer_inner(typed, *arg, visiting)
                }) || typed
                    .typed_symbol(*symbol)
                    .and_then(|symbol| symbol.declared_type)
                    .is_some_and(|declared| {
                        ownership_moves_on_transfer_inner(typed, declared, visiting)
                    })
            }
            Some(CheckedType::Array { element_type, .. })
            | Some(CheckedType::Vector { element_type })
            | Some(CheckedType::Sequence { element_type }) => ownership_moves_on_transfer_inner(
                typed,
                *element_type,
                visiting,
            ),
            Some(CheckedType::Optional { inner })
            | Some(CheckedType::Error { inner: Some(inner) }) => {
                ownership_moves_on_transfer_inner(typed, *inner, visiting)
            }
            Some(CheckedType::Set { member_types }) => member_types.iter().any(|member| {
                ownership_moves_on_transfer_inner(typed, *member, visiting)
            }),
            Some(CheckedType::Map {
                key_type,
                value_type,
            }) => {
                ownership_moves_on_transfer_inner(typed, *key_type, visiting)
                    || ownership_moves_on_transfer_inner(typed, *value_type, visiting)
            }
            Some(CheckedType::Record { fields }) => fields.values().any(|field| {
                ownership_moves_on_transfer_inner(typed, *field, visiting)
            }),
            Some(CheckedType::Entry { variants }) => variants.values().flatten().any(|variant| {
                ownership_moves_on_transfer_inner(typed, *variant, visiting)
            }),
            Some(CheckedType::Builtin(_))
            | Some(CheckedType::Borrowed { .. })
            | Some(CheckedType::Pointer { shared: true, .. })
            | Some(CheckedType::ChannelSender { .. })
            | Some(CheckedType::Error { inner: None })
            | Some(CheckedType::Routine(_))
            | None => false,
        }
    };
    visiting.remove(&type_id);
    moves
}

pub(crate) fn type_record_init(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    fields: &[fol_parser::ast::RecordInitField],
    expected_type: Option<crate::CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    use super::helpers::{apparent_type_id, describe_type};
    use crate::CheckedType;

    let initializer_origin = fields
        .first()
        .and_then(|field| node_origin(resolved, &field.value));
    let Some(expected_type) = expected_type else {
        return Err(initializer_origin.clone().map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "record initializers require an expected record type in V1",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    "record initializers require an expected record type in V1",
                    origin,
                )
            },
        ));
    };
    let apparent = apparent_type_id(typed, expected_type)?;
    let Some(CheckedType::Record {
        fields: expected_fields,
    }) = typed.type_table().get(apparent)
    else {
        return Err(initializer_origin.clone().map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "record initializer requires a record expected type, got '{}'",
                        describe_type(typed, expected_type)
                    ),
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "record initializer requires a record expected type, got '{}'",
                        describe_type(typed, expected_type)
                    ),
                    origin,
                )
            },
        ));
    };
    let expected_fields = expected_fields.clone();
    let mut seen = BTreeSet::new();
    let mut field_effects = Vec::new();

    for field in fields {
        let field_origin = node_origin(resolved, &field.value);
        let Some(field_type) = expected_fields.get(&field.name).copied() else {
            return Err(field_origin.clone().map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!(
                            "record initializer does not define a field named '{}'",
                            field.name
                        ),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!(
                            "record initializer does not define a field named '{}'",
                            field.name
                        ),
                        origin,
                    )
                },
            ));
        };
        if !seen.insert(field.name.clone()) {
            return Err(field_origin.clone().map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!("record initializer repeats the field '{}'", field.name),
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::InvalidInput,
                        format!("record initializer repeats the field '{}'", field.name),
                        origin,
                    )
                },
            ));
        }
        let actual_expr =
            type_node_with_expectation(typed, resolved, context, &field.value, Some(field_type))
                .map_err(|error| {
                    field_origin
                        .clone()
                        .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                })?;
        reject_recoverable_error_shell_conversion(
            typed,
            field_type,
            &actual_expr,
            field_origin.clone(),
            format!("record field '{}'", field.name),
        )?;
        let actual_expr = plain_value_expr(
            typed,
            context,
            actual_expr,
            field_origin.clone(),
            format!("record field '{}'", field.name),
        )?;
        field_effects.push(actual_expr.recoverable_effect);
        let actual = actual_expr
            .required_value(format!(
                "record initializer field '{}' does not have a type",
                field.name
            ))
            .map_err(|_| {
                field_origin.clone().map_or_else(
                    || {
                        TypecheckError::new(
                            TypecheckErrorKind::InvalidInput,
                            format!(
                                "record initializer field '{}' does not have a type",
                                field.name
                            ),
                        )
                    },
                    |origin| {
                        TypecheckError::with_origin(
                            TypecheckErrorKind::InvalidInput,
                            format!(
                                "record initializer field '{}' does not have a type",
                                field.name
                            ),
                            origin,
                        )
                    },
                )
            })?;
        ensure_assignable(
            typed,
            field_type,
            actual,
            format!("record field '{}'", field.name),
            field_origin.clone(),
        )?;
        track_value_transfer(typed, resolved, context, Some(&field.value), actual)?;
    }

    // Fields carrying a declared default may be omitted; the default fills
    // them during lowering. Only fields without a default stay required.
    let defaulted: BTreeSet<String> = typed
        .record_layout(apparent)
        .map(|layout| {
            layout
                .iter()
                .filter(|field| field.default.is_some())
                .map(|field| field.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let missing = expected_fields
        .keys()
        .filter(|name| !seen.contains(*name) && !defaulted.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(initializer_origin.map_or_else(
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

    let merged = merge_recoverable_effects(
        typed,
        initializer_origin.clone(),
        "record initializer",
        field_effects,
    )?;
    Ok(TypedExpr::value(expected_type).with_optional_effect(merged))
}
