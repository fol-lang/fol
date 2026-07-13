use super::*;
use fol_editor::{
    editor_highlight_file, editor_tree_generate_bundle, fol_tree_sitter_highlights_query,
    fol_tree_sitter_locals_query, fol_tree_sitter_symbols_query, EditorConfig, EditorDocumentUri,
    EditorLspServer, JsonRpcId, JsonRpcNotification, JsonRpcRequest, LspCompletionContext,
    LspCompletionList, LspCompletionParams, LspDefinitionParams, LspHover, LspHoverParams,
    LspLocation, LspPosition, LspTextDocumentIdentifier,
};

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("should create destination directory");
    for entry in std::fs::read_dir(src).expect("should read source directory") {
        let entry = entry.expect("should read source entry");
        let file_type = entry.file_type().expect("should read source file type");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("should copy source file");
        }
    }
}

fn copied_example_root(example_path: &str) -> std::path::PathBuf {
    let source = repo_root().join(example_path);
    let temp_root = unique_temp_root(&format!("editor_sync_{}", example_path.replace('/', "_")));
    let target = temp_root.join("workspace");
    copy_dir_all(&source, &target);
    std::fs::create_dir_all(target.join(".git"))
        .expect("copied editor example workspace marker should be creatable");
    let build_source = std::fs::read_to_string(target.join("build.fol")).unwrap_or_default();
    if build_source.contains("source = \"internal\"")
        && build_source.contains("target = \"standard\"")
    {
        let bundled_std_root =
            fol_package::available_bundled_std_root().expect("bundled std root should exist");
        let std_alias_root = target.join(".fol/pkg/std");
        copy_dir_all(&bundled_std_root, &std_alias_root);
        std::fs::write(
            target.join("fol.work.yaml"),
            "package_store_root: .fol/pkg\n",
        )
        .expect("should write workspace package-store override");
    }
    target
}

fn open_example_server(
    example_path: &str,
    source: &str,
) -> (std::path::PathBuf, String, EditorLspServer) {
    let root = copied_example_root(example_path);
    let source_path = root.join("src/main.fol");
    std::fs::write(&source_path, source).expect("should write example source");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_lsp_document(&mut server, uri.clone(), source);
    (root, uri, server)
}

#[test]
fn test_editor_sync_suite_exports_compiler_backed_highlight_metadata() {
    let path = repo_root().join("examples/std_echo_min/src/main.fol");
    let summary = editor_highlight_file(&path).expect("highlight summary should build");

    let expected_import_kinds = format!(
        "import_kinds={}",
        fol_typecheck::editor_source_kind_names().join(",")
    );
    let mut intrinsic_names = fol_typecheck::editor_implemented_intrinsics()
        .into_iter()
        .filter(|entry| entry.surface == fol_intrinsics::IntrinsicSurface::DotRootCall)
        .map(|entry| entry.name.to_string())
        .collect::<Vec<_>>();
    intrinsic_names.sort();
    let expected_intrinsics = format!("intrinsic_names={}", intrinsic_names.join(","));

    assert!(
        summary
            .details
            .iter()
            .any(|detail| detail == &expected_import_kinds),
        "highlight summary should surface compiler import kinds: {:#?}",
        summary.details
    );
    assert!(
        summary
            .details
            .iter()
            .any(|detail| detail == &expected_intrinsics),
        "highlight summary should surface compiler intrinsic names: {:#?}",
        summary.details
    );
}

#[test]
fn test_editor_sync_suite_bundle_writes_canonical_query_assets() {
    let root = unique_temp_root("editor_sync_bundle");
    let bundle = root.join("bundle");
    editor_tree_generate_bundle(&bundle).expect("bundle generation should succeed");

    let generated_highlights =
        std::fs::read_to_string(bundle.join("queries/fol/highlights.scm")).unwrap();
    let generated_locals = std::fs::read_to_string(bundle.join("queries/fol/locals.scm")).unwrap();
    let generated_symbols =
        std::fs::read_to_string(bundle.join("queries/fol/symbols.scm")).unwrap();

    assert_eq!(generated_highlights, fol_tree_sitter_highlights_query());
    assert_eq!(generated_locals, fol_tree_sitter_locals_query());
    assert_eq!(generated_symbols, fol_tree_sitter_symbols_query());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn test_editor_sync_suite_lsp_keeps_model_boundary_diagnostics() {
    let source = "fun[] main(): str = {\n    return \"bad\";\n};\n";
    let root = copied_example_root("examples/core_dfr");
    let source_path = root.join("src/main.fol");
    std::fs::write(&source_path, source).expect("should write example source");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = server
        .handle_notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: source.to_string(),
                    },
                })
                .unwrap(),
            ),
        })
        .expect("didOpen should return diagnostics")
        .into_iter()
        .flat_map(|published| published.diagnostics.into_iter())
        .map(|item| item.message)
        .collect::<Vec<_>>();

    assert!(
        diagnostics.iter().any(|message| message
            .contains("str requires heap support and is unavailable in 'fol_model = core'")),
        "core example should keep model-aware diagnostics: {diagnostics:?}"
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn test_editor_sync_suite_lsp_completion_respects_model_examples() {
    let core_source = "fun[] main(): int = {\n    return .;\n};\n";
    let (core_root, core_uri, mut core_server) =
        open_example_server("examples/core_dfr", core_source);
    let core_response = core_server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3401),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: core_uri.clone(),
                    },
                    position: LspPosition {
                        line: 1,
                        character: 12,
                    },
                    context: Some(LspCompletionContext {
                        trigger_kind: Some(2),
                        trigger_character: Some(".".to_string()),
                    }),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let core_labels = serde_json::from_value::<LspCompletionList>(core_response.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(!core_labels.iter().any(|label| label == "echo"));

    let std_source = "fun[] main(): int = {\n    return .;\n};\n";
    let (std_root, std_uri, mut std_server) =
        open_example_server("examples/std_echo_min", std_source);
    let std_response = std_server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3402),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: std_uri.clone(),
                    },
                    position: LspPosition {
                        line: 1,
                        character: 12,
                    },
                    context: Some(LspCompletionContext {
                        trigger_kind: Some(2),
                        trigger_character: Some(".".to_string()),
                    }),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let std_labels = serde_json::from_value::<LspCompletionList>(std_response.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(std_labels.iter().any(|label| label == "echo"));

    std::fs::remove_dir_all(core_root).ok();
    std::fs::remove_dir_all(std_root).ok();
}

#[test]
fn test_editor_sync_suite_lsp_handles_bundled_std_definition_requests_without_override() {
    let source = "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::math::answer();\n};\n";
    let root = copied_example_root("examples/std_bundled_fmt");
    let source_path = root.join("src/main.fol");
    std::fs::write(&source_path, source).expect("should write example source");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_lsp_document(&mut server, uri.clone(), source);
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3403),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 22,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _definition: Option<LspLocation> =
        serde_json::from_value(response.result.unwrap()).unwrap();

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn test_editor_sync_suite_lsp_handles_bundled_std_io_hover_and_definition_without_override() {
    let source =
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::io::echo_int(7);\n};\n";
    let root = copied_example_root("examples/std_bundled_io");
    let source_path = root.join("src/main.fol");
    std::fs::write(&source_path, source).expect("should write example source");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_lsp_document(&mut server, uri.clone(), source);

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3404),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 16,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3405),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 16,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();

    std::fs::remove_dir_all(root).ok();
}
