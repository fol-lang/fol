use crate::{EditorDocument, LspCompletionContext, LspPosition};

use super::types::EditorCompletionItem;

pub(super) const FALLBACK_ROUTINE_PREFIXES: &[&str] =
    &["fun[] ", "fun[", "log[] ", "log[", "pro[] ", "pro["];
pub(super) const FALLBACK_TYPE_PREFIXES: &[&str] = &["typ[] ", "typ[", "typ "];
pub(super) const FALLBACK_ALIAS_PREFIXES: &[&str] = &["ali[] ", "ali[", "ali "];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    Plain,
    PipeStage,
    TypePosition,
    ParameterOption {
        allow_borrow: bool,
        allow_mutex: bool,
    },
    PointerTypePosition {
        allow_shared: bool,
    },
    ChannelElementTypePosition,
    NestedTypePosition {
        forbid_channel: bool,
    },
    OwnedTypePosition {
        forbid_channel: bool,
    },
    BracketAccess {
        receiver: String,
    },
    HeapBinding,
    QualifiedPath {
        qualifier: String,
    },
    DotTrigger {
        receiver: Option<String>,
    },
}

pub(crate) fn completion_context(
    document: &EditorDocument,
    position: LspPosition,
) -> CompletionContext {
    let Some(offset) = position_to_offset(&document.text, position) else {
        return CompletionContext::Plain;
    };
    let source_scan = crate::source_scan::scan_source(&document.text[..offset]);
    if source_scan.terminal_protected {
        return CompletionContext::Plain;
    }
    let prefix = source_scan.masked_code.as_str();
    let line_prefix = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail)
        .unwrap_or(prefix);
    let trimmed = line_prefix.trim_end();

    if trimmed.ends_with('@') {
        let at_offset = prefix.len().saturating_sub(1);
        if at_sign_starts_owned_type(prefix, at_offset) {
            return CompletionContext::OwnedTypePosition {
                forbid_channel: true,
            };
        }
        return CompletionContext::HeapBinding;
    }

    if trimmed.rsplit_once('|').is_some_and(|(left, stage)| {
        !left.ends_with('|')
            && stage
                .trim_start()
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }) {
        return CompletionContext::PipeStage;
    }

    if trimmed.ends_with('.') {
        return CompletionContext::DotTrigger {
            receiver: trailing_dot_receiver(trimmed),
        };
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

    if let Some((receiver, receiver_start, open, type_ancestor_forbids_channel)) =
        trailing_bracket_receiver(prefix)
    {
        if is_routine_parameter_bracket(prefix, receiver_start) {
            let options = &prefix[open + 1..];
            let has_borrow = options
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|option| matches!(option, "bor" | "borrow" | "borrowing"));
            let has_mutex = options
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|option| matches!(option, "mux" | "mutex"));
            return CompletionContext::ParameterOption {
                allow_borrow: !has_borrow && !has_mutex,
                allow_mutex: !has_borrow && !has_mutex,
            };
        }
        match receiver.as_str() {
            "ptr" => {
                let tail = &prefix[open + 1..];
                return CompletionContext::PointerTypePosition {
                    allow_shared: !tail.contains(',')
                        && !tail
                            .split(|ch: char| !ch.is_ascii_alphanumeric())
                            .any(|part| part == "shared"),
                };
            }
            "chn" => return CompletionContext::ChannelElementTypePosition,
            _ if type_ancestor_forbids_channel => {
                return CompletionContext::NestedTypePosition {
                    forbid_channel: true,
                };
            }
            _ if bracket_receiver_is_in_type_position(prefix, receiver_start) => {
                return CompletionContext::NestedTypePosition {
                    forbid_channel: false,
                };
            }
            _ => return CompletionContext::BracketAccess { receiver },
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

fn trailing_dot_receiver(line_prefix: &str) -> Option<String> {
    let before_dot = line_prefix.strip_suffix('.')?;
    if before_dot
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace)
    {
        return None;
    }
    let receiver_start = before_dot
        .char_indices()
        .rev()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let receiver = &before_dot[receiver_start..];
    (!receiver.is_empty()).then(|| receiver.to_string())
}

fn trailing_bracket_receiver(prefix: &str) -> Option<(String, usize, usize, bool)> {
    let mut opens = Vec::new();
    for (index, ch) in prefix.char_indices() {
        match ch {
            '[' => opens.push(index),
            ']' => {
                let _ = opens.pop();
            }
            _ => {}
        }
    }
    let open = *opens.last()?;
    let tail = &prefix[open + 1..];
    if !tail.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '_' | ',' | ':' | '@' | '?' | '[' | ']')
            || ch.is_whitespace()
    }) {
        return None;
    }

    let before_open = &prefix[..open];
    let receiver_end = before_open.trim_end().len();
    let receiver_start = before_open[..receiver_end]
        .char_indices()
        .rev()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let receiver = before_open[receiver_start..receiver_end].to_string();
    if receiver.is_empty() {
        return None;
    }
    let type_ancestor_forbids_channel = opens[..opens.len().saturating_sub(1)]
        .iter()
        .filter_map(|open| bracket_receiver_before(prefix, *open).map(|(name, _)| name))
        .any(|name| matches!(name.as_str(), "ptr" | "chn"));
    Some((
        receiver,
        receiver_start,
        open,
        type_ancestor_forbids_channel,
    ))
}

fn bracket_receiver_before(prefix: &str, open: usize) -> Option<(String, usize)> {
    let before_open = &prefix[..open];
    let receiver_end = before_open.trim_end().len();
    let receiver_start = before_open[..receiver_end]
        .char_indices()
        .rev()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let receiver = before_open[receiver_start..receiver_end].to_string();
    (!receiver.is_empty()).then_some((receiver, receiver_start))
}

fn bracket_receiver_is_in_type_position(prefix: &str, receiver_start: usize) -> bool {
    let before_receiver = prefix[..receiver_start].trim_end();
    if before_receiver
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, ':' | '@' | '?' | '[' | ','))
    {
        return true;
    }
    before_receiver
        .rsplit_once('=')
        .is_some_and(|(head, tail)| {
            tail.trim().is_empty()
                && head
                    .split(|ch: char| !ch.is_ascii_alphanumeric())
                    .any(|word| matches!(word, "ali" | "typ"))
        })
}

fn at_sign_starts_owned_type(prefix: &str, at_offset: usize) -> bool {
    let before = prefix[..at_offset].trim_end();
    if before
        .chars()
        .next_back()
        .is_some_and(|ch| matches!(ch, ':' | '[' | ',' | '?' | '@'))
    {
        return true;
    }
    if before
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .next_back()
        .is_some_and(|word| word == "opt")
    {
        return true;
    }
    before.rsplit_once('=').is_some_and(|(head, tail)| {
        tail.trim().is_empty()
            && head
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|word| matches!(word, "ali" | "typ"))
    })
}

fn is_routine_parameter_bracket(prefix: &str, receiver_start: usize) -> bool {
    let before_receiver = &prefix[..receiver_start];
    let Some(parameter_open) = last_unclosed_round_bracket(before_receiver) else {
        return false;
    };
    let parameter_prefix = &before_receiver[parameter_open + 1..];
    let current_parameter = parameter_prefix
        .rsplit([',', ';'])
        .next()
        .unwrap_or(parameter_prefix);
    if current_parameter.contains(':') || current_parameter.contains('=') {
        return false;
    }
    let header_prefix = &before_receiver[..parameter_open];
    let header_start = header_prefix
        .rfind([';', '{', '}'])
        .map_or(0, |index| index + 1);
    header_prefix[header_start..]
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|token| matches!(token, "fun" | "pro" | "log"))
}

fn last_unclosed_round_bracket(text: &str) -> Option<usize> {
    let mut closed = 0usize;
    for (index, ch) in text.char_indices().rev() {
        match ch {
            ')' => closed += 1,
            '(' if closed == 0 => return Some(index),
            '(' => closed -= 1,
            _ => {}
        }
    }
    None
}

pub(crate) fn completion_context_with_lsp(
    document: &EditorDocument,
    position: LspPosition,
    context: Option<&LspCompletionContext>,
) -> CompletionContext {
    if completion_cursor_is_protected(document, position) {
        return CompletionContext::Plain;
    }
    if let Some(context) = context {
        if context.trigger_character.as_deref() == Some(".") {
            return match completion_context(document, position) {
                context @ CompletionContext::DotTrigger { .. } => context,
                _ => CompletionContext::DotTrigger { receiver: None },
            };
        }
        if context.trigger_character.as_deref() == Some(":") {
            let Some(offset) = position_to_offset(&document.text, position) else {
                return CompletionContext::Plain;
            };
            let masked_prefix = crate::source_scan::mask_non_code(&document.text[..offset]);
            let prefix = masked_prefix.as_str();
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

pub(crate) fn completion_cursor_is_protected(
    document: &EditorDocument,
    position: LspPosition,
) -> bool {
    position_to_offset(&document.text, position).is_some_and(|offset| {
        crate::source_scan::scan_source(&document.text[..offset]).terminal_protected
    })
}

pub(super) fn position_to_offset(text: &str, position: LspPosition) -> Option<usize> {
    crate::positions::scalar_position_to_offset(text, position)
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
    let masked = crate::source_scan::mask_non_code(&text[..offset]);
    let before_cursor = masked.as_str();
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
        let masked = crate::source_scan::mask_non_code(&text);
        for line in masked.lines() {
            let trimmed = line.trim();
            if let Some(name) = fallback_decl_name(
                trimmed,
                &[
                    "fun[exp] ",
                    "fun[",
                    "log[exp] ",
                    "log[",
                    "pro[exp] ",
                    "pro[",
                ],
            ) {
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
        fol_resolver::SymbolKind::Standard => 6,
    }
}

pub(super) fn render_checked_type(
    table: &fol_typecheck::TypeTable,
    type_id: fol_typecheck::CheckedTypeId,
) -> String {
    table.render_type(type_id)
}

pub(super) fn dedupe_completion_items(
    items: Vec<EditorCompletionItem>,
) -> Vec<EditorCompletionItem> {
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
    items
        .into_iter()
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
        fallback_decl_name, mark_fallback_completion_items, CompletionContext,
        EditorCompletionItem, FALLBACK_ALIAS_PREFIXES, FALLBACK_ROUTINE_PREFIXES,
        FALLBACK_TYPE_PREFIXES,
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
        let document = EditorDocument::new(
            uri,
            1,
            "fun[] main(): int = {\n    return \n};\n".to_string(),
        )
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

        assert_eq!(context, CompletionContext::DotTrigger { receiver: None });
    }

    #[test]
    fn completion_context_defaults_to_plain_positions() {
        let uri =
            EditorDocumentUri::from_file_path(PathBuf::from("/tmp/plain_context.fol")).unwrap();
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
    fn completion_context_detects_pipe_stage_prefixes_without_matching_recovery_pipes() {
        let uri =
            EditorDocumentUri::from_file_path(PathBuf::from("/tmp/pipe_context.fol")).unwrap();
        let document = EditorDocument::new(
            uri,
            1,
            "fun[] main(): int = {\n    var pending = work(1) | aw\n};\n".to_string(),
        )
        .unwrap();
        assert_eq!(
            completion_context(
                &document,
                LspPosition {
                    line: 1,
                    character: 30,
                },
            ),
            CompletionContext::PipeStage
        );

        let uri =
            EditorDocumentUri::from_file_path(PathBuf::from("/tmp/recovery_context.fol")).unwrap();
        let document = EditorDocument::new(
            uri,
            1,
            "fun[] main(): int = {\n    return probe() || fallback\n};\n".to_string(),
        )
        .unwrap();
        assert_eq!(
            completion_context(
                &document,
                LspPosition {
                    line: 1,
                    character: 30,
                },
            ),
            CompletionContext::Plain
        );
    }

    #[test]
    fn completion_context_detects_v3_bracket_and_heap_surfaces() {
        let cases = [
            (
                "fun[] inspect(item[",
                CompletionContext::ParameterOption {
                    allow_borrow: true,
                    allow_mutex: true,
                },
            ),
            (
                "fun[] inspect(default: int = make(), item[",
                CompletionContext::ParameterOption {
                    allow_borrow: true,
                    allow_mutex: true,
                },
            ),
            (
                "fun[] main(): int = { var value: ptr[",
                CompletionContext::PointerTypePosition { allow_shared: true },
            ),
            (
                "fun[] main(): int = { var value: ptr[shared, ",
                CompletionContext::PointerTypePosition {
                    allow_shared: false,
                },
            ),
            (
                "fun[] main(): int = { var value: ptr[vec[",
                CompletionContext::NestedTypePosition {
                    forbid_channel: true,
                },
            ),
            (
                "fun[] main(): int = { var channel: chn[",
                CompletionContext::ChannelElementTypePosition,
            ),
            (
                "fun[] main(): int = { return channel[",
                CompletionContext::BracketAccess {
                    receiver: "channel".to_string(),
                },
            ),
            ("fun[] main(): int = { @", CompletionContext::HeapBinding),
            (
                "fun[] main(): int = { var value: @",
                CompletionContext::OwnedTypePosition {
                    forbid_channel: true,
                },
            ),
        ];

        for (index, (text, expected)) in cases.into_iter().enumerate() {
            let uri = EditorDocumentUri::from_file_path(PathBuf::from(format!(
                "/tmp/v3_completion_context_{index}.fol"
            )))
            .unwrap();
            let document = EditorDocument::new(uri, 1, text.to_string()).unwrap();
            assert_eq!(
                completion_context(
                    &document,
                    LspPosition {
                        line: 0,
                        character: text.chars().count() as u32,
                    },
                ),
                expected,
                "context drifted for {text:?}"
            );
        }
    }

    #[test]
    fn completion_context_does_not_treat_indexing_as_parameter_options() {
        let text = "fun[] main(): int = { return values[";
        let uri =
            EditorDocumentUri::from_file_path(PathBuf::from("/tmp/index_context.fol")).unwrap();
        let document = EditorDocument::new(uri, 1, text.to_string()).unwrap();

        assert_eq!(
            completion_context(
                &document,
                LspPosition {
                    line: 0,
                    character: text.chars().count() as u32,
                },
            ),
            CompletionContext::BracketAccess {
                receiver: "values".to_string(),
            }
        );
    }

    #[test]
    fn completion_context_ignores_v3_looking_text_in_comments_and_quotes() {
        let cases = [
            "fun[] main(): int = {\n    ` var value: ptr[",
            "fun[] main(): int = {\n    /* channel[",
            "fun[] main(): int = {\n    var text = \"work() | as",
            "fun[] main(): int = {\n    var text = 'counter.",
        ];

        for (index, text) in cases.into_iter().enumerate() {
            let uri = EditorDocumentUri::from_file_path(PathBuf::from(format!(
                "/tmp/v3_masked_completion_context_{index}.fol"
            )))
            .unwrap();
            let document = EditorDocument::new(uri, 1, text.to_string()).unwrap();
            assert_eq!(
                completion_context(
                    &document,
                    LspPosition {
                        line: 1,
                        character: text.lines().nth(1).unwrap().chars().count() as u32,
                    },
                ),
                CompletionContext::Plain,
                "comment/quote content should not create V3 completion context: {text:?}"
            );
        }
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
        fol_resolver::SymbolKind::Standard => "standard",
    }
}

pub(super) fn completion_symbol_kind(kind: fol_resolver::SymbolKind) -> u8 {
    match kind {
        fol_resolver::SymbolKind::Routine => 3,
        fol_resolver::SymbolKind::Definition => 12,
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias => 22,
        fol_resolver::SymbolKind::ImportAlias | fol_resolver::SymbolKind::Segment => 9,
        fol_resolver::SymbolKind::Standard => 12,
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
