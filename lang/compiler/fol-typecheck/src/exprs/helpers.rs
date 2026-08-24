use crate::{
    CheckedType, CheckedTypeId, RecoverableCallEffect, TypecheckError, TypecheckErrorKind,
    TypedProgram,
};
use fol_parser::ast::{AstNode, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use fol_types::{FloatWidth, IntWidth};
use std::collections::BTreeSet;

use super::{ErrorCallMode, TypeContext, TypedExpr};

pub(crate) fn require_direct_channel_binding(
    resolved: &ResolvedProgram,
    reference_scope: ScopeId,
    channel: &AstNode,
) -> Result<(), TypecheckError> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(channel)
    else {
        return Err(with_node_origin(
            resolved,
            channel,
            TypecheckErrorKind::Unsupported,
            "channel endpoint access requires a direct local, parameter, or capture binding in V3; projected fields and container elements are not supported",
        ));
    };
    let Some(symbol) = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
        .and_then(|symbol| resolved.symbol(symbol))
    else {
        return Err(with_node_origin(
            resolved,
            channel,
            TypecheckErrorKind::Unsupported,
            "channel endpoint access requires a resolved routine-local binding in V3",
        ));
    };
    let local_kind = matches!(
        symbol.kind,
        SymbolKind::ValueBinding
            | SymbolKind::LabelBinding
            | SymbolKind::DestructureBinding
            | SymbolKind::Parameter
            | SymbolKind::Capture
            | SymbolKind::LoopBinder
            | SymbolKind::RollingBinder
    );
    let nearest_routine = |mut scope: Option<ScopeId>| {
        while let Some(scope_id) = scope {
            let resolved_scope = resolved.scope(scope_id)?;
            if matches!(resolved_scope.kind, fol_resolver::ScopeKind::Routine) {
                return Some(scope_id);
            }
            scope = resolved_scope.parent;
        }
        None
    };
    let symbol_routine = nearest_routine(Some(symbol.scope));
    let reference_routine = nearest_routine(Some(reference_scope));
    if local_kind && symbol_routine.is_some() && symbol_routine == reference_routine {
        return Ok(());
    }
    Err(with_node_origin(
        resolved,
        channel,
        TypecheckErrorKind::Unsupported,
        "channel endpoint access requires a direct binding owned by the current routine in V3; outer-routine and global channel values are not supported",
    ))
}

pub(crate) fn type_embeds_full_channel(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    fn embeds(
        typed: &TypedProgram,
        type_id: CheckedTypeId,
        root: bool,
        visiting: &mut BTreeSet<CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let result = if let Some(apparent) = typed.apparent_type_override(type_id) {
            embeds(typed, apparent, root, visiting)
        } else {
            match typed.type_table().get(type_id) {
                Some(CheckedType::Channel { element_type }) => {
                    !root || embeds(typed, *element_type, false, visiting)
                }
                Some(CheckedType::ChannelSender { element_type })
                | Some(CheckedType::ChannelReceiver { element_type }) => {
                    embeds(typed, *element_type, false, visiting)
                }
                Some(CheckedType::Declared { symbol, args, .. }) => {
                    args.iter().any(|arg| embeds(typed, *arg, false, visiting))
                        || typed
                            .typed_symbol(*symbol)
                            .and_then(|symbol| symbol.declared_type)
                            .is_some_and(|declared| embeds(typed, declared, root, visiting))
                }
                Some(CheckedType::Record { fields }) => fields
                    .values()
                    .any(|field| embeds(typed, *field, false, visiting)),
                Some(CheckedType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| embeds(typed, *variant, false, visiting)),
                Some(CheckedType::Array { element_type, .. })
                | Some(CheckedType::Vector { element_type })
                | Some(CheckedType::Sequence { element_type }) => {
                    embeds(typed, *element_type, false, visiting)
                }
                Some(CheckedType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| embeds(typed, *member, false, visiting)),
                Some(CheckedType::Map {
                    key_type,
                    value_type,
                }) => {
                    embeds(typed, *key_type, false, visiting)
                        || embeds(typed, *value_type, false, visiting)
                }
                Some(CheckedType::Optional { inner })
                | Some(CheckedType::Owned { inner })
                | Some(CheckedType::Borrowed { inner, .. })
                | Some(CheckedType::Pointer { target: inner, .. }) => {
                    embeds(typed, *inner, false, visiting)
                }
                Some(CheckedType::Error { inner }) => {
                    inner.is_some_and(|inner| embeds(typed, inner, false, visiting))
                }
                // An `evt[T]` is a scope-bound one-shot handle: like a full
                // channel, it cannot be embedded in an aggregate that could
                // outlive its parent scope `L` (V3_MEM §8.1). Conservative
                // until region-scoped storage proofs land.
                Some(CheckedType::Eventual {
                    value_type,
                    error_type,
                }) => {
                    !root
                        || embeds(typed, *value_type, false, visiting)
                        || error_type.is_some_and(|error| embeds(typed, error, false, visiting))
                }
                // A routine type is a wrapper too: a full channel or eventual
                // in its parameters or results would ride the routine value
                // past frame boundaries.
                Some(CheckedType::Routine(signature)) => {
                    let signature = signature.clone();
                    signature
                        .params
                        .iter()
                        .copied()
                        .chain(signature.return_type)
                        .chain(signature.error_type)
                        .any(|part| embeds(typed, part, false, visiting))
                }
                // An opaque handle embeds nothing: it is one address.
                Some(CheckedType::ForeignHandle { .. }) | Some(CheckedType::Builtin(_)) | None => {
                    false
                }
            }
        };
        visiting.remove(&type_id);
        result
    }

    embeds(typed, type_id, true, &mut BTreeSet::new())
}

pub(crate) fn reject_embedded_full_channel(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    if !type_embeds_full_channel(typed, type_id) {
        return Ok(());
    }
    let message = "full chn[T] and evt[T] values cannot be embedded in aggregate or wrapper types in V3; keep them as direct routine-local bindings or named-routine parameters";
    Err(origin.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
    ))
}

pub(crate) fn type_contains_shared_pointer(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    fn contains(
        typed: &TypedProgram,
        type_id: CheckedTypeId,
        visiting: &mut BTreeSet<CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let result = if let Some(apparent) = typed.apparent_type_override(type_id) {
            contains(typed, apparent, visiting)
        } else {
            match typed.type_table().get(type_id) {
                // `Rc`-backed (non-sync) shared pointers AND their weak
                // observers (`std::rc::Weak`) block a boundary crossing; the
                // `Arc`-backed sync forms are thread-safe and may cross (their
                // target is still checked recursively, so a sync pointer
                // wrapping an `Rc`-family value stays blocked).
                Some(CheckedType::Pointer {
                    sync: false,
                    shared,
                    weak,
                    ..
                }) if *shared || *weak => true,
                Some(CheckedType::Pointer { target, .. }) => contains(typed, *target, visiting),
                Some(CheckedType::Declared { symbol, args, .. }) => {
                    args.iter().any(|arg| contains(typed, *arg, visiting))
                        || typed
                            .typed_symbol(*symbol)
                            .and_then(|symbol| symbol.declared_type)
                            .is_some_and(|declared| contains(typed, declared, visiting))
                }
                Some(CheckedType::Record { fields }) => fields
                    .values()
                    .any(|field| contains(typed, *field, visiting)),
                Some(CheckedType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| contains(typed, *variant, visiting)),
                Some(CheckedType::Array { element_type, .. })
                | Some(CheckedType::Vector { element_type })
                | Some(CheckedType::Sequence { element_type })
                | Some(CheckedType::Channel { element_type })
                | Some(CheckedType::ChannelSender { element_type })
                | Some(CheckedType::ChannelReceiver { element_type }) => {
                    contains(typed, *element_type, visiting)
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
                Some(CheckedType::Optional { inner })
                | Some(CheckedType::Owned { inner })
                | Some(CheckedType::Borrowed { inner, .. }) => contains(typed, *inner, visiting),
                Some(CheckedType::Error { inner }) => {
                    inner.is_some_and(|inner| contains(typed, inner, visiting))
                }
                Some(CheckedType::Eventual {
                    value_type,
                    error_type,
                }) => {
                    contains(typed, *value_type, visiting)
                        || error_type.is_some_and(|error| contains(typed, error, visiting))
                }
                // An opaque handle is one address: nothing is nested inside it.
                Some(CheckedType::ForeignHandle { .. })
                | Some(CheckedType::Builtin(_))
                | Some(CheckedType::Routine(_))
                | None => false,
            }
        };
        visiting.remove(&type_id);
        result
    }

    contains(typed, type_id, &mut BTreeSet::new())
}

pub(crate) fn type_contains_borrowed(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    fn contains(
        typed: &TypedProgram,
        type_id: CheckedTypeId,
        visiting: &mut BTreeSet<CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let result = if let Some(apparent) = typed.apparent_type_override(type_id) {
            contains(typed, apparent, visiting)
        } else {
            match typed.type_table().get(type_id) {
                Some(CheckedType::Borrowed { .. }) => true,
                Some(CheckedType::Declared { symbol, args, .. }) => {
                    args.iter().any(|arg| contains(typed, *arg, visiting))
                        || typed
                            .typed_symbol(*symbol)
                            .and_then(|symbol| symbol.declared_type)
                            .is_some_and(|declared| contains(typed, declared, visiting))
                }
                Some(CheckedType::Record { fields }) => fields
                    .values()
                    .any(|field| contains(typed, *field, visiting)),
                Some(CheckedType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| contains(typed, *variant, visiting)),
                Some(CheckedType::Array { element_type, .. })
                | Some(CheckedType::Vector { element_type })
                | Some(CheckedType::Sequence { element_type })
                | Some(CheckedType::Channel { element_type })
                | Some(CheckedType::ChannelSender { element_type })
                | Some(CheckedType::ChannelReceiver { element_type }) => {
                    contains(typed, *element_type, visiting)
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
                Some(CheckedType::Optional { inner })
                | Some(CheckedType::Owned { inner })
                | Some(CheckedType::Pointer { target: inner, .. }) => {
                    contains(typed, *inner, visiting)
                }
                Some(CheckedType::Error { inner }) => {
                    inner.is_some_and(|inner| contains(typed, inner, visiting))
                }
                Some(CheckedType::Eventual {
                    value_type,
                    error_type,
                }) => {
                    contains(typed, *value_type, visiting)
                        || error_type.is_some_and(|error| contains(typed, error, visiting))
                }
                // An opaque handle is one address: nothing is nested inside it.
                Some(CheckedType::ForeignHandle { .. })
                | Some(CheckedType::Builtin(_))
                | Some(CheckedType::Routine(_))
                | None => false,
            }
        };
        visiting.remove(&type_id);
        result
    }

    contains(typed, type_id, &mut BTreeSet::new())
}

/// Whether `type_id` is (after peeling apparent-type overrides) an eventual
/// handle. Used to forbid eventuals from entering detached tasks (V3_MEM §8.1).
pub(crate) fn type_is_eventual(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    let resolved = typed.apparent_type_override(type_id).unwrap_or(type_id);
    matches!(
        typed.type_table().get(resolved),
        Some(CheckedType::Eventual { .. })
    )
}

/// Whether `type_id` is, or transitively contains a field/element of, a type
/// that claims custom finalization (`fin`). Used to forbid `fin` values in
/// positions that never run finalization, such as top-level/global storage.
pub(crate) fn type_contains_fin(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    fn contains(
        typed: &TypedProgram,
        type_id: CheckedTypeId,
        visiting: &mut BTreeSet<CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let result = if typed.type_claims_fin(type_id) {
            true
        } else if let Some(apparent) = typed.apparent_type_override(type_id) {
            contains(typed, apparent, visiting)
        } else {
            match typed.type_table().get(type_id) {
                Some(CheckedType::Declared { symbol, args, .. }) => {
                    args.iter().any(|arg| contains(typed, *arg, visiting))
                        || typed
                            .typed_symbol(*symbol)
                            .and_then(|symbol| symbol.declared_type)
                            .is_some_and(|declared| contains(typed, declared, visiting))
                }
                Some(CheckedType::Record { fields }) => fields
                    .values()
                    .any(|field| contains(typed, *field, visiting)),
                Some(CheckedType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| contains(typed, *variant, visiting)),
                Some(CheckedType::Array { element_type, .. })
                | Some(CheckedType::Vector { element_type })
                | Some(CheckedType::Sequence { element_type }) => {
                    contains(typed, *element_type, visiting)
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
                Some(CheckedType::Optional { inner }) | Some(CheckedType::Owned { inner }) => {
                    contains(typed, *inner, visiting)
                }
                _ => false,
            }
        };
        visiting.remove(&type_id);
        result
    }

    contains(typed, type_id, &mut BTreeSet::new())
}

/// Whether `type_id` stores a `fin` value *inside* another value — a record
/// field, container element, shell payload or type argument — as opposed to
/// being the finalized value itself. Finalization is registered per directly
/// owned binding or parameter, so a contained `fin` value has no owner that
/// would ever call its finalizer.
pub(crate) fn type_has_nested_fin(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    let mut current = type_id;
    let mut peeled = BTreeSet::new();
    loop {
        if !peeled.insert(current) {
            return false;
        }
        if let Some(apparent) = typed.apparent_type_override(current) {
            current = apparent;
            continue;
        }
        match typed.type_table().get(current) {
            Some(CheckedType::Declared { symbol, args, .. }) => {
                if args.iter().any(|arg| type_contains_fin(typed, *arg)) {
                    return true;
                }
                match typed
                    .typed_symbol(*symbol)
                    .and_then(|symbol| symbol.declared_type)
                {
                    Some(declared) => current = declared,
                    None => return false,
                }
            }
            // A record field is finalizable: lowering walks the field path at
            // scope exit and calls the field's own finalizer before the holder
            // dies. Only a field that *itself* hides an unreachable `fin`
            // deeper down is a problem, so recurse with this same rule.
            Some(CheckedType::Record { fields }) => {
                let fields = fields.clone();
                return fields.values().any(|field| {
                    !typed.type_resolves_to_fin(*field) && type_has_nested_fin(typed, *field)
                });
            }
            Some(CheckedType::Entry { variants }) => {
                return variants
                    .values()
                    .flatten()
                    .any(|variant| type_contains_fin(typed, *variant))
            }
            // A container whose elements are `fin` is finalized by iterating it
            // at scope exit. One that merely *contains* a `fin` deeper down is
            // not: reaching those would need a field walk per element, which
            // scope-exit iteration does not express.
            Some(CheckedType::Array { element_type, .. })
            | Some(CheckedType::Vector { element_type })
            | Some(CheckedType::Sequence { element_type }) => {
                let element_type = *element_type;
                return !typed.type_resolves_to_fin(element_type)
                    && type_contains_fin(typed, element_type);
            }
            Some(CheckedType::Set { member_types }) => {
                let member_types = member_types.clone();
                return member_types.iter().any(|member| {
                    !typed.type_resolves_to_fin(*member) && type_contains_fin(typed, *member)
                });
            }
            Some(CheckedType::Map {
                key_type,
                value_type,
            }) => {
                let (key_type, value_type) = (*key_type, *value_type);
                return [key_type, value_type].iter().any(|part| {
                    !typed.type_resolves_to_fin(*part) && type_contains_fin(typed, *part)
                });
            }
            Some(CheckedType::Optional { inner }) | Some(CheckedType::Owned { inner }) => {
                let inner = *inner;
                return !typed.type_resolves_to_fin(inner) && type_contains_fin(typed, inner);
            }
            _ => return false,
        }
    }
}

/// Reject a storage position that would swallow a nested `fin` value. Silently
/// skipping a finalizer is worse than refusing the program: the source reads as
/// if the resource is released (V3_MEM §6).
pub(crate) fn reject_nested_fin_storage(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    origin: Option<SyntaxOrigin>,
    subject: &str,
) -> Result<(), TypecheckError> {
    if !type_has_nested_fin(typed, type_id) {
        return Ok(());
    }
    let message = format!(
        "{subject} holds a 'fin' value buried inside '{}'; a 'fin' binding, record field, or container *element* is finalized at scope exit, but reaching one nested deeper than that — inside a container's element, an entry variant, or a generic argument — would need a per-element walk the compiler cannot emit, so the finalizer would never run — hold the 'fin' value directly, or give it its own binding and transfer it with '[mov]'",
        describe_type(typed, type_id)
    );
    Err(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    })
}

pub(crate) fn observe_context(context: TypeContext) -> TypeContext {
    TypeContext {
        error_call_mode: ErrorCallMode::Observe,
        ..context
    }
}

pub(crate) fn reject_recoverable_plain_use(
    origin: Option<SyntaxOrigin>,
    usage: impl Into<String>,
) -> Result<(), TypecheckError> {
    let usage = usage.into();
    let message = format!(
        "{usage} cannot use '/ ErrorType' routine results as plain values in V1; handle them immediately with '||' or check(...), or use err[...] when you need a storable value"
    );
    Err(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
    })
}

pub(crate) fn is_recoverable_eventual_type(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Eventual {
            error_type: Some(_),
            ..
        })
    )
}

pub(crate) fn reject_discarded_recoverable_eventual(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    if !is_recoverable_eventual_type(typed, type_id) {
        return Ok(());
    }
    let message = "discarding a recoverable eventual loses its error; bind it, await it, and handle the result with '||' or check(...)";
    Err(origin.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
    ))
}

fn recoverable_eventual_exit_error(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    predicate: impl Fn(&crate::model::RecoverableEventualObligation) -> bool,
    exit_origin: Option<SyntaxOrigin>,
    boundary: &str,
) -> Result<(), TypecheckError> {
    let outstanding = typed
        .recoverable_eventual_obligations()
        .find(|(_, obligation)| predicate(obligation))
        .map(|(symbol, obligation)| (symbol, obligation.clone()));
    let Some((symbol, obligation)) = outstanding else {
        return Ok(());
    };
    let name = resolved
        .symbol(symbol)
        .map(|symbol| symbol.name.as_str())
        .unwrap_or("<unknown>");
    let message = format!(
        "recoverable eventual binding '{name}' must be awaited and handled with '||' or check(...) before {boundary}"
    );
    let primary = exit_origin.clone().or_else(|| obligation.origin.clone());
    let mut error = primary.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::InvalidInput, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message.clone(), origin)
        },
    );
    if let (Some(exit_origin), Some(origin)) = (exit_origin.as_ref(), obligation.origin) {
        if exit_origin != &origin {
            error = error.with_related_origin(origin, "recoverable eventual created here");
        }
    }
    Err(error)
}

/// Reject crossing a processor boundary while a lifetime-scoped mutex guard
/// value is live (V3_MEM §8.3: "the guard cannot ... cross spawn, await,
/// blocking receive, or blocking select"). Only the guard-VALUE form
/// (`var[mut, bor] guard = ([bor]mux).lock()`) is bound; the handle-lock form
/// may cross for concurrent access.
pub(crate) fn reject_bound_guard_boundary(
    typed: &TypedProgram,
    boundary: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let Some(guard) = typed.active_bound_guard() else {
        return Ok(());
    };
    let message = format!(
        "a mutex guard cannot cross {boundary}; end it ('[end]guard' or scope exit) before {boundary}"
    );
    let error = match origin {
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin),
        None => TypecheckError::new(TypecheckErrorKind::Ownership, message),
    };
    Err(error.with_related_origin(guard.origin.clone(), "mutex guard acquired here"))
}

pub(crate) fn reject_recoverable_eventuals_in_scope(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    scope: ScopeId,
) -> Result<(), TypecheckError> {
    recoverable_eventual_exit_error(
        typed,
        resolved,
        |obligation| obligation.owner_scope == scope,
        None,
        "leaving its lexical scope",
    )
}

pub(crate) fn nearest_routine_scope(
    resolved: &ResolvedProgram,
    mut scope: ScopeId,
) -> Option<ScopeId> {
    loop {
        let resolved_scope = resolved.scope(scope)?;
        if resolved_scope.kind == fol_resolver::ScopeKind::Routine {
            return Some(scope);
        }
        scope = resolved_scope.parent?;
    }
}

pub(crate) fn reject_all_recoverable_eventuals(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    current_scope: ScopeId,
    exit_origin: Option<SyntaxOrigin>,
    boundary: &str,
) -> Result<(), TypecheckError> {
    let current_routine = nearest_routine_scope(resolved, current_scope);
    recoverable_eventual_exit_error(
        typed,
        resolved,
        |obligation| nearest_routine_scope(resolved, obligation.owner_scope) == current_routine,
        exit_origin,
        boundary,
    )
}

pub(crate) fn reject_recoverable_eventuals_leaving_scope(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    ancestor: ScopeId,
    exit_origin: Option<SyntaxOrigin>,
    boundary: &str,
) -> Result<(), TypecheckError> {
    recoverable_eventual_exit_error(
        typed,
        resolved,
        |obligation| {
            let mut obligation_scope = obligation.activation_scope;
            loop {
                if obligation_scope == ancestor {
                    break true;
                }
                let Some(parent) = resolved
                    .scope(obligation_scope)
                    .and_then(|scope| scope.parent)
                else {
                    break false;
                };
                obligation_scope = parent;
            }
        },
        exit_origin,
        boundary,
    )
}

pub(crate) fn merge_recoverable_effects(
    typed: &TypedProgram,
    origin: Option<SyntaxOrigin>,
    usage: &str,
    effects: impl IntoIterator<Item = Option<RecoverableCallEffect>>,
) -> Result<Option<RecoverableCallEffect>, TypecheckError> {
    let mut merged: Option<RecoverableCallEffect> = None;
    for effect in effects.into_iter().flatten() {
        match merged {
            None => merged = Some(effect),
            Some(existing) if existing.error_type == effect.error_type => {}
            Some(existing) => {
                let message = format!(
                    "{usage} mixes incompatible recoverable error types '{}' and '{}'",
                    describe_type(typed, existing.error_type),
                    describe_type(typed, effect.error_type),
                );
                return Err(match origin.clone() {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::IncompatibleType,
                        message,
                        origin,
                    ),
                    None => TypecheckError::new(TypecheckErrorKind::IncompatibleType, message),
                });
            }
        }
    }
    Ok(merged)
}

pub(crate) fn plain_value_expr(
    typed: &TypedProgram,
    context: TypeContext,
    expr: TypedExpr,
    origin: Option<SyntaxOrigin>,
    usage: impl Into<String>,
) -> Result<TypedExpr, TypecheckError> {
    if expr.recoverable_effect.is_some() {
        match context.error_call_mode {
            ErrorCallMode::Propagate => {
                let _ = typed;
                reject_recoverable_plain_use(origin, usage)?;
            }
            ErrorCallMode::Observe => {}
        }
    }
    Ok(expr)
}

pub(crate) fn apparent_type_id(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<CheckedTypeId, TypecheckError> {
    let mut current = type_id;
    let mut seen = BTreeSet::new();

    loop {
        if let Some(next) = typed.apparent_type_override(current) {
            if next == current {
                return Ok(current);
            }
            current = next;
            continue;
        }
        match typed.type_table().get(current) {
            Some(CheckedType::Owned { inner }) | Some(CheckedType::Borrowed { inner, .. }) => {
                current = *inner;
            }
            // A generic parameter is an opaque placeholder with no underlying
            // type, so there is nothing to expand to — the same reason the type
            // importer refuses to expand one. Expanding it is also unsound
            // across a package boundary: an imported parameter keeps its
            // DEFINING package's symbol id, which can collide with an unrelated
            // local symbol, and the expansion then yields that symbol's type.
            // That surfaced as `expects 'fun(): int'` on a call to an imported
            // `wrap(T)(value: T): T`, where the id happened to match local
            // `main`.
            Some(CheckedType::Declared {
                kind: crate::DeclaredTypeKind::GenericParameter,
                ..
            }) => {
                return Ok(current);
            }
            Some(CheckedType::Declared { symbol, .. }) => {
                if !seen.insert(*symbol) {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        "declared type expansion encountered a cycle",
                    ));
                }
                let Some(next) = typed
                    .typed_symbol(*symbol)
                    .and_then(|symbol| symbol.declared_type)
                else {
                    return Ok(current);
                };
                if next == current {
                    return Ok(current);
                }
                current = next;
            }
            _ => return Ok(current),
        }
    }
}

pub(crate) fn channel_element_type(
    typed: &TypedProgram,
    channel_type: CheckedTypeId,
) -> Result<CheckedTypeId, TypecheckError> {
    let apparent = apparent_type_id(typed, channel_type)?;
    match typed.type_table().get(apparent) {
        Some(CheckedType::Channel { element_type })
        | Some(CheckedType::ChannelSender { element_type }) => Ok(*element_type),
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "channel endpoint access requires chn[T], got '{}'",
                describe_type(typed, channel_type)
            ),
        )),
    }
}

pub(crate) fn expected_nil_shell_type(
    typed: &TypedProgram,
    expected_type: Option<CheckedTypeId>,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let Some(expected_type) = expected_type else {
        return Ok(None);
    };
    let expected_apparent = apparent_type_id(typed, expected_type)?;
    Ok(match typed.type_table().get(expected_apparent) {
        Some(CheckedType::Optional { .. }) | Some(CheckedType::Error { .. }) => Some(expected_type),
        _ => None,
    })
}

pub(crate) fn channel_receiver_element_type(
    typed: &TypedProgram,
    channel_type: CheckedTypeId,
) -> Result<CheckedTypeId, TypecheckError> {
    let apparent = apparent_type_id(typed, channel_type)?;
    match typed.type_table().get(apparent) {
        // A full channel receives on its own embedded receiver; a first-class
        // `chn[rx, T]` receiver value receives through the moved unique handle.
        Some(CheckedType::Channel { element_type })
        | Some(CheckedType::ChannelReceiver { element_type }) => Ok(*element_type),
        Some(CheckedType::ChannelSender { .. }) => Err(TypecheckError::new(
            TypecheckErrorKind::Ownership,
            "sender-only channel endpoints cannot receive; keep the single receiver in the owning routine",
        )),
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "channel receive requires chn[T], got '{}'",
                describe_type(typed, channel_type)
            ),
        )),
    }
}

pub(crate) fn is_error_shell_type(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    let apparent = apparent_type_id(typed, type_id)?;
    Ok(matches!(
        typed.type_table().get(apparent),
        Some(CheckedType::Error { .. })
    ))
}

pub(crate) fn reject_recoverable_error_shell_conversion(
    typed: &TypedProgram,
    expected_type: CheckedTypeId,
    actual_expr: &TypedExpr,
    origin: Option<SyntaxOrigin>,
    surface: impl Into<String>,
) -> Result<(), TypecheckError> {
    if actual_expr.recoverable_effect.is_none() || !is_error_shell_type(typed, expected_type)? {
        return Ok(());
    }

    let message = format!(
        "{} cannot turn a '/ ErrorType' routine result into err[...] in V1; err[...] is the storable error form, so handle the call with '||' or check(...)",
        surface.into()
    );
    Err(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    })
}

pub(crate) fn unwrap_shell_result_type(
    typed: &TypedProgram,
    operand_type: CheckedTypeId,
) -> Result<Option<CheckedTypeId>, TypecheckError> {
    let apparent = apparent_type_id(typed, operand_type)?;
    Ok(match typed.type_table().get(apparent) {
        Some(CheckedType::Optional { inner }) => Some(*inner),
        Some(CheckedType::Error { inner: Some(inner) }) => Some(*inner),
        Some(CheckedType::Error { inner: None }) => None,
        _ => None,
    })
}

pub(crate) fn origin_for(
    resolved: &ResolvedProgram,
    syntax_id: SyntaxNodeId,
) -> Option<SyntaxOrigin> {
    resolved.syntax_index().origin(syntax_id).cloned()
}

/// Recover the resolver-created Block scope for an inline statement body
/// (a `when` case body or default body). The resolver creates these scopes
/// anonymously, so they are located through the references recorded inside
/// the body; bodies that declare bindings are found via the declared symbol's
/// scope instead.
pub(crate) fn inline_body_block_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    parent_scope_id: ScopeId,
    body: &[AstNode],
) -> Option<ScopeId> {
    let mut syntax_ids = BTreeSet::new();
    for node in body {
        collect_syntax_ids(node, &mut syntax_ids);
    }

    let direct_child_below_parent = |mut scope_id: ScopeId| -> Option<ScopeId> {
        loop {
            let parent = resolved.scope(scope_id)?.parent?;
            if parent == parent_scope_id {
                return Some(scope_id);
            }
            scope_id = parent;
        }
    };

    let mut candidate_scopes = BTreeSet::new();
    for reference in resolved.references.iter() {
        let Some(syntax_id) = reference.syntax_id else {
            continue;
        };
        if !syntax_ids.contains(&syntax_id) {
            continue;
        }
        let Some(symbol_id) = reference.resolved else {
            continue;
        };
        let Some(symbol) = resolved.symbol(symbol_id) else {
            continue;
        };
        if symbol.source_unit != source_unit_id {
            continue;
        }
        let Some(body_scope_id) = direct_child_below_parent(symbol.scope) else {
            continue;
        };
        if resolved
            .scope(body_scope_id)
            .is_some_and(|scope| scope.kind == fol_resolver::ScopeKind::Block)
        {
            candidate_scopes.insert(body_scope_id);
        }
    }

    // A body's own bindings pin its scope even when nothing references them.
    // Bind through the declaration's exact syntax origin: sibling bodies may
    // legally declare the same name, so descendant name searches are
    // inherently ambiguous here.
    for node in body {
        let (name, kind, syntax_id) = match node {
            AstNode::VarDecl {
                name, syntax_id, ..
            } => (name.as_str(), SymbolKind::ValueBinding, *syntax_id),
            AstNode::LabDecl {
                name, syntax_id, ..
            } => (name.as_str(), SymbolKind::LabelBinding, *syntax_id),
            _ => continue,
        };
        let Some(declaration_origin) =
            syntax_id.and_then(|syntax_id| resolved.syntax_index().origin(syntax_id))
        else {
            continue;
        };
        for symbol in resolved.symbols.iter() {
            if symbol.source_unit != source_unit_id
                || symbol.kind != kind
                || symbol.name != name
                || symbol.origin.as_ref() != Some(declaration_origin)
            {
                continue;
            }
            let Some(body_scope_id) = direct_child_below_parent(symbol.scope) else {
                continue;
            };
            if resolved
                .scope(body_scope_id)
                .is_some_and(|scope| scope.kind == fol_resolver::ScopeKind::Block)
            {
                candidate_scopes.insert(body_scope_id);
            }
        }
    }

    // A nested construct that owns a scope -- a `dfr`/`edf` body, a loop, an
    // inner block -- pins the enclosing body just as well as a binding does:
    // walking up from its scope lands on the body's own block scope. Without
    // this, an arm whose only statement is `dfr { ... }` looks scopeless, falls
    // back to the parent, and the deferred block is then rejected for belonging
    // to a scope that is not its parent.
    for syntax_id in &syntax_ids {
        let Some(nested_scope_id) = resolved.scope_for_syntax(*syntax_id) else {
            continue;
        };
        let Some(body_scope_id) = direct_child_below_parent(nested_scope_id) else {
            continue;
        };
        if resolved
            .scope(body_scope_id)
            .is_some_and(|scope| scope.kind == fol_resolver::ScopeKind::Block)
        {
            candidate_scopes.insert(body_scope_id);
        }
    }

    single_scope(candidate_scopes)
}

pub(crate) fn loop_body_scope(
    resolved: &ResolvedProgram,
    syntax_id: Option<SyntaxNodeId>,
) -> Result<ScopeId, TypecheckError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        internal_error("loop syntax anchor disappeared before typechecking", None)
    })?;
    let scope_id = resolved.scope_for_syntax(syntax_id).ok_or_else(|| {
        internal_error(
            "resolved loop body scope disappeared before typechecking",
            None,
        )
    })?;
    let valid = resolved.scope(scope_id).is_some_and(|scope| {
        matches!(
            scope.kind,
            fol_resolver::ScopeKind::Block | fol_resolver::ScopeKind::LoopBinder
        )
    });
    if !valid {
        return Err(internal_error(
            "resolved loop syntax anchor does not point at a loop body scope",
            None,
        ));
    }
    Ok(scope_id)
}

fn single_scope(scopes: BTreeSet<ScopeId>) -> Option<ScopeId> {
    if scopes.len() == 1 {
        scopes.into_iter().next()
    } else {
        None
    }
}

pub(crate) fn collect_syntax_ids(node: &AstNode, syntax_ids: &mut BTreeSet<SyntaxNodeId>) {
    if let Some(syntax_id) = node.syntax_id() {
        syntax_ids.insert(syntax_id);
    }
    for child in node.children() {
        collect_syntax_ids(child, syntax_ids);
    }
}

pub(crate) fn node_origin(resolved: &ResolvedProgram, node: &AstNode) -> Option<SyntaxOrigin> {
    let mut syntax_ids = BTreeSet::new();
    collect_syntax_ids(node, &mut syntax_ids);
    syntax_ids
        .into_iter()
        .next()
        .and_then(|syntax_id| origin_for(resolved, syntax_id))
}

pub(crate) fn with_node_origin(
    resolved: &ResolvedProgram,
    node: &AstNode,
    kind: TypecheckErrorKind,
    message: impl Into<String>,
) -> TypecheckError {
    if let Some(origin) = node_origin(resolved, node) {
        TypecheckError::with_origin(kind, message, origin)
    } else {
        TypecheckError::new(kind, message)
    }
}

pub(crate) fn find_symbol_in_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
) -> Option<SymbolId> {
    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.scope == scope_id
                && symbol.name == name
                && symbol.kind == kind
        })
        .map(|(symbol_id, _)| symbol_id)
}

pub(crate) fn find_symbol_in_scope_chain(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
) -> Option<SymbolId> {
    let mut current_scope = Some(scope_id);
    while let Some(scope_id) = current_scope {
        if let Some(symbol_id) =
            find_symbol_in_scope(resolved, source_unit_id, scope_id, name, kind)
        {
            return Some(symbol_id);
        }
        current_scope = resolved.scope(scope_id).and_then(|scope| scope.parent);
    }
    None
}

pub(crate) fn record_symbol_type(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
    type_id: CheckedTypeId,
) -> Result<(), TypecheckError> {
    let Some(symbol_id) =
        find_symbol_in_scope_chain(resolved, source_unit_id, scope_id, name, kind)
    else {
        return Err(internal_error(
            format!("typed symbol facts lost local symbol '{name}'"),
            None,
        ));
    };
    let Some(symbol) = typed.typed_symbol_mut(symbol_id) else {
        return Err(internal_error(
            format!("typed symbol facts lost local symbol '{name}'"),
            None,
        ));
    };
    symbol.declared_type = Some(type_id);
    Ok(())
}

pub(crate) fn binding_kind_for(node: &AstNode) -> SymbolKind {
    match node {
        AstNode::LabDecl { .. } => SymbolKind::LabelBinding,
        _ => SymbolKind::ValueBinding,
    }
}

pub(crate) fn ensure_assignable(
    typed: &TypedProgram,
    expected: CheckedTypeId,
    actual: CheckedTypeId,
    surface: String,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    if is_v1_assignable(typed, expected, actual)? {
        return Ok(());
    }

    let message = format!(
        "{surface} expects '{}' but got '{}'",
        describe_type(typed, expected),
        describe_type(typed, actual)
    );
    Err(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::IncompatibleType, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::IncompatibleType, message),
    })
}

pub(crate) fn is_v1_assignable(
    typed: &TypedProgram,
    expected: CheckedTypeId,
    actual: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    if actual == typed.builtin_types().never {
        return Ok(true);
    }

    let expected_apparent = apparent_type_id(typed, expected)?;
    let actual_apparent = apparent_type_id(typed, actual)?;
    if expected == actual || expected_apparent == actual_apparent {
        return Ok(true);
    }

    Ok(match typed.type_table().get(expected_apparent) {
        Some(CheckedType::ChannelSender {
            element_type: expected_element,
        }) => matches!(
            typed.type_table().get(actual_apparent),
            Some(CheckedType::Channel {
                element_type: actual_element,
            }) if actual_element == expected_element
        ),
        Some(CheckedType::Owned { inner }) if *inner == actual_apparent => true,
        Some(CheckedType::Optional { inner }) => {
            apparent_type_id(typed, *inner)? == actual_apparent
        }
        Some(CheckedType::Error { inner: Some(inner) }) => {
            apparent_type_id(typed, *inner)? == actual_apparent
        }
        // Routine values are compatible on their callable SHAPE. Parameter
        // names and defaultedness are per-declaration metadata (they are
        // part of the interned identity so named-argument binding stays
        // correct), but they must not block passing one routine where a
        // same-shaped routine type is expected.
        Some(CheckedType::Routine(expected_routine)) => {
            match typed.type_table().get(actual_apparent) {
                Some(CheckedType::Routine(actual_routine)) => {
                    expected_routine.params == actual_routine.params
                        && expected_routine.return_type == actual_routine.return_type
                        && expected_routine.error_type == actual_routine.error_type
                        && expected_routine.variadic_index == actual_routine.variadic_index
                        && expected_routine.mutex_params == actual_routine.mutex_params
                        && expected_routine.generic_params == actual_routine.generic_params
                        && expected_routine.generic_constraints
                            == actual_routine.generic_constraints
                }
                _ => false,
            }
        }
        _ => false,
    })
}

pub(crate) fn describe_type(typed: &TypedProgram, type_id: CheckedTypeId) -> String {
    // Render every type through the shared renderer so diagnostics read as
    // FOL surface syntax (`int`, `bol`, `vec[int]`, `Point`) rather than the
    // internal Rust `Debug` form (`Builtin(Int)`, `Vector { element_type: .. }`).
    typed.type_table().render_type(type_id)
}

pub(crate) fn is_equality_type(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    // A generic parameter bound by `ord` is equatable, for the same reason it
    // is orderable: the bound promises a total order, and a total order decides
    // equality. Without this, `>` compiled on a `T: ord` while `==` did not,
    // which is an asymmetry with no meaning behind it — and it blocked every
    // generic container routine that has to compare elements.
    if let Some(CheckedType::Declared {
        kind: crate::DeclaredTypeKind::GenericParameter,
        symbol,
        ..
    }) = typed.type_table().get(type_id)
    {
        return typed
            .generic_capability_constraints(*symbol)
            .is_some_and(|bounds| bounds.contains("ord"));
    }
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Builtin(crate::BuiltinType::Int(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Float(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Bool))
            | Some(CheckedType::Builtin(crate::BuiltinType::Char(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Str))
            | Some(CheckedType::Entry { .. })
    )
}

pub(crate) fn is_ordered_type(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    // A generic parameter is orderable exactly when it declares the `ord`
    // capability bound; the call site re-checks the actual, so the body may
    // rely on the promise.
    if let Some(CheckedType::Declared {
        kind: crate::DeclaredTypeKind::GenericParameter,
        symbol,
        ..
    }) = typed.type_table().get(type_id)
    {
        return typed
            .generic_capability_constraints(*symbol)
            .is_some_and(|bounds| bounds.contains("ord"));
    }
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Builtin(crate::BuiltinType::Int(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Float(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Char(_)))
            | Some(CheckedType::Builtin(crate::BuiltinType::Str))
    )
}

pub(crate) fn internal_error(
    message: impl Into<String>,
    origin: Option<SyntaxOrigin>,
) -> TypecheckError {
    if let Some(origin) = origin {
        TypecheckError::with_origin(TypecheckErrorKind::Internal, message, origin)
    } else {
        TypecheckError::new(TypecheckErrorKind::Internal, message)
    }
}

pub(crate) fn ensure_assignable_target(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    target: &AstNode,
) -> Result<(), TypecheckError> {
    match strip_comments(target) {
        AstNode::Identifier { name, .. } => {
            ensure_binding_reassignable(typed, resolved, source_unit_id, scope_id, name)
        }
        AstNode::QualifiedIdentifier { path } => {
            ensure_binding_reassignable(typed, resolved, source_unit_id, scope_id, &path.joined())
        }
        // Field assignment into a mutable record instance, e.g. `counter.total = 5`.
        // The whole instance must be mutable; the book does not allow assigning
        // into only some fields (structs chapter, "Accessing").
        AstNode::FieldAccess { object, field } => {
            let binding_name = match strip_comments(object) {
                AstNode::Identifier { name, .. } => name.clone(),
                AstNode::QualifiedIdentifier { path } => path.joined(),
                // Nested field/index targets (`a.b.c = x`) are not documented as
                // assignment targets in the current book contract.
                _ => {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        format!(
                            "nested field assignment targets like '.{field}' are not supported; \
                             assign into a field of a mutable binding directly"
                        ),
                    ))
                }
            };
            if !binding_is_mutable_by_name(typed, resolved, source_unit_id, scope_id, &binding_name)
            {
                // Receivers are immutable views in V1; mutating through
                // `self` is later ownership work, so point at the boundary
                // instead of suggesting an impossible declaration change.
                if binding_name == "self" {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        format!(
                            "cannot assign into field '{field}' of the method receiver; \
                             receiver mutation is not part of the current V1 surface"
                        ),
                    ));
                }
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "cannot assign into field '{field}' of immutable binding '{binding_name}'; \
                         declare the instance with 'var[mut]' to allow field assignment"
                    ),
                ));
            }
            Ok(())
        }
        // Element assignment into a positional container, e.g. `cells[i] = 7`.
        // Only `arr[T,N]` and `vec[T]` are positional stores: `map` needs a key
        // to appear, which is growth; `set[...]` is the tuple-member form, where
        // each position has its own type; and `seq[T]` is a persistent linked
        // list whose contract is tail sharing, not in-place mutation.
        AstNode::IndexAccess {
            container,
            index: _,
        } => {
            // Either the binding IS the container (`cells[i]`) or it holds it in
            // one field (`self.ram[i]`). One hop only: a free routine cannot take
            // a `[mut, bor]` parameter, so real state lives in a record reached
            // through its receiver, and that is the case worth supporting.
            let (binding_name, through_field) = match strip_comments(container) {
                AstNode::Identifier { name, .. } => (name.clone(), None),
                AstNode::QualifiedIdentifier { path } => (path.joined(), None),
                AstNode::FieldAccess { object, field } => match strip_comments(object) {
                    AstNode::Identifier { name, .. } => (name.clone(), Some(field.clone())),
                    AstNode::QualifiedIdentifier { path } => (path.joined(), Some(field.clone())),
                    _ => {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            "nested indexed assignment targets are not supported; \
                             index a mutable container binding, or one of its fields, directly",
                        ))
                    }
                },
                _ => {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "nested indexed assignment targets are not supported; \
                         index a mutable container binding directly",
                    ))
                }
            };
            if !binding_is_mutable_by_name(typed, resolved, source_unit_id, scope_id, &binding_name)
            {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!(
                        "cannot assign into an element of immutable binding '{binding_name}'; \
                         declare it with 'var[mut]' to allow element assignment"
                    ),
                ));
            }
            let symbol = find_symbol_in_scope_chain(
                resolved,
                source_unit_id,
                scope_id,
                &binding_name,
                SymbolKind::ValueBinding,
            )
            .or_else(|| {
                find_symbol_in_scope_chain(
                    resolved,
                    source_unit_id,
                    scope_id,
                    &binding_name,
                    SymbolKind::Parameter,
                )
            });
            // A `mux[T]` binding is a managed mutex, not the container itself;
            // its elements are reached through a guard, not through the handle.
            if symbol
                .and_then(|symbol| typed.typed_symbol(symbol))
                .is_some_and(|symbol| symbol.is_mutex)
            {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "cannot assign into an element of 'mux[T]' binding '{binding_name}'; \
                         lock it first and assign through the guard"
                    ),
                ));
            }
            let declared = symbol
                .and_then(|symbol| typed.typed_symbol(symbol))
                .and_then(|symbol| symbol.declared_type);
            // A `var[mut]` binding of a shared loan is a mutable NAME for an
            // immutable view; the loan is what decides.
            if let Some(CheckedType::Borrowed { mutable: false, .. }) =
                declared.and_then(|type_id| typed.type_table().get(type_id))
            {
                return Err(with_node_origin(
                    resolved,
                    target,
                    TypecheckErrorKind::BorrowMutability,
                    format!(
                        "cannot assign into an element through the shared loan '{binding_name}'; \
                         a '[bor]' view is read-only"
                    ),
                ));
            }
            let Some(binding_type) = declared.map(|type_id| apparent_type_id(typed, type_id))
            else {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("indexed assignment target '{binding_name}' does not retain a type"),
                ));
            };
            let container_type = match &through_field {
                None => binding_type?,
                Some(field) => {
                    let Some(CheckedType::Record { fields }) =
                        typed.type_table().get(binding_type?)
                    else {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::InvalidInput,
                            format!(
                                "indexed assignment through '.{field}' requires a record binding"
                            ),
                        ));
                    };
                    let Some(field_type) = fields.get(field).copied() else {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::InvalidInput,
                            format!("'{binding_name}' does not expose a field named '{field}'"),
                        ));
                    };
                    apparent_type_id(typed, field_type)?
                }
            };
            let element_type =
                match typed.type_table().get(container_type) {
                    Some(CheckedType::Array { element_type, .. })
                    | Some(CheckedType::Vector { element_type }) => *element_type,
                    Some(CheckedType::Sequence { .. }) => {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            "'seq[T]' is the persistent linked-list family; positional in-place \
                         writes are not part of its contract — use 'vec[T]'",
                        ))
                    }
                    Some(CheckedType::Map { .. }) => return Err(TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "map elements cannot be assigned; adding or replacing a key is container \
                         growth, which is not implemented",
                    )),
                    Some(CheckedType::Set { .. }) => {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            "'set[...]' is the tuple-member form, where each position has its own \
                         type; positional assignment into a set is not defined",
                        ))
                    }
                    _ => {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::InvalidInput,
                            format!(
                                "indexed assignment requires an 'arr[T,N]' or 'vec[T]' binding, \
                             but '{binding_name}' is not one"
                            ),
                        ))
                    }
                };
            // A container's `fin` elements are finalized by a scope-exit walk,
            // so an element replaced in place would never have its finalizer
            // run -- the source would read as if the resource was released.
            if typed.type_resolves_to_fin(element_type) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "cannot replace a 'fin' element in place; a container's 'fin' elements are \
                     finalized by a scope-exit walk, and an overwritten element's finalizer would \
                     never run — move the value out and rebuild the container",
                ));
            }
            if super::bindings::ownership_moves_on_transfer(typed, element_type) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Ownership,
                    "cannot replace a move-only element in place; the displaced value would be \
                     dropped without a transfer",
                ));
            }
            Ok(())
        }
        AstNode::UnaryOp {
            op: fol_parser::ast::UnaryOperator::Deref,
            operand,
        } => {
            let name = match strip_comments(operand) {
                AstNode::Identifier { name, .. } => name.as_str(),
                _ => {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        "dereference assignment requires a pointer binding identifier",
                    ))
                }
            };
            ensure_binding_reassignable(typed, resolved, source_unit_id, scope_id, name)?;
            let pointer_type = find_symbol_in_scope_chain(
                resolved,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::ValueBinding,
            )
            .and_then(|symbol| typed.typed_symbol(symbol))
            .and_then(|symbol| symbol.declared_type)
            .and_then(|type_id| typed.type_table().get(type_id));
            match pointer_type {
                Some(CheckedType::Pointer { shared: false, .. }) => Ok(()),
                Some(CheckedType::Pointer { shared: true, .. }) => Err(with_node_origin(
                    resolved,
                    target,
                    TypecheckErrorKind::InvalidInput,
                    "cannot write through ptr[shared, T]; shared pointers are read-only",
                )),
                _ => Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "dereference assignment requires a pointer binding",
                )),
            }
        }
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "assignment targets must currently be plain identifiers, qualified identifiers, \
             a field of a mutable record binding, or a unique-pointer dereference",
        )),
    }
}

/// Reject whole-binding reassignment of immutable value/label bindings
/// (`con`/`var[imu]`/`lab`). Targets that do not resolve to a value/label binding
/// in the scope chain keep the previous permissive behavior.
fn ensure_binding_reassignable(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
) -> Result<(), TypecheckError> {
    let known_immutable = [SymbolKind::ValueBinding, SymbolKind::LabelBinding]
        .into_iter()
        .find_map(|kind| find_symbol_in_scope_chain(resolved, source_unit_id, scope_id, name, kind))
        .and_then(|symbol_id| typed.typed_symbol(symbol_id))
        .is_some_and(|symbol| !symbol.is_mutable);
    if known_immutable {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("cannot reassign immutable binding '{name}'"),
        ));
    }
    Ok(())
}

/// Whether a mutable borrow may be taken from this symbol.
///
/// A `var[mut]` owner qualifies, and so does a binding that IS already a mutable
/// loan — `self` inside a `pro (T[mut, bor])` method, which reborrows. Without
/// the second case the author is told to declare the owner `var[mut]`, which is
/// advice a receiver can never follow.
pub(crate) fn symbol_allows_mutable_borrow(typed: &TypedProgram, symbol: SymbolId) -> bool {
    typed.typed_symbol(symbol).is_some_and(|symbol| {
        symbol.is_mutable
            || symbol.is_mutex
            || matches!(
                symbol
                    .declared_type
                    .and_then(|type_id| typed.type_table().get(type_id)),
                Some(CheckedType::Borrowed { mutable: true, .. })
            )
    })
}

/// Whether the value/label binding reachable under `name` in the scope chain was
/// declared mutable. Bindings are immutable by default (variables chapter).
/// A growable container reached by method syntax (`values.push(x)`), resolved to
/// the binding that owns it. Mirrors the checks the indexed-assignment path runs
/// — same rules, own wording, because "indexed assignment" reads wrong on a
/// `.push`.
#[derive(Clone, Copy)]
pub(crate) enum ContainerFamily {
    Vector {
        element: CheckedTypeId,
    },
    Map {
        key: CheckedTypeId,
        value: CheckedTypeId,
    },
}

pub(crate) struct ContainerMethodReceiver {
    pub binding_name: String,
    pub family: ContainerFamily,
}

/// Resolve the receiver of a growable-container method to its owning binding and
/// element type, rejecting every receiver that cannot legally be mutated.
///
/// One field hop only, for the same reason the indexed path allows one: a free
/// routine cannot take a `[mut, bor]` parameter, so real state lives in a record
/// reached through its receiver.
pub(crate) fn resolve_container_method_receiver(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    receiver: &AstNode,
    method: &str,
    reads_only: bool,
) -> Result<ContainerMethodReceiver, TypecheckError> {
    let (binding_name, through_field) = match strip_comments(receiver) {
        AstNode::Identifier { name, .. } => (name.clone(), None),
        AstNode::QualifiedIdentifier { path } => (path.joined(), None),
        AstNode::FieldAccess { object, field } => match strip_comments(object) {
            AstNode::Identifier { name, .. } => (name.clone(), Some(field.clone())),
            AstNode::QualifiedIdentifier { path } => (path.joined(), Some(field.clone())),
            _ => {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "'.{method}' requires a container binding, or one of its fields, directly"
                    ),
                ))
            }
        },
        _ => {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                format!("'.{method}' requires a container binding directly"),
            ))
        }
    };
    // A capture is looked for first: inside a routine value the body's `c` names
    // the capture, not the outer binding it was taken from, and only the capture
    // carries that body's ownership state.
    let symbol = find_symbol_in_scope_chain(
        resolved,
        source_unit_id,
        scope_id,
        &binding_name,
        SymbolKind::Capture,
    )
    .or_else(|| {
        find_symbol_in_scope_chain(
            resolved,
            source_unit_id,
            scope_id,
            &binding_name,
            SymbolKind::ValueBinding,
        )
    })
    .or_else(|| {
        find_symbol_in_scope_chain(
            resolved,
            source_unit_id,
            scope_id,
            &binding_name,
            SymbolKind::Parameter,
        )
    });
    // A container method reaches a place, so the receiver carries the same
    // ownership obligations the identifier path enforces: a moved binding, a
    // moved field, and a live borrow of the owner all rule it out. Without
    // these, `.push` was the one way to reach a container the model had
    // already invalidated.
    if let Some(symbol) = symbol {
        if let Some(move_origin) = typed.moved_binding_origin(symbol).cloned() {
            let move_only = typed
                .typed_symbol(symbol)
                .and_then(|symbol| symbol.declared_type)
                .is_some_and(|type_id| {
                    super::bindings::ownership_moves_on_transfer(typed, type_id)
                });
            let message = if move_only {
                format!("use of moved heap-owned binding '{binding_name}'")
            } else {
                format!("use of moved binding '{binding_name}'")
            };
            return Err(TypecheckError::new(TypecheckErrorKind::Ownership, message)
                .with_related_origin(move_origin, "ownership moved here"));
        }
        if let Some(field) = &through_field {
            if let Some(field_origin) = typed.moved_field_origin(symbol, field).cloned() {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Ownership,
                    format!("use of moved field '{binding_name}.{field}'"),
                )
                .with_related_origin(field_origin, "field moved here"));
            }
        }
        if let Some(borrow) = typed.active_borrow_for_owner(symbol).cloned() {
            return Err(TypecheckError::new(
                TypecheckErrorKind::OwnerBorrowed,
                format!("owner '{binding_name}' is inaccessible while borrowed"),
            )
            .with_related_origin(borrow.origin, "borrow created here"));
        }
    }
    // A read-only method (`get`, `contains`, `keys`, `values`) needs no mutable
    // place: requiring one would force every caller to declare `var[mut]` just
    // to look inside a container.
    if !reads_only {
        // The loan is checked before plain mutability so a `[bor]` parameter is
        // told its view is read-only, rather than to "declare it 'var[mut]'" —
        // advice a parameter cannot take.
        if let Some(CheckedType::Borrowed { mutable: false, .. }) = symbol
            .and_then(|symbol| typed.typed_symbol(symbol))
            .and_then(|symbol| symbol.declared_type)
            .and_then(|type_id| typed.type_table().get(type_id))
        {
            return Err(TypecheckError::new(
                TypecheckErrorKind::BorrowMutability,
                format!(
                    "cannot '.{method}' through the shared loan '{binding_name}'; \
                     a '[bor]' view is read-only"
                ),
            ));
        }
        if !binding_is_mutable_by_name(typed, resolved, source_unit_id, scope_id, &binding_name) {
            return Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "cannot '.{method}' through immutable binding '{binding_name}'; \
                     declare it with 'var[mut]' to allow container mutation"
                ),
            ));
        }
    }
    // A `mux[T]` binding is a managed mutex, not the container itself; its
    // contents are reached through a guard, not through the handle.
    if symbol
        .and_then(|symbol| typed.typed_symbol(symbol))
        .is_some_and(|symbol| symbol.is_mutex)
    {
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            format!(
                "cannot '.{method}' on 'mux[T]' binding '{binding_name}'; \
                 lock it first and mutate through the guard"
            ),
        ));
    }
    let declared = symbol
        .and_then(|symbol| typed.typed_symbol(symbol))
        .and_then(|symbol| symbol.declared_type);
    let Some(binding_type) = declared.map(|type_id| apparent_type_id(typed, type_id)) else {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("'.{method}' receiver '{binding_name}' does not retain a type"),
        ));
    };
    let container_type = match &through_field {
        None => binding_type?,
        Some(field) => {
            let Some(CheckedType::Record { fields }) = typed.type_table().get(binding_type?) else {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("'.{method}' through '.{field}' requires a record binding"),
                ));
            };
            let Some(field_type) = fields.get(field).copied() else {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("'{binding_name}' does not expose a field named '{field}'"),
                ));
            };
            apparent_type_id(typed, field_type)?
        }
    };
    let family = match typed.type_table().get(container_type) {
        Some(CheckedType::Vector { element_type }) => ContainerFamily::Vector {
            element: *element_type,
        },
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => ContainerFamily::Map {
            key: *key_type,
            value: *value_type,
        },
        Some(CheckedType::Array { .. }) => {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                format!(
                    "'arr[T,N]' is fixed-size, so '.{method}' cannot resize it; use 'vec[T]', \
                     or assign an element with 'values[i] = ...'"
                ),
            ))
        }
        Some(CheckedType::Sequence { .. }) => {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                format!(
                    "'seq[T]' is the persistent linked-list family; '.{method}' mutates in place, \
                     which is not part of its contract — use 'vec[T]'"
                ),
            ))
        }
        Some(CheckedType::Set { .. }) => {
            return Err(TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                format!(
                    "'set[...]' is the tuple-member form, where each position has its own type; \
                     '.{method}' needs a growable container — use 'vec[T]' or 'map[K,V]'"
                ),
            ))
        }
        _ => {
            return Err(TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "'.{method}' requires a 'vec[T]' or 'map[K,V]' binding, but '{binding_name}' \
                     is not one"
                ),
            ))
        }
    };
    // A container's `fin` elements are finalized by a scope-exit walk, which does
    // not model a set that changes size. Keeping the gate aligned with the
    // indexed path means one rule to reason about for `fin` containers.
    let finalized = match family {
        ContainerFamily::Vector { element } => typed.type_resolves_to_fin(element),
        ContainerFamily::Map { value, .. } => typed.type_resolves_to_fin(value),
    };
    if finalized {
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            format!(
                "cannot '.{method}' on a container of 'fin' elements; its elements are finalized \
                 by a scope-exit walk, which does not model resizing"
            ),
        ));
    }
    Ok(ContainerMethodReceiver {
        binding_name,
        family,
    })
}

fn binding_is_mutable_by_name(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
) -> bool {
    [
        SymbolKind::ValueBinding,
        SymbolKind::LabelBinding,
        SymbolKind::Parameter,
    ]
    .into_iter()
    .find_map(|kind| find_symbol_in_scope_chain(resolved, source_unit_id, scope_id, name, kind))
    .and_then(|symbol_id| typed.typed_symbol(symbol_id))
    .map(|symbol| {
        // `var[mut]`/`lab[mut]`, a `mux[T]` owner, OR a mutable borrow — the
        // last covers a `[mut, bor]` method receiver (`self`) and any
        // `var[mut, bor]` binding, whose fields are assignable through the loan
        // (V3_MEM §4.2/§8.3).
        symbol.is_mutable
            || symbol.is_mutex
            || matches!(
                symbol
                    .declared_type
                    .and_then(|type_id| typed.type_table().get(type_id)),
                Some(CheckedType::Borrowed { mutable: true, .. })
            )
    })
    .unwrap_or(false)
}

pub(crate) fn strip_comments(node: &AstNode) -> &AstNode {
    match node {
        AstNode::Commented { node, .. } => strip_comments(node),
        _ => node,
    }
}

pub(crate) fn invalid_binary_operator_error(
    typed: &TypedProgram,
    op: &fol_parser::ast::BinaryOperator,
    left: CheckedTypeId,
    right: CheckedTypeId,
) -> TypecheckError {
    TypecheckError::new(
        TypecheckErrorKind::InvalidInput,
        format!(
            "binary operator '{:?}' is not valid for '{}' and '{}'",
            op,
            describe_type(typed, left),
            describe_type(typed, right)
        ),
    )
}

pub(crate) fn invalid_unary_operator_error(
    typed: &TypedProgram,
    op: &fol_parser::ast::UnaryOperator,
    operand: CheckedTypeId,
) -> TypecheckError {
    TypecheckError::new(
        TypecheckErrorKind::InvalidInput,
        format!(
            "unary operator '{:?}' is not valid for '{}'",
            op,
            describe_type(typed, operand)
        ),
    )
}

pub(crate) fn unsupported_binary_surface(
    resolved: &ResolvedProgram,
    left: &AstNode,
    right: &AstNode,
    message: impl Into<String>,
) -> TypecheckError {
    if let Some(origin) = node_origin(resolved, left).or_else(|| node_origin(resolved, right)) {
        TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
    } else {
        TypecheckError::new(TypecheckErrorKind::Unsupported, message)
    }
}

pub(crate) fn unsupported_conversion_intrinsic(
    resolved: &ResolvedProgram,
    left: &AstNode,
    right: &AstNode,
    name: &str,
) -> TypecheckError {
    use fol_intrinsics::{select_intrinsic, IntrinsicSurface};
    let message = match select_intrinsic(IntrinsicSurface::OperatorAlias, name) {
        Ok(entry) => fol_intrinsics::unsupported_intrinsic_message(entry),
        Err(_) => format!("unsupported conversion operator '{name}'"),
    };
    unsupported_binary_surface(resolved, left, right, message)
}

pub(crate) fn unsupported_node_surface(
    resolved: &ResolvedProgram,
    node: &AstNode,
    message: impl Into<String>,
) -> TypecheckError {
    if let Some(origin) = node_origin(resolved, node) {
        TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
    } else {
        TypecheckError::new(TypecheckErrorKind::Unsupported, message)
    }
}

/// The canonical width for a parsed integer type. An unsized `int` is the
/// default width, which is `i64`; every other spelling keeps exactly what it
/// was written as (`plan/V4_SCALAR_WIDTHS.md`).
pub fn int_width_of(size: Option<&fol_parser::ast::IntSize>, signed: bool) -> IntWidth {
    use fol_parser::ast::IntSize;
    match (size, signed) {
        (None, _) => IntWidth::DEFAULT,
        (Some(IntSize::I8), true) => IntWidth::I8,
        (Some(IntSize::I16), true) => IntWidth::I16,
        (Some(IntSize::I32), true) => IntWidth::I32,
        (Some(IntSize::I64), true) => IntWidth::I64,
        (Some(IntSize::I128), true) => IntWidth::I128,
        (Some(IntSize::Arch), true) => IntWidth::Arch,
        (Some(IntSize::I8), false) => IntWidth::U8,
        (Some(IntSize::I16), false) => IntWidth::U16,
        (Some(IntSize::I32), false) => IntWidth::U32,
        (Some(IntSize::I64), false) => IntWidth::U64,
        (Some(IntSize::I128), false) => IntWidth::U128,
        (Some(IntSize::Arch), false) => IntWidth::UArch,
    }
}

/// The canonical width for a parsed float type; an unsized `flt` is `f64`.
pub fn float_width_of(size: Option<&fol_parser::ast::FloatSize>) -> FloatWidth {
    use fol_parser::ast::FloatSize;
    match size {
        None => FloatWidth::DEFAULT,
        Some(FloatSize::F32) => FloatWidth::F32,
        Some(FloatSize::F64) => FloatWidth::F64,
        Some(FloatSize::Arch) => FloatWidth::Arch,
    }
}
