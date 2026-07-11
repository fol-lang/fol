use crate::{
    format_document_in_place,
    fol_tree_sitter_config, fol_tree_sitter_corpus, fol_tree_sitter_grammar,
    fol_tree_sitter_highlights_query, fol_tree_sitter_query_snapshots,
    fol_tree_sitter_symbols_query, EditorConfig, EditorDocumentUri, EditorError,
    EditorErrorKind, EditorLspServer, EditorResult, JsonRpcId, JsonRpcNotification,
    JsonRpcRequest, LspCompletionList, LspCompletionParams, LspDidOpenTextDocumentParams,
    LspLocation, LspPosition, LspReferenceContext, LspReferenceParams, LspRenameParams,
    LspSemanticTokens, LspSemanticTokensParams, LspTextDocumentIdentifier,
    LspTextDocumentItem, LspWorkspaceEdit,
};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommandSummary {
    pub command: String,
    pub summary: String,
    pub details: Vec<String>,
}

impl EditorCommandSummary {
    pub fn new(command: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }
}

pub fn editor_lsp_entrypoint() -> EditorResult<EditorCommandSummary> {
    Ok(EditorCommandSummary::new(
        "lsp",
        "ready to serve diagnostics, hover, definition, formatting, code actions, signature help, references, rename, semantic tokens, symbols, and completion through `fol tool lsp`",
    )
    .with_detail("transport=stdio")
    .with_detail("features=diagnostics,hover,definition,formatting,codeAction,signatureHelp,references,rename,semanticTokens,symbols,completion"))
}

fn source_line_count(source: &str) -> usize {
    source.lines().count()
}

fn sorted_query_captures(query: &str) -> Vec<String> {
    let mut captures = query
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ')' | '(' | '[' | ']'))
        .filter_map(|part| part.strip_prefix('@'))
        .map(|capture| capture.trim_end_matches(|ch: char| ch == ')' || ch == ']'))
        .filter(|capture| !capture.is_empty())
        .map(|capture| capture.to_string())
        .collect::<Vec<_>>();
    captures.sort();
    captures.dedup();
    captures
}

fn compiler_import_kinds_csv() -> String {
    fol_typecheck::editor_source_kind_names().join(",")
}

fn compiler_dot_intrinsic_names_csv() -> String {
    let mut names = fol_typecheck::editor_implemented_intrinsics()
        .into_iter()
        .filter(|entry| entry.surface == fol_intrinsics::IntrinsicSurface::DotRootCall)
        .map(|entry| entry.name.to_string())
        .collect::<Vec<_>>();
    names.sort();
    names.join(",")
}

fn semantic_token_legend_csv() -> String {
    crate::lsp::semantic_token_type_names().join(",")
}

pub fn editor_parse_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", path.display()),
        )
    })?;
    Ok(EditorCommandSummary::new(
        "parse",
        format!("loaded {} bytes for tree-sitter parsing", source.len()),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("bytes={}", source.len()))
    .with_detail(format!("grammar_bytes={}", fol_tree_sitter_grammar().len())))
}

pub fn editor_format_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let result = format_document_in_place(path)?;
    Ok(EditorCommandSummary::new(
        "format",
        if result.changed {
            format!("formatted {}", result.canonical_path.display())
        } else {
            format!("already formatted {}", result.canonical_path.display())
        },
    )
    .with_detail(format!("path={}", result.canonical_path.display()))
    .with_detail(format!("lines={}", result.line_count()))
    .with_detail(format!("changed={}", result.changed))
    .with_detail(format!("changed_lines={}", result.changed_line_count()))
    .with_detail("style=hybrid-line"))
}

pub fn editor_highlight_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", path.display()),
        )
    })?;
    let query = fol_tree_sitter_highlights_query();
    let captures = sorted_query_captures(query);
    Ok(EditorCommandSummary::new(
        "highlight",
        format!("highlight query ready with {} captures", captures.len()),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("query_bytes={}", query.len()))
    .with_detail(format!("capture_count={}", captures.len()))
    .with_detail(format!("captures={}", captures.join(",")))
    .with_detail(format!("import_kinds={}", compiler_import_kinds_csv()))
    .with_detail(format!(
        "intrinsic_names={}",
        compiler_dot_intrinsic_names_csv()
    )))
}

pub fn editor_symbols_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", path.display()),
        )
    })?;
    let symbol_count = source.matches("fun ").count()
        + source.matches("log ").count()
        + source.matches("typ ").count()
        + source.matches("ali ").count();
    Ok(EditorCommandSummary::new(
        "symbols",
        format!(
            "symbol query ready with {} bytes",
            fol_tree_sitter_symbols_query().len()
        ),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("symbol_candidates={symbol_count}"))
    .with_detail(format!(
        "query_snapshots={}",
        fol_tree_sitter_query_snapshots().len()
    )))
}

pub fn editor_semantic_tokens_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let canonical = path.canonicalize().map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to resolve '{}': {error}", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", canonical.display()),
        )
    })?;
    let uri = EditorDocumentUri::from_file_path(canonical.clone())?;
    let mut server = EditorLspServer::new(EditorConfig::default());
    server.handle_notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(
            serde_json::to_value(LspDidOpenTextDocumentParams {
                text_document: LspTextDocumentItem {
                    uri: uri.as_str().to_string(),
                    language_id: "fol".to_string(),
                    version: 1,
                    text: source.clone(),
                },
            })
            .expect("didOpen params should serialize"),
        ),
    })?;
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/semanticTokens/full".to_string(),
            params: Some(
                serde_json::to_value(LspSemanticTokensParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.as_str().to_string(),
                    },
                })
                .expect("semantic token params should serialize"),
            ),
        })?
        .expect("semantic token request should return a response");
    let tokens: LspSemanticTokens =
        serde_json::from_value(response.result.expect("semantic tokens result should exist"))
            .expect("semantic tokens should deserialize");
    let encoded_entries = tokens.data.len();
    let token_count = encoded_entries / 5;

    Ok(EditorCommandSummary::new(
        "semantic-tokens",
        format!("semantic token snapshot ready with {token_count} tokens"),
    )
    .with_detail(format!("path={}", canonical.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("token_count={token_count}"))
    .with_detail(format!("encoded_entries={encoded_entries}"))
    .with_detail(format!("legend={}", semantic_token_legend_csv())))
}

pub fn editor_references_file(
    path: &Path,
    position: LspPosition,
    include_declaration: bool,
) -> EditorResult<EditorCommandSummary> {
    let canonical = path.canonicalize().map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to resolve '{}': {error}", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", canonical.display()),
        )
    })?;
    let uri = EditorDocumentUri::from_file_path(canonical.clone())?;
    let mut server = EditorLspServer::new(EditorConfig::default());
    server.handle_notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(
            serde_json::to_value(LspDidOpenTextDocumentParams {
                text_document: LspTextDocumentItem {
                    uri: uri.as_str().to_string(),
                    language_id: "fol".to_string(),
                    version: 1,
                    text: source.clone(),
                },
            })
            .expect("didOpen params should serialize"),
        ),
    })?;
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(2),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.as_str().to_string(),
                    },
                    position,
                    context: LspReferenceContext {
                        include_declaration,
                    },
                })
                .expect("reference params should serialize"),
            ),
        })?
        .expect("references request should return a response");
    let references: Vec<LspLocation> =
        serde_json::from_value(response.result.expect("references result should exist"))
            .expect("references should deserialize");
    let cross_file_count = references
        .iter()
        .filter(|location| location.uri != uri.as_str())
        .count();

    Ok(EditorCommandSummary::new(
        "references",
        format!("resolved {} references at the requested position", references.len()),
    )
    .with_detail(format!("path={}", canonical.display()))
    .with_detail(format!("line={}", position.line))
    .with_detail(format!("character={}", position.character))
    .with_detail(format!("include_declaration={include_declaration}"))
    .with_detail(format!("reference_count={}", references.len()))
    .with_detail(format!("cross_file_count={cross_file_count}")))
}

pub fn editor_rename_file(
    path: &Path,
    position: LspPosition,
    new_name: &str,
) -> EditorResult<EditorCommandSummary> {
    let canonical = path.canonicalize().map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to resolve '{}': {error}", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", canonical.display()),
        )
    })?;
    let uri = EditorDocumentUri::from_file_path(canonical.clone())?;
    let mut server = EditorLspServer::new(EditorConfig::default());
    server.handle_notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(
            serde_json::to_value(LspDidOpenTextDocumentParams {
                text_document: LspTextDocumentItem {
                    uri: uri.as_str().to_string(),
                    language_id: "fol".to_string(),
                    version: 1,
                    text: source.clone(),
                },
            })
            .expect("didOpen params should serialize"),
        ),
    })?;
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.as_str().to_string(),
                    },
                    position,
                    new_name: new_name.to_string(),
                })
                .expect("rename params should serialize"),
            ),
        })?
        .expect("rename request should return a response");
    let edit: LspWorkspaceEdit =
        serde_json::from_value(response.result.expect("rename result should exist"))
            .expect("rename edit should deserialize");
    let change_count = edit.changes.values().map(Vec::len).sum::<usize>();
    let touched_files = edit.changes.len();

    Ok(EditorCommandSummary::new(
        "rename",
        format!("prepared {change_count} rename edits for '{new_name}'"),
    )
    .with_detail(format!("path={}", canonical.display()))
    .with_detail(format!("line={}", position.line))
    .with_detail(format!("character={}", position.character))
    .with_detail(format!("new_name={new_name}"))
    .with_detail(format!("edit_count={change_count}"))
    .with_detail(format!("touched_files={touched_files}")))
}

pub fn editor_completion_file(
    path: &Path,
    position: LspPosition,
) -> EditorResult<EditorCommandSummary> {
    let canonical = path.canonicalize().map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to resolve '{}': {error}", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", canonical.display()),
        )
    })?;
    let uri = EditorDocumentUri::from_file_path(canonical.clone())?;
    let mut server = EditorLspServer::new(EditorConfig::default());
    server.handle_notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(
            serde_json::to_value(LspDidOpenTextDocumentParams {
                text_document: LspTextDocumentItem {
                    uri: uri.as_str().to_string(),
                    language_id: "fol".to_string(),
                    version: 1,
                    text: source.clone(),
                },
            })
            .expect("didOpen params should serialize"),
        ),
    })?;
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(4),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.as_str().to_string(),
                    },
                    position,
                    context: None,
                })
                .expect("completion params should serialize"),
            ),
        })?
        .expect("completion request should return a response");
    let completions: LspCompletionList =
        serde_json::from_value(response.result.expect("completion result should exist"))
            .expect("completions should deserialize");
    let labels: Vec<String> = completions
        .items
        .iter()
        .map(|item| {
            let detail = item.detail.as_deref().unwrap_or("");
            format!("{}  ({})", item.label, detail)
        })
        .collect();

    Ok(EditorCommandSummary::new(
        "completion",
        format!("{} completion items at {}:{}", completions.items.len(), position.line, position.character),
    )
    .with_detail(format!("path={}", canonical.display()))
    .with_detail(format!("line={}", position.line))
    .with_detail(format!("character={}", position.character))
    .with_detail(format!("item_count={}", completions.items.len()))
    .with_detail(format!("items={}", labels.join(", "))))
}

pub fn editor_tree_generate_bundle(path: &Path) -> EditorResult<EditorCommandSummary> {
    if path.exists() && !path.is_dir() {
        return Err(EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("tree output root '{}' is not a directory", path.display()),
        ));
    }

    let cleaned_existing_root = if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to clear existing tree output root '{}': {error}",
                    path.display()
                ),
            )
        })?;
        true
    } else {
        false
    };

    let queries_root = path.join("queries").join("fol");
    let corpus_root = path.join("test").join("corpus");
    std::fs::create_dir_all(&queries_root).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to create query root '{}': {error}",
                queries_root.display()
            ),
        )
    })?;
    std::fs::create_dir_all(&corpus_root).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to create corpus root '{}': {error}",
                corpus_root.display()
            ),
        )
    })?;

    write_bundle_file(&path.join("grammar.js"), fol_tree_sitter_grammar())?;
    for snapshot in fol_tree_sitter_query_snapshots() {
        write_bundle_file(
            &queries_root.join(format!("{}.scm", snapshot.name)),
            snapshot.query,
        )?;
    }
    write_bundle_file(&path.join("package.json"), TREE_SITTER_PACKAGE_JSON)?;
    write_bundle_file(&path.join("tree-sitter.json"), fol_tree_sitter_config())?;

    for case in fol_tree_sitter_corpus() {
        write_bundle_file(&corpus_root.join(format!("{}.txt", case.name)), case.source)?;
    }

    let mut summary = EditorCommandSummary::new(
        "tree generate",
        format!("tree-sitter bundle ready at {}", path.display()),
    )
    .with_detail(format!("root={}", path.display()))
    .with_detail(format!("cleaned_existing_root={cleaned_existing_root}"))
    .with_detail(format!(
        "query_files={}",
        fol_tree_sitter_query_snapshots().len()
    ))
    .with_detail(format!("corpus_files={}", fol_tree_sitter_corpus().len()))
    .with_detail(format!("grammar_bytes={}", fol_tree_sitter_grammar().len()));

    match std::process::Command::new("tree-sitter")
        .arg("generate")
        .arg("--js-runtime")
        .arg("native")
        .current_dir(path)
        .status()
    {
        Ok(status) if status.success() => {
            let parser_path = path.join("src").join("parser.c");
            if parser_path.is_file() {
                summary = summary
                    .with_detail("parser_generated=true")
                    .with_detail(format!("parser={}", parser_path.display()))
                    .with_detail("tree_sitter_runtime=native");
            } else {
                return Err(EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "tree-sitter reported success but did not produce '{}'",
                        parser_path.display()
                    ),
                )
                .with_note("the generated bundle is incomplete")
                .with_note("check the tree-sitter CLI version and grammar assets"));
            }
        }
        Ok(status) => {
            return Err(EditorError::new(
                EditorErrorKind::Internal,
                format!("tree-sitter parser generation failed with status {status}"),
            )
            .with_note("`fol tool tree generate` requires a working `tree-sitter` CLI")
            .with_note("this command uses `tree-sitter generate --js-runtime native`")
            .with_note("fix the grammar or your local tree-sitter install, then try again"));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(EditorError::new(
                EditorErrorKind::Internal,
                "failed to run `tree-sitter generate --js-runtime native`",
            )
            .with_note("install the `tree-sitter` CLI and retry")
            .with_note("no Node.js runtime is required for this command"));
        }
        Err(error) => {
            return Err(EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to run tree-sitter parser generation: {error}"),
            )
            .with_note("the bundle files were written, but parser generation did not complete")
            .with_note("no Node.js runtime is required for this command"));
        }
    }

    Ok(summary)
}

const TREE_SITTER_PACKAGE_JSON: &str = r#"{
  "name": "tree-sitter-fol",
  "version": "0.1.0",
  "private": true,
  "grammars": [
    {
      "name": "fol",
      "scope": "source.fol",
      "file-types": ["fol"]
    }
  ]
}
"#;

fn write_bundle_file(path: &Path, contents: &str) -> EditorResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to create '{}' : {error}", parent.display()),
            )
        })?;
    }
    std::fs::write(path, contents).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to write '{}': {error}", path.display()),
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        compiler_dot_intrinsic_names_csv, compiler_import_kinds_csv, semantic_token_legend_csv,
        editor_format_file, editor_highlight_file, editor_lsp_entrypoint, editor_parse_file,
        editor_references_file, editor_rename_file, editor_semantic_tokens_file,
        editor_symbols_file, editor_tree_generate_bundle, sorted_query_captures,
    };
    use crate::{fol_tree_sitter_grammar, fol_tree_sitter_query_snapshots, LspPosition};
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root should resolve")
    }

    /// Builds a self-contained temp package whose entry file contains a
    /// same-package top-level routine that is safe to rename. Rename resolution
    /// needs a resolved workspace, which only exists for real packages, so the
    /// package-less repo fixtures used for the other file commands cannot back
    /// a rename smoke test. Returns the entry document path and a position on a
    /// renameable routine usage.
    fn rename_probe_package(label: &str) -> (PathBuf, LspPosition) {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_rename_probe_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"rename_probe\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"rename_probe\", root = \"src/main.fol\", fol_model = \"core\" });\n    return;\n};\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/main.fol"),
            "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
        )
        .unwrap();
        (
            root.join("src/main.fol"),
            LspPosition {
                line: 5,
                character: 11,
            },
        )
    }

    #[test]
    fn lsp_entrypoint_summary_is_stable() {
        let summary = editor_lsp_entrypoint().unwrap();
        assert_eq!(summary.command, "lsp");
        assert!(summary.summary.contains("fol tool lsp"));
        assert!(summary.summary.contains("completion"));
        assert!(summary.summary.contains("formatting"));
        assert!(summary.summary.contains("references"));
        assert!(summary.summary.contains("rename"));
        assert!(summary.summary.contains("semantic tokens"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "transport=stdio"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail
                == "features=diagnostics,hover,definition,formatting,codeAction,signatureHelp,references,rename,semanticTokens,symbols,completion"));
    }

    #[test]
    fn file_backed_editor_commands_report_path_and_shape() {
        let path = repo_root().join("test/apps/fixtures/record_flow/main.fol");
        let format_root = std::env::temp_dir().join(format!(
            "fol_editor_format_command_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&format_root).unwrap();
        let format_path = format_root.join("sample.fol");
        std::fs::write(&format_path, "fun[] main(): int = {\nreturn 0;\n};\n").unwrap();
        let format = editor_format_file(&format_path).unwrap();
        let parse = editor_parse_file(&path).unwrap();
        let highlight = editor_highlight_file(&path).unwrap();
        let symbols = editor_symbols_file(&path).unwrap();
        let semantic_tokens = editor_semantic_tokens_file(&path).unwrap();
        let references = editor_references_file(
            &path,
            LspPosition {
                line: 5,
                character: 11,
            },
            true,
        )
        .unwrap();
        let (rename_path, rename_position) = rename_probe_package("file_backed");
        let rename = editor_rename_file(&rename_path, rename_position, "count").unwrap();
        std::fs::remove_dir_all(rename_path.parent().and_then(Path::parent).unwrap()).ok();

        assert!(format.details.iter().any(|detail| detail.contains("path=")));
        assert!(format.details.iter().any(|detail| detail == "changed=true"));
        assert!(format
            .details
            .iter()
            .any(|detail| detail.starts_with("changed_lines=")));
        assert!(format
            .details
            .iter()
            .any(|detail| detail == "style=hybrid-line"));
        assert!(parse.details.iter().any(|detail| detail.contains("path=")));
        assert!(parse.details.iter().any(|detail| detail.contains("lines=")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail.contains("capture_count=")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail.contains("captures=")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail == &format!("import_kinds={}", compiler_import_kinds_csv())));
        assert!(highlight.details.iter().any(|detail| {
            detail == &format!("intrinsic_names={}", compiler_dot_intrinsic_names_csv())
        }));
        assert!(symbols
            .details
            .iter()
            .any(|detail| detail.contains("symbol_candidates=")));
        assert!(semantic_tokens
            .details
            .iter()
            .any(|detail| detail.contains("token_count=")));
        assert!(semantic_tokens
            .details
            .iter()
            .any(|detail| detail == &format!("legend={}", semantic_token_legend_csv())));
        assert!(references
            .details
            .iter()
            .any(|detail| detail.contains("reference_count=")));
        assert!(rename
            .details
            .iter()
            .any(|detail| detail.contains("edit_count=")));

        std::fs::remove_dir_all(format_root).ok();
    }

    #[test]
    fn real_fixtures_keep_editor_command_summaries_stable() {
        let showcase = repo_root().join("test/apps/showcases/full_v1_showcase/app/main.fol");
        let package = repo_root().join("xtra/logtiny/src/log.fol");
        let format_root = std::env::temp_dir().join(format!(
            "fol_editor_format_stable_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&format_root).unwrap();
        let format_path = format_root.join("sample.fol");
        std::fs::write(&format_path, "fun[] main(): int = {\nreturn 0;\n};\n").unwrap();

        let format = editor_format_file(&format_path).unwrap();
        let parse = editor_parse_file(&showcase).unwrap();
        let highlight = editor_highlight_file(&showcase).unwrap();
        let symbols = editor_symbols_file(&package).unwrap();
        let semantic_tokens = editor_semantic_tokens_file(&showcase).unwrap();
        let references = editor_references_file(
            &package,
            LspPosition {
                line: 44,
                character: 11,
            },
            true,
        )
        .unwrap();
        let (rename_path, rename_position) = rename_probe_package("real_fixtures");
        let rename = editor_rename_file(&rename_path, rename_position, "count").unwrap();
        std::fs::remove_dir_all(rename_path.parent().and_then(Path::parent).unwrap()).ok();
        let highlight_captures = sorted_query_captures(crate::fol_tree_sitter_highlights_query());

        let showcase_text = std::fs::read_to_string(&showcase).unwrap();
        let showcase_lines = showcase_text.lines().count();
        let showcase_bytes = showcase_text.len();

        assert_eq!(format.command, "format");
        assert_eq!(
            format.details,
            vec![
                format!("path={}", format_path.display()),
                "lines=3".to_string(),
                "changed=true".to_string(),
                "changed_lines=1".to_string(),
                "style=hybrid-line".to_string(),
            ]
        );
        assert_eq!(parse.command, "parse");
        assert_eq!(
            parse.details,
            vec![
                format!("path={}", showcase.display()),
                format!("lines={showcase_lines}"),
                format!("bytes={showcase_bytes}"),
                format!("grammar_bytes={}", fol_tree_sitter_grammar().len()),
            ]
        );
        assert_eq!(highlight.command, "highlight");
        assert_eq!(
            highlight.details,
            vec![
                format!("path={}", showcase.display()),
                format!("lines={showcase_lines}"),
                format!(
                    "query_bytes={}",
                    crate::fol_tree_sitter_highlights_query().len()
                ),
                format!("capture_count={}", highlight_captures.len()),
                format!("captures={}", highlight_captures.join(",")),
                format!("import_kinds={}", compiler_import_kinds_csv()),
                format!("intrinsic_names={}", compiler_dot_intrinsic_names_csv()),
            ]
        );
        let package_text = std::fs::read_to_string(&package).unwrap();
        let package_lines = package_text.lines().count();
        let package_symbol_candidates = package_text.matches("fun ").count()
            + package_text.matches("log ").count()
            + package_text.matches("typ ").count()
            + package_text.matches("ali ").count();

        assert_eq!(symbols.command, "symbols");
        assert_eq!(
            symbols.details,
            vec![
                format!("path={}", package.display()),
                format!("lines={package_lines}"),
                format!("symbol_candidates={package_symbol_candidates}"),
                format!(
                    "query_snapshots={}",
                    fol_tree_sitter_query_snapshots().len()
                ),
            ]
        );
        assert_eq!(semantic_tokens.command, "semantic-tokens");
        assert_eq!(semantic_tokens.details[0], format!("path={}", showcase.display()));
        assert_eq!(semantic_tokens.details[1], format!("lines={showcase_lines}"));
        let token_count = semantic_tokens.details[2]
            .strip_prefix("token_count=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let encoded_entries = semantic_tokens.details[3]
            .strip_prefix("encoded_entries=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(encoded_entries, token_count * 5);
        assert_eq!(
            semantic_tokens.details[4],
            format!("legend={}", semantic_token_legend_csv())
        );
        assert_eq!(references.command, "references");
        assert_eq!(references.details[0], format!("path={}", package.display()));
        assert_eq!(references.details[1], "line=44");
        assert_eq!(references.details[2], "character=11");
        assert_eq!(references.details[3], "include_declaration=true");
        let reference_count = references.details[4]
            .strip_prefix("reference_count=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let cross_file_count = references.details[5]
            .strip_prefix("cross_file_count=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(reference_count >= cross_file_count);
        assert_eq!(rename.command, "rename");
        assert_eq!(rename.details[0], format!("path={}", rename_path.display()));
        assert_eq!(rename.details[1], "line=5");
        assert_eq!(rename.details[2], "character=11");
        assert_eq!(rename.details[3], "new_name=count");
        let edit_count = rename.details[4]
            .strip_prefix("edit_count=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let touched_files = rename.details[5]
            .strip_prefix("touched_files=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(edit_count >= touched_files);

        std::fs::remove_dir_all(format_root).ok();
    }

    #[test]
    fn command_summary_metadata_helpers_stay_in_sync_with_compiler_facts() {
        let expected_import_kinds = fol_typecheck::editor_source_kind_names().join(",");
        assert_eq!(compiler_import_kinds_csv(), expected_import_kinds);

        let mut expected_intrinsics = fol_typecheck::editor_implemented_intrinsics()
            .into_iter()
            .filter(|entry| entry.surface == fol_intrinsics::IntrinsicSurface::DotRootCall)
            .map(|entry| entry.name.to_string())
            .collect::<Vec<_>>();
        expected_intrinsics.sort();
        assert_eq!(
            compiler_dot_intrinsic_names_csv(),
            expected_intrinsics.join(",")
        );

        assert_eq!(
            semantic_token_legend_csv(),
            crate::lsp::semantic_token_type_names().join(",")
        );
    }

    #[test]
    fn tree_generate_bundle_writes_editor_consumable_assets() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let summary = editor_tree_generate_bundle(&root).unwrap();

        assert_eq!(summary.command, "tree generate");
        assert!(root.join("grammar.js").is_file());
        assert!(root.join("queries/fol/highlights.scm").is_file());
        assert!(root.join("queries/fol/locals.scm").is_file());
        assert!(root.join("queries/fol/symbols.scm").is_file());
        assert!(root.join("test/corpus/declarations.txt").is_file());
        assert!(summary
            .details
            .iter()
            .any(|detail| detail.contains("query_files=3")));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail.contains("parser_generated=")));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_exports_every_registered_query_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_queries_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        editor_tree_generate_bundle(&root).unwrap();

        for snapshot in fol_tree_sitter_query_snapshots() {
            let exported = root
                .join("queries/fol")
                .join(format!("{}.scm", snapshot.name));
            assert!(
                exported.is_file(),
                "missing exported query snapshot: {}",
                exported.display()
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_keeps_exported_assets_exactly_in_sync() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_exact_assets_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        editor_tree_generate_bundle(&root).unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("grammar.js")).unwrap(),
            fol_tree_sitter_grammar()
        );
        assert_eq!(
            std::fs::read_to_string(root.join("tree-sitter.json")).unwrap(),
            crate::fol_tree_sitter_config()
        );
        for snapshot in fol_tree_sitter_query_snapshots() {
            assert_eq!(
                std::fs::read_to_string(
                    root.join("queries/fol")
                        .join(format!("{}.scm", snapshot.name))
                )
                .unwrap(),
                snapshot.query
            );
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_stays_neovim_consumable() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_nvim_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let summary = editor_tree_generate_bundle(&root).unwrap();

        assert!(root.join("src/parser.c").is_file());
        assert!(root.join("package.json").is_file());
        assert!(root.join("tree-sitter.json").is_file());
        assert!(root.join("queries/fol/highlights.scm").is_file());
        assert!(root.join("queries/fol/locals.scm").is_file());
        assert!(root.join("queries/fol/symbols.scm").is_file());
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "parser_generated=true"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "tree_sitter_runtime=native"));

        let package_json = std::fs::read_to_string(root.join("package.json")).unwrap();
        assert!(
            package_json.contains("\"scope\": \"source.fol\"")
                || package_json.contains("\"file-types\": [\"fol\"]")
        );
        let config = std::fs::read_to_string(root.join("tree-sitter.json")).unwrap();
        assert!(config.contains("\"highlights\": \"queries/fol/highlights.scm\""));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_clears_existing_output_roots() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_stale_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let stale_file = root.join("queries/fol/stale.scm");
        std::fs::create_dir_all(stale_file.parent().unwrap()).unwrap();
        std::fs::write(&stale_file, "(broken)").unwrap();

        let summary = editor_tree_generate_bundle(&root).unwrap();

        assert!(!stale_file.exists());
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "cleaned_existing_root=true"));

        std::fs::remove_dir_all(root).ok();
    }
}
