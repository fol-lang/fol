use crate::{
    errors::{format_origin_brief, symbol_kind_label},
    model::{ResolvedImport, ResolvedProgram, ResolvedSymbol, SymbolKind},
    ImportId, ResolverError, ResolverErrorKind, ScopeId, SourceUnitId, SymbolId,
};
use fol_parser::ast::{
    AstNode, BindingPattern, FolType, ParsedDeclScope, ParsedTopLevel,
    UsePathSegment,
};

pub fn collect_top_level_symbols(program: &mut ResolvedProgram) -> Result<(), Vec<ResolverError>> {
    let mut errors = Vec::new();
    let work_items = program
        .syntax()
        .source_units
        .iter()
        .enumerate()
        .flat_map(|(source_unit_id, syntax_unit)| {
            syntax_unit
                .items
                .iter()
                .cloned()
                .map(move |item| (SourceUnitId(source_unit_id), item))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (source_unit_id, item) in work_items {
        let scope_id = match top_level_scope_id(program, source_unit_id, &item) {
            Ok(id) => id,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let origin = program.syntax_index().origin(item.node_id).cloned();

        if let Err(error) =
            collect_symbols_from_top_level(program, source_unit_id, scope_id, &item, origin)
        {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub(crate) fn top_level_scope_id(
    program: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    item: &ParsedTopLevel,
) -> Result<ScopeId, ResolverError> {
    let source_unit = program
        .source_unit(source_unit_id)
        .ok_or_else(|| ResolverError::new(
            ResolverErrorKind::Internal,
            format!(
                "source unit {:?} not found during symbol collection",
                source_unit_id
            ),
        ))?;

    match item.meta.scope {
        Some(ParsedDeclScope::File) => Ok(source_unit.scope_id),
        Some(ParsedDeclScope::Namespace) => program
            .namespace_scope(&source_unit.namespace)
            .ok_or_else(|| ResolverError::new(
                ResolverErrorKind::Internal,
                format!(
                    "namespace scope for '{}' not found for namespace-scoped declaration",
                    source_unit.namespace
                ),
            )),
        Some(ParsedDeclScope::Package) | None => Ok(program.program_scope),
    }
}

fn collect_symbols_from_top_level(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    item: &ParsedTopLevel,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<(), ResolverError> {
    match semantic_node(&item.node) {
        AstNode::VarDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::ValueBinding,
                item,
                origin,
            )?;
        }
        AstNode::LabDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::LabelBinding,
                item,
                origin,
            )?;
        }
        AstNode::DestructureDecl { pattern, .. } => {
            for name in binding_names(pattern) {
                insert_symbol(
                    program,
                    source_unit_id,
                    scope_id,
                    &name,
                    SymbolKind::DestructureBinding,
                    item,
                    origin.clone(),
                )?;
            }
        }
        AstNode::FunDecl { name, .. }
        | AstNode::ProDecl { name, .. }
        | AstNode::LogDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Routine,
                item,
                origin,
            )?;
        }
        AstNode::TypeDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Type,
                item,
                origin,
            )?;
        }
        AstNode::AliasDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Alias,
                item,
                origin,
            )?;
        }
        AstNode::DefDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Definition,
                item,
                origin,
            )?;
        }
        AstNode::SegDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Segment,
                item,
                origin,
            )?;
        }
        AstNode::StdDecl { name, .. } => {
            insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::Standard,
                item,
                origin,
            )?;
        }
        AstNode::UseDecl {
            name,
            path_type,
            import_target,
            path_segments,
            ..
        } => {
            let symbol_id = insert_symbol(
                program,
                source_unit_id,
                scope_id,
                name,
                SymbolKind::ImportAlias,
                item,
                origin,
            )?;
            insert_import_record(
                program,
                source_unit_id,
                scope_id,
                symbol_id,
                name,
                path_type.clone(),
                import_target.clone(),
                path_segments.clone(),
            );
        }
        AstNode::Comment { .. } => {}
        node => {
            return Err(ResolverError::new(
                ResolverErrorKind::Unsupported,
                format!(
                    "top-level node is not a collectable declaration: {:?}",
                    node
                ),
            ));
        }
    }

    Ok(())
}

fn insert_symbol(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    kind: SymbolKind,
    item: &ParsedTopLevel,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    let canonical_name = fol_types::canonical_identifier_key(name);
    let duplicate_key = top_level_duplicate_key(semantic_node(&item.node), &canonical_name);
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
                "duplicate symbol '{}' conflicts with existing {} declaration first declared at {}",
                name,
                symbol_kind_label(existing.kind),
                existing_site
            ),
            origin.unwrap_or_else(|| {
                existing
                    .origin
                    .clone()
                    .unwrap_or(fol_parser::ast::SyntaxOrigin {
                        file: None,
                        line: 1,
                        column: 1,
                        length: name.len(),
                    })
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
        visibility: item.meta.visibility,
        declaration_scope: item.meta.scope,
        mounted_from: None,
        is_mutable: binding_options(&item.node)
            .map(fol_parser::ast::binding_is_mutable)
            .unwrap_or(false),
    });

    if let Some(symbol) = program.symbols.get_mut(symbol_id) {
        symbol.id = symbol_id;
    }

    let scope = program
        .scopes
        .get_mut(scope_id)
        .expect("top-level symbol target scope should exist");
    scope.symbols.push(symbol_id);
    scope
        .symbol_keys
        .entry(canonical_name)
        .or_default()
        .push(symbol_id);

    Ok(symbol_id)
}

pub(crate) fn semantic_node(node: &AstNode) -> &AstNode {
    match node {
        AstNode::Commented { node, .. } => semantic_node(node),
        _ => node,
    }
}

/// Variable-declaration options for a binding node, unwrapping comments.
/// Returns `None` for nodes that are not `var`/`lab` declarations.
pub(crate) fn binding_options(node: &AstNode) -> Option<&[fol_parser::ast::VarOption]> {
    match semantic_node(node) {
        AstNode::VarDecl { options, .. } | AstNode::LabDecl { options, .. } => Some(options),
        _ => None,
    }
}

pub(crate) fn binding_names(pattern: &BindingPattern) -> Vec<String> {
    let mut names = Vec::new();
    collect_binding_names(pattern, &mut names);
    names
}

fn collect_binding_names(pattern: &BindingPattern, output: &mut Vec<String>) {
    match pattern {
        BindingPattern::Name(name) | BindingPattern::Rest(name) => output.push(name.clone()),
        BindingPattern::Sequence(parts) => {
            for part in parts {
                collect_binding_names(part, output);
            }
        }
    }
}

pub(crate) fn top_level_duplicate_key(node: &AstNode, canonical_name: &str) -> String {
    match node {
        AstNode::FunDecl {
            receiver_type,
            params,
            ..
        }
        | AstNode::ProDecl {
            receiver_type,
            params,
            ..
        }
        | AstNode::LogDecl {
            receiver_type,
            params,
            ..
        } => {
            let receiver = receiver_type
                .as_ref()
                .map(fol_type_key)
                .unwrap_or_else(|| "_".to_string());
            let params = params
                .iter()
                .map(|param| fol_type_key(&param.param_type))
                .collect::<Vec<_>>()
                .join(",");
            format!("routine#{}#{}#{}", canonical_name, receiver, params)
        }
        _ => format!("symbol#{}", canonical_name),
    }
}

pub(crate) fn insert_import_record(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    alias_symbol: SymbolId,
    alias_name: &str,
    path_type: FolType,
    import_target: String,
    path_segments: Vec<UsePathSegment>,
) -> ImportId {
    let import_id = program.imports.push(ResolvedImport {
        id: ImportId(0),
        alias_symbol,
        alias_name: alias_name.to_string(),
        path_type,
        import_target,
        path_segments,
        scope: scope_id,
        source_unit: source_unit_id,
        target_scope: None,
    });
    if let Some(import) = program.imports.get_mut(import_id) {
        import.id = import_id;
    }
    import_id
}

fn fol_type_key(typ: &FolType) -> String {
    match typ {
        FolType::Named { name, .. } => fol_types::canonical_identifier_key(name),
        FolType::QualifiedNamed { path } => path
            .segments
            .iter()
            .map(|segment| fol_types::canonical_identifier_key(segment))
            .collect::<Vec<_>>()
            .join("::"),
        other => other
            .named_text()
            .map(|text| fol_types::canonical_identifier_key(&text))
            .unwrap_or_else(|| format!("{:?}", other)),
    }
}
