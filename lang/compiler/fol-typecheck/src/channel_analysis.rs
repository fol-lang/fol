use crate::{CheckedType, CheckedTypeId, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, ChannelEndpoint, ParsedSourceUnitKind, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};

type ParamKey = (SymbolId, usize);

#[derive(Clone)]
struct RoutineInfo {
    symbol: SymbolId,
    source_unit: SourceUnitId,
    scope: ScopeId,
    param_names: Vec<String>,
    param_symbols: Vec<SymbolId>,
    channel_params: BTreeMap<usize, CheckedTypeId>,
    body: Vec<AstNode>,
    inquiries: Vec<AstNode>,
}

/// Refine every `chn[T]` routine parameter to the capability it actually
/// needs. A parameter that can reach a receive operation stays a full channel;
/// all other channel parameters become sender-only. The receive effect is
/// propagated through aliases and the named call graph before bodies are type
/// checked, so spawn and imported-call checks cannot be bypassed by wrappers.
pub(crate) fn refine_channel_parameters(typed: &mut TypedProgram) -> Result<(), TypecheckError> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();
    let mut routines = Vec::new();
    for (source_index, unit) in syntax.source_units.iter().enumerate() {
        if unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit = SourceUnitId(source_index);
        for item in &unit.items {
            collect_routines(typed, &resolved, source_unit, &item.node, &mut routines)?;
        }
    }

    let routine_by_symbol = routines
        .iter()
        .enumerate()
        .map(|(index, info)| (info.symbol, index))
        .collect::<BTreeMap<_, _>>();
    let local_channel_keys = routines
        .iter()
        .flat_map(|info| {
            info.channel_params
                .keys()
                .map(move |index| ((info.symbol, *index), ()))
        })
        .collect::<BTreeMap<_, _>>();

    let mut receivers = BTreeSet::new();
    let mut dependencies = Vec::new();
    for info in &routines {
        analyze_routine(
            typed,
            &resolved,
            info,
            &routines,
            &routine_by_symbol,
            &local_channel_keys,
            &mut receivers,
            &mut dependencies,
        );
    }

    loop {
        let mut changed = false;
        for (caller, callee) in &dependencies {
            if receivers.contains(callee) {
                changed |= receivers.insert(*caller);
            }
        }
        if !changed {
            break;
        }
    }

    for info in &routines {
        let Some(CheckedType::Routine(mut signature)) = typed
            .typed_symbol(info.symbol)
            .and_then(|symbol| symbol.declared_type)
            .and_then(|type_id| typed.type_table().get(type_id))
            .cloned()
        else {
            continue;
        };
        let mut receiver_params = BTreeSet::new();
        for (index, element_type) in &info.channel_params {
            if receivers.contains(&(info.symbol, *index)) {
                receiver_params.insert(*index);
                continue;
            }
            let sender_type = typed.type_table_mut().intern(CheckedType::ChannelSender {
                element_type: *element_type,
            });
            if let Some(param) = signature.params.get_mut(*index) {
                *param = sender_type;
            }
            if let Some(param_symbol) = info.param_symbols.get(*index).copied() {
                if let Some(symbol) = typed.typed_symbol_mut(param_symbol) {
                    symbol.declared_type = Some(sender_type);
                }
            }
        }
        let signature_type = typed
            .type_table_mut()
            .intern(CheckedType::Routine(signature));
        if let Some(symbol) = typed.typed_symbol_mut(info.symbol) {
            symbol.declared_type = Some(signature_type);
            symbol.channel_receiver_params = receiver_params;
        }
    }

    Ok(())
}

pub(crate) fn validate_endpoint_lifecycles(typed: &TypedProgram) -> Result<(), TypecheckError> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();
    let mut routines = Vec::new();
    for (source_index, unit) in syntax.source_units.iter().enumerate() {
        if unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit = SourceUnitId(source_index);
        for item in &unit.items {
            collect_routines(typed, &resolved, source_unit, &item.node, &mut routines)?;
        }
    }
    for info in &routines {
        validate_endpoint_lifecycle(typed, &resolved, info)?;
    }
    Ok(())
}

fn collect_routines(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit: SourceUnitId,
    node: &AstNode,
    routines: &mut Vec<RoutineInfo>,
) -> Result<(), TypecheckError> {
    match strip_comments(node) {
        AstNode::FunDecl {
            syntax_id,
            name,
            receiver_type,
            params,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            syntax_id,
            name,
            receiver_type,
            params,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            syntax_id,
            name,
            receiver_type,
            params,
            body,
            inquiries,
            ..
        } => {
            let symbol = crate::decls::find_routine_symbol_id(
                resolved,
                source_unit,
                name,
                receiver_type.as_ref(),
                params,
            )?;
            let scope = syntax_id
                .and_then(|id| resolved.scope_for_syntax(id))
                .or_else(|| resolved.symbol(symbol).map(|symbol| symbol.scope))
                .ok_or_else(|| {
                    crate::exprs::helpers::internal_error(
                        format!("routine '{name}' lost its signature scope"),
                        resolved
                            .symbol(symbol)
                            .and_then(|symbol| symbol.origin.clone()),
                    )
                })?;
            let mut param_symbols = Vec::with_capacity(params.len());
            let mut channel_params = BTreeMap::new();
            for (index, param) in params.iter().enumerate() {
                let param_symbol = crate::decls::find_symbol_id_in_scope(
                    resolved,
                    source_unit,
                    scope,
                    &[SymbolKind::Parameter],
                    &param.name,
                )?;
                if let Some(CheckedType::Channel { element_type }) = typed
                    .typed_symbol(param_symbol)
                    .and_then(|symbol| symbol.declared_type)
                    .and_then(|type_id| typed.type_table().get(type_id))
                {
                    channel_params.insert(index, *element_type);
                }
                param_symbols.push(param_symbol);
            }
            routines.push(RoutineInfo {
                symbol,
                source_unit,
                scope,
                param_names: params.iter().map(|param| param.name.clone()).collect(),
                param_symbols,
                channel_params,
                body: body.clone(),
                inquiries: inquiries.clone(),
            });
            for child in body.iter().chain(inquiries) {
                collect_nested_routines(typed, resolved, source_unit, child, routines)?;
            }
        }
        other => {
            for child in other.children() {
                collect_routines(typed, resolved, source_unit, child, routines)?;
            }
        }
    }
    Ok(())
}

fn collect_nested_routines(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    source_unit: SourceUnitId,
    node: &AstNode,
    routines: &mut Vec<RoutineInfo>,
) -> Result<(), TypecheckError> {
    let node = strip_comments(node);
    if matches!(
        node,
        AstNode::FunDecl { .. } | AstNode::ProDecl { .. } | AstNode::LogDecl { .. }
    ) {
        return collect_routines(typed, resolved, source_unit, node, routines);
    }
    if matches!(
        node,
        AstNode::AnonymousFun { .. } | AstNode::AnonymousPro { .. } | AstNode::AnonymousLog { .. }
    ) {
        return Ok(());
    }
    for child in node.children() {
        collect_nested_routines(typed, resolved, source_unit, child, routines)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn analyze_routine(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    routines: &[RoutineInfo],
    routine_by_symbol: &BTreeMap<SymbolId, usize>,
    local_channel_keys: &BTreeMap<ParamKey, ()>,
    receivers: &mut BTreeSet<ParamKey>,
    dependencies: &mut Vec<(ParamKey, ParamKey)>,
) {
    let mut aliases = info
        .channel_params
        .keys()
        .filter_map(|index| {
            info.param_symbols
                .get(*index)
                .copied()
                .map(|symbol| (symbol, BTreeSet::from([*index])))
        })
        .collect::<BTreeMap<_, _>>();

    loop {
        let before = aliases.clone();
        for node in info.body.iter().chain(&info.inquiries) {
            collect_aliases(resolved, info, node, &mut aliases);
        }
        if aliases == before {
            break;
        }
    }

    let mut safe_identifiers = BTreeSet::new();
    for node in info.body.iter().chain(&info.inquiries) {
        collect_safe_uses_and_calls(
            typed,
            resolved,
            info,
            node,
            &aliases,
            routines,
            routine_by_symbol,
            local_channel_keys,
            receivers,
            dependencies,
            &mut safe_identifiers,
        );
    }
    for node in info.body.iter().chain(&info.inquiries) {
        collect_unsafe_uses(
            resolved,
            info.symbol,
            node,
            &aliases,
            &safe_identifiers,
            receivers,
        );
    }
}

fn collect_aliases(
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    node: &AstNode,
    aliases: &mut BTreeMap<SymbolId, BTreeSet<usize>>,
) {
    let node = strip_comments(node);
    if is_routine_node(node) {
        return;
    }
    match node {
        AstNode::VarDecl {
            name,
            syntax_id,
            value: Some(value),
            ..
        }
        | AstNode::LabDecl {
            name,
            syntax_id,
            value: Some(value),
            ..
        } => {
            if let (Some(source), Some(target)) = (
                identifier_symbol(resolved, value),
                binding_symbol(resolved, info, name, *syntax_id),
            ) {
                if let Some(origins) = aliases.get(&source).cloned() {
                    aliases.entry(target).or_default().extend(origins);
                }
            }
        }
        AstNode::Assignment { target, value } => {
            if let (Some(source), Some(target)) = (
                identifier_symbol(resolved, value),
                identifier_symbol(resolved, target),
            ) {
                if let Some(origins) = aliases.get(&source).cloned() {
                    aliases.entry(target).or_default().extend(origins);
                }
            }
        }
        _ => {}
    }
    for child in node.children() {
        collect_aliases(resolved, info, child, aliases);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_safe_uses_and_calls(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    node: &AstNode,
    aliases: &BTreeMap<SymbolId, BTreeSet<usize>>,
    routines: &[RoutineInfo],
    routine_by_symbol: &BTreeMap<SymbolId, usize>,
    local_channel_keys: &BTreeMap<ParamKey, ()>,
    receivers: &mut BTreeSet<ParamKey>,
    dependencies: &mut Vec<(ParamKey, ParamKey)>,
    safe: &mut BTreeSet<SyntaxNodeId>,
) {
    let node = strip_comments(node);
    if is_routine_node(node) {
        return;
    }
    match node {
        AstNode::VarDecl {
            value: Some(value), ..
        }
        | AstNode::LabDecl {
            value: Some(value), ..
        } => mark_direct_identifier_safe(value, safe),
        AstNode::Assignment { target, value } => {
            mark_direct_identifier_safe(target, safe);
            mark_direct_identifier_safe(value, safe);
        }
        AstNode::ChannelAccess {
            channel,
            endpoint: ChannelEndpoint::Tx,
        } => mark_direct_identifier_safe(channel, safe),
        AstNode::FunctionCall {
            syntax_id, args, ..
        } => analyze_call(
            typed,
            resolved,
            info,
            *syntax_id,
            ReferenceKind::FunctionCall,
            args,
            aliases,
            routines,
            routine_by_symbol,
            local_channel_keys,
            receivers,
            dependencies,
            safe,
        ),
        AstNode::QualifiedFunctionCall { path, args } => analyze_call(
            typed,
            resolved,
            info,
            path.syntax_id(),
            ReferenceKind::QualifiedFunctionCall,
            args,
            aliases,
            routines,
            routine_by_symbol,
            local_channel_keys,
            receivers,
            dependencies,
            safe,
        ),
        _ => {}
    }
    for child in node.children() {
        collect_safe_uses_and_calls(
            typed,
            resolved,
            info,
            child,
            aliases,
            routines,
            routine_by_symbol,
            local_channel_keys,
            receivers,
            dependencies,
            safe,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn analyze_call(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    syntax_id: Option<SyntaxNodeId>,
    reference_kind: ReferenceKind,
    args: &[AstNode],
    aliases: &BTreeMap<SymbolId, BTreeSet<usize>>,
    routines: &[RoutineInfo],
    routine_by_symbol: &BTreeMap<SymbolId, usize>,
    local_channel_keys: &BTreeMap<ParamKey, ()>,
    receivers: &mut BTreeSet<ParamKey>,
    dependencies: &mut Vec<(ParamKey, ParamKey)>,
    safe: &mut BTreeSet<SyntaxNodeId>,
) {
    let Some(target) = syntax_id.and_then(|syntax_id| {
        resolved
            .references
            .iter()
            .find(|reference| {
                reference.syntax_id == Some(syntax_id) && reference.kind == reference_kind
            })
            .and_then(|reference| reference.resolved)
    }) else {
        mark_call_aliases_receiver(resolved, info.symbol, args, aliases, receivers, safe);
        return;
    };

    let (param_names, param_types) = if let Some(index) = routine_by_symbol.get(&target) {
        let target_info = &routines[*index];
        let types = typed
            .typed_symbol(target)
            .and_then(|symbol| symbol.declared_type)
            .and_then(|type_id| typed.type_table().get(type_id))
            .and_then(|typ| match typ {
                CheckedType::Routine(signature) => Some(signature.params.clone()),
                _ => None,
            })
            .unwrap_or_default();
        (target_info.param_names.clone(), types)
    } else {
        typed
            .typed_symbol(target)
            .and_then(|symbol| symbol.declared_type)
            .and_then(|type_id| typed.type_table().get(type_id))
            .and_then(|typ| match typ {
                CheckedType::Routine(signature) => {
                    Some((signature.param_names.clone(), signature.params.clone()))
                }
                _ => None,
            })
            .unwrap_or_default()
    };
    let bound = bind_argument_positions(args, &param_names);
    for (arg, param_index) in bound {
        let arg = match strip_comments(arg) {
            AstNode::NamedArgument { value, .. } => strip_comments(value),
            other => other,
        };
        let Some(symbol) = identifier_symbol(resolved, arg) else {
            continue;
        };
        let Some(origins) = aliases.get(&symbol) else {
            continue;
        };
        mark_direct_identifier_safe(arg, safe);
        for caller_param in origins {
            let caller = (info.symbol, *caller_param);
            if routine_by_symbol.contains_key(&target) {
                let callee = (target, param_index);
                if local_channel_keys.contains_key(&callee) {
                    dependencies.push((caller, callee));
                } else {
                    receivers.insert(caller);
                }
                continue;
            }
            let expected = param_types.get(param_index).copied();
            if !expected.is_some_and(|expected| {
                matches!(
                    typed.type_table().get(expected),
                    Some(CheckedType::ChannelSender { .. })
                )
            }) {
                receivers.insert(caller);
            }
        }
    }
}

fn mark_call_aliases_receiver(
    resolved: &ResolvedProgram,
    routine: SymbolId,
    args: &[AstNode],
    aliases: &BTreeMap<SymbolId, BTreeSet<usize>>,
    receivers: &mut BTreeSet<ParamKey>,
    safe: &mut BTreeSet<SyntaxNodeId>,
) {
    for arg in args {
        let arg = match strip_comments(arg) {
            AstNode::NamedArgument { value, .. } => strip_comments(value),
            other => other,
        };
        let Some(symbol) = identifier_symbol(resolved, arg) else {
            continue;
        };
        mark_direct_identifier_safe(arg, safe);
        if let Some(origins) = aliases.get(&symbol) {
            receivers.extend(origins.iter().map(|index| (routine, *index)));
        }
    }
}

fn collect_unsafe_uses(
    resolved: &ResolvedProgram,
    routine: SymbolId,
    node: &AstNode,
    aliases: &BTreeMap<SymbolId, BTreeSet<usize>>,
    safe: &BTreeSet<SyntaxNodeId>,
    receivers: &mut BTreeSet<ParamKey>,
) {
    let node = strip_comments(node);
    if is_routine_node(node) {
        return;
    }
    if let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = node
    {
        if !safe.contains(syntax_id) {
            if let Some(origins) =
                identifier_symbol(resolved, node).and_then(|symbol| aliases.get(&symbol))
            {
                receivers.extend(origins.iter().map(|index| (routine, *index)));
            }
        }
    }
    for child in node.children() {
        collect_unsafe_uses(resolved, routine, child, aliases, safe, receivers);
    }
}

fn bind_argument_positions<'a>(
    args: &'a [AstNode],
    param_names: &[String],
) -> Vec<(&'a AstNode, usize)> {
    let mut bound = Vec::new();
    let mut claimed = BTreeSet::new();
    let mut next = 0usize;
    for arg in args {
        if let AstNode::NamedArgument { name, .. } = strip_comments(arg) {
            if let Some(index) = param_names.iter().position(|param| param == name) {
                claimed.insert(index);
                bound.push((arg, index));
            }
            continue;
        }
        while claimed.contains(&next) {
            next += 1;
        }
        bound.push((arg, next));
        claimed.insert(next);
        next += 1;
    }
    bound
}

fn binding_symbol(
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    name: &str,
    syntax_id: Option<SyntaxNodeId>,
) -> Option<SymbolId> {
    let origin = syntax_id.and_then(|id| resolved.syntax_index().origin(id));
    let candidates = resolved
        .symbols
        .iter_with_ids()
        .filter(|(_, symbol)| {
            symbol.source_unit == info.source_unit
                && symbol.name == name
                && matches!(
                    symbol.kind,
                    SymbolKind::ValueBinding | SymbolKind::LabelBinding
                )
                && scope_descends_from(resolved, symbol.scope, info.scope)
        })
        .collect::<Vec<_>>();
    if let Some(origin) = origin {
        if let Some((id, _)) = candidates
            .iter()
            .copied()
            .find(|(_, symbol)| symbol.origin.as_ref() == Some(origin))
        {
            return Some(id);
        }
    }
    if candidates.len() == 1 {
        Some(candidates[0].0)
    } else {
        None
    }
}

fn scope_descends_from(resolved: &ResolvedProgram, mut scope: ScopeId, ancestor: ScopeId) -> bool {
    loop {
        if scope == ancestor {
            return true;
        }
        let Some(parent) = resolved.scope(scope).and_then(|scope| scope.parent) else {
            return false;
        };
        scope = parent;
    }
}

fn identifier_symbol(resolved: &ResolvedProgram, node: &AstNode) -> Option<SymbolId> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(node)
    else {
        return None;
    };
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
}

fn mark_direct_identifier_safe(node: &AstNode, safe: &mut BTreeSet<SyntaxNodeId>) {
    if let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(node)
    {
        safe.insert(*syntax_id);
    }
}

fn validate_endpoint_lifecycle(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
) -> Result<(), TypecheckError> {
    let mut aliases = resolved
        .symbols
        .iter_with_ids()
        .filter(|(symbol_id, symbol)| {
            symbol.source_unit == info.source_unit
                && scope_descends_from(resolved, symbol.scope, info.scope)
                && typed
                    .typed_symbol(*symbol_id)
                    .and_then(|symbol| symbol.declared_type)
                    .is_some_and(|type_id| is_full_channel_type(typed, type_id))
        })
        .map(|(symbol_id, _)| (symbol_id, BTreeSet::from([symbol_id])))
        .collect::<BTreeMap<_, _>>();

    loop {
        let before = aliases.clone();
        for node in info.body.iter().chain(&info.inquiries) {
            collect_endpoint_aliases(resolved, info, node, &mut aliases);
        }
        if aliases == before {
            break;
        }
    }

    let mut consumed = BTreeMap::new();
    for node in info.body.iter().chain(&info.inquiries) {
        validate_endpoint_node(typed, resolved, info, node, &aliases, &mut consumed)?;
    }
    Ok(())
}

fn is_full_channel_type(typed: &TypedProgram, type_id: CheckedTypeId) -> bool {
    if let Some(apparent) = typed.apparent_type_override(type_id) {
        if apparent != type_id {
            return is_full_channel_type(typed, apparent);
        }
    }
    matches!(
        typed.type_table().get(type_id),
        Some(CheckedType::Channel { .. })
    )
}

fn collect_endpoint_aliases(
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    node: &AstNode,
    aliases: &mut BTreeMap<SymbolId, BTreeSet<SymbolId>>,
) {
    let node = strip_comments(node);
    if is_routine_node(node) {
        return;
    }
    match node {
        AstNode::VarDecl {
            name,
            syntax_id,
            value: Some(value),
            ..
        }
        | AstNode::LabDecl {
            name,
            syntax_id,
            value: Some(value),
            ..
        } => {
            if let (Some(source), Some(target)) = (
                identifier_symbol(resolved, value),
                binding_symbol(resolved, info, name, *syntax_id),
            ) {
                if let Some(origins) = aliases.get(&source).cloned() {
                    aliases.entry(target).or_default().extend(origins);
                }
            }
        }
        AstNode::Assignment { target, value } => {
            if let (Some(source), Some(target)) = (
                identifier_symbol(resolved, value),
                identifier_symbol(resolved, target),
            ) {
                if let Some(origins) = aliases.get(&source).cloned() {
                    aliases.entry(target).or_default().extend(origins);
                }
            }
        }
        _ => {}
    }
    for child in node.children() {
        collect_endpoint_aliases(resolved, info, child, aliases);
    }
}

fn validate_endpoint_node(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    node: &AstNode,
    aliases: &BTreeMap<SymbolId, BTreeSet<SymbolId>>,
    consumed: &mut BTreeMap<SymbolId, SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let node = strip_comments(node);
    match node {
        AstNode::AnonymousFun {
            syntax_id,
            captures,
            ..
        }
        | AstNode::AnonymousPro {
            syntax_id,
            captures,
            ..
        }
        | AstNode::AnonymousLog {
            syntax_id,
            captures,
            ..
        } => {
            let capture_scope = syntax_id
                .and_then(|syntax_id| resolved.scope_for_syntax(syntax_id))
                .and_then(|routine_scope| resolved.scope(routine_scope))
                .and_then(|scope| scope.parent)
                .unwrap_or(info.scope);
            for capture in captures {
                if !matches!(capture.endpoint, Some(ChannelEndpoint::Tx)) {
                    continue;
                }
                let symbol = [
                    SymbolKind::ValueBinding,
                    SymbolKind::Parameter,
                    SymbolKind::Capture,
                ]
                .into_iter()
                .find_map(|kind| {
                    crate::exprs::helpers::find_symbol_in_scope_chain(
                        resolved,
                        info.source_unit,
                        capture_scope,
                        &capture.name,
                        kind,
                    )
                });
                if let Some(origins) = symbol.and_then(|symbol| aliases.get(&symbol)) {
                    reject_consumed_tx(resolved, node, &capture.name, origins, consumed)?;
                }
            }
            return Ok(());
        }
        AstNode::FunDecl { .. } | AstNode::ProDecl { .. } | AstNode::LogDecl { .. } => {
            return Ok(())
        }
        AstNode::Dfr { body, .. } | AstNode::Edf { body, .. } => {
            if let Some(channel_use) = body
                .iter()
                .find_map(|node| first_full_channel_use(resolved, info, node, aliases))
            {
                let message = "channel endpoint acquisition is not allowed inside dfr/edf; acquire sender handles before the deferred block and perform receiver operations in ordinary control flow";
                return Err(
                    crate::exprs::helpers::node_origin(resolved, channel_use).map_or_else(
                        || TypecheckError::new(TypecheckErrorKind::Ownership, message),
                        |origin| {
                            TypecheckError::with_origin(
                                TypecheckErrorKind::Ownership,
                                message,
                                origin,
                            )
                        },
                    ),
                );
            }
            return Ok(());
        }
        AstNode::Loop { .. } => {
            premark_receiver_acquisitions(resolved, node, aliases, consumed);
        }
        AstNode::ChannelAccess { channel, endpoint } => {
            if let Some(symbol) = identifier_symbol(resolved, channel) {
                if let Some(origins) = aliases.get(&symbol) {
                    match endpoint {
                        ChannelEndpoint::Rx => {
                            if let Some(origin) = crate::exprs::helpers::node_origin(resolved, node)
                            {
                                for root in origins {
                                    consumed.entry(*root).or_insert_with(|| origin.clone());
                                }
                            }
                        }
                        ChannelEndpoint::Tx => {
                            let name = match strip_comments(channel) {
                                AstNode::Identifier { name, .. } => name.as_str(),
                                _ => "channel",
                            };
                            reject_consumed_tx(resolved, node, name, origins, consumed)?;
                        }
                    }
                }
            }
        }
        AstNode::Select { arms, .. } => {
            for arm in arms {
                let channel = match strip_comments(&arm.channel) {
                    AstNode::ChannelAccess {
                        channel,
                        endpoint: ChannelEndpoint::Rx,
                    } => channel.as_ref(),
                    other => other,
                };
                let Some(symbol) = identifier_symbol(resolved, channel) else {
                    continue;
                };
                let Some(origins) = aliases.get(&symbol) else {
                    continue;
                };
                if let Some(origin) = crate::exprs::helpers::node_origin(resolved, &arm.channel) {
                    for root in origins {
                        consumed.entry(*root).or_insert_with(|| origin.clone());
                    }
                }
            }
        }
        AstNode::FunctionCall {
            syntax_id, args, ..
        } => validate_sender_call(
            typed,
            resolved,
            info,
            *syntax_id,
            ReferenceKind::FunctionCall,
            args,
            aliases,
            consumed,
        )?,
        AstNode::QualifiedFunctionCall { path, args } => validate_sender_call(
            typed,
            resolved,
            info,
            path.syntax_id(),
            ReferenceKind::QualifiedFunctionCall,
            args,
            aliases,
            consumed,
        )?,
        AstNode::MethodCall {
            syntax_id, args, ..
        } => {
            if let Some(target) =
                syntax_id.and_then(|syntax_id| typed.method_call_target(syntax_id))
            {
                validate_sender_call_target(typed, resolved, target, args, aliases, consumed)?;
            }
        }
        _ => {}
    }
    for child in node.children() {
        validate_endpoint_node(typed, resolved, info, child, aliases, consumed)?;
    }
    Ok(())
}

fn first_full_channel_use<'a>(
    resolved: &ResolvedProgram,
    info: &RoutineInfo,
    node: &'a AstNode,
    aliases: &BTreeMap<SymbolId, BTreeSet<SymbolId>>,
) -> Option<&'a AstNode> {
    let node = strip_comments(node);
    match node {
        AstNode::AnonymousFun {
            syntax_id,
            captures,
            ..
        }
        | AstNode::AnonymousPro {
            syntax_id,
            captures,
            ..
        }
        | AstNode::AnonymousLog {
            syntax_id,
            captures,
            ..
        } => {
            let capture_scope = syntax_id
                .and_then(|syntax_id| resolved.scope_for_syntax(syntax_id))
                .and_then(|routine_scope| resolved.scope(routine_scope))
                .and_then(|scope| scope.parent)
                .unwrap_or(info.scope);
            let captures_endpoint = captures.iter().any(|capture| {
                capture.endpoint.is_some()
                    && [
                        SymbolKind::ValueBinding,
                        SymbolKind::Parameter,
                        SymbolKind::Capture,
                    ]
                    .into_iter()
                    .find_map(|kind| {
                        crate::exprs::helpers::find_symbol_in_scope_chain(
                            resolved,
                            info.source_unit,
                            capture_scope,
                            &capture.name,
                            kind,
                        )
                    })
                    .is_some_and(|symbol| aliases.contains_key(&symbol))
            });
            return captures_endpoint.then_some(node);
        }
        AstNode::FunDecl { .. } | AstNode::ProDecl { .. } | AstNode::LogDecl { .. } => return None,
        _ => {}
    }
    if identifier_symbol(resolved, node).is_some_and(|symbol| aliases.contains_key(&symbol)) {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| first_full_channel_use(resolved, info, child, aliases))
}

fn premark_receiver_acquisitions(
    resolved: &ResolvedProgram,
    node: &AstNode,
    aliases: &BTreeMap<SymbolId, BTreeSet<SymbolId>>,
    consumed: &mut BTreeMap<SymbolId, SyntaxOrigin>,
) {
    let node = strip_comments(node);
    if is_routine_node(node) {
        return;
    }
    match node {
        AstNode::ChannelAccess {
            channel,
            endpoint: ChannelEndpoint::Rx,
        } => {
            if let Some(origins) =
                identifier_symbol(resolved, channel).and_then(|symbol| aliases.get(&symbol))
            {
                if let Some(origin) = crate::exprs::helpers::node_origin(resolved, node) {
                    for root in origins {
                        consumed.entry(*root).or_insert_with(|| origin.clone());
                    }
                }
            }
        }
        AstNode::Select { arms, .. } => {
            for arm in arms {
                let channel = match strip_comments(&arm.channel) {
                    AstNode::ChannelAccess {
                        channel,
                        endpoint: ChannelEndpoint::Rx,
                    } => channel.as_ref(),
                    other => other,
                };
                if let Some(origins) =
                    identifier_symbol(resolved, channel).and_then(|symbol| aliases.get(&symbol))
                {
                    if let Some(origin) = crate::exprs::helpers::node_origin(resolved, &arm.channel)
                    {
                        for root in origins {
                            consumed.entry(*root).or_insert_with(|| origin.clone());
                        }
                    }
                }
            }
        }
        _ => {}
    }
    for child in node.children() {
        premark_receiver_acquisitions(resolved, child, aliases, consumed);
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_sender_call(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    _info: &RoutineInfo,
    syntax_id: Option<SyntaxNodeId>,
    reference_kind: ReferenceKind,
    args: &[AstNode],
    aliases: &BTreeMap<SymbolId, BTreeSet<SymbolId>>,
    consumed: &BTreeMap<SymbolId, SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let Some(target) = syntax_id.and_then(|syntax_id| {
        resolved
            .references
            .iter()
            .find(|reference| {
                reference.syntax_id == Some(syntax_id) && reference.kind == reference_kind
            })
            .and_then(|reference| reference.resolved)
    }) else {
        return Ok(());
    };
    validate_sender_call_target(typed, resolved, target, args, aliases, consumed)
}

fn validate_sender_call_target(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    target: SymbolId,
    args: &[AstNode],
    aliases: &BTreeMap<SymbolId, BTreeSet<SymbolId>>,
    consumed: &BTreeMap<SymbolId, SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let Some(signature) = typed
        .typed_symbol(target)
        .and_then(|symbol| symbol.declared_type)
        .and_then(|type_id| typed.type_table().get(type_id))
        .and_then(|typ| match typ {
            CheckedType::Routine(signature) => Some(signature),
            _ => None,
        })
    else {
        return Ok(());
    };
    for (arg, param_index) in bind_argument_positions(args, &signature.param_names) {
        let expected = signature.params.get(param_index).copied();
        if !expected.is_some_and(|expected| {
            matches!(
                typed.type_table().get(expected),
                Some(CheckedType::ChannelSender { .. })
            )
        }) {
            continue;
        }
        let arg = match strip_comments(arg) {
            AstNode::NamedArgument { value, .. } => strip_comments(value),
            other => other,
        };
        let Some(symbol) = identifier_symbol(resolved, arg) else {
            continue;
        };
        let Some(origins) = aliases.get(&symbol) else {
            continue;
        };
        let name = match arg {
            AstNode::Identifier { name, .. } => name.as_str(),
            _ => "channel",
        };
        reject_consumed_tx(resolved, arg, name, origins, consumed)?;
    }
    Ok(())
}

fn reject_consumed_tx(
    resolved: &ResolvedProgram,
    node: &AstNode,
    name: &str,
    origins: &BTreeSet<SymbolId>,
    consumed: &BTreeMap<SymbolId, SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    let Some(acquired_at) = origins.iter().find_map(|root| consumed.get(root)).cloned() else {
        return Ok(());
    };
    let message = format!(
        "channel transmitter '{name}[tx]' is no longer available after receiver acquisition; clone or capture every sender before the first '{name}[rx]'"
    );
    let mut error = crate::exprs::helpers::node_origin(resolved, node).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
        },
    );
    error = error.with_related_origin(acquired_at, "receiver acquired here");
    Err(error)
}

fn is_routine_node(node: &AstNode) -> bool {
    matches!(
        node,
        AstNode::FunDecl { .. }
            | AstNode::ProDecl { .. }
            | AstNode::LogDecl { .. }
            | AstNode::AnonymousFun { .. }
            | AstNode::AnonymousPro { .. }
            | AstNode::AnonymousLog { .. }
    )
}

fn strip_comments(mut node: &AstNode) -> &AstNode {
    while let AstNode::Commented { node: inner, .. } = node {
        node = inner;
    }
    node
}
