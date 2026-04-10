use crate::{EditorDocument, LspCompletionContext, LspPosition};

use super::types::EditorCompletionItem;

pub(super) const FALLBACK_ROUTINE_PREFIXES: &[&str] =
    &["fun[] ", "fun[", "log[] ", "log[", "pro[] ", "pro["];
pub(super) const FALLBACK_TYPE_PREFIXES: &[&str] = &["typ[] ", "typ[", "typ "];
pub(super) const FALLBACK_ALIAS_PREFIXES: &[&str] = &["ali[] ", "ali[", "ali "];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    Plain,
    TypePosition,
    QualifiedPath { qualifier: String },
    DotTrigger,
}

pub(crate) fn completion_context(
    document: &EditorDocument,
    position: LspPosition,
) -> CompletionContext {
    let Some(offset) = position_to_offset(&document.text, position) else {
        return CompletionContext::Plain;
    };
    let prefix = &document.text[..offset];
    let line_prefix = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail)
        .unwrap_or(prefix);
    let trimmed = line_prefix.trim_end();

    if trimmed.ends_with('.') {
        return CompletionContext::DotTrigger;
    }

    if let Some((qualifier, _)) = trimmed.rsplit_once("::") {
        let qualifier = qualifier
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
            .next()
            .unwrap_or("")
            .trim_matches(':')
            .to_string();
        if !qualifier.is_empty() {
            return CompletionContext::QualifiedPath { qualifier };
        }
    }

    if line_prefix
        .rsplit_once(':')
        .map(|(_, tail)| tail.trim())
        .is_some_and(|tail| {
            tail.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
        })
    {
        return CompletionContext::TypePosition;
    }

    CompletionContext::Plain
}

pub(crate) fn completion_context_with_lsp(
    document: &EditorDocument,
    position: LspPosition,
    context: Option<&LspCompletionContext>,
) -> CompletionContext {
    if let Some(context) = context {
        if context.trigger_character.as_deref() == Some(".") {
            return CompletionContext::DotTrigger;
        }
        if context.trigger_character.as_deref() == Some(":") {
            let Some(offset) = position_to_offset(&document.text, position) else {
                return CompletionContext::Plain;
            };
            let prefix = &document.text[..offset];
            let line_prefix = prefix
                .rsplit_once('\n')
                .map(|(_, tail)| tail)
                .unwrap_or(prefix);
            let trimmed = line_prefix.trim_end();
            if let Some((qualifier, _)) = trimmed.rsplit_once("::") {
                let qualifier = qualifier
                    .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
                    .next()
                    .unwrap_or("")
                    .trim_matches(':')
                    .to_string();
                if !qualifier.is_empty() {
                    return CompletionContext::QualifiedPath { qualifier };
                }
            }
        }
    }
    completion_context(document, position)
}

pub(super) fn position_to_offset(text: &str, position: LspPosition) -> Option<usize> {
    let mut line = 0u32;
    let mut character = 0u32;
    for (offset, ch) in text.char_indices() {
        if line == position.line && character == position.character {
            return Some(offset);
        }
        if ch == '\n' {
            line += 1;
            character = 0;
            if line == position.line && position.character == 0 {
                return Some(offset + ch.len_utf8());
            }
        } else if line == position.line {
            character += 1;
        }
    }

    (line == position.line && character == position.character).then_some(text.len())
}

pub(super) fn fallback_decl_name(line: &str, prefixes: &[&str]) -> Option<String> {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = if prefix.ends_with('[') {
                rest.split_once(']')
                    .map(|(_, tail)| tail.trim_start())
                    .unwrap_or(rest)
            } else {
                rest
            };
            let name = rest
                .split(|ch: char| ch == ':' || ch == '=' || ch == '(' || ch.is_whitespace())
                .next()
                .unwrap_or("")
                .trim_matches(|ch: char| ch == '[' || ch == ']');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub(super) fn current_routine_name(text: &str, position: LspPosition) -> Option<String> {
    let offset = position_to_offset(text, position).unwrap_or(text.len());
    let before_cursor = &text[..offset];
    let header = before_cursor
        .rmatch_indices("fun")
        .next()
        .map(|(index, _)| &before_cursor[index..])?;
    let rest = header.strip_prefix("fun").unwrap_or(header);
    let rest =
        rest.trim_start_matches(|ch: char| ch == '[' || ch == ']' || !ch.is_ascii_alphanumeric());
    let open = rest.find('(')?;
    let name = rest[..open]
        .trim()
        .trim_matches(|ch: char| ch == '[' || ch == ']');
    (!name.is_empty()).then(|| name.to_string())
}

pub(super) fn fallback_items_from_package_dir(root: &std::path::Path) -> Vec<EditorCompletionItem> {
    let mut items = Vec::new();
    collect_fallback_items_from_dir(root, &mut items);
    items
}

pub(super) fn collect_fallback_items_from_dir(
    root: &std::path::Path,
    items: &mut Vec<EditorCompletionItem>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_fallback_items_from_dir(&path, items);
            continue;
        }
        if !file_type.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("fol") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(name) =
                fallback_decl_name(trimmed, &["fun[exp] ", "fun[", "log[exp] ", "log[", "pro[exp] ", "pro["])
            {
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 3,
                    detail: Some("routine".to_string()),
                    insert_text: None,
                });
            } else if let Some(name) = fallback_decl_name(trimmed, &["typ[exp] ", "typ["]) {
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 22,
                    detail: Some("type".to_string()),
                    insert_text: None,
                });
            } else if let Some(name) = fallback_decl_name(trimmed, &["ali[exp] ", "ali["]) {
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 22,
                    detail: Some("type alias".to_string()),
                    insert_text: None,
                });
            }
        }
    }
}

pub(super) fn render_symbol_kind(kind: fol_resolver::SymbolKind) -> &'static str {
    kind.display_name()
}

pub(super) fn symbol_kind_code(kind: fol_resolver::SymbolKind) -> u8 {
    match kind {
        fol_resolver::SymbolKind::Routine | fol_resolver::SymbolKind::Definition => 12,
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias => 5,
        fol_resolver::SymbolKind::ImportAlias => 3,
        fol_resolver::SymbolKind::ValueBinding
        | fol_resolver::SymbolKind::LabelBinding
        | fol_resolver::SymbolKind::DestructureBinding
        | fol_resolver::SymbolKind::Parameter
        | fol_resolver::SymbolKind::Capture
        | fol_resolver::SymbolKind::GenericParameter
        | fol_resolver::SymbolKind::LoopBinder
        | fol_resolver::SymbolKind::RollingBinder => 13,
        fol_resolver::SymbolKind::Segment => 2,
        fol_resolver::SymbolKind::Implementation | fol_resolver::SymbolKind::Standard => 6,
    }
}

pub(super) fn render_checked_type(
    table: &fol_typecheck::TypeTable,
    type_id: fol_typecheck::CheckedTypeId,
) -> String {
    table.render_type(type_id)
}

pub(super) fn dedupe_completion_items(items: Vec<EditorCompletionItem>) -> Vec<EditorCompletionItem> {
    let mut best_by_label = std::collections::BTreeMap::new();
    for item in items {
        if item.label.is_empty() {
            continue;
        }
        let label = item.label.clone();
        match best_by_label.get(&label) {
            Some(current) if completion_item_cmp(&item, current).is_lt() => {
                best_by_label.insert(label, item);
            }
            None => {
                best_by_label.insert(label, item);
            }
            _ => {}
        }
    }
    let mut filtered = best_by_label.into_values().collect::<Vec<_>>();
    filtered.sort_by(completion_item_cmp);
    filtered
}

pub(super) fn mark_fallback_completion_items(
    items: Vec<EditorCompletionItem>,
) -> Vec<EditorCompletionItem> {
    items.into_iter()
        .map(|mut item| {
            item.detail = Some(match item.detail {
                Some(detail) => format!("{detail} (fallback)"),
                None => "fallback".to_string(),
            });
            item
        })
        .collect()
}

fn completion_item_cmp(
    left: &EditorCompletionItem,
    right: &EditorCompletionItem,
) -> std::cmp::Ordering {
    completion_item_priority(left)
        .cmp(&completion_item_priority(right))
        .then(completion_item_detail_priority(left).cmp(&completion_item_detail_priority(right)))
        .then(left.label.cmp(&right.label))
        .then(left.detail.cmp(&right.detail))
        .then(left.insert_text.cmp(&right.insert_text))
}

fn completion_item_priority(item: &EditorCompletionItem) -> u8 {
    match item.kind {
        6 => 0,
        3 | 12 => 1,
        22 => 2,
        9 => 3,
        2 => 4,
        _ => 5,
    }
}

fn completion_item_detail_priority(item: &EditorCompletionItem) -> u8 {
    match item.detail.as_deref() {
        Some("builtin type") => 0,
        Some("type") | Some("type alias") => 1,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        completion_context, completion_context_with_lsp, dedupe_completion_items,
        fallback_decl_name, mark_fallback_completion_items,
        FALLBACK_ALIAS_PREFIXES, FALLBACK_ROUTINE_PREFIXES, FALLBACK_TYPE_PREFIXES,
        CompletionContext,
        EditorCompletionItem,
    };
    use crate::{EditorDocument, EditorDocumentUri, LspCompletionContext, LspPosition};
    use std::path::PathBuf;

    #[test]
    fn dedupe_completion_items_keeps_higher_priority_symbol_for_same_label() {
        let items = dedupe_completion_items(vec![
            EditorCompletionItem {
                label: "helper".to_string(),
                kind: 3,
                detail: Some("routine".to_string()),
                insert_text: None,
            },
            EditorCompletionItem {
                label: "helper".to_string(),
                kind: 6,
                detail: Some("binding".to_string()),
                insert_text: None,
            },
        ]);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "helper");
        assert_eq!(items[0].detail.as_deref(), Some("binding"));
    }

    #[test]
    fn completion_context_with_lsp_prefers_explicit_dot_trigger() {
        let uri = EditorDocumentUri::from_file_path(PathBuf::from("/tmp/context.fol")).unwrap();
        let document = EditorDocument::new(uri, 1, "fun[] main(): int = {\n    return \n};\n".to_string())
            .unwrap();

        let context = completion_context_with_lsp(
            &document,
            LspPosition {
                line: 1,
                character: 12,
            },
            Some(&LspCompletionContext {
                trigger_kind: Some(2),
                trigger_character: Some(".".to_string()),
            }),
        );

        assert_eq!(context, CompletionContext::DotTrigger);
    }

    #[test]
    fn completion_context_defaults_to_plain_positions() {
        let uri = EditorDocumentUri::from_file_path(PathBuf::from("/tmp/plain_context.fol")).unwrap();
        let document = EditorDocument::new(
            uri,
            1,
            "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    ret\n};\n"
                .to_string(),
        )
        .unwrap();

        let context = completion_context(
            &document,
            LspPosition {
                line: 5,
                character: 7,
            },
        );

        assert_eq!(context, CompletionContext::Plain);
    }

    #[test]
    fn fallback_prefix_tables_match_current_v1_declaration_surface() {
        assert_eq!(
            FALLBACK_ROUTINE_PREFIXES,
            &["fun[] ", "fun[", "log[] ", "log[", "pro[] ", "pro["]
        );
        assert_eq!(FALLBACK_TYPE_PREFIXES, &["typ[] ", "typ[", "typ "]);
        assert_eq!(FALLBACK_ALIAS_PREFIXES, &["ali[] ", "ali[", "ali "]);
        assert!(fallback_decl_name("def[] old(): int = {", FALLBACK_ROUTINE_PREFIXES).is_none());
        assert_eq!(
            fallback_decl_name("fun[exp] helper(): int = {", FALLBACK_ROUTINE_PREFIXES).as_deref(),
            Some("helper")
        );
        assert_eq!(
            fallback_decl_name("typ[exp] LocalRec: rec = {", FALLBACK_TYPE_PREFIXES).as_deref(),
            Some("LocalRec")
        );
        assert_eq!(
            fallback_decl_name("ali[exp] LocalAlias: int;", FALLBACK_ALIAS_PREFIXES).as_deref(),
            Some("LocalAlias")
        );
    }

    #[test]
    fn mark_fallback_completion_items_preserves_labels_and_marks_details() {
        let items = mark_fallback_completion_items(vec![
            EditorCompletionItem {
                label: "helper".to_string(),
                kind: 3,
                detail: Some("routine".to_string()),
                insert_text: None,
            },
            EditorCompletionItem {
                label: "mystery".to_string(),
                kind: 1,
                detail: None,
                insert_text: None,
            },
        ]);

        assert_eq!(items[0].label, "helper");
        assert_eq!(items[0].detail.as_deref(), Some("routine (fallback)"));
        assert_eq!(items[1].detail.as_deref(), Some("fallback"));
    }
}

pub(super) fn completion_builtin_type_item(label: &str) -> EditorCompletionItem {
    EditorCompletionItem {
        label: label.to_string(),
        kind: 22,
        detail: Some("builtin type".to_string()),
        insert_text: None,
    }
}

pub(super) fn completion_namespace_item(label: String) -> EditorCompletionItem {
    EditorCompletionItem {
        label,
        kind: 9,
        detail: Some("namespace".to_string()),
        insert_text: None,
    }
}

pub(super) fn completion_intrinsic_item(label: &str) -> EditorCompletionItem {
    EditorCompletionItem {
        label: label.to_string(),
        kind: 2,
        detail: Some("intrinsic".to_string()),
        insert_text: Some(label.to_string()),
    }
}

pub(super) fn completion_item_from_symbol(
    symbol: &fol_resolver::ResolvedSymbol,
) -> EditorCompletionItem {
    EditorCompletionItem {
        label: symbol.name.clone(),
        kind: completion_symbol_kind(symbol.kind),
        detail: Some(completion_symbol_detail(symbol.kind).to_string()),
        insert_text: None,
    }
}

pub(super) fn completion_symbol_detail(kind: fol_resolver::SymbolKind) -> &'static str {
    match kind {
        fol_resolver::SymbolKind::Type => "type",
        fol_resolver::SymbolKind::Alias => "type alias",
        fol_resolver::SymbolKind::Routine => "routine",
        fol_resolver::SymbolKind::Definition => "definition",
        fol_resolver::SymbolKind::ValueBinding
        | fol_resolver::SymbolKind::LabelBinding
        | fol_resolver::SymbolKind::DestructureBinding
        | fol_resolver::SymbolKind::LoopBinder
        | fol_resolver::SymbolKind::RollingBinder => "binding",
        fol_resolver::SymbolKind::Parameter | fol_resolver::SymbolKind::GenericParameter => {
            "parameter"
        }
        fol_resolver::SymbolKind::Capture => "capture",
        fol_resolver::SymbolKind::ImportAlias => "namespace",
        fol_resolver::SymbolKind::Segment => "namespace segment",
        fol_resolver::SymbolKind::Implementation => "implementation",
        fol_resolver::SymbolKind::Standard => "standard",
    }
}

pub(super) fn completion_symbol_kind(kind: fol_resolver::SymbolKind) -> u8 {
    match kind {
        fol_resolver::SymbolKind::Routine => 3,
        fol_resolver::SymbolKind::Definition => 12,
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias => 22,
        fol_resolver::SymbolKind::ImportAlias | fol_resolver::SymbolKind::Segment => 9,
        fol_resolver::SymbolKind::Implementation | fol_resolver::SymbolKind::Standard => 12,
        fol_resolver::SymbolKind::ValueBinding
        | fol_resolver::SymbolKind::LabelBinding
        | fol_resolver::SymbolKind::DestructureBinding
        | fol_resolver::SymbolKind::Parameter
        | fol_resolver::SymbolKind::Capture
        | fol_resolver::SymbolKind::GenericParameter
        | fol_resolver::SymbolKind::LoopBinder
        | fol_resolver::SymbolKind::RollingBinder => 6,
    }
}

pub(super) fn completion_symbol_is_root_visible(
    program: &fol_resolver::ResolvedProgram,
    symbol: &fol_resolver::ResolvedSymbol,
) -> bool {
    matches!(
        program.scope(symbol.scope).map(|scope| &scope.kind),
        Some(
            fol_resolver::ScopeKind::ProgramRoot { .. }
                | fol_resolver::ScopeKind::NamespaceRoot { .. }
                | fol_resolver::ScopeKind::SourceUnitRoot { .. }
        )
    )
}

pub(super) fn completion_symbol_is_plain_top_level_candidate(
    program: &fol_resolver::ResolvedProgram,
    symbol: &fol_resolver::ResolvedSymbol,
) -> bool {
    completion_symbol_is_root_visible(program, symbol)
        && matches!(
            symbol.kind,
            fol_resolver::SymbolKind::Routine
                | fol_resolver::SymbolKind::Type
                | fol_resolver::SymbolKind::Alias
                | fol_resolver::SymbolKind::Definition
                | fol_resolver::SymbolKind::ValueBinding
        )
}

pub(super) fn symbol_visibility_matches_namespace_root(
    symbol: &fol_resolver::ResolvedSymbol,
    imported_root: bool,
) -> bool {
    if imported_root {
        symbol.mounted_from.is_some()
    } else {
        symbol.mounted_from.is_none()
    }
}
