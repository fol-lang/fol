pub(crate) mod analysis;
pub(crate) mod completion_helpers;
mod semantic;
mod transport;
mod types;

pub use transport::run_lsp_stdio;
#[cfg(test)]
pub(crate) use transport::run_lsp_stdio_with_io;
pub use types::{
    EditorCompletionItem, JsonRpcError, JsonRpcId, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, LspCodeAction, LspCodeActionContext, LspCodeActionParams,
    LspCompletionContext, LspCompletionItem, LspCompletionList, LspCompletionOptions,
    LspCompletionParams, LspDefinitionParams, LspDidChangeTextDocumentParams,
    LspDidCloseTextDocumentParams, LspDidOpenTextDocumentParams, LspDocumentFormattingParams,
    LspDocumentHighlight, LspDocumentSymbol, LspDocumentSymbolParams, LspFoldingRange,
    LspFoldingRangeParams, LspHover, LspHoverParams, LspInitializeParams, LspInitializeResult,
    LspInlayHint, LspInlayHintParams, LspParameterInformation, LspPrepareRenameResult,
    LspPublishDiagnosticsParams, LspReferenceContext, LspReferenceParams, LspRenameOptions,
    LspRenameParams, LspSelectionRange, LspSelectionRangeParams, LspSemanticTokens,
    LspSemanticTokensLegend, LspSemanticTokensOptions, LspSemanticTokensParams,
    LspServerCapabilities, LspServerInfo, LspSignatureHelp, LspSignatureHelpOptions,
    LspSignatureHelpParams, LspSignatureInformation, LspTextDocumentContentChangeEvent,
    LspTextDocumentIdentifier, LspTextDocumentItem, LspTextDocumentSyncOptions, LspTextEdit,
    LspVersionedTextDocumentIdentifier, LspWorkspaceEdit, LspWorkspaceSymbol,
    LspWorkspaceSymbolParams,
};

use crate::{
    dedup_lsp_diagnostics, formatting_edit, EditorConfig, EditorDocument, EditorDocumentUri,
    EditorError, EditorErrorKind, EditorResult, EditorSession, EditorWorkspaceMapping,
    EditorWorkspaceRoots, LspLocation, LspPosition, LspRange,
};

fn serialize_result(value: &impl serde::Serialize) -> EditorResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("LSP response serialization failed: {e}"),
        )
    })
}

fn word_span_at_position(line: &str, character: u32) -> Option<(usize, usize, String)> {
    let chars = line.chars().collect::<Vec<_>>();
    let cursor = (character as usize).min(chars.len());
    let mut start = cursor;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let mut end = cursor;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }
    (start < end).then(|| (start, end, chars[start..end].iter().collect()))
}

fn identifier_positions_before(line: &str, before: usize) -> Vec<u32> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut cursor = before.min(chars.len());
    let mut positions = Vec::new();
    while cursor > 0 {
        while cursor > 0 && !(chars[cursor - 1].is_alphanumeric() || chars[cursor - 1] == '_') {
            cursor -= 1;
        }
        let end = cursor;
        while cursor > 0 && (chars[cursor - 1].is_alphanumeric() || chars[cursor - 1] == '_') {
            cursor -= 1;
        }
        if cursor < end && (chars[cursor].is_alphabetic() || chars[cursor] == '_') {
            positions.push(cursor as u32);
        }
    }
    positions
}

fn bracket_role_operand(
    line: &str,
    role_span: &(usize, usize, String),
    require_trailing_colon: bool,
) -> Option<(u32, String)> {
    let chars = line.chars().collect::<Vec<_>>();
    let (role_start, role_end, _) = role_span;

    let mut cursor = *role_start;
    while cursor > 0 && chars[cursor - 1].is_whitespace() {
        cursor -= 1;
    }
    if cursor == 0 || chars[cursor - 1] != '[' {
        return None;
    }
    let open_bracket = cursor - 1;

    cursor = *role_end;
    while cursor < chars.len() && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    if cursor >= chars.len() || chars[cursor] != ']' {
        return None;
    }
    cursor += 1;
    if require_trailing_colon {
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        if cursor >= chars.len() || chars[cursor] != ':' {
            return None;
        }
    }

    cursor = open_bracket;
    while cursor > 0 && chars[cursor - 1].is_whitespace() {
        cursor -= 1;
    }
    let operand_end = cursor;
    while cursor > 0 && (chars[cursor - 1].is_alphanumeric() || chars[cursor - 1] == '_') {
        cursor -= 1;
    }
    if cursor == operand_end || !(chars[cursor].is_alphabetic() || chars[cursor] == '_') {
        return None;
    }

    Some((
        cursor as u32,
        chars[cursor..operand_end].iter().collect::<String>(),
    ))
}

fn identifier_position_after(line: &str, after: usize) -> Option<u32> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut cursor = after.min(chars.len());
    while cursor < chars.len() && !(chars[cursor].is_alphabetic() || chars[cursor] == '_') {
        cursor += 1;
    }
    (cursor < chars.len()).then_some(cursor as u32)
}

fn scalar_position_for_document(
    document: &EditorDocument,
    position: LspPosition,
) -> EditorResult<LspPosition> {
    crate::positions::utf16_position_to_scalar_tolerant(&document.text, position).ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::InvalidInput,
            format!(
                "invalid UTF-16 position {}:{} for '{}'",
                position.line,
                position.character,
                document.uri.as_str()
            ),
        )
    })
}

fn scalar_range_for_document(document: &EditorDocument, range: LspRange) -> EditorResult<LspRange> {
    crate::positions::utf16_range_to_scalar_tolerant(&document.text, range).ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::InvalidInput,
            format!("invalid UTF-16 range for '{}'", document.uri.as_str()),
        )
    })
}

fn canonical_file_uri(uri: &str) -> Option<EditorDocumentUri> {
    EditorDocumentUri::parse(uri).ok().or_else(|| {
        uri.strip_prefix("file://")
            .map(std::path::PathBuf::from)
            .and_then(|path| EditorDocumentUri::from_file_path(path).ok())
    })
}

fn source_text_for_uri<'a>(
    session: &'a EditorSession,
    uri: &EditorDocumentUri,
) -> Option<std::borrow::Cow<'a, str>> {
    if let Some(document) = session.documents.get(uri) {
        return Some(std::borrow::Cow::Borrowed(&document.text));
    }
    std::fs::read_to_string(uri.to_file_path().ok()?)
        .ok()
        .map(std::borrow::Cow::Owned)
}

fn location_to_utf16(session: &EditorSession, location: &mut LspLocation) {
    let Some(uri) = canonical_file_uri(&location.uri) else {
        return;
    };
    if let Some(text) = source_text_for_uri(session, &uri) {
        if let Some(range) = crate::positions::scalar_range_to_utf16(&text, location.range) {
            location.range = range;
        }
    }
    location.uri = uri.as_str().to_string();
}

fn diagnostic_to_utf16(
    session: &EditorSession,
    current_document: &EditorDocument,
    diagnostic: &mut crate::LspDiagnostic,
) {
    if let Some(range) =
        crate::positions::scalar_range_to_utf16(&current_document.text, diagnostic.range)
    {
        diagnostic.range = range;
    }
    for related in &mut diagnostic.related_information {
        location_to_utf16(session, &mut related.location);
    }
}

fn diagnostic_to_utf16_for_uri(
    session: &EditorSession,
    uri: &str,
    diagnostic: &mut crate::LspDiagnostic,
) {
    if let Ok(uri) = EditorDocumentUri::parse(uri) {
        if let Some(text) = source_text_for_uri(session, &uri) {
            if let Some(range) = crate::positions::scalar_range_to_utf16(&text, diagnostic.range) {
                diagnostic.range = range;
            }
        }
    }
    for related in &mut diagnostic.related_information {
        location_to_utf16(session, &mut related.location);
    }
}

fn diagnostic_to_scalar(current_document: &EditorDocument, diagnostic: &mut crate::LspDiagnostic) {
    if let Some(range) =
        crate::positions::utf16_range_to_scalar_tolerant(&current_document.text, diagnostic.range)
    {
        diagnostic.range = range;
    }
}

fn workspace_edit_to_utf16(session: &EditorSession, edit: &mut LspWorkspaceEdit) {
    let changes = std::mem::take(&mut edit.changes);
    for (raw_uri, mut edits) in changes {
        let Some(uri) = canonical_file_uri(&raw_uri) else {
            edit.changes.insert(raw_uri, edits);
            continue;
        };
        if let Some(text) = source_text_for_uri(session, &uri) {
            for edit in &mut edits {
                if let Some(range) = crate::positions::scalar_range_to_utf16(&text, edit.range) {
                    edit.range = range;
                }
            }
        }
        edit.changes
            .entry(uri.as_str().to_string())
            .or_default()
            .append(&mut edits);
    }
}

fn document_symbol_to_utf16(text: &str, symbol: &mut LspDocumentSymbol) {
    if let Some(range) = crate::positions::scalar_range_to_utf16(text, symbol.range) {
        symbol.range = range;
    }
    if let Some(range) = crate::positions::scalar_range_to_utf16(text, symbol.selection_range) {
        symbol.selection_range = range;
    }
    for child in &mut symbol.children {
        document_symbol_to_utf16(text, child);
    }
}

fn selection_range_to_utf16(text: &str, selection: &mut LspSelectionRange) {
    if let Some(range) = crate::positions::scalar_range_to_utf16(text, selection.range) {
        selection.range = range;
    }
    if let Some(parent) = selection.parent.as_mut() {
        selection_range_to_utf16(text, parent);
    }
}

use crate::workspace::discover_workspace_roots;
use analysis::analyze_document_semantics;
use completion_helpers::{completion_context_with_lsp, completion_cursor_is_protected};
use std::fs;
use std::sync::Arc;
use transport::from_params;

pub(crate) fn semantic_token_type_names() -> &'static [&'static str] {
    semantic::semantic_token_types()
}

pub struct EditorLspServer {
    pub session: EditorSession,
    /// Last per-URI diagnostic groups contributed by each open document's
    /// compiler analysis. Keeping ownership lets a change/close publish empty
    /// groups for stale dependency diagnostics without clearing a URI that is
    /// still diagnosed by another open document.
    published_diagnostics_by_document: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, Vec<crate::LspDiagnostic>>,
    >,
}

impl EditorLspServer {
    pub fn new(config: EditorConfig) -> Self {
        Self {
            session: EditorSession::new(config),
            published_diagnostics_by_document: std::collections::BTreeMap::new(),
        }
    }

    pub fn handle_request(
        &mut self,
        request: JsonRpcRequest,
    ) -> EditorResult<Option<JsonRpcResponse>> {
        match request.method.as_str() {
            "initialize" => Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(
                    serde_json::to_value(LspInitializeResult {
                        capabilities: LspServerCapabilities {
                            position_encoding: "utf-16".to_string(),
                            text_document_sync: LspTextDocumentSyncOptions {
                                open_close: true,
                                change: if self.session.config.full_document_sync {
                                    1
                                } else {
                                    2
                                },
                            },
                            hover_provider: true,
                            definition_provider: true,
                            document_symbol_provider: true,
                            workspace_symbol_provider: Some(true),
                            formatting_provider: Some(true),
                            code_action_provider: Some(true),
                            signature_help_provider: Some(LspSignatureHelpOptions {
                                trigger_characters: vec!["(".to_string(), ",".to_string()],
                            }),
                            references_provider: Some(true),
                            rename_provider: Some(LspRenameOptions {
                                prepare_provider: true,
                            }),
                            semantic_tokens_provider: Some(LspSemanticTokensOptions {
                                legend: LspSemanticTokensLegend {
                                    token_types: semantic::semantic_token_types()
                                        .iter()
                                        .map(|kind| kind.to_string())
                                        .collect(),
                                    token_modifiers: Vec::new(),
                                },
                                full: true,
                            }),
                            completion_provider: Some(LspCompletionOptions {
                                trigger_characters: vec![".".to_string()],
                                resolve_provider: true,
                            }),
                            type_definition_provider: Some(true),
                            implementation_provider: Some(true),
                            document_highlight_provider: Some(true),
                            folding_range_provider: Some(true),
                            selection_range_provider: Some(true),
                            inlay_hint_provider: Some(true),
                        },
                        server_info: LspServerInfo {
                            name: "fol-editor".to_string(),
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        },
                    })
                    .map_err(|e| {
                        EditorError::new(EditorErrorKind::Internal, format!("LSP serialize: {e}"))
                    })?,
                ),
                error: None,
            })),
            "shutdown" => {
                self.session.shutdown_requested = true;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serde_json::Value::Null),
                    error: None,
                }))
            }
            "textDocument/hover" => {
                let params: LspHoverParams = from_params(request.params)?;
                let result = self.hover(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/definition" => {
                let params: LspDefinitionParams = from_params(request.params)?;
                let result = self.definition(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/codeAction" => {
                let params: LspCodeActionParams = from_params(request.params)?;
                let result = self.code_actions(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.range,
                    &params.context.diagnostics,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/formatting" => {
                let params: LspDocumentFormattingParams = from_params(request.params)?;
                let result =
                    self.format_document(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/signatureHelp" => {
                let params: LspSignatureHelpParams = from_params(request.params)?;
                let result = self.signature_help(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/references" => {
                let params: LspReferenceParams = from_params(request.params)?;
                let result = self.references(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                    params.context.include_declaration,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/rename" => {
                let params: LspRenameParams = from_params(request.params)?;
                let result = self.rename(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                    &params.new_name,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/semanticTokens/full" => {
                let params: LspSemanticTokensParams = from_params(request.params)?;
                let result =
                    self.semantic_tokens(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/documentSymbol" => {
                let params: LspDocumentSymbolParams = from_params(request.params)?;
                let result =
                    self.document_symbols(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "workspace/symbol" => {
                let params: LspWorkspaceSymbolParams = from_params(request.params)?;
                let result = self.workspace_symbols(&params.query)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/completion" => {
                let params: LspCompletionParams = from_params(request.params)?;
                let result = self.completion(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                    params.context.as_ref(),
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "completionItem/resolve" => {
                let item: EditorCompletionItem = from_params(request.params)?;
                let result = self.resolve_completion_item(item);
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/typeDefinition" => {
                let params: LspDefinitionParams = from_params(request.params)?;
                let result = self.type_definition(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/implementation" => {
                let params: LspDefinitionParams = from_params(request.params)?;
                let result = self.implementation(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/documentHighlight" => {
                let params: LspDefinitionParams = from_params(request.params)?;
                let result = self.document_highlights(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/prepareRename" => {
                let params: LspDefinitionParams = from_params(request.params)?;
                let result = self.prepare_rename(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.position,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/foldingRange" => {
                let params: LspFoldingRangeParams = from_params(request.params)?;
                let result =
                    self.folding_ranges(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/selectionRange" => {
                let params: LspSelectionRangeParams = from_params(request.params)?;
                let result = self.selection_ranges(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    &params.positions,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            "textDocument/inlayHint" => {
                let params: LspInlayHintParams = from_params(request.params)?;
                let result = self.inlay_hints(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                    params.range,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serialize_result(&result)?),
                    error: None,
                }))
            }
            _ => Ok(Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("method '{}' not supported", request.method),
                }),
            })),
        }
    }

    pub fn handle_notification(
        &mut self,
        notification: JsonRpcNotification,
    ) -> EditorResult<Vec<LspPublishDiagnosticsParams>> {
        match notification.method.as_str() {
            "initialized" => Ok(Vec::new()),
            "exit" => {
                self.session.shutdown_requested = true;
                Ok(Vec::new())
            }
            "textDocument/didOpen" => {
                let params: LspDidOpenTextDocumentParams = from_params(notification.params)?;
                let uri = EditorDocumentUri::parse(&params.text_document.uri)?;
                let document = EditorDocument::new(
                    uri.clone(),
                    params.text_document.version,
                    params.text_document.text,
                )?;
                let mapping = self.cached_document_mapping(document.path.as_path())?;
                self.session
                    .mappings
                    .insert(uri.as_str().to_string(), mapping);
                self.session.documents.open(document);
                self.session.diagnostic_snapshots.remove(uri.as_str());
                self.session.semantic_snapshots.remove(uri.as_str());
                self.publish_analysis_diagnostics(&uri)
            }
            "textDocument/didChange" => {
                let params: LspDidChangeTextDocumentParams = from_params(notification.params)?;
                let uri = EditorDocumentUri::parse(&params.text_document.uri)?;
                if params.content_changes.is_empty() {
                    return Err(EditorError::new(
                        EditorErrorKind::InvalidInput,
                        "didChange requires at least one content change",
                    ));
                }
                for change in params.content_changes {
                    if let Some(range) = change.range {
                        self.session.documents.apply_incremental_change(
                            &uri,
                            params.text_document.version,
                            range,
                            change.text,
                        )?;
                    } else {
                        self.session.documents.apply_full_change(
                            &uri,
                            params.text_document.version,
                            change.text,
                        )?;
                    }
                }
                self.session.diagnostic_snapshots.remove(uri.as_str());
                self.session.semantic_snapshots.remove(uri.as_str());
                self.publish_analysis_diagnostics(&uri)
            }
            "textDocument/didClose" => {
                let params: LspDidCloseTextDocumentParams = from_params(notification.params)?;
                let uri = EditorDocumentUri::parse(&params.text_document.uri)?;
                self.session.documents.close(&uri);
                self.session.mappings.remove(uri.as_str());
                self.session.diagnostic_snapshots.remove(uri.as_str());
                self.session.semantic_snapshots.remove(uri.as_str());
                Ok(self.remove_analysis_diagnostic_publications(&uri))
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn publish_diagnostics(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<LspPublishDiagnosticsParams> {
        let document = self.open_document(uri)?.clone();
        let diagnostics = dedup_lsp_diagnostics(self.diagnostic_snapshot(uri, &document)?);
        Ok(LspPublishDiagnosticsParams {
            uri: uri.as_str().to_string(),
            diagnostics,
        })
    }

    fn publish_analysis_diagnostics(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<Vec<LspPublishDiagnosticsParams>> {
        let document = self.open_document(uri)?.clone();
        let current_diagnostics = dedup_lsp_diagnostics(self.diagnostic_snapshot(uri, &document)?);
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut by_uri = snapshot.diagnostics_by_source_uri();
        by_uri.insert(uri.as_str().to_string(), current_diagnostics);

        for (target_uri, diagnostics) in &mut by_uri {
            if target_uri != uri.as_str() {
                for diagnostic in diagnostics.iter_mut() {
                    diagnostic_to_utf16_for_uri(&self.session, target_uri, diagnostic);
                }
                *diagnostics = dedup_lsp_diagnostics(std::mem::take(diagnostics));
            }
        }

        let owner = uri.as_str().to_string();
        let previous = self
            .published_diagnostics_by_document
            .insert(owner.clone(), by_uri);
        let mut affected = previous
            .into_iter()
            .flat_map(|groups| groups.into_keys())
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(groups) = self.published_diagnostics_by_document.get(&owner) {
            affected.extend(groups.keys().cloned());
        }
        affected.insert(owner.clone());

        Ok(self.aggregate_diagnostic_publications(&owner, affected))
    }

    fn remove_analysis_diagnostic_publications(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> Vec<LspPublishDiagnosticsParams> {
        let owner = uri.as_str().to_string();
        let mut affected = self
            .published_diagnostics_by_document
            .remove(&owner)
            .into_iter()
            .flat_map(|groups| groups.into_keys())
            .collect::<std::collections::BTreeSet<_>>();
        affected.insert(owner.clone());
        self.aggregate_diagnostic_publications(&owner, affected)
    }

    fn aggregate_diagnostic_publications(
        &self,
        primary_uri: &str,
        affected: std::collections::BTreeSet<String>,
    ) -> Vec<LspPublishDiagnosticsParams> {
        let mut ordered = affected.into_iter().collect::<Vec<_>>();
        ordered.sort_by_key(|uri| (uri != primary_uri, uri.clone()));

        ordered
            .into_iter()
            .map(|target_uri| {
                let diagnostics = self
                    .published_diagnostics_by_document
                    .values()
                    .filter_map(|groups| groups.get(&target_uri))
                    .flat_map(|diagnostics| diagnostics.iter().cloned())
                    .collect::<Vec<_>>();
                LspPublishDiagnosticsParams {
                    uri: target_uri,
                    diagnostics: dedup_lsp_diagnostics(diagnostics),
                }
            })
            .collect()
    }

    pub fn hover(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspHover>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut hit = snapshot
            .reference_at(position)
            .as_ref()
            .and_then(|reference| snapshot.hover_for_reference(reference, position))
            .or_else(|| {
                snapshot
                    .method_target_symbol_at(position)
                    .and_then(|symbol_id| snapshot.hover_for_symbol(symbol_id, position, None))
            });
        let current_line = document.text.lines().nth(position.line as usize);
        let raw_fallback_allowed =
            !crate::source_scan::position_is_protected(&document.text, position);
        let word_span =
            current_line.and_then(|line| word_span_at_position(line, position.character));
        let word_at_position = word_span.as_ref().map(|(_, _, word)| word.clone());
        if hit.is_none() && raw_fallback_allowed && word_at_position.as_deref() == Some("edf") {
            hit = Some(LspHover {
                contents: "edf: error-only defer (runs on recoverable error exit only)".to_string(),
                range: None,
            });
        }
        if hit.is_none() && raw_fallback_allowed && word_at_position.as_deref() == Some("mux") {
            hit = current_line
                .zip(word_span.as_ref())
                .and_then(|(line, role_span)| {
                    let (character, operand) = bracket_role_operand(line, role_span, true)?;
                    snapshot.hover_for_mutex_binding(
                        LspPosition {
                            line: position.line,
                            character,
                        },
                        &operand,
                    )
                });
        }
        if hit.is_none() && raw_fallback_allowed && word_at_position.as_deref() == Some("async") {
            hit = current_line
                .zip(word_span.as_ref())
                .and_then(|(line, (start, _, _))| {
                    identifier_positions_before(line, *start)
                        .into_iter()
                        .find_map(|character| {
                            snapshot.hover_for_processor_pipe_stage(
                                LspPosition {
                                    line: position.line,
                                    character,
                                },
                                "async",
                            )
                        })
                })
                .or_else(|| {
                    Some(LspHover {
                        contents:
                            "| async: spawns an OS thread; yields an eventual (internal type)"
                                .to_string(),
                        range: None,
                    })
                });
        }
        if hit.is_none() && raw_fallback_allowed && word_at_position.as_deref() == Some("await") {
            hit = current_line
                .zip(word_span.as_ref())
                .and_then(|(line, (start, _, _))| {
                    identifier_positions_before(line, *start)
                        .into_iter()
                        .find_map(|character| {
                            snapshot.hover_for_processor_pipe_stage(
                                LspPosition {
                                    line: position.line,
                                    character,
                                },
                                "await",
                            )
                        })
                })
                .or_else(|| {
                    Some(LspHover {
                        contents: "| await: blocks for the eventual value".to_string(),
                        range: None,
                    })
                });
        }
        let spawn_marker_at_position = document
            .text
            .lines()
            .nth(position.line as usize)
            .is_some_and(|line| {
                line.match_indices("[>]").any(|(byte_offset, marker)| {
                    let start = line[..byte_offset].chars().count() as u32;
                    let end = start + marker.chars().count() as u32;
                    position.character >= start && position.character <= end
                })
            });
        if hit.is_none() && raw_fallback_allowed && spawn_marker_at_position {
            hit = Some(LspHover {
                contents: "[>]: spawns a task (joined at process exit)".to_string(),
                range: None,
            });
        }
        if hit.is_none()
            && raw_fallback_allowed
            && matches!(word_at_position.as_deref(), Some("tx" | "rx"))
        {
            let endpoint = word_at_position.as_deref().unwrap_or_default();
            hit = current_line
                .zip(word_span.as_ref())
                .and_then(|(line, role_span)| {
                    let (character, _) = bracket_role_operand(line, role_span, false)?;
                    snapshot.hover_for_channel_endpoint(
                        LspPosition {
                            line: position.line,
                            character,
                        },
                        endpoint,
                    )
                });
        }
        let deref_at_position = current_line.is_some_and(|line| {
            line.chars()
                .nth(position.character as usize)
                .is_some_and(|character| character == '*')
        });
        if hit.is_none() && raw_fallback_allowed && deref_at_position {
            hit = current_line
                .and_then(|line| identifier_position_after(line, position.character as usize + 1))
                .and_then(|character| {
                    snapshot.hover_for_dereference(LspPosition {
                        line: position.line,
                        character,
                    })
                });
        }
        let owned_type_site = document
            .text
            .lines()
            .nth(position.line as usize)
            .map(|line| {
                let prefix = line
                    .chars()
                    .take(position.character as usize)
                    .collect::<String>();
                prefix
                    .trim_end_matches(|character: char| {
                        character.is_alphanumeric() || character == '_'
                    })
                    .ends_with('@')
            })
            .unwrap_or(false);
        if raw_fallback_allowed && owned_type_site {
            if let Some(hover) = hit.as_mut() {
                hover
                    .contents
                    .push_str(" (owned heap type; allocation requires memo+)");
            }
        }
        if let Some(range) = hit.as_mut().and_then(|hover| hover.range.as_mut()) {
            if let Some(converted) = crate::positions::scalar_range_to_utf16(&document.text, *range)
            {
                *range = converted;
            }
        }
        Ok(hit)
    }

    pub fn definition(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut location = snapshot
            .reference_at(position)
            .as_ref()
            .and_then(|reference| snapshot.definition_for_reference(reference))
            .or_else(|| {
                snapshot
                    .method_target_symbol_at(position)
                    .and_then(|symbol_id| snapshot.definition_for_symbol(symbol_id))
            });
        if let Some(location) = location.as_mut() {
            location_to_utf16(&self.session, location);
        }
        Ok(location)
    }

    pub fn signature_help(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspSignatureHelp>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.signature_help(&document, position))
    }

    pub fn code_actions(
        &mut self,
        uri: &EditorDocumentUri,
        range: LspRange,
        diagnostics: &[crate::LspDiagnostic],
    ) -> EditorResult<Vec<LspCodeAction>> {
        let document = self.open_document(uri)?.clone();
        let range = scalar_range_for_document(&document, range)?;
        let mut diagnostics = diagnostics.to_vec();
        for diagnostic in &mut diagnostics {
            diagnostic_to_scalar(&document, diagnostic);
        }
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut actions = snapshot.code_actions(uri.as_str(), range, &diagnostics);
        for action in &mut actions {
            for diagnostic in &mut action.diagnostics {
                diagnostic_to_utf16(&self.session, &document, diagnostic);
            }
            workspace_edit_to_utf16(&self.session, &mut action.edit);
        }
        Ok(actions)
    }

    pub fn format_document(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<Vec<crate::LspTextEdit>> {
        let document = self.open_document(uri)?.clone();
        Ok(formatting_edit(&document.text).into_iter().collect())
    }

    pub fn document_symbols(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<Vec<LspDocumentSymbol>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut symbols = snapshot.document_symbols_for_current_path();
        for symbol in &mut symbols {
            document_symbol_to_utf16(&document.text, symbol);
        }
        Ok(symbols)
    }

    pub fn references(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
        include_declaration: bool,
    ) -> EditorResult<Vec<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut locations = snapshot
            .reference_at(position)
            .as_ref()
            .map(|reference| snapshot.references_for_reference(reference, include_declaration))
            .unwrap_or_default();
        for location in &mut locations {
            location_to_utf16(&self.session, location);
        }
        Ok(locations)
    }

    pub fn rename(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
        new_name: &str,
    ) -> EditorResult<LspWorkspaceEdit> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let reference = snapshot.reference_at(position).ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                format!(
                    "no rename target at {}:{}",
                    position.line, position.character
                ),
            )
        })?;
        let mut edit = snapshot.rename_for_reference(&reference, new_name)?;
        workspace_edit_to_utf16(&self.session, &mut edit);
        Ok(edit)
    }

    pub fn workspace_symbols(&mut self, query: &str) -> EditorResult<Vec<LspWorkspaceSymbol>> {
        let workspace_documents = self.workspace_symbol_documents()?;
        let mut symbols = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for (uri, document) in workspace_documents {
            let snapshot = self.semantic_snapshot(&uri, &document)?;
            for mut symbol in snapshot.workspace_symbols(query) {
                location_to_utf16(&self.session, &mut symbol.location);
                // A workspace symbol is uniquely identified by its kind and
                // source location. The container name is derived from the
                // per-request analysis overlay root, so it varies between the
                // separate document analyses that surface the same declaration;
                // excluding it keeps the same symbol from appearing twice.
                let key = (
                    symbol.kind,
                    symbol.location.uri.clone(),
                    symbol.location.range.start.line,
                    symbol.location.range.start.character,
                );
                if seen.insert(key) {
                    symbols.push(symbol);
                }
            }
        }

        symbols.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.container_name.cmp(&right.container_name))
                .then(left.location.uri.cmp(&right.location.uri))
                .then(
                    left.location
                        .range
                        .start
                        .line
                        .cmp(&right.location.range.start.line),
                )
                .then(
                    left.location
                        .range
                        .start
                        .character
                        .cmp(&right.location.range.start.character),
                )
        });
        Ok(symbols)
    }

    fn workspace_symbol_documents(
        &mut self,
    ) -> EditorResult<Vec<(EditorDocumentUri, EditorDocument)>> {
        let mut open_documents = Vec::new();
        for (uri, document) in self.session.documents.iter() {
            open_documents.push((EditorDocumentUri::parse(uri)?, document.clone()));
        }
        let mut documents = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for (uri, document) in &open_documents {
            if seen.insert(uri.clone()) {
                documents.push((uri.clone(), document.clone()));
            }
        }

        for (_, document) in &open_documents {
            let mapping = self.cached_document_mapping(document.path.as_path())?;
            for path in collect_fol_files(&mapping.analysis_root) {
                let uri = EditorDocumentUri::from_file_path(path.clone())?;
                if seen.contains(&uri) {
                    continue;
                }
                let text = match fs::read_to_string(&path) {
                    Ok(text) => text,
                    Err(_) => continue,
                };
                let document = EditorDocument::new(uri.clone(), 0, text)?;
                seen.insert(uri.clone());
                documents.push((uri, document));
            }
        }

        Ok(documents)
    }

    pub fn completion(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
        context: Option<&LspCompletionContext>,
    ) -> EditorResult<LspCompletionList> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        if completion_cursor_is_protected(&document, position) {
            return Ok(LspCompletionList {
                is_incomplete: false,
                items: Vec::new(),
            });
        }
        let completion_context = completion_context_with_lsp(&document, position, context);
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(LspCompletionList {
            is_incomplete: false,
            items: snapshot
                .completion_items(&document, position, completion_context)
                .into_iter()
                .map(|item| LspCompletionItem {
                    label: item.label,
                    kind: item.kind,
                    detail: item.detail,
                    insert_text: item.insert_text,
                })
                .collect(),
        })
    }

    pub fn resolve_completion_item(&mut self, item: EditorCompletionItem) -> EditorCompletionItem {
        semantic::resolve_completion_item(item)
    }

    pub fn type_definition(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut location = snapshot.type_definition_at(position);
        if let Some(location) = location.as_mut() {
            location_to_utf16(&self.session, location);
        }
        Ok(location)
    }

    pub fn implementation(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Vec<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut locations = snapshot.implementations_at(position);
        for location in &mut locations {
            location_to_utf16(&self.session, location);
        }
        Ok(locations)
    }

    pub fn document_highlights(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Vec<LspDocumentHighlight>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut highlights = snapshot.document_highlights_at(position);
        for highlight in &mut highlights {
            if let Some(range) =
                crate::positions::scalar_range_to_utf16(&document.text, highlight.range)
            {
                highlight.range = range;
            }
        }
        Ok(highlights)
    }

    pub fn prepare_rename(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspPrepareRenameResult>> {
        let document = self.open_document(uri)?.clone();
        let position = scalar_position_for_document(&document, position)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut result = snapshot.prepare_rename_at(position);
        if let Some(result) = result.as_mut() {
            if let Some(range) =
                crate::positions::scalar_range_to_utf16(&document.text, result.range)
            {
                result.range = range;
            }
        }
        Ok(result)
    }

    pub fn folding_ranges(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<Vec<LspFoldingRange>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.folding_ranges(&document))
    }

    pub fn selection_ranges(
        &mut self,
        uri: &EditorDocumentUri,
        positions: &[LspPosition],
    ) -> EditorResult<Vec<LspSelectionRange>> {
        let document = self.open_document(uri)?.clone();
        let positions = positions
            .iter()
            .copied()
            .map(|position| scalar_position_for_document(&document, position))
            .collect::<EditorResult<Vec<_>>>()?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut selections = snapshot.selection_ranges(&document, &positions);
        for selection in &mut selections {
            selection_range_to_utf16(&document.text, selection);
        }
        Ok(selections)
    }

    pub fn inlay_hints(
        &mut self,
        uri: &EditorDocumentUri,
        range: LspRange,
    ) -> EditorResult<Vec<LspInlayHint>> {
        let document = self.open_document(uri)?.clone();
        let range = scalar_range_for_document(&document, range)?;
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let mut hints = snapshot.inlay_hints(&document, range);
        for hint in &mut hints {
            if let Some(position) =
                crate::positions::scalar_position_to_utf16(&document.text, hint.position)
            {
                hint.position = position;
            }
        }
        Ok(hints)
    }

    pub fn semantic_tokens(&mut self, uri: &EditorDocumentUri) -> EditorResult<LspSemanticTokens> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let data = snapshot.semantic_tokens_for_current_path();
        Ok(LspSemanticTokens {
            data: crate::positions::scalar_semantic_tokens_to_utf16(&document.text, &data)
                .unwrap_or(data),
        })
    }

    fn open_document(&self, uri: &EditorDocumentUri) -> EditorResult<&EditorDocument> {
        self.session.documents.get(uri).ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::DocumentNotOpen,
                format!("document '{}' is not open", uri.as_str()),
            )
        })
    }

    fn document_mapping(
        &mut self,
        document: &EditorDocument,
        uri: &EditorDocumentUri,
    ) -> EditorResult<EditorWorkspaceMapping> {
        if let Some(mapping) = self.session.mappings.get(uri.as_str()) {
            return Ok(mapping.clone());
        }
        self.cached_document_mapping(document.path.as_path())
    }

    fn cached_document_mapping(
        &mut self,
        path: &std::path::Path,
    ) -> EditorResult<EditorWorkspaceMapping> {
        let absolute = crate::workspace::canonical_document_path(path)?;
        let directory = absolute.parent().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidDocumentPath,
                format!("document '{}' has no parent directory", absolute.display()),
            )
        })?;
        let _ = self.cached_workspace_roots(directory);
        crate::map_document_workspace(&absolute, &self.session.config)
    }

    fn cached_workspace_roots(&mut self, directory: &std::path::Path) -> EditorWorkspaceRoots {
        if let Some(roots) = self.session.workspace_roots.get(directory) {
            return roots.clone();
        }
        let roots = discover_workspace_roots(directory, &self.session.config);
        self.session
            .workspace_roots
            .insert(directory.to_path_buf(), roots.clone());
        roots
    }

    fn semantic_snapshot(
        &mut self,
        uri: &EditorDocumentUri,
        document: &EditorDocument,
    ) -> EditorResult<Arc<semantic::SemanticSnapshot>> {
        if let Some(cached) = self.session.semantic_snapshots.get(uri.as_str()) {
            if cached.document_version == document.version {
                return Ok(Arc::clone(&cached.snapshot));
            }
        }

        let mapping = self.document_mapping(document, uri)?;
        let snapshot = Arc::new(analyze_document_semantics(document, &mapping)?);
        self.session.semantic_snapshots.insert(
            uri.as_str().to_string(),
            analysis::CachedSemanticSnapshot {
                document_version: document.version,
                snapshot: Arc::clone(&snapshot),
            },
        );
        Ok(snapshot)
    }

    fn diagnostic_snapshot(
        &mut self,
        uri: &EditorDocumentUri,
        document: &EditorDocument,
    ) -> EditorResult<Vec<crate::LspDiagnostic>> {
        if let Some(cached) = self.session.diagnostic_snapshots.get(uri.as_str()) {
            if cached.document_version == document.version {
                return Ok(cached.diagnostics.clone());
            }
        }

        // Diagnostics come from the same compiler-backed analysis as every
        // semantic request, so build (and cache) the shared semantic snapshot
        // once and reuse it for both.
        analysis::note_diagnostic_snapshot_build();
        let snapshot = self.semantic_snapshot(uri, document)?;
        let mut diagnostics = snapshot.diagnostics.clone();
        for diagnostic in &mut diagnostics {
            diagnostic_to_utf16(&self.session, document, diagnostic);
        }
        self.session.diagnostic_snapshots.insert(
            uri.as_str().to_string(),
            analysis::CachedDiagnosticSnapshot {
                document_version: document.version,
                diagnostics: diagnostics.clone(),
            },
        );
        Ok(diagnostics)
    }
}

fn collect_fol_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    fn visit(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                visit(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "fol") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    visit(root, &mut files);
    files.sort();
    files
}

#[cfg(test)]
mod tests;
