use crate::model::ActiveBorrow;
use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, UnaryOperator};
use fol_resolver::{ReferenceKind, ResolvedProgram, SymbolId};
use std::collections::BTreeSet;

use super::helpers::{
    ensure_assignable, find_symbol_in_scope_chain, internal_error, is_recoverable_eventual_type,
    merge_recoverable_effects, node_origin, plain_value_expr,
    reject_recoverable_error_shell_conversion, reject_recoverable_plain_use,
};
use super::type_node_with_expectation;
use super::{TypeContext, TypedExpr};

// Binding initialization needs the complete declaration mode in addition to
// the shared type context; retaining explicit booleans keeps call sites clear.
#[allow(clippy::too_many_arguments)]
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
        reject_unsupported_top_level_binding_type(
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
                reject_unsupported_top_level_binding_type(
                    typed,
                    resolved,
                    symbol_id,
                    borrowed,
                    binding_origin.clone(),
                )?;
                Ok(TypedExpr::value(borrowed))
            } else {
                // §2.2: transferring an existing owned value into a binding must
                // state the operation explicitly.
                if let Some(v) = value {
                    reject_untagged_owned_transfer(
                        typed,
                        resolved,
                        v,
                        actual,
                        &format!("transferred into '{name}'"),
                    )?;
                }
                track_value_transfer(typed, resolved, context, value, actual)?;
                register_recoverable_eventual_binding(
                    typed,
                    symbol_id,
                    actual,
                    context.scope_id,
                    binding_origin
                        .clone()
                        .or_else(|| value.and_then(|node| node_origin(resolved, node))),
                );
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
            reject_unsupported_top_level_binding_type(
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
                register_recoverable_eventual_binding(
                    typed,
                    symbol_id,
                    inferred,
                    context.scope_id,
                    binding_origin
                        .clone()
                        .or_else(|| value.and_then(|node| node_origin(resolved, node))),
                );
            }
            Ok(TypedExpr::value(inferred))
        }
        (Some(expected), None) => {
            reject_uninitialized_ownership_binding(name, borrowing, heap_owned, binding_origin)?;
            Ok(TypedExpr::value(expected))
        }
        (None, None) => {
            reject_uninitialized_ownership_binding(name, borrowing, heap_owned, binding_origin)?;
            Ok(TypedExpr::none())
        }
    }
}

/// A borrowed or heap-allocating declaration must identify its source at the
/// declaration site. `var[bor] view: T;` and `@var owned: T;` without an
/// initializer are rejected: a borrow has no owner to loan from, and a heap
/// allocation has no value to move or clone in. Only a plain `var[mut] slot: T;`
/// may be declared uninitialized, and definite-initialization then guards its
/// first use. This closes the Slice A soundness gap where an uninitialized
/// borrow binding silently became a default value in emitted Rust.
fn reject_uninitialized_ownership_binding(
    name: &str,
    borrowing: bool,
    heap_owned: bool,
    binding_origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let message = if borrowing {
        Some(format!(
            "borrow binding '{name}' requires an initializer; a 'var[bor]' declaration must identify the owner it loans from"
        ))
    } else if heap_owned {
        Some(format!(
            "heap-allocating binding '{name}' requires an initializer; '@var'/'[new]' must be given a value to move or clone into the allocation"
        ))
    } else {
        None
    };
    let Some(message) = message else {
        return Ok(());
    };
    Err(match binding_origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Uninitialized, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Uninitialized, message),
    })
}

pub(crate) fn register_recoverable_eventual_binding(
    typed: &mut TypedProgram,
    symbol: SymbolId,
    type_id: crate::CheckedTypeId,
    activation_scope: fol_resolver::ScopeId,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) {
    if !is_recoverable_eventual_type(typed, type_id) {
        return;
    }
    let Some(scope) = typed.typed_symbol(symbol).map(|symbol| symbol.scope_id) else {
        return;
    };
    typed.register_recoverable_eventual_obligation(symbol, scope, activation_scope, origin);
}

pub(crate) fn reject_recoverable_eventual_overwrite(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    symbol: SymbolId,
    overwrite_origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let Some(obligation) = typed.recoverable_eventual_obligation(symbol).cloned() else {
        return Ok(());
    };
    let name = resolved
        .symbol(symbol)
        .map(|symbol| symbol.name.as_str())
        .unwrap_or("<unknown>");
    let message = format!(
        "recoverable eventual binding '{name}' cannot be overwritten before it is awaited and handled with '||' or check(...)"
    );
    let primary = overwrite_origin
        .clone()
        .or_else(|| obligation.origin.clone());
    let mut error = primary.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::InvalidInput, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message.clone(), origin)
        },
    );
    if let (Some(overwrite_origin), Some(origin)) = (overwrite_origin.as_ref(), obligation.origin) {
        if overwrite_origin != &origin {
            error = error.with_related_origin(origin, "recoverable eventual created here");
        }
    }
    Err(error)
}

pub(crate) fn reject_unsupported_top_level_binding_type(
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
    let apparent = super::helpers::apparent_type_id(typed, type_id)?;
    if matches!(
        typed.type_table().get(apparent),
        Some(CheckedType::Channel { .. })
    ) {
        let message =
            "top-level channel bindings are not supported in V3; declare the channel inside its receiving routine";
        return Err(origin.map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
        ));
    }
    let message = if super::helpers::type_contains_fin(typed, type_id) {
        Some(
            "top-level bindings of a 'fin' type are forbidden in V3; a finalized value must be owned by a routine scope, not global storage",
        )
    } else if ownership_moves_on_transfer(typed, type_id) {
        Some(
            "top-level move-only bindings are not supported in V3; global loads cannot transfer unique ownership, so declare the value inside a routine",
        )
    } else if super::helpers::type_contains_borrowed(typed, type_id) {
        Some(
            "top-level bindings containing borrowed values are not supported in V3; global storage cannot preserve lexical borrow lifetimes",
        )
    } else if super::helpers::type_contains_shared_pointer(typed, type_id) {
        Some(
            "top-level bindings containing ptr[shared, T] are not supported in V3; Rc-backed values are not thread-safe global storage",
        )
    } else {
        None
    };
    let Some(message) = message else {
        return Ok(());
    };
    Err(origin.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
    ))
}

fn is_borrow_from_expression(node: &AstNode) -> bool {
    let node = super::helpers::strip_comments(node);
    if let AstNode::OwnershipOp { options, .. } = node {
        return options.contains(&fol_parser::ast::OwnershipOption::Borrow);
    }
    matches!(
        node,
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
        // The canonical borrow operation `[bor]owner` / `[mut, bor]owner` peels
        // to its owner operand, exactly like the legacy `#owner`.
        AstNode::OwnershipOp {
            options, operand, ..
        } if options.contains(&fol_parser::ast::OwnershipOption::Borrow) => operand.as_ref(),
        other => other,
    };
    // A borrow of a place rooted in a binding is a place-borrow of the whole
    // aggregate: `[bor]obj.field`, `[bor]slot[]`, and `[bor]arr[i]` all lock the
    // root binding while the borrow is live (V3_MEM §3.3 inner-place borrows).
    fn root_identifier(node: &AstNode) -> Option<(&AstNode, fol_parser::ast::SyntaxNodeId)> {
        match node {
            AstNode::Identifier {
                syntax_id: Some(syntax_id),
                ..
            } => Some((node, *syntax_id)),
            AstNode::FieldAccess { object, .. } => root_identifier(object),
            AstNode::IndexAccess { container, .. } => root_identifier(container),
            AstNode::PatternAccess { container, .. } => root_identifier(container),
            AstNode::Commented { node, .. } => root_identifier(node),
            _ => None,
        }
    }
    root_identifier(node)
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

/// Reborrowing is permitted (Slice C §5.2): a borrowed place may be borrowed
/// again, producing a child loan whose owner is the parent borrow binding. The
/// existing scope-stack machinery enforces the reborrow rules — the parent is
/// inaccessible while the child is active (owner-inaccessible check), a second
/// conflicting loan is rejected (register_borrow conflict check), and a reborrow
/// that ultimately roots in an owned local cannot escape the routine (the
/// transitive escaping-borrow check at `return`). Kept as a named seam so the
/// call sites read intentionally and future reborrow-specific rules have a home.
pub(crate) fn reject_reborrow_source(
    _typed: &TypedProgram,
    _symbol: SymbolId,
    _origin: fol_parser::ast::SyntaxOrigin,
) -> Result<(), TypecheckError> {
    Ok(())
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
                | Some(CheckedType::ChannelSender { element_type })
                | Some(CheckedType::ChannelReceiver { element_type }) => {
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
    // Only short-circuit a borrow of a bare identifier owner, whose borrowed type
    // is the owner's value type. A borrow through an accessor (`[bor]slot[]`,
    // `[bor]obj.field`) must be typed fully so the borrowed type is the accessed
    // place's type, not the whole aggregate's type.
    let operand = match super::helpers::strip_comments(value) {
        AstNode::UnaryOp {
            op: UnaryOperator::BorrowFrom,
            operand,
        } => operand.as_ref(),
        AstNode::OwnershipOp {
            options, operand, ..
        } if options.contains(&fol_parser::ast::OwnershipOption::Borrow) => operand.as_ref(),
        other => other,
    };
    if !matches!(
        super::helpers::strip_comments(operand),
        AstNode::Identifier { .. }
    ) {
        return None;
    }
    let symbol = borrow_source_symbol(resolved, value)?;
    let type_id = typed.typed_symbol(symbol)?.declared_type?;
    Some(Ok(TypedExpr::value(owned_or_borrowed_inner(
        typed, type_id,
    ))))
}

/// Collect the owner symbols of every explicit borrow argument (`#x` / `[bor]x`)
/// reachable inside a borrow-returning call initializer. A routine that returns
/// `T[bor=L]` may alias any of its `[bor=L]` inputs, so binding its result must
/// lock every owner the caller lent it (Slice C multi-owner borrows). Gated on
/// explicit borrow syntax so plain by-value/by-move arguments are not locked;
/// recurses through nested calls so a borrow threaded through an inner call is
/// still captured.
fn collect_call_borrow_owners(resolved: &ResolvedProgram, node: &AstNode, out: &mut Vec<SymbolId>) {
    let node = super::helpers::strip_comments(node);
    let is_explicit_borrow = matches!(
        node,
        AstNode::UnaryOp {
            op: UnaryOperator::BorrowFrom,
            ..
        }
    ) || matches!(
        node,
        AstNode::OwnershipOp { options, .. }
            if options.contains(&fol_parser::ast::OwnershipOption::Borrow)
    );
    if is_explicit_borrow {
        if let Some(owner) = borrow_source_symbol(resolved, node) {
            if !out.contains(&owner) {
                out.push(owner);
            }
        }
        return;
    }
    match node {
        AstNode::FunctionCall { args, .. } | AstNode::QualifiedFunctionCall { args, .. } => {
            for arg in args {
                collect_call_borrow_owners(resolved, arg, out);
            }
        }
        AstNode::MethodCall { object, args, .. } => {
            collect_call_borrow_owners(resolved, object, out);
            for arg in args {
                collect_call_borrow_owners(resolved, arg, out);
            }
        }
        AstNode::NamedArgument { value, .. } => {
            collect_call_borrow_owners(resolved, value, out);
        }
        _ => {}
    }
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
    let origin = node_origin(resolved, value)
        .or_else(|| {
            resolved
                .symbol(binding)
                .and_then(|symbol| symbol.origin.clone())
        })
        .ok_or_else(|| internal_error("borrow binding lost its syntax origin", None))?;

    // The initializer is either a direct owner (`#owner` / an identifier) or a
    // borrow-returning call whose lent-in owners must all be locked.
    let owners = match borrow_source_symbol(resolved, value) {
        Some(owner) => vec![owner],
        None => {
            let mut owners = Vec::new();
            collect_call_borrow_owners(resolved, value, &mut owners);
            if owners.is_empty() {
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::InvalidInput,
                    "borrow bindings require an identifier owner, '[bor]owner', or a borrow-returning call",
                    origin,
                ));
            }
            owners
        }
    };

    for owner in &owners {
        if let Some(move_origin) = typed.moved_binding_origin(*owner).cloned() {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::Ownership,
                "cannot borrow from an owner whose value was already moved",
                origin,
            )
            .with_related_origin(move_origin, "ownership moved here"));
        }
        reject_reborrow_source(typed, *owner, origin.clone())?;

        // A `mux[T]` owner is interior-mutable: `([bor]state).lock()` yields a
        // mutable guard over `T` without the handle being `var[mut]` (V3_MEM
        // §8.3). Exempt mutex owners from the mutable-borrow `var[mut]` rule.
        let owner_is_mutex = typed
            .typed_symbol(*owner)
            .is_some_and(|symbol| symbol.is_mutex);
        if mutable
            && !typed
                .typed_symbol(*owner)
                .is_some_and(|symbol| symbol.is_mutable || symbol.is_mutex)
        {
            return Err(TypecheckError::with_origin(
                TypecheckErrorKind::BorrowMutability,
                "mutable borrow requires an owner declared with 'var[mut]'",
                origin,
            ));
        }
        // Capturing a `mux[T]` lock into a `var[mut, bor]` binding is the
        // lifetime-scoped guard-VALUE form: promote the active lock so it is
        // rejected from crossing spawn/await/blocking-receive/blocking-select
        // while live (V3_MEM §8.3). The handle-lock form stays unbound. The
        // binding itself is also flagged so the whole guard cannot be moved,
        // copied, or cloned as a value.
        if mutable && owner_is_mutex {
            typed.mark_guard_bound(*owner);
            if let Some(guard_symbol) = typed.typed_symbol_mut(binding) {
                guard_symbol.is_mutex_guard = true;
            }
        }
    }

    for owner in &owners {
        let borrow = ActiveBorrow {
            owner: *owner,
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
            let message =
                format!("move-only value cannot be transferred out of borrow binding '{name}'");
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

    // Static-place partial move (Slice C §3.1): transferring a single named
    // field of a direct owned binding invalidates only that field. The rest of
    // the aggregate stays readable, matching the Rust field-move emission.
    let object_is_direct_identifier = matches!(
        super::helpers::strip_comments(object),
        AstNode::Identifier { .. }
    );
    if object_is_direct_identifier {
        if let (Some(symbol), Some(origin)) = (symbol, node_origin(resolved, value)) {
            reject_repeated_outer_move(resolved, context, value, symbol, name)?;
            // A `fin` value must be moved whole-value; a partial field move is
            // rejected because its finalizer requires the complete value (plan
            // §3). The whole-value move path (below) remains allowed.
            if root_binding_type_claims_fin(typed, symbol) {
                let message = format!(
                    "cannot partially move field '.{field}' out of the 'fin' value '{name}'; a finalized value must be moved whole so its finalizer sees the complete value"
                );
                return Err(TypecheckError::with_origin(
                    TypecheckErrorKind::Ownership,
                    message,
                    origin,
                ));
            }
            typed.mark_field_moved(symbol, field, origin);
            return Ok(());
        }
    }
    // A deeper projection (`base.inner.field`) has no single-level place we can
    // track precisely; conservatively consume the entire root binding.
    track_identifier_transfer(typed, resolved, context, root, syntax_id, name, actual_type)
}

/// Whether the binding's declared type is (or peels/resolves to) a type that
/// claims custom finalization, so a partial move out of it must be rejected.
fn root_binding_type_claims_fin(typed: &TypedProgram, symbol: SymbolId) -> bool {
    typed
        .typed_symbol(symbol)
        .and_then(|symbol| symbol.declared_type)
        .is_some_and(|type_id| typed.type_resolves_to_fin(type_id))
}

fn projection_root_identifier(
    node: &AstNode,
) -> Option<(&AstNode, fol_parser::ast::SyntaxNodeId, &str)> {
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
    let message =
        format!("move-only {surface} cannot be transferred in V3; partial moves are not supported");
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
        internal_error(
            "resolved move-only binding disappeared before move checking",
            None,
        )
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

/// §2.2: reject an untagged transfer of a CONCRETE move-only value at a transfer
/// boundary (assignment, return, call argument, ...). Fires only for a bare
/// identifier whose type moves on transfer and is not a generic parameter —
/// generic-`T` transfers are conditional obligations tagged at the concrete call
/// site. `describe` names the boundary in the diagnostic.
pub(crate) fn reject_untagged_owned_transfer(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    value: &AstNode,
    actual: crate::CheckedTypeId,
    describe: &str,
) -> Result<(), TypecheckError> {
    if matches!(
        super::helpers::strip_comments(value),
        AstNode::Identifier { .. }
    ) && ownership_moves_on_transfer(typed, actual)
        && !crate::decls::checked_type_contains_generic_param(typed, actual)
    {
        let message = format!(
            "an owned value {describe} must state its operation: use '[mov]', '[cpy]', or '[cln]'"
        );
        return Err(node_origin(resolved, value).map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
            |origin| {
                TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
            },
        ));
    }
    Ok(())
}

/// Whether transferring a checked value consumes its source.
///
/// Lowering also uses this compiler-owned classification when an operation's
/// value category cannot be recovered faithfully from structural lowered
/// types alone (notably nominal pointer pointees).
pub fn ownership_moves_on_transfer(typed: &TypedProgram, type_id: crate::CheckedTypeId) -> bool {
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
    let moves = if typed.type_claims_fin(type_id) {
        // A `fin` value owns a finalizable resource; it is affine and must move
        // on transfer, never duplicate — regardless of how copy-safe its fields
        // look structurally (plan §4.1: `copy` and `fin` cannot coexist).
        true
    } else if let Some(apparent) = typed.apparent_type_override(type_id) {
        ownership_moves_on_transfer_inner(typed, apparent, visiting)
    } else {
        match typed.type_table().get(type_id) {
            Some(CheckedType::Owned { .. })
            // A unique `ptr[T]` (`shared: false, weak: false`) is the sole,
            // move-only owner. A `ptr[weak, T]` is clone-safe like the shared
            // `Rc`/`Arc` it observes (`std::rc::Weak`/`sync::Weak` are `Clone`
            // and never keep the pointee alive), so it is NOT affine here.
            | Some(CheckedType::Pointer { shared: false, weak: false, .. })
            | Some(CheckedType::Eventual { .. })
            | Some(CheckedType::Channel { .. })
            // A `chn[rx, T]` receiver is unique: it is affine and always moves,
            // never clones (unlike the clone-capable `chn[tx, T]` sender).
            | Some(CheckedType::ChannelReceiver { .. }) => true,
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
                args.iter()
                    .any(|arg| ownership_moves_on_transfer_inner(typed, *arg, visiting))
                    || typed
                        .typed_symbol(*symbol)
                        .and_then(|symbol| symbol.declared_type)
                        .is_some_and(|declared| {
                            ownership_moves_on_transfer_inner(typed, declared, visiting)
                        })
            }
            Some(CheckedType::Array { element_type, .. })
            | Some(CheckedType::Vector { element_type })
            | Some(CheckedType::Sequence { element_type }) => {
                ownership_moves_on_transfer_inner(typed, *element_type, visiting)
            }
            Some(CheckedType::Optional { inner })
            | Some(CheckedType::Error { inner: Some(inner) }) => {
                ownership_moves_on_transfer_inner(typed, *inner, visiting)
            }
            Some(CheckedType::Set { member_types }) => member_types
                .iter()
                .any(|member| ownership_moves_on_transfer_inner(typed, *member, visiting)),
            Some(CheckedType::Map {
                key_type,
                value_type,
            }) => {
                ownership_moves_on_transfer_inner(typed, *key_type, visiting)
                    || ownership_moves_on_transfer_inner(typed, *value_type, visiting)
            }
            Some(CheckedType::Record { fields }) => fields
                .values()
                .any(|field| ownership_moves_on_transfer_inner(typed, *field, visiting)),
            Some(CheckedType::Entry { variants }) => variants
                .values()
                .flatten()
                .any(|variant| ownership_moves_on_transfer_inner(typed, *variant, visiting)),
            Some(CheckedType::Builtin(_))
            | Some(CheckedType::Borrowed { .. })
            | Some(CheckedType::Pointer { shared: true, .. })
            // A `ptr[weak, T]` observer is clone-safe (never keeps the pointee
            // alive), so it is not affine — like the shared pointer it observes.
            | Some(CheckedType::Pointer { shared: false, weak: true, .. })
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
        // §2.2: initializing a record field from an existing owned value is a
        // transfer boundary and must state its operation explicitly.
        reject_untagged_owned_transfer(
            typed,
            resolved,
            &field.value,
            actual,
            &format!("assigned to field '{}'", field.name),
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
