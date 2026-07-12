use super::helpers::{open_document, sample_package_root};
use super::super::{
    EditorCompletionItem, EditorLspServer, JsonRpcId, JsonRpcRequest, LspCodeLens,
    LspDefinitionParams, LspDocumentHighlight, LspFoldingRange, LspFoldingRangeParams, LspInlayHint,
    LspInlayHintParams, LspLocation, LspPosition, LspPrepareRenameResult, LspRange,
    LspSelectionRange, LspSelectionRangeParams, LspTextDocumentIdentifier,
};
use crate::EditorConfig;
use std::fs;

fn open(name: &str, source: &str) -> (std::path::PathBuf, String, EditorLspServer) {
    let (root, uri) = sample_package_root(name);
    fs::write(root.join("src/main.fol"), source).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    (root, uri, server)
}

fn position_request(
    server: &mut EditorLspServer,
    uri: &str,
    method: &str,
    line: u32,
    character: u32,
) -> serde_json::Value {
    server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: method.to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.to_string(),
                    },
                    position: LspPosition { line, character },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap()
}

#[test]
fn lsp_server_resolves_type_definition_to_the_declaring_type() {
    let (root, uri, mut server) = open(
        "type_definition",
        "typ Point: rec = {\n    x: int\n};\n\nfun[] main(): int = {\n    var p: Point = { x = 1 };\n    return p.x;\n};\n",
    );
    // `p` usage on `return p.x;` (line 6).
    let value = position_request(&mut server, &uri, "textDocument/typeDefinition", 6, 11);
    let location: Option<LspLocation> = serde_json::from_value(value).unwrap();
    let location = location.expect("type definition should resolve to the type decl");
    assert_eq!(location.range.start.line, 0, "should jump to `typ Point`");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_resolves_implementations_of_a_protocol_standard() {
    let (root, uri, mut server) = open(
        "implementation",
        "std geo: pro = {\n    fun area(): int;\n};\n\ntyp Rect()(geo): rec = {\n    value: int\n};\n\nfun (Rect)area(): int = {\n    return 1;\n};\n\nfun[] main(): int = {\n    var r: Rect = { value = 1 };\n    return r.area();\n};\n",
    );
    // `geo` reference inside the conformance header `typ Rect()(geo)` (line 4).
    let value = position_request(&mut server, &uri, "textDocument/implementation", 4, 11);
    let locations: Vec<LspLocation> = serde_json::from_value(value).unwrap();
    assert!(
        !locations.is_empty(),
        "standard should resolve to its conforming type(s): {locations:?}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_highlights_symbol_occurrences_in_the_current_file() {
    let (root, uri, mut server) = open(
        "document_highlight",
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
    );
    // `helper` call on `return helper();` (line 5); declaration is line 0.
    let value = position_request(&mut server, &uri, "textDocument/documentHighlight", 5, 12);
    let highlights: Vec<LspDocumentHighlight> = serde_json::from_value(value).unwrap();
    assert_eq!(
        highlights.len(),
        2,
        "declaration + call should both highlight: {highlights:?}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_prepares_rename_with_range_and_placeholder() {
    let (root, uri, mut server) = open(
        "prepare_rename",
        "fun[] main(): int = {\n    var value: int = 7;\n    return value;\n};\n",
    );
    let value = position_request(&mut server, &uri, "textDocument/prepareRename", 2, 12);
    let result: Option<LspPrepareRenameResult> = serde_json::from_value(value).unwrap();
    let result = result.expect("prepareRename should offer a rename range");
    assert_eq!(result.placeholder, "value");
    assert_eq!(result.range.start.line, 2);
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_offers_type_inlay_hints_only_for_unannotated_bindings() {
    let (root, uri, mut server) = open(
        "inlay_hints",
        "fun[] main(): int = {\n    var typed: int = 1;\n    var inferred = 2;\n    return typed + inferred;\n};\n",
    );
    let value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/inlayHint".to_string(),
            params: Some(
                serde_json::to_value(LspInlayHintParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    range: LspRange {
                        start: LspPosition { line: 0, character: 0 },
                        end: LspPosition { line: 10, character: 0 },
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let hints: Vec<LspInlayHint> = serde_json::from_value(value).unwrap();
    // Only `inferred` (line 2) gets a hint; the annotated `typed` does not.
    assert_eq!(hints.len(), 1, "exactly the unannotated binding: {hints:?}");
    assert_eq!(hints[0].position.line, 2);
    assert_eq!(hints[0].label, ": int");
    assert_eq!(hints[0].kind, Some(1));
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_offers_a_run_code_lens_above_main() {
    let (root, uri, mut server) = open(
        "code_lens",
        "fun[] helper(): int = {\n    return 1;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
    );
    let value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/codeLens".to_string(),
            params: Some(
                serde_json::to_value(LspFoldingRangeParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let lenses: Vec<LspCodeLens> = serde_json::from_value(value).unwrap();
    assert_eq!(lenses.len(), 1, "one Run lens above main: {lenses:?}");
    let command = lenses[0].command.as_ref().expect("lens carries a command");
    assert!(command.title.contains("Run"));
    assert_eq!(command.command, "fol.run");
    assert_eq!(lenses[0].range.start.line, 4, "above `fun[] main`");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_folds_multiline_brace_blocks() {
    let (root, uri, mut server) = open(
        "folding",
        "typ Point: rec = {\n    x: int\n};\n\nfun[] main(): int = {\n    return 0;\n};\n",
    );
    let value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/foldingRange".to_string(),
            params: Some(
                serde_json::to_value(LspFoldingRangeParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let ranges: Vec<LspFoldingRange> = serde_json::from_value(value).unwrap();
    // The `Point` record body (0..2) and the `main` body (4..6).
    assert!(ranges.iter().any(|r| r.start_line == 0 && r.end_line == 2));
    assert!(ranges.iter().any(|r| r.start_line == 4 && r.end_line == 6));
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_expands_selection_from_word_to_block_to_file() {
    let (root, uri, mut server) = open(
        "selection_range",
        "fun[] main(): int = {\n    return 7;\n};\n",
    );
    let value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/selectionRange".to_string(),
            params: Some(
                serde_json::to_value(LspSelectionRangeParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    positions: vec![LspPosition { line: 1, character: 11 }],
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let ranges: Vec<LspSelectionRange> = serde_json::from_value(value).unwrap();
    assert_eq!(ranges.len(), 1);
    let mut depth = 0;
    let mut current = Some(&ranges[0]);
    while let Some(node) = current {
        depth += 1;
        current = node.parent.as_deref();
    }
    assert!(depth >= 2, "selection should nest word -> block -> file: depth {depth}");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_resolves_keyword_completion_items_with_a_category() {
    let mut server = {
        let (_root, _uri, server) = open("resolve", "fun[] main(): int = {\n    return 0;\n};\n");
        server
    };
    let value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "completionItem/resolve".to_string(),
            params: Some(
                serde_json::to_value(EditorCompletionItem {
                    label: "fun".to_string(),
                    kind: 14,
                    detail: None,
                    insert_text: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let item: EditorCompletionItem = serde_json::from_value(value).unwrap();
    assert_eq!(item.detail.as_deref(), Some("declaration keyword"));
}
