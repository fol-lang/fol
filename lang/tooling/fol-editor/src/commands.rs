use crate::tree_sitter::{
    execute_fol_tree_sitter_parse, execute_fol_tree_sitter_query, fol_tree_sitter_showcase_fixture,
    TreeSitterParseResult, TreeSitterQueryCapture, TreeSitterSyntaxIssue,
};
use crate::{
    fol_tree_sitter_config, fol_tree_sitter_corpus, fol_tree_sitter_grammar,
    fol_tree_sitter_highlights_query, fol_tree_sitter_query_snapshots,
    fol_tree_sitter_symbols_query, format_document_in_place, EditorConfig, EditorDocumentUri,
    EditorError, EditorErrorKind, EditorLspServer, EditorResult, JsonRpcId, JsonRpcNotification,
    JsonRpcRequest, LspCompletionList, LspCompletionParams, LspDidOpenTextDocumentParams,
    LspLocation, LspPosition, LspReferenceContext, LspReferenceParams, LspRenameParams,
    LspSemanticTokens, LspSemanticTokensParams, LspTextDocumentIdentifier, LspTextDocumentItem,
    LspWorkspaceEdit,
};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
        "ready to serve compiler-backed diagnostics, hover, definition, type definition, implementation, document highlights, references, prepare rename, rename, semantic tokens, document and workspace symbols, completion, signature help, hints, formatting, code actions, folding, and selection through `fol tool lsp`",
    )
    .with_detail("transport=stdio")
    .with_detail("features=diagnostics,hover,definition,typeDefinition,implementation,documentHighlight,formatting,codeAction,signatureHelp,references,prepareRename,rename,semanticTokens,documentSymbols,workspaceSymbols,completion,inlayHint,foldingRange,selectionRange"))
}

fn source_line_count(source: &str) -> usize {
    source.lines().count()
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

fn read_editor_tool_source(path: &Path) -> EditorResult<String> {
    std::fs::read_to_string(path).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", path.display()),
        )
    })
}

fn open_semantic_tool_document(
    server: &mut EditorLspServer,
    uri: &EditorDocumentUri,
    source: &str,
) -> EditorResult<()> {
    let published = server.handle_notification(JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(
            serde_json::to_value(LspDidOpenTextDocumentParams {
                text_document: LspTextDocumentItem {
                    uri: uri.as_str().to_string(),
                    language_id: "fol".to_string(),
                    version: 1,
                    text: source.to_string(),
                },
            })
            .expect("didOpen params should serialize"),
        ),
    })?;
    let diagnostics = published
        .iter()
        .flat_map(|publication| publication.diagnostics.iter())
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }

    let rendered = diagnostics
        .iter()
        .map(|diagnostic| format!("[{}] {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ");
    Err(EditorError::new(
        EditorErrorKind::InvalidInput,
        format!(
            "semantic analysis reported {} diagnostic(s) for '{}': {rendered}",
            diagnostics.len(),
            uri.as_str()
        ),
    ))
}

fn tree_sitter_command_error(operation: &str, error: String) -> EditorError {
    EditorError::new(
        EditorErrorKind::Internal,
        format!("tree-sitter {operation} failed: {error}"),
    )
}

fn escaped_tool_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn syntax_issue_detail(label: &str, issue: &TreeSitterSyntaxIssue) -> String {
    format!(
        "{label}={}@{}:{}-{}:{}:{}",
        issue.kind,
        issue.start_row,
        issue.start_column,
        issue.end_row,
        issue.end_column,
        escaped_tool_text(&issue.text)
    )
}

fn capture_detail(label: &str, capture: &TreeSitterQueryCapture) -> String {
    format!(
        "{label}={}@{}:{}-{}:{}:{}",
        capture.name,
        capture.start_row,
        capture.start_column,
        capture.end_row,
        capture.end_column,
        escaped_tool_text(&capture.text)
    )
}

fn with_tree_sitter_parse_details(
    mut summary: EditorCommandSummary,
    parse: &TreeSitterParseResult,
) -> EditorCommandSummary {
    summary = summary
        .with_detail(format!(
            "parse_status={}",
            if parse.has_error() { "ERROR" } else { "ok" }
        ))
        .with_detail(format!("root_kind={}", parse.root_kind))
        .with_detail(format!("node_count={}", parse.node_count))
        .with_detail(format!("named_node_count={}", parse.named_node_count))
        .with_detail(format!("error_count={}", parse.errors.len()))
        .with_detail(format!("missing_count={}", parse.missing.len()));
    for issue in &parse.errors {
        summary = summary.with_detail(syntax_issue_detail("error", issue));
    }
    for issue in &parse.missing {
        summary = summary.with_detail(syntax_issue_detail("missing", issue));
    }
    summary
}

pub fn editor_parse_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let source = read_editor_tool_source(path)?;
    let parse = execute_fol_tree_sitter_parse(&source)
        .map_err(|error| tree_sitter_command_error("parse", error))?;
    let summary = EditorCommandSummary::new(
        "parse",
        format!(
            "tree-sitter parsed {} named nodes with {} ERROR and {} missing nodes",
            parse.named_node_count,
            parse.errors.len(),
            parse.missing.len()
        ),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("bytes={}", source.len()));
    Ok(with_tree_sitter_parse_details(summary, &parse)
        .with_detail(format!("syntax_tree={}", parse.syntax_tree)))
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
    let source = read_editor_tool_source(path)?;
    let query = fol_tree_sitter_highlights_query();
    let result = execute_fol_tree_sitter_query(&source, query)
        .map_err(|error| tree_sitter_command_error("highlight query", error))?;
    let capture_kinds = result
        .captures
        .iter()
        .map(|capture| capture.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut summary = EditorCommandSummary::new(
        "highlight",
        format!(
            "tree-sitter highlight query matched {} captures",
            result.captures.len()
        ),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("query_bytes={}", query.len()))
    .with_detail(format!("capture_count={}", result.captures.len()))
    .with_detail(format!(
        "capture_kinds={}",
        capture_kinds.into_iter().collect::<Vec<_>>().join(",")
    ))
    .with_detail(format!("import_kinds={}", compiler_import_kinds_csv()))
    .with_detail(format!(
        "intrinsic_names={}",
        compiler_dot_intrinsic_names_csv()
    ));
    summary = with_tree_sitter_parse_details(summary, &result.parse);
    for capture in &result.captures {
        summary = summary.with_detail(capture_detail("capture", capture));
    }
    Ok(summary)
}

pub fn editor_symbols_file(path: &Path) -> EditorResult<EditorCommandSummary> {
    let source = read_editor_tool_source(path)?;
    let query = fol_tree_sitter_symbols_query();
    let result = execute_fol_tree_sitter_query(&source, query)
        .map_err(|error| tree_sitter_command_error("symbol query", error))?;
    let scope_count = result
        .captures
        .iter()
        .filter(|capture| capture.name == "symbol.scope")
        .count();
    let symbols = result
        .captures
        .iter()
        .filter(|capture| capture.name != "symbol.scope")
        .collect::<Vec<_>>();
    let symbol_kinds = symbols
        .iter()
        .map(|capture| capture.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut summary = EditorCommandSummary::new(
        "symbols",
        format!(
            "tree-sitter symbol query matched {} symbols in {scope_count} scopes",
            symbols.len()
        ),
    )
    .with_detail(format!("path={}", path.display()))
    .with_detail(format!("lines={}", source_line_count(&source)))
    .with_detail(format!("query_bytes={}", query.len()))
    .with_detail(format!("symbol_count={}", symbols.len()))
    .with_detail(format!("scope_count={scope_count}"))
    .with_detail(format!(
        "symbol_kinds={}",
        symbol_kinds.into_iter().collect::<Vec<_>>().join(",")
    ));
    summary = with_tree_sitter_parse_details(summary, &result.parse);
    for symbol in symbols {
        summary = summary.with_detail(capture_detail("symbol", symbol));
    }
    Ok(summary)
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
    open_semantic_tool_document(&mut server, &uri, &source)?;
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
    let tokens: LspSemanticTokens = serde_json::from_value(
        response
            .result
            .expect("semantic tokens result should exist"),
    )
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
    open_semantic_tool_document(&mut server, &uri, &source)?;
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
        format!(
            "resolved {} references at the requested position",
            references.len()
        ),
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
    open_semantic_tool_document(&mut server, &uri, &source)?;
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
        format!(
            "{} completion items at {}:{}",
            completions.items.len(),
            position.line,
            position.character
        ),
    )
    .with_detail(format!("path={}", canonical.display()))
    .with_detail(format!("line={}", position.line))
    .with_detail(format!("character={}", position.character))
    .with_detail(format!("item_count={}", completions.items.len()))
    .with_detail(format!("items={}", labels.join(", "))))
}

pub fn editor_tree_generate_bundle(path: &Path) -> EditorResult<EditorCommandSummary> {
    editor_tree_generate_bundle_with(path, run_tree_sitter_generate, |_| Ok(()))
}

fn editor_tree_generate_bundle_with<Generate, BeforeCommit>(
    path: &Path,
    generate: Generate,
    mut before_commit: BeforeCommit,
) -> EditorResult<EditorCommandSummary>
where
    Generate: FnOnce(&Path) -> EditorResult<()>,
    BeforeCommit: FnMut(&Path) -> EditorResult<()>,
{
    let (destination, updated_existing_root) = checked_tree_bundle_destination(path)?;
    let previously_generated_files = read_generated_tree_bundle_manifest(&destination)?;
    let generated_files = generated_tree_bundle_files();
    validate_tree_bundle_targets(&destination, &previously_generated_files, &generated_files)?;

    // Parser generation happens in a sibling staging directory. A missing or
    // failing external CLI therefore cannot partially rewrite the live bundle.
    let mut staging = TreeBundleStaging::create(&destination)?;
    populate_staged_tree_bundle(staging.path())?;
    generate(staging.path())?;

    let staged_parser = staging.path().join("src/parser.c");
    let parser_metadata = std::fs::symlink_metadata(&staged_parser).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "tree-sitter reported success but did not produce '{}': {error}",
                staged_parser.display()
            ),
        )
        .with_note("the staged generated bundle is incomplete")
        .with_note("the destination bundle was not changed")
    })?;
    if parser_metadata.file_type().is_symlink() || !parser_metadata.is_file() {
        return Err(EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "tree-sitter reported success but did not produce a regular parser file at '{}'",
                staged_parser.display()
            ),
        )
        .with_note("the staged generated bundle is incomplete")
        .with_note("the destination bundle was not changed"));
    }

    write_generated_tree_bundle_manifest(staging.path(), &generated_files)?;
    commit_staged_tree_bundle(
        &destination,
        &mut staging,
        updated_existing_root,
        &previously_generated_files,
        &generated_files,
        &mut before_commit,
    )?;

    let mut summary = EditorCommandSummary::new(
        "tree generate",
        format!("tree-sitter bundle ready at {}", path.display()),
    )
    .with_detail(format!("root={}", path.display()))
    .with_detail(format!("updated_existing_root={updated_existing_root}"))
    .with_detail(format!(
        "query_files={}",
        fol_tree_sitter_query_snapshots().len()
    ))
    .with_detail(format!("corpus_files={}", fol_tree_sitter_corpus().len()))
    .with_detail("fixture_files=1")
    .with_detail(format!("grammar_bytes={}", fol_tree_sitter_grammar().len()));

    summary = summary
        .with_detail("parser_generated=true")
        .with_detail(format!("parser={}", path.join("src/parser.c").display()))
        .with_detail("tree_sitter_runtime=native")
        .with_detail(format!("generated_files={}", generated_files.len()))
        .with_detail(format!(
            "manifest={}",
            path.join(TREE_SITTER_BUNDLE_MANIFEST).display()
        ));

    Ok(summary)
}

fn run_tree_sitter_generate(path: &Path) -> EditorResult<()> {
    match std::process::Command::new("tree-sitter")
        .arg("generate")
        .arg("--js-runtime")
        .arg("native")
        .current_dir(path)
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(EditorError::new(
            EditorErrorKind::Internal,
            format!("tree-sitter parser generation failed with status {status}"),
        )
        .with_note("`fol tool tree generate` requires a working `tree-sitter` CLI")
        .with_note("this command uses `tree-sitter generate --js-runtime native`")
        .with_note("the destination bundle was not changed")
        .with_note("fix the grammar or your local tree-sitter install, then try again")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(EditorError::new(
            EditorErrorKind::Internal,
            "failed to run `tree-sitter generate --js-runtime native`",
        )
        .with_note("install the `tree-sitter` CLI and retry")
        .with_note("no Node.js runtime is required for this command")
        .with_note("the destination bundle was not changed")),
        Err(error) => Err(EditorError::new(
            EditorErrorKind::Internal,
            format!("failed to run tree-sitter parser generation: {error}"),
        )
        .with_note("the staged parser generation did not complete")
        .with_note("no Node.js runtime is required for this command")
        .with_note("the destination bundle was not changed")),
    }
}

const TREE_SITTER_BUNDLE_MANIFEST: &str = ".fol-tree-generated";
const TREE_SITTER_CLI_GENERATED_FILES: &[&str] = &[
    "src/grammar.json",
    "src/node-types.json",
    "src/parser.c",
    "src/tree_sitter/alloc.h",
    "src/tree_sitter/array.h",
    "src/tree_sitter/parser.h",
];

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

fn generated_tree_bundle_files() -> BTreeSet<String> {
    let mut files = TREE_SITTER_CLI_GENERATED_FILES
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();
    files.extend(
        ["grammar.js", "package.json", "tree-sitter.json"]
            .into_iter()
            .map(str::to_string),
    );
    files.extend(
        fol_tree_sitter_query_snapshots()
            .iter()
            .map(|snapshot| format!("queries/fol/{}.scm", snapshot.name)),
    );
    files.extend(
        fol_tree_sitter_corpus()
            .iter()
            .map(|case| format!("test/corpus/{}.txt", case.name)),
    );
    files.insert("test/fixtures/showcase.fol".to_string());
    files
}

static TREE_BUNDLE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TreeBundleStaging {
    path: PathBuf,
    remove_on_drop: bool,
}

impl TreeBundleStaging {
    fn create(destination: &Path) -> EditorResult<Self> {
        let mut staging_parent = destination.parent().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidDocumentPath,
                format!(
                    "tree output root '{}' has no parent directory",
                    destination.display()
                ),
            )
        })?;
        while !staging_parent.exists() {
            staging_parent = staging_parent.parent().ok_or_else(|| {
                EditorError::new(
                    EditorErrorKind::InvalidDocumentPath,
                    format!(
                        "tree output root '{}' has no existing ancestor",
                        destination.display()
                    ),
                )
            })?;
        }
        ensure_no_symlink_components(staging_parent)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..128 {
            let counter = TREE_BUNDLE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = staging_parent.join(format!(
                ".fol-tree-stage-{}-{timestamp}-{counter}",
                std::process::id()
            ));
            match std::fs::create_dir(&candidate) {
                Ok(()) => {
                    return Ok(Self {
                        path: candidate,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(EditorError::new(
                        EditorErrorKind::Internal,
                        format!(
                            "failed to create tree bundle staging directory '{}': {error}",
                            candidate.display()
                        ),
                    ));
                }
            }
        }

        Err(EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to reserve a tree bundle staging directory under '{}'",
                staging_parent.display()
            ),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn keep(&mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for TreeBundleStaging {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Debug)]
struct BundleFileSnapshot {
    path: PathBuf,
    contents: Option<Vec<u8>>,
    permissions: Option<std::fs::Permissions>,
}

fn checked_tree_bundle_destination(path: &Path) -> EditorResult<(PathBuf, bool)> {
    if path.as_os_str().is_empty() {
        return Err(EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            "tree output root cannot be empty",
        ));
    }
    let destination = normalized_absolute_path(path)?;
    if destination.parent().is_none() {
        return Err(EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!(
                "tree output root '{}' cannot be a filesystem root",
                path.display()
            ),
        ));
    }
    ensure_no_symlink_components(&destination)?;

    match std::fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.is_dir() => Ok((destination, true)),
        Ok(_) => Err(EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("tree output root '{}' is not a directory", path.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((destination, false)),
        Err(error) => Err(EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!(
                "failed to inspect tree output root '{}': {error}",
                path.display()
            ),
        )),
    }
}

fn normalized_absolute_path(path: &Path) -> EditorResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                EditorError::new(
                    EditorErrorKind::Internal,
                    format!("failed to resolve the current directory: {error}"),
                )
            })?
            .join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    return Err(EditorError::new(
                        EditorErrorKind::InvalidDocumentPath,
                        format!(
                            "tree output root '{}' escapes the filesystem root",
                            path.display()
                        ),
                    ));
                }
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn ensure_no_symlink_components(path: &Path) -> EditorResult<()> {
    let mut current = PathBuf::new();
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!(
                        "tree bundle path '{}' contains symlink component '{}'",
                        path.display(),
                        current.display()
                    ),
                )
                .with_note("refusing to follow links while generating managed bundle assets"));
            }
            Ok(metadata) if components.peek().is_some() && !metadata.is_dir() => {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!(
                        "tree bundle path '{}' crosses non-directory component '{}'",
                        path.display(),
                        current.display()
                    ),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to inspect tree bundle path component '{}': {error}",
                        current.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_bundle_file_target(path: &Path) -> EditorResult<()> {
    ensure_no_symlink_components(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(EditorError::new(
            EditorErrorKind::InvalidInput,
            format!(
                "tree bundle managed path '{}' is not a regular file",
                path.display()
            ),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to inspect tree bundle managed path '{}': {error}",
                path.display()
            ),
        )),
    }
}

fn validate_tree_bundle_targets(
    destination: &Path,
    previous_files: &BTreeSet<String>,
    generated_files: &BTreeSet<String>,
) -> EditorResult<()> {
    validate_bundle_file_target(&destination.join(TREE_SITTER_BUNDLE_MANIFEST))?;
    for relative in previous_files.union(generated_files) {
        validate_bundle_file_target(&destination.join(relative))?;
    }
    Ok(())
}

fn populate_staged_tree_bundle(path: &Path) -> EditorResult<()> {
    let queries_root = path.join("queries/fol");
    let corpus_root = path.join("test/corpus");
    let fixtures_root = path.join("test/fixtures");

    write_staged_bundle_file(&path.join("grammar.js"), fol_tree_sitter_grammar())?;
    for snapshot in fol_tree_sitter_query_snapshots() {
        write_staged_bundle_file(
            &queries_root.join(format!("{}.scm", snapshot.name)),
            snapshot.query,
        )?;
    }
    write_staged_bundle_file(&path.join("package.json"), TREE_SITTER_PACKAGE_JSON)?;
    write_staged_bundle_file(&path.join("tree-sitter.json"), fol_tree_sitter_config())?;
    for case in fol_tree_sitter_corpus() {
        write_staged_bundle_file(&corpus_root.join(format!("{}.txt", case.name)), case.source)?;
    }
    write_staged_bundle_file(
        &fixtures_root.join("showcase.fol"),
        fol_tree_sitter_showcase_fixture(),
    )
}

fn read_generated_tree_bundle_manifest(path: &Path) -> EditorResult<BTreeSet<String>> {
    let manifest = path.join(TREE_SITTER_BUNDLE_MANIFEST);
    validate_bundle_file_target(&manifest)?;
    let contents = match std::fs::read_to_string(&manifest) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to read '{}': {error}", manifest.display()),
            ));
        }
    };

    let mut files = BTreeSet::new();
    for relative in contents.lines().filter(|line| !line.is_empty()) {
        let relative_path = Path::new(relative);
        let is_safe = relative != TREE_SITTER_BUNDLE_MANIFEST
            && relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !is_safe {
            return Err(EditorError::new(
                EditorErrorKind::InvalidInput,
                format!(
                    "tree bundle manifest '{}' contains unsafe path '{relative}'",
                    manifest.display()
                ),
            )
            .with_note("refusing to remove files outside the generated bundle inventory"));
        }
        files.insert(relative.to_string());
    }
    Ok(files)
}

fn commit_staged_tree_bundle<BeforeCommit>(
    destination: &Path,
    staging: &mut TreeBundleStaging,
    updated_existing_root: bool,
    previous_files: &BTreeSet<String>,
    generated_files: &BTreeSet<String>,
    before_commit: &mut BeforeCommit,
) -> EditorResult<()>
where
    BeforeCommit: FnMut(&Path) -> EditorResult<()>,
{
    validate_tree_bundle_targets(destination, previous_files, generated_files)?;

    if !updated_existing_root {
        before_commit(destination)?;
        let mut created_directories = Vec::new();
        let parent = destination
            .parent()
            .expect("destination parent was validated");
        if let Err(error) = create_directory_path(parent, &mut created_directories) {
            remove_created_directories(&created_directories);
            return Err(error);
        }
        if let Err(error) = ensure_no_symlink_components(destination) {
            remove_created_directories(&created_directories);
            return Err(error);
        }
        match std::fs::rename(staging.path(), destination) {
            Ok(()) => {
                staging.keep();
                return Ok(());
            }
            Err(error) => {
                remove_created_directories(&created_directories);
                return Err(EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to install staged tree bundle at '{}': {error}",
                        destination.display()
                    ),
                ));
            }
        }
    }

    install_staged_tree_bundle_into_existing(
        destination,
        staging.path(),
        previous_files,
        generated_files,
        before_commit,
    )
}

fn install_staged_tree_bundle_into_existing<BeforeCommit>(
    destination: &Path,
    staging: &Path,
    previous_files: &BTreeSet<String>,
    generated_files: &BTreeSet<String>,
    before_commit: &mut BeforeCommit,
) -> EditorResult<()>
where
    BeforeCommit: FnMut(&Path) -> EditorResult<()>,
{
    let staged_files = generated_files
        .iter()
        .map(|relative| {
            let staged = staging.join(relative);
            validate_bundle_file_target(&staged)?;
            let contents = std::fs::read(&staged).map_err(|error| {
                EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to read staged asset '{}': {error}",
                        staged.display()
                    ),
                )
            })?;
            Ok((relative.clone(), contents))
        })
        .collect::<EditorResult<Vec<_>>>()?;
    let staged_manifest =
        std::fs::read(staging.join(TREE_SITTER_BUNDLE_MANIFEST)).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to read staged tree bundle manifest: {error}"),
            )
        })?;

    let mut affected_files = previous_files
        .union(generated_files)
        .cloned()
        .collect::<BTreeSet<_>>();
    affected_files.insert(TREE_SITTER_BUNDLE_MANIFEST.to_string());
    let snapshots = affected_files
        .iter()
        .map(|relative| capture_bundle_file_snapshot(&destination.join(relative)))
        .collect::<EditorResult<Vec<_>>>()?;

    let mut created_directories = Vec::new();
    let commit_result = (|| {
        for (relative, contents) in &staged_files {
            let target = destination.join(relative);
            before_commit(&target)?;
            replace_bundle_file(&target, contents, &mut created_directories)?;
        }

        for relative in previous_files.difference(generated_files) {
            let retired = destination.join(relative);
            before_commit(&retired)?;
            validate_bundle_file_target(&retired)?;
            match std::fs::remove_file(&retired) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(EditorError::new(
                        EditorErrorKind::Internal,
                        format!(
                            "failed to remove retired generated asset '{}': {error}",
                            retired.display()
                        ),
                    ));
                }
            }
        }

        let manifest = destination.join(TREE_SITTER_BUNDLE_MANIFEST);
        before_commit(&manifest)?;
        replace_bundle_file(&manifest, &staged_manifest, &mut created_directories)
    })();

    if let Err(mut error) = commit_result {
        match restore_bundle_file_snapshots(&snapshots, &created_directories) {
            Ok(()) => error
                .notes
                .push("the destination bundle was restored after commit failed".to_string()),
            Err(rollback_error) => error.notes.push(format!(
                "tree bundle rollback also failed: {rollback_error}"
            )),
        }
        return Err(error);
    }

    Ok(())
}

fn capture_bundle_file_snapshot(path: &Path) -> EditorResult<BundleFileSnapshot> {
    validate_bundle_file_target(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            let contents = std::fs::read(path).map_err(|error| {
                EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to snapshot bundle asset '{}': {error}",
                        path.display()
                    ),
                )
            })?;
            Ok(BundleFileSnapshot {
                path: path.to_path_buf(),
                contents: Some(contents),
                permissions: Some(metadata.permissions()),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BundleFileSnapshot {
            path: path.to_path_buf(),
            contents: None,
            permissions: None,
        }),
        Err(error) => Err(EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to snapshot bundle asset '{}': {error}",
                path.display()
            ),
        )),
    }
}

fn replace_bundle_file(
    path: &Path,
    contents: &[u8],
    created_directories: &mut Vec<PathBuf>,
) -> EditorResult<()> {
    ensure_no_symlink_components(path)?;
    let parent = path.parent().ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::InvalidInput,
            format!("tree bundle file '{}' has no parent", path.display()),
        )
    })?;
    create_directory_path(parent, created_directories)?;
    ensure_no_symlink_components(path)?;

    let mut temporary = None;
    for _ in 0..128 {
        let counter = TREE_BUNDLE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".fol-tree-install-{}-{counter}",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(contents).map_err(|error| {
                    let _ = std::fs::remove_file(&candidate);
                    EditorError::new(
                        EditorErrorKind::Internal,
                        format!(
                            "failed to write staged replacement for '{}': {error}",
                            path.display()
                        ),
                    )
                })?;
                temporary = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to create staged replacement for '{}': {error}",
                        path.display()
                    ),
                ));
            }
        }
    }
    let temporary = temporary.ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to reserve a staged replacement for '{}'",
                path.display()
            ),
        )
    })?;

    #[cfg(windows)]
    if path.exists() {
        if let Err(error) = std::fs::remove_file(path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to replace bundle asset '{}': {error}",
                    path.display()
                ),
            ));
        }
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to replace bundle asset '{}': {error}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn create_directory_path(path: &Path, created: &mut Vec<PathBuf>) -> EditorResult<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!(
                        "refusing to create tree bundle directory through symlink '{}'",
                        current.display()
                    ),
                ));
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!(
                        "tree bundle directory component '{}' is not a directory",
                        current.display()
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|error| {
                    EditorError::new(
                        EditorErrorKind::Internal,
                        format!(
                            "failed to create tree bundle directory '{}': {error}",
                            current.display()
                        ),
                    )
                })?;
                created.push(current.clone());
            }
            Err(error) => {
                return Err(EditorError::new(
                    EditorErrorKind::Internal,
                    format!(
                        "failed to inspect tree bundle directory '{}': {error}",
                        current.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn restore_bundle_file_snapshots(
    snapshots: &[BundleFileSnapshot],
    created_directories: &[PathBuf],
) -> Result<(), String> {
    let mut rollback_errors = Vec::new();
    let mut rollback_directories = Vec::new();

    for snapshot in snapshots.iter().rev() {
        match &snapshot.contents {
            Some(contents) => {
                if let Err(error) =
                    replace_bundle_file(&snapshot.path, contents, &mut rollback_directories)
                {
                    rollback_errors.push(error.to_string());
                    continue;
                }
                if let Some(permissions) = &snapshot.permissions {
                    if let Err(error) =
                        std::fs::set_permissions(&snapshot.path, permissions.clone())
                    {
                        rollback_errors.push(format!(
                            "failed to restore permissions for '{}': {error}",
                            snapshot.path.display()
                        ));
                    }
                }
            }
            None => match std::fs::symlink_metadata(&snapshot.path) {
                Ok(metadata) if metadata.is_file() || metadata.file_type().is_symlink() => {
                    if let Err(error) = std::fs::remove_file(&snapshot.path) {
                        rollback_errors.push(format!(
                            "failed to remove newly installed asset '{}': {error}",
                            snapshot.path.display()
                        ));
                    }
                }
                Ok(_) => rollback_errors.push(format!(
                    "newly installed asset '{}' is no longer a file",
                    snapshot.path.display()
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => rollback_errors.push(format!(
                    "failed to inspect newly installed asset '{}': {error}",
                    snapshot.path.display()
                )),
            },
        }
    }

    remove_created_directories_collect(created_directories, &mut rollback_errors);
    remove_created_directories_collect(&rollback_directories, &mut rollback_errors);
    if rollback_errors.is_empty() {
        Ok(())
    } else {
        Err(rollback_errors.join("; "))
    }
}

fn remove_created_directories(created_directories: &[PathBuf]) {
    for directory in created_directories.iter().rev() {
        let _ = std::fs::remove_dir(directory);
    }
}

fn remove_created_directories_collect(created_directories: &[PathBuf], errors: &mut Vec<String>) {
    for directory in created_directories.iter().rev() {
        match std::fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => errors.push(format!(
                "failed to remove transaction directory '{}': {error}",
                directory.display()
            )),
        }
    }
}

fn write_generated_tree_bundle_manifest(
    path: &Path,
    generated_files: &BTreeSet<String>,
) -> EditorResult<()> {
    let mut contents = generated_files
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    contents.push('\n');
    write_staged_bundle_file(&path.join(TREE_SITTER_BUNDLE_MANIFEST), &contents)
}

fn write_staged_bundle_file(path: &Path, contents: &str) -> EditorResult<()> {
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
        compiler_dot_intrinsic_names_csv, compiler_import_kinds_csv, editor_format_file,
        editor_highlight_file, editor_lsp_entrypoint, editor_parse_file, editor_references_file,
        editor_rename_file, editor_semantic_tokens_file, editor_symbols_file,
        editor_tree_generate_bundle, editor_tree_generate_bundle_with, fol_tree_sitter_corpus,
        fol_tree_sitter_showcase_fixture, generated_tree_bundle_files, semantic_token_legend_csv,
        TREE_SITTER_BUNDLE_MANIFEST, TREE_SITTER_CLI_GENERATED_FILES,
    };
    use crate::{
        fol_tree_sitter_grammar, fol_tree_sitter_query_snapshots, EditorError, EditorErrorKind,
        EditorResult, LspPosition,
    };
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    mod v3_example_inventory {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/v3_example_inventory.rs"
        ));
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should resolve")
    }

    fn tree_bundle_test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ))
    }

    fn seed_staged_parser_assets(staging: &Path) -> EditorResult<()> {
        let checked_in = repo_root().join("lang/tooling/fol-editor/tree-sitter");
        for relative in TREE_SITTER_CLI_GENERATED_FILES {
            let destination = staging.join(relative);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(checked_in.join(relative), destination).unwrap();
        }
        Ok(())
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
        std::fs::create_dir_all(root.join(".git")).unwrap();
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

    fn numeric_detail(summary: &super::EditorCommandSummary, key: &str) -> usize {
        summary
            .details
            .iter()
            .find_map(|detail| detail.strip_prefix(key))
            .unwrap_or_else(|| panic!("missing command detail '{key}'"))
            .parse::<usize>()
            .unwrap_or_else(|error| panic!("invalid numeric command detail '{key}': {error}"))
    }

    fn has_capture_text(summary: &super::EditorCommandSummary, text: &str) -> bool {
        let suffix = format!(":{text}");
        summary
            .details
            .iter()
            .any(|detail| detail.starts_with("capture=") && detail.ends_with(&suffix))
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
        assert!(summary.summary.contains("folding"));
        assert!(summary.summary.contains("selection"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "transport=stdio"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail
                == "features=diagnostics,hover,definition,typeDefinition,implementation,documentHighlight,formatting,codeAction,signatureHelp,references,prepareRename,rename,semanticTokens,documentSymbols,workspaceSymbols,completion,inlayHint,foldingRange,selectionRange"));
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
        assert!(parse
            .details
            .iter()
            .any(|detail| detail == "parse_status=ok"));
        assert!(parse.details.iter().any(|detail| detail == "error_count=0"));
        assert!(parse
            .details
            .iter()
            .any(|detail| detail.starts_with("syntax_tree=(source_file")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail.contains("capture_count=")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail.starts_with("capture=")));
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
            .any(|detail| detail.starts_with("symbol_count=")));
        assert!(symbols
            .details
            .iter()
            .any(|detail| detail.starts_with("symbol=")));
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
    fn hosted_v3_semantic_commands_resolve_declared_bundled_std() {
        let path = repo_root().join("examples/proc_spawn_m1/src/main.fol");
        let semantic_tokens = editor_semantic_tokens_file(&path).unwrap();
        let references = editor_references_file(
            &path,
            LspPosition {
                line: 8,
                character: 25,
            },
            true,
        )
        .unwrap();
        let rename = editor_rename_file(
            &path,
            LspPosition {
                line: 8,
                character: 25,
            },
            "task",
        )
        .unwrap();

        assert!(numeric_detail(&semantic_tokens, "token_count=") > 0);
        assert_eq!(numeric_detail(&references, "reference_count="), 2);
        assert_eq!(numeric_detail(&rename, "edit_count="), 2);
    }

    #[test]
    fn semantic_commands_reject_documents_with_did_open_diagnostics() {
        let (path, position) = rename_probe_package("diagnostic_rejection");
        std::fs::write(&path, "fun[] main(: int = {\n    return missing;\n};\n").unwrap();

        for error in [
            editor_semantic_tokens_file(&path).unwrap_err(),
            editor_references_file(&path, position, true).unwrap_err(),
            editor_rename_file(&path, position, "renamed").unwrap_err(),
        ] {
            assert_eq!(error.kind, EditorErrorKind::InvalidInput);
            assert!(error.message.contains("semantic analysis reported"));
            assert!(error.message.contains("[P1001]"));
        }

        std::fs::remove_dir_all(path.parent().and_then(Path::parent).unwrap()).ok();
    }

    #[test]
    fn real_fixtures_keep_editor_command_summaries_stable() {
        let showcase = repo_root().join("test/apps/showcases/full_v1_showcase/app/main.fol");
        let package = repo_root().join("test/fixtures/logtiny/src/log.fol");
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
        assert_eq!(parse.details[0], format!("path={}", showcase.display()));
        assert_eq!(parse.details[1], format!("lines={showcase_lines}"));
        assert_eq!(parse.details[2], format!("bytes={showcase_bytes}"));
        assert!(parse
            .details
            .iter()
            .any(|detail| detail == "parse_status=ok"));
        assert!(parse.details.iter().any(|detail| detail == "error_count=0"));
        assert!(parse
            .details
            .iter()
            .any(|detail| detail.starts_with("syntax_tree=(source_file")));
        assert_eq!(highlight.command, "highlight");
        assert_eq!(highlight.details[0], format!("path={}", showcase.display()));
        assert_eq!(highlight.details[1], format!("lines={showcase_lines}"));
        assert_eq!(
            highlight.details[2],
            format!(
                "query_bytes={}",
                crate::fol_tree_sitter_highlights_query().len()
            )
        );
        let capture_count = highlight.details[3]
            .strip_prefix("capture_count=")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(capture_count > 0);
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail == "parse_status=ok"));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail.starts_with("capture=")));
        assert!(highlight
            .details
            .iter()
            .any(|detail| detail == &format!("import_kinds={}", compiler_import_kinds_csv())));
        assert!(highlight.details.iter().any(|detail| {
            detail == &format!("intrinsic_names={}", compiler_dot_intrinsic_names_csv())
        }));
        let package_text = std::fs::read_to_string(&package).unwrap();
        let package_lines = package_text.lines().count();

        assert_eq!(symbols.command, "symbols");
        assert_eq!(symbols.details[0], format!("path={}", package.display()));
        assert_eq!(symbols.details[1], format!("lines={package_lines}"));
        let symbol_count = symbols
            .details
            .iter()
            .find_map(|detail| detail.strip_prefix("symbol_count="))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(symbol_count > 0);
        assert!(symbols
            .details
            .iter()
            .any(|detail| detail == "parse_status=ok"));
        assert!(symbols
            .details
            .iter()
            .any(|detail| detail.starts_with("symbol=")));
        assert_eq!(semantic_tokens.command, "semantic-tokens");
        assert_eq!(
            semantic_tokens.details[0],
            format!("path={}", showcase.display())
        );
        assert_eq!(
            semantic_tokens.details[1],
            format!("lines={showcase_lines}")
        );
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
    fn v3_tool_commands_execute_every_canonical_positive_example() {
        let root = repo_root();
        v3_example_inventory::assert_checked_in_example_directories(&root);

        for example in v3_example_inventory::positive_example_paths() {
            let relative = format!("{example}/src/main.fol");
            let path = root.join(&relative);
            let parse = editor_parse_file(&path)
                .unwrap_or_else(|error| panic!("parse command failed for {relative}: {error}"));
            let highlight = editor_highlight_file(&path)
                .unwrap_or_else(|error| panic!("highlight command failed for {relative}: {error}"));
            let symbols = editor_symbols_file(&path)
                .unwrap_or_else(|error| panic!("symbols command failed for {relative}: {error}"));

            assert!(
                parse
                    .details
                    .iter()
                    .any(|detail| detail == "parse_status=ok"),
                "canonical V3 example did not parse cleanly: {relative}\n{:#?}",
                parse.details
            );
            assert!(
                parse
                    .details
                    .iter()
                    .any(|detail| detail.starts_with("syntax_tree=(source_file")),
                "parse command did not return the real syntax tree for {relative}"
            );
            assert_eq!(numeric_detail(&parse, "error_count="), 0, "{relative}");
            assert_eq!(numeric_detail(&parse, "missing_count="), 0, "{relative}");

            assert!(
                numeric_detail(&highlight, "capture_count=") > 0,
                "highlight query returned no captures for {relative}"
            );
            assert!(
                highlight
                    .details
                    .iter()
                    .any(|detail| detail.starts_with("capture=")),
                "highlight command returned only query metadata for {relative}"
            );
            assert!(
                numeric_detail(&symbols, "symbol_count=") > 0,
                "symbol query returned no symbols for {relative}"
            );
            assert!(
                symbols
                    .details
                    .iter()
                    .any(|detail| detail.starts_with("symbol=symbol.function@")
                        && detail.ends_with(":main")),
                "symbol query did not return the main routine for {relative}\n{:#?}",
                symbols.details
            );
        }
    }

    #[test]
    fn removed_select_call_form_reports_a_real_tree_sitter_error() {
        let path = repo_root().join("examples/fail_proc_select_old_form_m3/src/main.fol");
        let parse = editor_parse_file(&path).expect("error-tolerant parse command should succeed");

        assert!(parse
            .details
            .iter()
            .any(|detail| detail == "parse_status=ERROR"));
        assert!(numeric_detail(&parse, "error_count=") > 0);
        assert!(parse
            .details
            .iter()
            .any(|detail| detail.starts_with("error=ERROR@")));
        assert!(parse
            .details
            .iter()
            .any(|detail| detail.contains("syntax_tree=") && detail.contains("(ERROR)")));
    }

    #[test]
    fn public_tree_commands_accept_compiler_comment_and_raw_quote_boundaries() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_lexical_boundaries_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("lexical.fol");
        std::fs::write(
            &path,
            concat!(
                "/* block comment with [>] { }\nsecond line */\n",
                "` backtick comment with dfr {\n",
                "edf { counter[mux] } } `\n",
                "`[doc] documentation with #view\n",
                "and !view`\n",
                "fun[] main(): str = {\n",
                "    var raw: str = 'raw [>] text\nwith braces { }';\n",
                "    var empty: str = '';\n",
                "    var character: chr = 'z';\n",
                "    return raw;\n",
                "};\n",
            ),
        )
        .unwrap();

        let parse = editor_parse_file(&path).expect("public parse command should succeed");
        assert!(parse
            .details
            .iter()
            .any(|detail| detail == "parse_status=ok"));
        assert_eq!(numeric_detail(&parse, "error_count="), 0);
        assert_eq!(numeric_detail(&parse, "missing_count="), 0);
        assert!(parse.details.iter().any(|detail| {
            detail.starts_with("syntax_tree=")
                && detail.contains("(doc_comment)")
                && detail.contains("(raw_string_literal)")
                && detail.contains("(char_literal)")
        }));

        let highlight =
            editor_highlight_file(&path).expect("public highlight command should succeed");
        for capture in ["capture=comment@", "capture=comment.documentation@"] {
            assert!(
                highlight
                    .details
                    .iter()
                    .any(|detail| detail.starts_with(capture)),
                "public highlight command lost '{capture}': {:#?}",
                highlight.details
            );
        }
        assert!(has_capture_text(&highlight, "''"));
        assert!(has_capture_text(&highlight, "'z'"));
        assert!(highlight.details.iter().any(|detail| {
            detail.starts_with("capture=string@")
                && detail.ends_with(":'raw [>] text\\nwith braces { }'")
        }));

        std::fs::remove_dir_all(root).ok();
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
        assert!(root.join("test/fixtures/showcase.fol").is_file());
        assert!(summary
            .details
            .iter()
            .any(|detail| detail.contains("query_files=3")));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "corpus_files=11"));
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "fixture_files=1"));
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
        for case in fol_tree_sitter_corpus() {
            assert_eq!(
                std::fs::read_to_string(
                    root.join("test/corpus").join(format!("{}.txt", case.name))
                )
                .unwrap(),
                case.source
            );
        }
        assert_eq!(
            std::fs::read_to_string(root.join("test/fixtures/showcase.fol")).unwrap(),
            fol_tree_sitter_showcase_fixture()
        );

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
    fn tree_generate_bundle_cleans_only_manifest_owned_files() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_stale_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let unmanaged_file = root.join("editor-notes.txt");
        let unmanaged_query = root.join("queries/fol/custom.scm");
        let retired_query = root.join("queries/fol/retired.scm");
        std::fs::create_dir_all(unmanaged_query.parent().unwrap()).unwrap();
        std::fs::write(&unmanaged_file, "keep this").unwrap();
        std::fs::write(&unmanaged_query, "(identifier) @custom").unwrap();
        std::fs::write(&retired_query, "(identifier) @retired").unwrap();
        std::fs::write(
            root.join(TREE_SITTER_BUNDLE_MANIFEST),
            "queries/fol/retired.scm\n",
        )
        .unwrap();
        std::fs::write(root.join("grammar.js"), "stale grammar").unwrap();

        let summary = editor_tree_generate_bundle(&root).unwrap();

        assert_eq!(
            std::fs::read_to_string(&unmanaged_file).unwrap(),
            "keep this"
        );
        assert_eq!(
            std::fs::read_to_string(&unmanaged_query).unwrap(),
            "(identifier) @custom"
        );
        assert!(!retired_query.exists());
        assert_eq!(
            std::fs::read_to_string(root.join("grammar.js")).unwrap(),
            fol_tree_sitter_grammar()
        );
        let manifest = std::fs::read_to_string(root.join(TREE_SITTER_BUNDLE_MANIFEST)).unwrap();
        assert_eq!(
            manifest.lines().collect::<BTreeSet<_>>(),
            generated_tree_bundle_files()
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        );
        assert!(summary
            .details
            .iter()
            .any(|detail| detail == "updated_existing_root=true"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_rejects_unsafe_manifest_paths() {
        let root = std::env::temp_dir().join(format!(
            "fol_editor_tree_bundle_unsafe_manifest_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let outside = root.with_extension("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, "keep this").unwrap();
        std::fs::write(
            root.join(TREE_SITTER_BUNDLE_MANIFEST),
            format!("../{}\n", outside.file_name().unwrap().to_string_lossy()),
        )
        .unwrap();

        let error = editor_tree_generate_bundle(&root).unwrap_err();

        assert_eq!(error.kind, EditorErrorKind::InvalidInput);
        assert!(error.message.contains("unsafe path"));
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep this");
        assert!(!root.join("grammar.js").exists());

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_file(outside).ok();
    }

    #[test]
    fn tree_generate_bundle_generator_failure_leaves_destination_unchanged() {
        let root = tree_bundle_test_root("failed_generator");
        let retired = root.join("legacy/retired.scm");
        std::fs::create_dir_all(retired.parent().unwrap()).unwrap();
        std::fs::write(root.join("grammar.js"), "old grammar").unwrap();
        std::fs::write(&retired, "old retired query").unwrap();
        std::fs::write(root.join("editor-notes.txt"), "user owned").unwrap();
        std::fs::write(
            root.join(TREE_SITTER_BUNDLE_MANIFEST),
            "legacy/retired.scm\n",
        )
        .unwrap();

        let error = editor_tree_generate_bundle_with(
            &root,
            |_| {
                Err(EditorError::new(
                    EditorErrorKind::Internal,
                    "forced parser generation failure",
                ))
            },
            |_| Ok(()),
        )
        .unwrap_err();

        assert_eq!(error.message, "forced parser generation failure");
        assert_eq!(
            std::fs::read_to_string(root.join("grammar.js")).unwrap(),
            "old grammar"
        );
        assert_eq!(
            std::fs::read_to_string(&retired).unwrap(),
            "old retired query"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("editor-notes.txt")).unwrap(),
            "user owned"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(TREE_SITTER_BUNDLE_MANIFEST)).unwrap(),
            "legacy/retired.scm\n"
        );
        assert!(!root.join("package.json").exists());
        assert!(!root.join("queries").exists());
        assert!(!root.join("src").exists());
        assert!(!root.join("test").exists());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_missing_staged_parser_leaves_destination_unchanged() {
        let root = tree_bundle_test_root("missing_staged_parser");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("grammar.js"), "old grammar").unwrap();
        std::fs::write(root.join(TREE_SITTER_BUNDLE_MANIFEST), "grammar.js\n").unwrap();

        let error = editor_tree_generate_bundle_with(&root, |_| Ok(()), |_| Ok(())).unwrap_err();

        assert!(error.message.contains("did not produce"));
        assert!(error
            .notes
            .iter()
            .any(|note| note == "the destination bundle was not changed"));
        assert_eq!(
            std::fs::read_to_string(root.join("grammar.js")).unwrap(),
            "old grammar"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(TREE_SITTER_BUNDLE_MANIFEST)).unwrap(),
            "grammar.js\n"
        );
        assert!(!root.join("package.json").exists());
        assert!(!root.join("queries").exists());
        assert!(!root.join("src").exists());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_commit_failure_rolls_back_every_managed_asset() {
        let root = tree_bundle_test_root("failed_commit");
        let retired = root.join("legacy/retired.scm");
        std::fs::create_dir_all(retired.parent().unwrap()).unwrap();
        std::fs::write(root.join("grammar.js"), "old grammar").unwrap();
        std::fs::write(&retired, "old retired query").unwrap();
        std::fs::write(root.join("editor-notes.txt"), "user owned").unwrap();
        std::fs::write(
            root.join(TREE_SITTER_BUNDLE_MANIFEST),
            "legacy/retired.scm\n",
        )
        .unwrap();

        let error = editor_tree_generate_bundle_with(&root, seed_staged_parser_assets, |target| {
            if target.ends_with(TREE_SITTER_BUNDLE_MANIFEST) {
                Err(EditorError::new(
                    EditorErrorKind::Internal,
                    "forced manifest commit failure",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert_eq!(error.message, "forced manifest commit failure");
        assert!(error
            .notes
            .iter()
            .any(|note| note.contains("destination bundle was restored")));
        assert_eq!(
            std::fs::read_to_string(root.join("grammar.js")).unwrap(),
            "old grammar"
        );
        assert_eq!(
            std::fs::read_to_string(&retired).unwrap(),
            "old retired query"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("editor-notes.txt")).unwrap(),
            "user owned"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(TREE_SITTER_BUNDLE_MANIFEST)).unwrap(),
            "legacy/retired.scm\n"
        );
        assert!(!root.join("package.json").exists());
        assert!(!root.join("queries").exists());
        assert!(!root.join("src").exists());
        assert!(!root.join("test").exists());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tree_generate_bundle_failure_does_not_create_missing_destination_parents() {
        let base = tree_bundle_test_root("missing_parent");
        let root = base.join("missing/target");
        std::fs::create_dir_all(&base).unwrap();

        editor_tree_generate_bundle_with(
            &root,
            |_| {
                Err(EditorError::new(
                    EditorErrorKind::Internal,
                    "forced parser generation failure",
                ))
            },
            |_| Ok(()),
        )
        .unwrap_err();

        assert!(!base.join("missing").exists());
        std::fs::remove_dir_all(base).ok();
    }

    #[cfg(unix)]
    #[test]
    fn tree_generate_bundle_rejects_root_and_managed_file_symlinks() {
        use std::os::unix::fs::symlink;

        let base = tree_bundle_test_root("symlink_roots");
        let actual_root = base.join("actual");
        let linked_root = base.join("linked");
        let outside = base.join("outside.txt");
        std::fs::create_dir_all(&actual_root).unwrap();
        std::fs::write(&outside, "outside sentinel").unwrap();
        symlink(&actual_root, &linked_root).unwrap();

        let root_error = editor_tree_generate_bundle(&linked_root).unwrap_err();
        assert_eq!(root_error.kind, EditorErrorKind::InvalidInput);
        assert!(root_error.message.contains("symlink component"));

        symlink(&outside, actual_root.join("grammar.js")).unwrap();
        let file_error = editor_tree_generate_bundle(&actual_root).unwrap_err();
        assert_eq!(file_error.kind, EditorErrorKind::InvalidInput);
        assert!(file_error.message.contains("symlink component"));
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "outside sentinel"
        );

        std::fs::remove_file(linked_root).ok();
        std::fs::remove_dir_all(base).ok();
    }

    #[cfg(unix)]
    #[test]
    fn tree_generate_bundle_rejects_intermediate_and_retired_symlink_paths() {
        use std::os::unix::fs::symlink;

        let base = tree_bundle_test_root("symlink_components");
        let outside = base.join("outside");
        let generated_root = base.join("generated-root");
        let retired_root = base.join("retired-root");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&generated_root).unwrap();
        std::fs::create_dir_all(&retired_root).unwrap();

        symlink(&outside, generated_root.join("queries")).unwrap();
        let generated_error = editor_tree_generate_bundle(&generated_root).unwrap_err();
        assert_eq!(generated_error.kind, EditorErrorKind::InvalidInput);
        assert!(generated_error.message.contains("symlink component"));
        assert!(!outside.join("fol/highlights.scm").exists());

        std::fs::write(outside.join("retired.scm"), "outside retired sentinel").unwrap();
        symlink(&outside, retired_root.join("legacy")).unwrap();
        std::fs::write(
            retired_root.join(TREE_SITTER_BUNDLE_MANIFEST),
            "legacy/retired.scm\n",
        )
        .unwrap();
        let retired_error = editor_tree_generate_bundle(&retired_root).unwrap_err();
        assert_eq!(retired_error.kind, EditorErrorKind::InvalidInput);
        assert!(retired_error.message.contains("symlink component"));
        assert_eq!(
            std::fs::read_to_string(outside.join("retired.scm")).unwrap(),
            "outside retired sentinel"
        );

        std::fs::remove_dir_all(base).ok();
    }
}
