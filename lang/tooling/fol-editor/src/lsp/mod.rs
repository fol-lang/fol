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
    JsonRpcResponse, LspCodeAction, LspCodeActionContext, LspCodeActionParams, LspCodeLens,
    LspCodeLensOptions, LspCodeLensParams, LspCompletionContext, LspCompletionItem,
    LspCompletionList, LspCompletionOptions, LspCompletionParams, LspDefinitionParams,
    LspDidChangeTextDocumentParams, LspDidCloseTextDocumentParams, LspDidOpenTextDocumentParams,
    LspDocumentFormattingParams, LspDocumentHighlight, LspDocumentSymbol, LspDocumentSymbolParams,
    LspFoldingRange, LspFoldingRangeParams, LspHover, LspHoverParams, LspInitializeParams,
    LspInitializeResult, LspInlayHint, LspInlayHintParams, LspParameterInformation,
    LspPrepareRenameResult, LspPublishDiagnosticsParams, LspReferenceContext, LspReferenceParams,
    LspRenameOptions, LspRenameParams, LspSelectionRange, LspSelectionRangeParams,
    LspSemanticTokens, LspSemanticTokensLegend, LspSemanticTokensOptions, LspSemanticTokensParams,
    LspServerCapabilities, LspServerInfo, LspSignatureHelp, LspSignatureHelpOptions,
    LspSignatureHelpParams, LspSignatureInformation, LspTextDocumentContentChangeEvent,
    LspTextDocumentIdentifier, LspTextDocumentItem, LspTextDocumentSyncOptions, LspTextEdit,
    LspVersionedTextDocumentIdentifier, LspWorkspaceEdit, LspWorkspaceSymbol,
    LspWorkspaceSymbolParams,
};

use crate::{
    dedup_lsp_diagnostics, EditorConfig, EditorDocument, EditorDocumentUri, EditorError,
    EditorErrorKind, EditorResult, EditorSession, EditorWorkspaceMapping, EditorWorkspaceRoots,
    LspLocation, LspPosition, LspRange, formatting_edit,
};

fn serialize_result(value: &impl serde::Serialize) -> EditorResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| {
        EditorError::new(EditorErrorKind::Internal, format!("LSP response serialization failed: {e}"))
    })
}
use analysis::analyze_document_semantics;
use completion_helpers::completion_context_with_lsp;
use std::fs;
use std::sync::Arc;
use transport::from_params;
use crate::workspace::discover_workspace_roots;

pub(crate) fn semantic_token_type_names() -> &'static [&'static str] {
    semantic::semantic_token_types()
}

pub struct EditorLspServer {
    pub session: EditorSession,
}

impl EditorLspServer {
    pub fn new(config: EditorConfig) -> Self {
        Self {
            session: EditorSession::new(config),
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
                            code_lens_provider: Some(LspCodeLensOptions {
                                resolve_provider: false,
                            }),
                        },
                        server_info: LspServerInfo {
                            name: "fol-editor".to_string(),
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        },
                    })
                    .map_err(|e| EditorError::new(EditorErrorKind::Internal, format!("LSP serialize: {e}")))?,
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
                    error: None,
                }))
            }
            "textDocument/formatting" => {
                let params: LspDocumentFormattingParams = from_params(request.params)?;
                let result = self.format_document(
                    &EditorDocumentUri::parse(&params.text_document.uri)?,
                )?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
                    error: None,
                }))
            }
            "workspace/symbol" => {
                let params: LspWorkspaceSymbolParams = from_params(request.params)?;
                let result = self.workspace_symbols(&params.query)?;
                Ok(Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                    result: Some(
                        serialize_result(&result)?,
                    ),
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
                let result = self
                    .folding_ranges(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
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
            "textDocument/codeLens" => {
                let params: LspCodeLensParams = from_params(request.params)?;
                let result =
                    self.code_lenses(&EditorDocumentUri::parse(&params.text_document.uri)?)?;
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
        use crate::LspPublishDiagnosticsParams;
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
                let diagnostics = self.publish_diagnostics(&uri)?;
                Ok(vec![diagnostics])
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
                let diagnostics = self.publish_diagnostics(&uri)?;
                Ok(vec![diagnostics])
            }
            "textDocument/didClose" => {
                let params: LspDidCloseTextDocumentParams = from_params(notification.params)?;
                let uri = EditorDocumentUri::parse(&params.text_document.uri)?;
                self.session.documents.close(&uri);
                self.session.mappings.remove(uri.as_str());
                self.session.diagnostic_snapshots.remove(uri.as_str());
                self.session.semantic_snapshots.remove(uri.as_str());
                Ok(vec![LspPublishDiagnosticsParams {
                    uri: uri.as_str().to_string(),
                    diagnostics: Vec::new(),
                }])
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

    pub fn hover(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspHover>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let hit = snapshot
            .reference_at(position)
            .as_ref()
            .and_then(|reference| snapshot.hover_for_reference(reference))
            .or_else(|| {
                snapshot
                    .method_target_symbol_at(position)
                    .and_then(|symbol_id| snapshot.hover_for_symbol(symbol_id))
            });
        Ok(hit)
    }

    pub fn definition(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot
            .reference_at(position)
            .as_ref()
            .and_then(|reference| snapshot.definition_for_reference(reference))
            .or_else(|| {
                snapshot
                    .method_target_symbol_at(position)
                    .and_then(|symbol_id| snapshot.definition_for_symbol(symbol_id))
            }))
    }

    pub fn signature_help(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspSignatureHelp>> {
        let document = self.open_document(uri)?.clone();
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
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.code_actions(uri.as_str(), range, diagnostics))
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
        Ok(snapshot.document_symbols_for_current_path())
    }

    pub fn references(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
        include_declaration: bool,
    ) -> EditorResult<Vec<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot
            .reference_at(position)
            .as_ref()
            .map(|reference| snapshot.references_for_reference(reference, include_declaration))
            .unwrap_or_default())
    }

    pub fn rename(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
        new_name: &str,
    ) -> EditorResult<LspWorkspaceEdit> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        let reference = snapshot.reference_at(position).ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                format!("no rename target at {}:{}", position.line, position.character),
            )
        })?;
        snapshot.rename_for_reference(&reference, new_name)
    }

    pub fn workspace_symbols(
        &mut self,
        query: &str,
    ) -> EditorResult<Vec<LspWorkspaceSymbol>> {
        let workspace_documents = self.workspace_symbol_documents()?;
        let mut symbols = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for (uri, document) in workspace_documents {
            let snapshot = self.semantic_snapshot(&uri, &document)?;
            for symbol in snapshot.workspace_symbols(query) {
                // A workspace symbol is uniquely identified by its kind and
                // source location. The container name is derived from the
                // per-request analysis overlay root, so it varies between the
                // separate document analyses that surface the same declaration;
                // excluding it keeps the same symbol from appearing twice.
                let key = (
                    symbol.name.clone(),
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
                .then(left.location.range.start.line.cmp(&right.location.range.start.line))
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
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.type_definition_at(position))
    }

    pub fn implementation(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Vec<LspLocation>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.implementations_at(position))
    }

    pub fn document_highlights(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Vec<LspDocumentHighlight>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.document_highlights_at(position))
    }

    pub fn prepare_rename(
        &mut self,
        uri: &EditorDocumentUri,
        position: LspPosition,
    ) -> EditorResult<Option<LspPrepareRenameResult>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.prepare_rename_at(position))
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
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.selection_ranges(&document, positions))
    }

    pub fn inlay_hints(
        &mut self,
        uri: &EditorDocumentUri,
        range: LspRange,
    ) -> EditorResult<Vec<LspInlayHint>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.inlay_hints(&document, range))
    }

    pub fn code_lenses(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<Vec<LspCodeLens>> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(snapshot.code_lenses(&document))
    }

    pub fn semantic_tokens(
        &mut self,
        uri: &EditorDocumentUri,
    ) -> EditorResult<LspSemanticTokens> {
        let document = self.open_document(uri)?.clone();
        let snapshot = self.semantic_snapshot(uri, &document)?;
        Ok(LspSemanticTokens {
            data: snapshot.semantic_tokens_for_current_path(),
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

    fn cached_document_mapping(&mut self, path: &std::path::Path) -> EditorResult<EditorWorkspaceMapping> {
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

    fn cached_workspace_roots(
        &mut self,
        directory: &std::path::Path,
    ) -> EditorWorkspaceRoots {
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
        let diagnostics = snapshot.diagnostics.clone();
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
