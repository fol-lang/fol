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
                mark_plain_identifier_move(typed, resolved, value, actual)?;
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
            let inferred_view = owned_or_borrowed_inner(typed, inferred);
            let inferred = if borrowing {
                register_borrow_binding(
                    typed,
                    resolved,
                    context,
                    symbol_id,
                    value,
                    inferred_view,
                    mutable_borrow,
                )?
            } else if heap_owned {
                typed
                    .type_table_mut()
                    .intern(CheckedType::Owned { inner: inferred })
            } else {
                inferred
            };
            let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
                internal_error("typed symbol table lost an inferred binding", None)
            })?;
            symbol.declared_type = Some(inferred);
            if !borrowing {
                mark_plain_identifier_move(
                    typed,
                    resolved,
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

fn borrow_source_symbol(resolved: &ResolvedProgram, node: &AstNode) -> Option<SymbolId> {
    let (_, syntax_id) = borrow_source_identifier(node)?;
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id)
                && reference.kind == ReferenceKind::Identifier
        })?
        .resolved
}

fn owned_or_borrowed_inner(
    typed: &TypedProgram,
    type_id: crate::CheckedTypeId,
) -> crate::CheckedTypeId {
    match typed.type_table().get(type_id) {
        Some(CheckedType::Owned { inner }) | Some(CheckedType::Borrowed { inner, .. }) => *inner,
        _ => type_id,
    }
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
        .or_else(|| resolved.symbol(binding).and_then(|symbol| symbol.origin.clone()))
        .ok_or_else(|| internal_error("borrow binding lost its syntax origin", None))?;

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

pub(crate) fn mark_plain_identifier_move(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    value: Option<&AstNode>,
    actual_type: crate::CheckedTypeId,
) -> Result<(), TypecheckError> {
    let Some(AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    }) = value
    else {
        return Ok(());
    };
    if !matches!(
        typed.type_table().get(actual_type),
        Some(CheckedType::Owned { .. }) | Some(CheckedType::Pointer { shared: false, .. })
    ) {
        return Ok(());
    }
    let Some(reference) = resolved.references.iter().find(|reference| {
        reference.syntax_id == Some(*syntax_id)
            && reference.kind == fol_resolver::ReferenceKind::Identifier
    }) else {
        return Ok(());
    };
    let Some(symbol) = reference.resolved else {
        return Ok(());
    };
    if let Some(origin) = node_origin(resolved, value.expect("identifier value exists")) {
        typed.mark_binding_moved(symbol, origin);
    }
    Ok(())
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
        mark_plain_identifier_move(typed, resolved, Some(&field.value), actual)?;
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
