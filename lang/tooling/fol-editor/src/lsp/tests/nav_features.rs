use super::super::{
    EditorCompletionItem, EditorLspServer, JsonRpcId, JsonRpcRequest, LspDefinitionParams,
    LspDocumentHighlight, LspFoldingRange, LspFoldingRangeParams, LspInlayHint, LspInlayHintParams,
    LspLocation, LspPosition, LspPrepareRenameResult, LspRange, LspSelectionRange,
    LspSelectionRangeParams, LspTextDocumentIdentifier,
};
use super::helpers::{hosted_sample_package_root, open_document, sample_package_root};
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
fn lsp_server_resolves_v3_wrapped_type_definitions() {
    let source = concat!(
        "typ Node: rec = {\n",
        "    value: int\n",
        "};\n",
        "\n",
        "fun[] inspect(borrowed[bor]: Node, pointer: ptr[Node], channel: chn[Node]): int = {\n",
        "    return borrowed.value;\n",
        "};\n",
        "\n",
        "fun[] main(): int = {\n",
        "    @var owned: Node = { value = 1 };\n",
        "    return owned.value;\n",
        "};\n",
    );
    let (root, uri) = hosted_sample_package_root("type_definition_v3_wrappers");
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), source);
    assert!(
        diagnostics[0].diagnostics.is_empty(),
        "V3 type-definition fixture should typecheck: {:?}",
        diagnostics[0].diagnostics
    );

    for (line, name) in [
        (5, "borrowed"),
        (4, "pointer"),
        (4, "channel"),
        (10, "owned"),
    ] {
        let character = source
            .lines()
            .nth(line)
            .and_then(|source_line| source_line.find(name))
            .expect("fixture symbol should exist") as u32;
        let value = position_request(
            &mut server,
            &uri,
            "textDocument/typeDefinition",
            line as u32,
            character,
        );
        let location: Option<LspLocation> = serde_json::from_value(value).unwrap();
        assert_eq!(
            location.map(|location| location.range.start.line),
            Some(0),
            "{name} should unwrap to the Node declaration"
        );
    }

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
                        start: LspPosition {
                            line: 0,
                            character: 0,
                        },
                        end: LspPosition {
                            line: 10,
                            character: 0,
                        },
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
fn lsp_server_ignores_comment_braces_for_folding_and_selection() {
    let source = concat!(
        "` fake outer { `\n",
        "fun[] main(): int = {\n",
        "    // fake } { line pair\n",
        "    /* fake {\n",
        "       } block pair */\n",
        "    return 7;\n",
        "};\n",
        "` fake outer } `\n",
    );
    let (root, uri, mut server) = open("comment_braces", source);
    let folding_value = server
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
    let folds: Vec<LspFoldingRange> = serde_json::from_value(folding_value).unwrap();
    assert_eq!(
        folds.len(),
        1,
        "only the routine body should fold: {folds:?}"
    );
    assert_eq!((folds[0].start_line, folds[0].end_line), (1, 6));

    let selection_value = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(2),
            method: "textDocument/selectionRange".to_string(),
            params: Some(
                serde_json::to_value(LspSelectionRangeParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    positions: vec![LspPosition {
                        line: 5,
                        character: 11,
                    }],
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap()
        .result
        .unwrap();
    let ranges: Vec<LspSelectionRange> = serde_json::from_value(selection_value).unwrap();
    let mut chain = Vec::new();
    let mut current = ranges.first();
    while let Some(range) = current {
        chain.push(range.range);
        current = range.parent.as_deref();
    }
    assert_eq!(chain.len(), 3, "word -> routine block -> file: {chain:?}");
    assert_eq!(chain[1].start.line, 1);
    assert_eq!(chain[1].end.line, 6);
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
                    positions: vec![LspPosition {
                        line: 1,
                        character: 11,
                    }],
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
    assert!(
        depth >= 2,
        "selection should nest word -> block -> file: depth {depth}"
    );
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
