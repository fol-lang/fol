use crate::{LspDiagnostic, LspPosition, LspRange};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspTextDocumentSyncOptions {
    pub open_close: bool,
    pub change: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspServerCapabilities {
    pub position_encoding: String,
    pub text_document_sync: LspTextDocumentSyncOptions,
    pub hover_provider: bool,
    pub definition_provider: bool,
    pub document_symbol_provider: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_symbol_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatting_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_action_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_help_provider: Option<LspSignatureHelpOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename_provider: Option<LspRenameOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_tokens_provider: Option<LspSemanticTokensOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<LspCompletionOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_definition_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_highlight_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folding_range_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_range_provider: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inlay_hint_provider: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trigger_characters: Vec<String>,
    #[serde(default)]
    pub resolve_provider: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSignatureHelpOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trigger_characters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSemanticTokensLegend {
    pub token_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSemanticTokensOptions {
    pub legend: LspSemanticTokensLegend,
    pub full: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspInitializeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspInitializeResult {
    pub capabilities: LspServerCapabilities,
    pub server_info: LspServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspTextDocumentItem {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspVersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspTextDocumentContentChangeEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<LspRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_length: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDidOpenTextDocumentParams {
    pub text_document: LspTextDocumentItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDidChangeTextDocumentParams {
    pub text_document: LspVersionedTextDocumentIdentifier,
    pub content_changes: Vec<LspTextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDidCloseTextDocumentParams {
    pub text_document: LspVersionedTextDocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspPublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspTextDocumentIdentifier {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspHoverParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDefinitionParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSignatureHelpParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspReferenceContext {
    pub include_declaration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspReferenceParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
    pub context: LspReferenceContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspRenameParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSemanticTokensParams {
    pub text_document: LspTextDocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentSymbolParams {
    pub text_document: LspTextDocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspWorkspaceSymbolParams {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentFormattingParams {
    pub text_document: LspTextDocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCodeActionContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCodeActionParams {
    pub text_document: LspTextDocumentIdentifier,
    pub range: LspRange,
    pub context: LspCodeActionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_kind: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_character: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionParams {
    pub text_document: LspTextDocumentIdentifier,
    pub position: LspPosition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<LspCompletionContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionList {
    pub is_incomplete: bool,
    pub items: Vec<LspCompletionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionItem {
    pub label: String,
    pub kind: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorCompletionItem {
    pub label: String,
    pub kind: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSignatureHelp {
    pub signatures: Vec<LspSignatureInformation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_signature: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSignatureInformation {
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<LspParameterInformation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspParameterInformation {
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspHover {
    pub contents: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<LspRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentSymbol {
    pub name: String,
    pub kind: u8,
    pub range: LspRange,
    pub selection_range: LspRange,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<LspDocumentSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspWorkspaceSymbol {
    pub name: String,
    pub kind: u8,
    pub location: crate::LspLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspTextEdit {
    pub range: LspRange,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspWorkspaceEdit {
    pub changes: std::collections::BTreeMap<String, Vec<LspTextEdit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspCodeAction {
    pub title: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<LspDiagnostic>,
    pub edit: LspWorkspaceEdit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSemanticTokens {
    pub data: Vec<u32>,
}

// ---- Tier 1/2 navigation, annotation, and structural features ----

/// `textDocument/documentHighlight` result item. `kind`: 1=Text, 2=Read, 3=Write.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentHighlight {
    pub range: LspRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
}

/// `textDocument/prepareRename` result (range + placeholder form).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspPrepareRenameResult {
    pub range: LspRange,
    pub placeholder: String,
}

/// `textDocument/foldingRange` result item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspFoldingRange {
    pub start_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_character: Option<u32>,
    pub end_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_character: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspFoldingRangeParams {
    pub text_document: LspTextDocumentIdentifier,
}

/// `textDocument/selectionRange` result — a range plus its enclosing parent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSelectionRange {
    pub range: LspRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Box<LspSelectionRange>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspSelectionRangeParams {
    pub text_document: LspTextDocumentIdentifier,
    pub positions: Vec<LspPosition>,
}

/// `textDocument/inlayHint` result item. `kind`: 1=Type, 2=Parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspInlayHint {
    pub position: LspPosition,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub padding_left: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub padding_right: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspInlayHintParams {
    pub text_document: LspTextDocumentIdentifier,
    pub range: LspRange,
}

/// Advertised as `renameProvider: { prepareProvider: true }` so clients issue
/// `textDocument/prepareRename` before renaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspRenameOptions {
    pub prepare_provider: bool,
}
