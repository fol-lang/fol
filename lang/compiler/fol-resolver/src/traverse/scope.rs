use crate::{
    collect::{semantic_node, top_level_duplicate_key},
    errors::{format_origin_brief, symbol_kind_label},
    model::{ResolvedProgram, ResolvedSymbol, SymbolKind},
    ResolverError, ResolverErrorKind, ScopeId, SourceUnitId, SymbolId,
};
use fol_parser::ast::{AstNode, Generic};

pub fn insert_local_named_symbol(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    node: &AstNode,
    kind: SymbolKind,
) -> Result<SymbolId, ResolverError> {
    let name = match semantic_node(node) {
        AstNode::FunDecl { name, .. }
        | AstNode::ProDecl { name, .. }
        | AstNode::LogDecl { name, .. } => name.as_str(),
        _ => {
            return Err(ResolverError::new(
                ResolverErrorKind::Internal,
                "attempted to bind a local named symbol from a non-routine node",
            ));
        }
    };
    let canonical_name = fol_types::canonical_identifier_key(name);
    let duplicate_key = top_level_duplicate_key(semantic_node(node), &canonical_name);
    let origin = semantic_node(node)
        .syntax_id()
        .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
        .cloned();
    insert_local_symbol_with_origin(
        program,
        source_unit_id,
        scope_id,
        name,
        kind,
        duplicate_key,
        origin,
    )
}

pub fn insert_local_symbol(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
    duplicate_key: String,
) -> Result<SymbolId, ResolverError> {
    insert_local_symbol_with_origin(
        program,
        source_unit_id,
        scope_id,
        name,
        kind,
        duplicate_key,
        None,
    )
}

pub fn insert_local_symbol_with_origin(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
    duplicate_key: String,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    let canonical_name = fol_types::canonical_identifier_key(name);
    // Resolver contract: one scope cannot redefine the same binding shape, but
    // nested scopes may intentionally shadow names from parent scopes.
    if let Some(existing) = program
        .scope(scope_id)
        .and_then(|scope| scope.symbol_keys.get(&canonical_name))
        .into_iter()
        .flat_map(|ids| ids.iter())
        .filter_map(|id| program.symbol(*id))
        .find(|symbol| symbol.duplicate_key == duplicate_key)
    {
        let existing_site = existing
            .origin
            .as_ref()
            .map(format_origin_brief)
            .unwrap_or_else(|| "an unknown location".to_string());
        return Err(ResolverError::with_origin(
            ResolverErrorKind::DuplicateSymbol,
            format!(
                "duplicate local symbol '{}' conflicts with existing {} declaration first declared at {}",
                name,
                symbol_kind_label(existing.kind),
                existing_site
            ),
            existing
                .origin
                .clone()
                .unwrap_or(fol_parser::ast::SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: name.len(),
                }),
        )
        .with_related_origin(
            existing
                .origin
                .clone()
                .unwrap_or(fol_parser::ast::SyntaxOrigin {
                    file: None,
                    line: 1,
                    column: 1,
                    length: name.len(),
                }),
            format!("first {} declaration", symbol_kind_label(existing.kind)),
        ));
    }

    let symbol_id = program.symbols.push(ResolvedSymbol {
        id: SymbolId(0),
        name: name.to_string(),
        canonical_name: canonical_name.clone(),
        duplicate_key,
        kind,
        scope: scope_id,
        source_unit: source_unit_id,
        origin,
        visibility: None,
        declaration_scope: None,
        mounted_from: None,
        is_mutable: false,
    });
    if let Some(symbol) = program.symbols.get_mut(symbol_id) {
        symbol.id = symbol_id;
    }

    let scope = program
        .scopes
        .get_mut(scope_id)
        .expect("local symbol target scope should exist");
    scope.symbols.push(symbol_id);
    scope
        .symbol_keys
        .entry(canonical_name)
        .or_default()
        .push(symbol_id);

    Ok(symbol_id)
}

pub fn insert_generic_symbols(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    generics: &[Generic],
) -> Result<(), ResolverError> {
    for generic in generics {
        insert_local_symbol(
            program,
            source_unit_id,
            scope_id,
            &generic.name,
            SymbolKind::GenericParameter,
            format!(
                "symbol#{}",
                fol_types::canonical_identifier_key(&generic.name)
            ),
        )?;
    }

    Ok(())
}
