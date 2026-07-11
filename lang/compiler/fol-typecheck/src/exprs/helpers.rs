use crate::{
    CheckedType, CheckedTypeId, RecoverableCallEffect, TypecheckError, TypecheckErrorKind,
    TypedProgram,
};
use fol_parser::ast::{AstNode, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::{BTreeSet, VecDeque};

use super::{ErrorCallMode, TypeContext, TypedExpr};

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
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
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

pub(crate) fn origin_for(resolved: &ResolvedProgram, syntax_id: SyntaxNodeId) -> Option<SyntaxOrigin> {
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

    let descends_from_parent = |mut scope_id: ScopeId| -> bool {
        loop {
            if scope_id == parent_scope_id {
                return true;
            }
            match resolved.scope(scope_id).and_then(|scope| scope.parent) {
                Some(parent) => scope_id = parent,
                None => return false,
            }
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
        let Some(scope) = resolved.scope(symbol.scope) else {
            continue;
        };
        if scope.kind == fol_resolver::ScopeKind::Block
            && symbol.scope != parent_scope_id
            && descends_from_parent(symbol.scope)
        {
            candidate_scopes.insert(symbol.scope);
        }
    }

    // A body's own bindings pin its scope even when nothing references them.
    for node in body {
        let (name, kind) = match node {
            AstNode::VarDecl { name, .. } => (name.as_str(), SymbolKind::ValueBinding),
            AstNode::LabDecl { name, .. } => (name.as_str(), SymbolKind::LabelBinding),
            _ => continue,
        };
        for symbol in resolved.symbols.iter() {
            if symbol.source_unit == source_unit_id
                && symbol.kind == kind
                && symbol.name == name
                && symbol.scope != parent_scope_id
                && resolved
                    .scope(symbol.scope)
                    .is_some_and(|scope| scope.kind == fol_resolver::ScopeKind::Block)
                && descends_from_parent(symbol.scope)
            {
                candidate_scopes.insert(symbol.scope);
            }
        }
    }

    single_scope(candidate_scopes)
}

pub(crate) fn loop_binder_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    parent_scope_id: ScopeId,
    binder_name: &str,
    condition: &Option<Box<AstNode>>,
    body: &[AstNode],
) -> Result<ScopeId, TypecheckError> {
    let mut syntax_ids = BTreeSet::new();
    if let Some(condition) = condition.as_deref() {
        collect_syntax_ids(condition, &mut syntax_ids);
    }
    for node in body {
        collect_syntax_ids(node, &mut syntax_ids);
    }

    let mut referenced_scopes = BTreeSet::new();
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
        if symbol.source_unit == source_unit_id
            && symbol.kind == SymbolKind::LoopBinder
            && symbol.name == binder_name
        {
            referenced_scopes.insert(symbol.scope);
        }
    }

    if let Some(scope_id) = single_scope(referenced_scopes) {
        return Ok(scope_id);
    }

    let mut queue = VecDeque::from([parent_scope_id]);
    let mut matched_scopes = BTreeSet::new();
    while let Some(scope_id) = queue.pop_front() {
        for (child_scope_id, child_scope) in resolved.scopes.iter_with_ids() {
            if child_scope.parent != Some(scope_id)
                || child_scope.source_unit != Some(source_unit_id)
            {
                continue;
            }
            queue.push_back(child_scope_id);
            if child_scope.kind != fol_resolver::ScopeKind::LoopBinder {
                continue;
            }
            if resolved.symbols.iter().any(|symbol| {
                symbol.source_unit == source_unit_id
                    && symbol.scope == child_scope_id
                    && symbol.kind == SymbolKind::LoopBinder
                    && symbol.name == binder_name
            }) {
                matched_scopes.insert(child_scope_id);
            }
        }
    }

    single_scope(matched_scopes).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::Internal,
            format!("could not uniquely recover the loop binder scope for '{binder_name}'"),
        )
    })
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
        Some(CheckedType::Optional { inner }) if *inner == actual_apparent => true,
        Some(CheckedType::Error { inner: Some(inner) }) if *inner == actual_apparent => true,
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
        _ => Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            "assignment targets must currently be plain identifiers, qualified identifiers, \
             or a field of a mutable record binding",
        )),
    }
}

/// Reject whole-binding reassignment of immutable value/label bindings
/// (`con`/`let`/`lab`). Targets that do not resolve to a value/label binding
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
        .find_map(|kind| {
            find_symbol_in_scope_chain(resolved, source_unit_id, scope_id, name, kind)
        })
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
    [SymbolKind::ValueBinding, SymbolKind::LabelBinding]
        .into_iter()
        .find_map(|kind| {
            find_symbol_in_scope_chain(resolved, source_unit_id, scope_id, name, kind)
        })
        .and_then(|symbol_id| typed.typed_symbol(symbol_id))
        .map(|symbol| symbol.is_mutable)
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
