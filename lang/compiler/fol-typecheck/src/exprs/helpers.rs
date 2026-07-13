use crate::{
    CheckedType, CheckedTypeId, RecoverableCallEffect, TypecheckError, TypecheckErrorKind,
    TypedProgram,
};
use fol_parser::ast::{AstNode, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{
    ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind,
};
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
            reference.syntax_id == Some(*syntax_id)
                && reference.kind == ReferenceKind::Identifier
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

pub(crate) fn type_embeds_full_channel(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> bool {
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
                Some(CheckedType::ChannelSender { element_type }) => {
                    embeds(typed, *element_type, false, visiting)
                }
                Some(CheckedType::Declared { symbol, args, .. }) => {
                    args.iter()
                        .any(|arg| embeds(typed, *arg, false, visiting))
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
                Some(CheckedType::Error { inner }) => inner
                    .is_some_and(|inner| embeds(typed, inner, false, visiting)),
                Some(CheckedType::Eventual {
                    value_type,
                    error_type,
                }) => {
                    embeds(typed, *value_type, false, visiting)
                        || error_type
                            .is_some_and(|error| embeds(typed, error, false, visiting))
                }
                Some(CheckedType::Builtin(_))
                | Some(CheckedType::Routine(_))
                | None => false,
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
    let message = "full chn[T] values cannot be embedded in aggregate or wrapper types in V3; keep channels as direct routine-local bindings or named-routine parameters";
    Err(origin.map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
    ))
}

pub(crate) fn type_contains_shared_pointer(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> bool {
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
                Some(CheckedType::Pointer { shared: true, .. }) => true,
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
                | Some(CheckedType::ChannelSender { element_type }) => {
                    contains(typed, *element_type, visiting)
                }
                Some(CheckedType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| contains(typed, *member, visiting)),
                Some(CheckedType::Map {
                    key_type,
                    value_type,
                }) => {
                    contains(typed, *key_type, visiting)
                        || contains(typed, *value_type, visiting)
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
                Some(CheckedType::Builtin(_))
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
                | Some(CheckedType::ChannelSender { element_type }) => {
                    contains(typed, *element_type, visiting)
                }
                Some(CheckedType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| contains(typed, *member, visiting)),
                Some(CheckedType::Map {
                    key_type,
                    value_type,
                }) => {
                    contains(typed, *key_type, visiting)
                        || contains(typed, *value_type, visiting)
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
                Some(CheckedType::Builtin(_))
                | Some(CheckedType::Routine(_))
                | None => false,
            }
        };
        visiting.remove(&type_id);
        result
    }

    contains(typed, type_id, &mut BTreeSet::new())
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
        Some(CheckedType::Channel { element_type }) => Ok(*element_type),
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
        internal_error("resolved loop body scope disappeared before typechecking", None)
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
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Builtin(crate::BuiltinType::Int))
            | Some(CheckedType::Builtin(crate::BuiltinType::Float))
            | Some(CheckedType::Builtin(crate::BuiltinType::Bool))
            | Some(CheckedType::Builtin(crate::BuiltinType::Char))
            | Some(CheckedType::Builtin(crate::BuiltinType::Str))
    )
}

pub(crate) fn is_ordered_type(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Builtin(crate::BuiltinType::Int))
            | Some(CheckedType::Builtin(crate::BuiltinType::Float))
            | Some(CheckedType::Builtin(crate::BuiltinType::Char))
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

/// Whether the value/label binding reachable under `name` in the scope chain was
/// declared mutable. Bindings are immutable by default (variables chapter).
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
    .map(|symbol| symbol.is_mutable || symbol.is_mutex)
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
