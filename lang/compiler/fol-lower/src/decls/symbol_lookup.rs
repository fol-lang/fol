use fol_resolver::{SourceUnitId, SymbolId, SymbolKind};

/// Resolve a local declaration through its exact parsed syntax origin.
/// Local names can repeat in sibling lexical scopes, so lowering must never
/// choose one by descendant distance or iteration order.
pub(crate) fn find_symbol_for_declaration(
    typed_program: &fol_typecheck::TypedProgram,
    source_unit_id: SourceUnitId,
    kind: SymbolKind,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
) -> Option<SymbolId> {
    let target_origin = syntax_id
        .and_then(|syntax_id| typed_program.resolved().syntax_index().origin(syntax_id))?;
    typed_program
        .resolved()
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.kind == kind
                && symbol.name == name
                && symbol.mounted_from.is_none()
                && symbol.origin.as_ref() == Some(target_origin)
        })
        .map(|(symbol_id, _)| symbol_id)
}

/// Resolve the exact routine symbol for a parsed top-level item. Same-named
/// routines (method sugar on different receivers) are distinguished by their
/// name-anchored declaration origin; name matching is only a fallback for
/// items without a recorded origin.
pub(crate) fn find_routine_symbol_for_item(
    typed_program: &fol_typecheck::TypedProgram,
    source_unit_id: SourceUnitId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
) -> Option<SymbolId> {
    if let Some(target_origin) = syntax_id
        .and_then(|syntax_id| typed_program.resolved().syntax_index().origin(syntax_id))
    {
        let by_origin = typed_program
            .resolved()
            .symbols
            .iter_with_ids()
            .find(|(_, symbol)| {
                symbol.source_unit == source_unit_id
                    && symbol.kind == SymbolKind::Routine
                    && symbol.name == name
                    && symbol.mounted_from.is_none()
                    && symbol.origin.as_ref() == Some(target_origin)
            })
            .map(|(symbol_id, _)| symbol_id);
        if by_origin.is_some() {
            return by_origin;
        }
    }
    find_local_symbol_id(typed_program, source_unit_id, SymbolKind::Routine, name)
}

pub(crate) fn find_local_symbol_id(
    typed_program: &fol_typecheck::TypedProgram,
    source_unit_id: SourceUnitId,
    kind: SymbolKind,
    name: &str,
) -> Option<SymbolId> {
    typed_program
        .resolved()
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.kind == kind
                && symbol.name == name
                && symbol.mounted_from.is_none()
        })
        .map(|(symbol_id, _)| symbol_id)
}

pub(crate) fn find_symbol_in_exact_scope(
    typed_program: &fol_typecheck::TypedProgram,
    source_unit_id: SourceUnitId,
    scope_id: fol_resolver::ScopeId,
    kind: SymbolKind,
    name: &str,
) -> Option<SymbolId> {
    typed_program
        .resolved()
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.scope == scope_id
                && symbol.kind == kind
                && symbol.name == name
                && symbol.mounted_from.is_none()
        })
        .map(|(symbol_id, _)| symbol_id)
}

pub(crate) fn find_symbol_in_scope_or_descendants(
    typed_program: &fol_typecheck::TypedProgram,
    source_unit_id: SourceUnitId,
    scope_id: fol_resolver::ScopeId,
    kind: SymbolKind,
    name: &str,
) -> Option<SymbolId> {
    let exact = find_symbol_in_exact_scope(typed_program, source_unit_id, scope_id, kind, name);
    if exact.is_some() {
        return exact;
    }

    let mut candidates = typed_program
        .resolved()
        .symbols
        .iter_with_ids()
        .filter_map(|(symbol_id, symbol)| {
            (symbol.source_unit == source_unit_id
                && symbol.kind == kind
                && symbol.name == name
                && symbol.mounted_from.is_none())
            .then_some((symbol_id, symbol.scope))
        })
        .filter_map(|(symbol_id, candidate_scope)| {
            scope_distance_from(typed_program.resolved(), candidate_scope, scope_id)
                .map(|distance| (distance, symbol_id))
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|(distance, symbol_id)| (*distance, symbol_id.0));
    let (best_distance, best_symbol) = candidates.first().copied()?;
    let next_distance = candidates.get(1).map(|(distance, _)| *distance);
    (next_distance != Some(best_distance)).then_some(best_symbol)
}

fn scope_distance_from(
    resolved: &fol_resolver::ResolvedProgram,
    mut candidate_scope: fol_resolver::ScopeId,
    target_scope: fol_resolver::ScopeId,
) -> Option<usize> {
    let mut distance = 0usize;
    loop {
        if candidate_scope == target_scope {
            return Some(distance);
        }
        let scope = resolved.scopes.get(candidate_scope)?;
        candidate_scope = scope.parent?;
        distance += 1;
    }
}
