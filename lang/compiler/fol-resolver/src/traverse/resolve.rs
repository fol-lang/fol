use crate::{
    errors::symbol_kind_label,
    model::{ResolvedProgram, ResolvedSymbol, ScopeKind, SymbolKind},
    ResolverError, ResolverErrorKind, ScopeId, SymbolId,
};
use fol_diagnostics::{DiagnosticLocation, DiagnosticSuggestion};
use fol_parser::ast::ParsedDeclVisibility;
use fol_parser::ast::QualifiedPath;

pub fn resolve_visible_symbol(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    match resolve_lexical_symbol_of_kinds(program, starting_scope, name, &[], None, origin.clone())
    {
        Ok(symbol_id) => Ok(symbol_id),
        Err(error) if error.kind() == ResolverErrorKind::UnresolvedName => {
            resolve_imported_symbol_of_kinds(program, starting_scope, name, &[], None, origin)
        }
        Err(error) => Err(error),
    }
}

pub fn resolve_lexical_symbol_of_kinds(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    missing_role: Option<&str>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    let canonical_name = fol_types::canonical_identifier_key(name);
    let mut current_scope = Some(starting_scope);

    while let Some(scope_id) = current_scope {
        let symbols = program.symbols_named_in_scope(scope_id, &canonical_name);
        if !symbols.is_empty() {
            let matching_symbols = if allowed_kinds.is_empty() {
                symbols
            } else {
                symbols
                    .into_iter()
                    .filter(|symbol| allowed_kinds.contains(&symbol.kind))
                    .collect::<Vec<_>>()
            };

            if matching_symbols.len() == 1 {
                return Ok(matching_symbols[0].id);
            }
            if matching_symbols.len() > 1 {
                return Err(ambiguity_error_with_optional_origin(
                    lexical_ambiguity_message(name, missing_role, &matching_symbols),
                    origin,
                    &matching_symbols,
                ));
            }

            // This scope has a symbol with the same (case-folded) name but of a
            // non-matching kind — e.g. a value binding `node` shadowing the type
            // `Node`. Types and values occupy distinct namespaces, so a wrong-kind
            // symbol must not halt the lookup: fall through to the enclosing
            // scope, where a matching-kind symbol may live. If none is found in
            // any scope, the post-loop error reports the unresolved name.
        }

        current_scope = program.scope(scope_id).and_then(|scope| scope.parent);
    }

    Err(with_unresolved_name_suggestion(
        error_with_optional_origin(
            ResolverErrorKind::UnresolvedName,
            format!(
                "could not resolve {} '{}'",
                missing_role.unwrap_or("name"),
                name
            ),
            origin.clone(),
        ),
        program,
        starting_scope,
        name,
        allowed_kinds,
        origin.as_ref(),
    ))
}

pub fn resolve_imported_symbol_of_kinds(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    missing_role: Option<&str>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    let canonical_name = fol_types::canonical_identifier_key(name);
    let mut current_scope = Some(starting_scope);
    let mut matches = std::collections::BTreeMap::new();

    while let Some(scope_id) = current_scope {
        for import in program.imports_in_scope(scope_id) {
            let Some(target_scope) = import.target_scope else {
                continue;
            };
            let imported_symbols = program.symbols_named_in_scope(target_scope, &canonical_name);
            if allowed_kinds.is_empty() {
                for symbol in imported_symbols.into_iter().filter(import_visible_symbol) {
                    matches.entry(symbol.id).or_insert(symbol);
                }
            } else {
                for symbol in imported_symbols
                    .into_iter()
                    .filter(import_visible_symbol)
                    .filter(|symbol| allowed_kinds.contains(&symbol.kind))
                {
                    matches.entry(symbol.id).or_insert(symbol);
                }
            }
        }
        current_scope = program.scope(scope_id).and_then(|scope| scope.parent);
    }

    let matches = matches.into_values().collect::<Vec<_>>();

    match matches.as_slice() {
        [symbol] => Ok(symbol.id),
        [] => Err(with_unresolved_name_suggestion(
            error_with_optional_origin(
                ResolverErrorKind::UnresolvedName,
                format!(
                    "could not resolve {} '{}'",
                    missing_role.unwrap_or("name"),
                    name
                ),
                origin.clone(),
            ),
            program,
            starting_scope,
            name,
            allowed_kinds,
            origin.as_ref(),
        )),
        _ => Err(ambiguity_error_with_optional_origin(
            lexical_ambiguity_message(name, missing_role, &matches),
            origin,
            &matches,
        )),
    }
}

pub fn resolve_visible_or_imported_symbol_of_kinds(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    missing_role: Option<&str>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    match resolve_lexical_symbol_of_kinds(
        program,
        starting_scope,
        name,
        allowed_kinds,
        missing_role,
        origin.clone(),
    ) {
        Ok(symbol_id) => Ok(symbol_id),
        Err(error) if error.kind() == ResolverErrorKind::UnresolvedName => {
            resolve_imported_symbol_of_kinds(
                program,
                starting_scope,
                name,
                allowed_kinds,
                missing_role,
                origin,
            )
        }
        Err(error) => Err(error),
    }
}

pub fn resolve_visible_symbol_of_kinds(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    missing_role: Option<&str>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    resolve_lexical_symbol_of_kinds(
        program,
        starting_scope,
        name,
        allowed_kinds,
        missing_role,
        origin,
    )
}

pub fn resolve_qualified_symbol(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    path: &QualifiedPath,
    allowed_kinds: &[SymbolKind],
    missing_role: &str,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    if path.segments.len() < 2 {
        return Err(ResolverError::new(
            ResolverErrorKind::InvalidInput,
            format!(
                "qualified path '{}' must contain at least two segments",
                path.joined()
            ),
        ));
    }

    let root = resolve_qualified_root(
        program,
        starting_scope,
        &path.segments[0],
        &path.joined(),
        missing_role,
        origin.clone(),
    )?;
    match resolve_qualified_from_root(program, root, path, allowed_kinds, missing_role, &origin) {
        Ok(symbol_id) => Ok(symbol_id),
        Err(error) if error.kind() == ResolverErrorKind::UnresolvedName => {
            // A package may import a library whose alias matches the
            // package's own name (an `icetea` workspace importing its
            // `icetea` library). The self-name root shadows the alias, so
            // when nothing resolves under the package root, retry through
            // the import alias before reporting.
            let Some(alias_root) =
                import_alias_root(program, starting_scope, &path.segments[0], &origin)
            else {
                return Err(error);
            };
            resolve_qualified_from_root(
                program,
                alias_root,
                path,
                allowed_kinds,
                missing_role,
                &origin,
            )
            .map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

fn resolve_qualified_from_root(
    program: &ResolvedProgram,
    root: (ScopeId, String, bool),
    path: &QualifiedPath,
    allowed_kinds: &[SymbolKind],
    missing_role: &str,
    origin: &Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<SymbolId, ResolverError> {
    let (mut current_scope, mut current_namespace, crosses_import) = root;
    for segment in &path.segments[1..path.segments.len() - 1] {
        current_namespace.push_str("::");
        current_namespace.push_str(segment);
        current_scope = program.namespace_scope(&current_namespace).ok_or_else(|| {
            error_with_optional_origin(
                ResolverErrorKind::UnresolvedName,
                format!("could not resolve {} '{}'", missing_role, path.joined()),
                origin.clone(),
            )
        })?;
    }

    let final_name = path
        .segments
        .last()
        .expect("qualified paths with at least two segments should have a final segment");
    resolve_symbol_in_scope(
        program,
        current_scope,
        final_name,
        allowed_kinds,
        &path.joined(),
        missing_role,
        origin.clone(),
        crosses_import,
    )
}

/// The import-alias root for a segment, if one is visible: the mounted scope
/// the alias targets. Used as the fallback when the segment also names the
/// current package.
fn import_alias_root(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    root_segment: &str,
    origin: &Option<fol_parser::ast::SyntaxOrigin>,
) -> Option<(ScopeId, String, bool)> {
    let import_symbol = resolve_visible_symbol_of_kinds(
        program,
        starting_scope,
        root_segment,
        &[SymbolKind::ImportAlias],
        Some("import alias"),
        origin.clone(),
    )
    .ok()?;
    let target_scope = program
        .imports
        .iter()
        .find(|import| import.alias_symbol == import_symbol)
        .and_then(|import| import.target_scope)?;
    let namespace = scope_namespace(program, target_scope).ok()?;
    Some((target_scope, namespace, true))
}

fn resolve_qualified_root(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    root_segment: &str,
    full_path: &str,
    missing_role: &str,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<(ScopeId, String, bool), ResolverError> {
    if root_segment == program.package_name() {
        return Ok((
            program.program_scope,
            program.package_name().to_string(),
            false,
        ));
    }

    if let Ok(import_symbol) = resolve_visible_symbol_of_kinds(
        program,
        starting_scope,
        root_segment,
        &[SymbolKind::ImportAlias],
        Some("import alias"),
        origin.clone(),
    ) {
        let import = program
            .imports
            .iter()
            .find(|import| import.alias_symbol == import_symbol)
            .and_then(|import| import.target_scope);
        if let Some(target_scope) = import {
            return Ok((target_scope, scope_namespace(program, target_scope)?, true));
        }
    }

    let namespace = format!("{}::{}", program.package_name(), root_segment);
    if let Some(scope_id) = program.namespace_scope(&namespace) {
        return Ok((scope_id, namespace, false));
    }

    Err(error_with_optional_origin(
        ResolverErrorKind::UnresolvedName,
        format!("could not resolve {} '{}'", missing_role, full_path),
        origin,
    ))
}

#[allow(clippy::too_many_arguments)]
fn resolve_symbol_in_scope(
    program: &ResolvedProgram,
    scope_id: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    full_path: &str,
    missing_role: &str,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
    require_exported: bool,
) -> Result<SymbolId, ResolverError> {
    let canonical_name = fol_types::canonical_identifier_key(name);
    let symbols = program.symbols_named_in_scope(scope_id, &canonical_name);
    let matching_symbols = if allowed_kinds.is_empty() {
        symbols
    } else {
        symbols
            .into_iter()
            .filter(|symbol| allowed_kinds.contains(&symbol.kind))
            .collect::<Vec<_>>()
    };

    // A qualified path that entered through an import alias may only land on
    // exported declarations; the build/workspace path mounts nothing else, so
    // resolving a private symbol here would greenlight code that cannot build.
    if require_exported && !matching_symbols.is_empty() {
        let exported = matching_symbols
            .iter()
            .copied()
            .filter(import_visible_symbol)
            .collect::<Vec<_>>();
        if exported.is_empty() {
            return Err(error_with_optional_origin(
                ResolverErrorKind::UnresolvedName,
                format!(
                    "'{full_path}' names a declaration that is not exported; mark it '[exp]' in its package to import it"
                ),
                origin,
            ));
        }
        return match exported.as_slice() {
            [symbol] => Ok(symbol.id),
            _ => Err(ambiguity_error_with_optional_origin(
                format!(
                    "{} '{}' is ambiguous; candidates: {}",
                    missing_role,
                    full_path,
                    describe_symbol_candidates(&exported)
                ),
                origin,
                &exported,
            )),
        };
    }

    match matching_symbols.as_slice() {
        [symbol] => Ok(symbol.id),
        [] => Err(with_unresolved_name_suggestion(
            error_with_optional_origin(
                ResolverErrorKind::UnresolvedName,
                format!("could not resolve {} '{}'", missing_role, full_path),
                origin.clone(),
            ),
            program,
            scope_id,
            name,
            allowed_kinds,
            origin.as_ref(),
        )),
        _ => Err(ambiguity_error_with_optional_origin(
            format!(
                "{} '{}' is ambiguous; candidates: {}",
                missing_role,
                full_path,
                describe_symbol_candidates(&matching_symbols)
            ),
            origin,
            &matching_symbols,
        )),
    }
}

pub fn error_with_optional_origin(
    kind: ResolverErrorKind,
    message: String,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> ResolverError {
    match origin {
        Some(origin) => ResolverError::with_origin(kind, message, origin),
        None => ResolverError::new(kind, message),
    }
}

fn with_unresolved_name_suggestion(
    error: ResolverError,
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
    origin: Option<&fol_parser::ast::SyntaxOrigin>,
) -> ResolverError {
    let Some(origin) = origin else {
        return error;
    };
    let Some(replacement) =
        closest_visible_symbol_name(program, starting_scope, name, allowed_kinds)
    else {
        return error;
    };
    error.with_suggestion(DiagnosticSuggestion {
        message: format!("replace with '{replacement}'"),
        replacement: Some(replacement),
        location: Some(DiagnosticLocation {
            file: origin.file.clone(),
            line: origin.line,
            column: origin.column,
            length: Some(origin.length),
        }),
    })
}

pub fn ambiguity_error_with_optional_origin(
    message: String,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
    symbols: &[&ResolvedSymbol],
) -> ResolverError {
    let mut error =
        error_with_optional_origin(ResolverErrorKind::AmbiguousReference, message, origin);
    let mut seen = std::collections::BTreeSet::new();

    for symbol in symbols {
        let Some(symbol_origin) = symbol.origin.clone() else {
            continue;
        };
        let dedupe_key = (
            symbol_origin.file.clone(),
            symbol_origin.line,
            symbol_origin.column,
            symbol_origin.length,
        );
        if seen.insert(dedupe_key) {
            error = error.with_related_origin(
                symbol_origin,
                format!("candidate {} declaration", symbol_kind_label(symbol.kind)),
            );
        }
    }

    error
}

pub fn qualified_path_origin(
    program: &ResolvedProgram,
    path: &QualifiedPath,
) -> Option<fol_parser::ast::SyntaxOrigin> {
    path.syntax_id()
        .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
        .cloned()
}

fn scope_namespace(program: &ResolvedProgram, scope_id: ScopeId) -> Result<String, ResolverError> {
    let kind = program
        .scope(scope_id)
        .map(|scope| &scope.kind)
        .ok_or_else(|| {
            ResolverError::new(
                ResolverErrorKind::Internal,
                "qualified path scope should exist",
            )
        })?;
    match kind {
        ScopeKind::ProgramRoot { package } => Ok(package.clone()),
        ScopeKind::NamespaceRoot { namespace } => Ok(namespace.clone()),
        other => Err(ResolverError::new(
            ResolverErrorKind::Internal,
            format!("qualified path root scope must be package or namespace, got {other:?}"),
        )),
    }
}

fn describe_symbol_candidates(symbols: &[&ResolvedSymbol]) -> String {
    use crate::errors::format_origin_brief;
    symbols
        .iter()
        .map(|symbol| {
            let site = symbol
                .origin
                .as_ref()
                .map(format_origin_brief)
                .unwrap_or_else(|| "an unknown location".to_string());
            format!(
                "{} '{}' at {}",
                symbol_kind_label(symbol.kind),
                symbol.name,
                site
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn import_visible_symbol(symbol: &&ResolvedSymbol) -> bool {
    symbol.visibility == Some(ParsedDeclVisibility::Exported)
}

fn lexical_ambiguity_message(
    name: &str,
    missing_role: Option<&str>,
    symbols: &[&ResolvedSymbol],
) -> String {
    let subject = match missing_role {
        Some(role) => format!("{role} '{name}'"),
        None => format!("name '{name}'"),
    };
    format!(
        "{subject} is ambiguous in lexical scope; candidates: {}",
        describe_symbol_candidates(symbols)
    )
}

fn closest_visible_symbol_name(
    program: &ResolvedProgram,
    starting_scope: ScopeId,
    name: &str,
    allowed_kinds: &[SymbolKind],
) -> Option<String> {
    let needle = fol_types::canonical_identifier_key(name);
    let mut candidates = std::collections::BTreeSet::new();
    let mut current_scope = Some(starting_scope);

    while let Some(scope_id) = current_scope {
        for symbol in program.symbols_in_scope(scope_id) {
            if !allowed_kinds.is_empty() && !allowed_kinds.contains(&symbol.kind) {
                continue;
            }
            candidates.insert(symbol.name.clone());
        }
        for import in program.imports_in_scope(scope_id) {
            let Some(target_scope) = import.target_scope else {
                continue;
            };
            for symbol in program
                .symbols_in_scope(target_scope)
                .into_iter()
                .filter(import_visible_symbol)
            {
                if !allowed_kinds.is_empty() && !allowed_kinds.contains(&symbol.kind) {
                    continue;
                }
                candidates.insert(symbol.name.clone());
            }
        }
        current_scope = program.scope(scope_id).and_then(|scope| scope.parent);
    }

    candidates
        .into_iter()
        .filter(|candidate| fol_types::canonical_identifier_key(candidate) != needle)
        .filter_map(|candidate| {
            let canonical = fol_types::canonical_identifier_key(&candidate);
            let score = bounded_edit_distance(&needle, &canonical, 3)?;
            Some((score, candidate))
        })
        .min_by(|(left_score, left_name), (right_score, right_name)| {
            left_score.cmp(right_score).then(left_name.cmp(right_name))
        })
        .and_then(|(score, candidate)| {
            let max_distance = if needle.len() <= 4 { 1 } else { 2 };
            (score <= max_distance).then_some(candidate)
        })
}

fn bounded_edit_distance(left: &str, right: &str, limit: usize) -> Option<usize> {
    if left == right {
        return Some(0);
    }
    if left.len().abs_diff(right.len()) > limit {
        return None;
    }
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let cost = usize::from(left_char != *right_char);
            current[right_index + 1] = std::cmp::min(
                std::cmp::min(previous[right_index + 1] + 1, current[right_index] + 1),
                previous[right_index] + cost,
            );
            row_min = row_min.min(current[right_index + 1]);
        }
        if row_min > limit {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }

    (previous[right_chars.len()] <= limit).then_some(previous[right_chars.len()])
}
