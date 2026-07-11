//! Editor tooling foundations for the FOL language.
//!
//! `fol-editor` hosts the hand-authored tree-sitter structure layer and the
//! compiler-backed language-server layer. Name families and capability-aware
//! editor facts should come from compiler-owned metadata instead of copied
//! editor registries.

mod commands;
mod convert;
mod documents;
mod error;
mod format;
mod lsp;
mod paths;
mod session;
mod tree_sitter;
mod workspace;

pub use commands::{
    editor_completion_file, editor_format_file, editor_highlight_file, editor_lsp_entrypoint,
    editor_parse_file, editor_references_file, editor_rename_file,
    editor_semantic_tokens_file, editor_symbols_file, editor_tree_generate_bundle,
    EditorCommandSummary,
};
pub use convert::{
    dedup_lsp_diagnostics, diagnostic_to_lsp, location_to_range, LspDiagnostic,
    LspDiagnosticRelatedInformation, LspDiagnosticSeverity, LspLocation, LspPosition, LspRange,
};
pub use documents::{EditorDocument, EditorDocumentStore};
pub use error::{EditorError, EditorErrorKind, EditorResult};
pub use format::{format_document, format_document_in_place};
pub(crate) use format::formatting_edit;
pub use lsp::{
    run_lsp_stdio, EditorCompletionItem, EditorLspServer, JsonRpcError, JsonRpcId,
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, LspCodeAction,
    LspCodeActionContext, LspCodeActionParams, LspCompletionContext, LspCompletionItem,
    LspCompletionList, LspCompletionOptions, LspCompletionParams, LspDefinitionParams,
    LspDocumentFormattingParams,
    LspDidChangeTextDocumentParams, LspDidCloseTextDocumentParams, LspDidOpenTextDocumentParams,
    LspDocumentSymbol, LspDocumentSymbolParams, LspHover, LspHoverParams, LspInitializeParams,
    LspInitializeResult, LspParameterInformation, LspPublishDiagnosticsParams,
    LspReferenceContext, LspReferenceParams, LspRenameParams, LspSemanticTokens,
    LspSemanticTokensLegend, LspSemanticTokensOptions, LspSemanticTokensParams,
    LspServerCapabilities, LspServerInfo, LspSignatureHelp, LspSignatureHelpOptions,
    LspSignatureHelpParams, LspSignatureInformation,
    LspTextDocumentContentChangeEvent, LspTextDocumentIdentifier, LspTextDocumentItem,
    LspTextDocumentSyncOptions, LspTextEdit, LspVersionedTextDocumentIdentifier,
    LspWorkspaceEdit, LspWorkspaceSymbol, LspWorkspaceSymbolParams,
};
pub use paths::{EditorDocumentPath, EditorDocumentUri};
pub use session::{EditorConfig, EditorSession};
pub use tree_sitter::{
    fol_tree_sitter_config, fol_tree_sitter_corpus, fol_tree_sitter_grammar,
    fol_tree_sitter_highlights_query, fol_tree_sitter_locals_query,
    fol_tree_sitter_query_snapshots, fol_tree_sitter_symbols_query, TreeSitterCorpusCase,
    TreeSitterQuerySnapshot,
};
pub use workspace::{
    map_document_workspace, materialize_analysis_overlay, EditorAnalysisOverlay,
    EditorWorkspaceRoots,
    EditorWorkspaceMapping,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Editor;

pub const CRATE_NAME: &str = "fol-editor";

impl Editor {
    pub fn new() -> Self {
        Self
    }
}

pub fn crate_name() -> &'static str {
    CRATE_NAME
}

#[cfg(test)]
mod tests {
    use super::{
        crate_name, diagnostic_to_lsp, editor_format_file, editor_highlight_file,
        editor_lsp_entrypoint, editor_parse_file, editor_references_file, editor_rename_file,
        editor_semantic_tokens_file, editor_symbols_file, editor_tree_generate_bundle,
        fol_tree_sitter_corpus, fol_tree_sitter_grammar, fol_tree_sitter_highlights_query,
        fol_tree_sitter_locals_query, fol_tree_sitter_query_snapshots,
        fol_tree_sitter_symbols_query, map_document_workspace, materialize_analysis_overlay,
        Editor, EditorConfig, EditorDocument, EditorDocumentPath, EditorDocumentStore,
        EditorDocumentUri, EditorError, EditorErrorKind, EditorLspServer, EditorResult,
        EditorSession, LspDiagnosticSeverity, LspPosition, CRATE_NAME,
    };
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root should resolve")
    }

    fn lsp_message(value: &str) -> String {
        format!("Content-Length: {}\r\n\r\n{}", value.len(), value)
    }

    #[test]
    fn crate_name_matches_editor_identity() {
        assert_eq!(crate_name(), CRATE_NAME);
    }

    #[test]
    fn public_editor_shell_is_constructible() {
        assert_eq!(Editor::new(), Editor);
    }

    #[test]
    fn public_editor_types_are_constructible() {
        let error = EditorError::new(EditorErrorKind::Internal, "boom");
        let uri = EditorDocumentUri::from_file_path(PathBuf::from("/tmp/demo.fol")).unwrap();
        let path = EditorDocumentPath::new(PathBuf::from("/tmp/demo.fol"));
        let result: EditorResult<()> = Ok(());

        assert_eq!(error.kind, EditorErrorKind::Internal);
        assert_eq!(uri.as_str(), "file:///tmp/demo.fol");
        assert_eq!(path.as_path(), PathBuf::from("/tmp/demo.fol").as_path());
        assert!(result.is_ok());
    }

    #[test]
    fn document_store_and_session_shells_are_constructible() {
        let uri = EditorDocumentUri::from_file_path(PathBuf::from("/tmp/demo.fol")).unwrap();
        let document = EditorDocument::new(uri, 1, "fun main(): int = 0".to_string()).unwrap();
        let mut store = EditorDocumentStore::default();
        let config = EditorConfig::default();
        let mut session = EditorSession::new(config.clone());

        store.open(document.clone());
        session.documents.open(document);

        assert_eq!(store.len(), 1);
        assert_eq!(session.config, config);
        assert_eq!(session.documents.len(), 1);
    }

    #[test]
    fn tree_sitter_assets_are_publicly_reachable() {
        assert!(fol_tree_sitter_grammar().contains("module.exports = grammar"));
        assert!(fol_tree_sitter_highlights_query().contains("@keyword"));
        assert!(fol_tree_sitter_locals_query().contains("@local.definition"));
        assert!(fol_tree_sitter_symbols_query().contains("@symbol"));
        assert!(!fol_tree_sitter_query_snapshots().is_empty());
        assert!(!fol_tree_sitter_corpus().is_empty());
    }

    #[test]
    fn editor_commands_are_callable() {
        let path = repo_root().join("test/apps/fixtures/record_flow/main.fol");

        assert_eq!(editor_lsp_entrypoint().unwrap().command, "lsp");
        let format_root = std::env::temp_dir().join("fol_editor_public_format_smoke");
        std::fs::create_dir_all(&format_root).unwrap();
        let format_path = format_root.join("sample.fol");
        std::fs::write(&format_path, "fun[] main(): int = {\nreturn 0;\n};\n").unwrap();
        assert_eq!(editor_format_file(&format_path).unwrap().command, "format");
        assert_eq!(editor_parse_file(&path).unwrap().command, "parse");
        assert_eq!(editor_highlight_file(&path).unwrap().command, "highlight");
        assert_eq!(editor_symbols_file(&path).unwrap().command, "symbols");
        assert_eq!(
            editor_references_file(
                &path,
                LspPosition {
                    line: 5,
                    character: 11,
                },
                true,
            )
            .unwrap()
            .command,
            "references"
        );
        assert_eq!(
            editor_rename_file(
                &path,
                LspPosition {
                    line: 5,
                    character: 11,
                },
                "count",
            )
            .unwrap()
            .command,
            "rename"
        );
        assert_eq!(
            editor_semantic_tokens_file(&path).unwrap().command,
            "semantic-tokens"
        );
        let root = std::env::temp_dir().join("fol_editor_public_tree_bundle_smoke");
        assert_eq!(
            editor_tree_generate_bundle(&root).unwrap().command,
            "tree generate"
        );
        std::fs::remove_dir_all(format_root).ok();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn lsp_and_workspace_shells_are_publicly_constructible() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_public_lsp_workspace_{}_{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch").as_nanos()
        ));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        std::fs::write(root.join("build.fol"), "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n").unwrap();
        let file = src.join("main.fol");
        let text = "fun[] main(): int = {\n    return 0;\n};\n";
        std::fs::write(&file, text).unwrap();

        let path = file;
        let mapping = map_document_workspace(&path, &EditorConfig::default()).unwrap();
        let document = EditorDocument::new(
            EditorDocumentUri::from_file_path(path.clone()).unwrap(),
            1,
            std::fs::read_to_string(&path).unwrap(),
        )
        .unwrap();
        let overlay = materialize_analysis_overlay(&mapping, &document).unwrap();
        let server = EditorLspServer::new(EditorConfig::default());
        let diagnostic = diagnostic_to_lsp(&fol_diagnostics::Diagnostic::error("E1000", "boom"));

        assert!(overlay.analysis_root().is_dir());
        assert!(mapping.package_root.is_some());
        assert!(server.session.documents.is_empty());
        assert_eq!(diagnostic.severity, LspDiagnosticSeverity::Error);

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn lsp_stdio_loop_handles_initialize_and_exit() {
        let initialize = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}";
        let exit = "{\"jsonrpc\":\"2.0\",\"method\":\"exit\",\"params\":null}";
        let input = format!("{}{}", lsp_message(initialize), lsp_message(exit));
        let mut output = Vec::new();
        crate::lsp::run_lsp_stdio_with_io(
            Cursor::new(input.into_bytes()),
            &mut output,
            EditorConfig::default(),
        )
        .unwrap();
        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("Content-Length:"));
        assert!(!rendered.contains("\"method\":\"initialize\""));
        assert!(rendered.contains("\"hoverProvider\":true"));
        assert!(rendered.contains("\"formattingProvider\":true"));
        assert!(!rendered.contains("\"rangeFormattingProvider\":true"));
        assert!(rendered.contains("\"codeActionProvider\":true"));
        assert!(rendered.contains("\"workspaceSymbolProvider\":true"));
        assert!(rendered.contains("\"signatureHelpProvider\""));
        assert!(
            rendered.contains("\"completion_provider\"")
                || rendered.contains("\"completionProvider\"")
        );
    }

    #[test]
    fn lsp_stdio_loop_handles_completion_requests() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_stdio_completion_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        std::fs::write(
            root.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n",
        )
        .unwrap();
        let file = src.join("main.fol");
        let text = "fun[] main(): int = {\n    var value: int = 7;\n    return value;\n};\n";
        std::fs::write(&file, text).unwrap();
        let uri = format!("file://{}", file.display());

        let initialize = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}";
        let did_open = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "fol",
                    "version": 1,
                    "text": text,
                }
            }
        })
        .to_string();
        let completion = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {
                    "uri": uri,
                },
                "position": {
                    "line": 2,
                    "character": 12,
                }
            }
        })
        .to_string();
        let exit = "{\"jsonrpc\":\"2.0\",\"method\":\"exit\",\"params\":null}";
        let input = format!(
            "{}{}{}{}",
            lsp_message(initialize),
            lsp_message(&did_open),
            lsp_message(&completion),
            lsp_message(exit)
        );

        let mut output = Vec::new();
        crate::lsp::run_lsp_stdio_with_io(
            Cursor::new(input.into_bytes()),
            &mut output,
            EditorConfig::default(),
        )
        .unwrap();
        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("\"formattingProvider\":true"));
        assert!(!rendered.contains("\"rangeFormattingProvider\":true"));
        assert!(rendered.contains("\"codeActionProvider\":true"));
        assert!(rendered.contains("\"workspaceSymbolProvider\":true"));
        assert!(rendered.contains("\"signatureHelpProvider\""));
        assert!(rendered.contains("\"completionProvider\""));
        assert!(rendered.contains("\"isIncomplete\":false"));
        assert!(rendered.contains("\"label\":\"value\""));

        std::fs::remove_dir_all(root).ok();
    }
}
