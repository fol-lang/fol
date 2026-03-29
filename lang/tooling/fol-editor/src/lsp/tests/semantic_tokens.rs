use super::helpers::{open_document, sample_package_root};
use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspSemanticTokens,
    LspSemanticTokensParams, LspTextDocumentIdentifier,
};
use crate::EditorConfig;
use std::fs;

fn decode_semantic_tokens(data: &[u32]) -> Vec<(u32, u32, u32, u32, u32)> {
    let mut decoded = Vec::new();
    let mut line = 0_u32;
    let mut start = 0_u32;
    for chunk in data.chunks_exact(5) {
        let delta_line = chunk[0];
        let delta_start = chunk[1];
        if delta_line == 0 {
            start += delta_start;
        } else {
            line += delta_line;
            start = delta_start;
        }
        decoded.push((line, start, chunk[2], chunk[3], chunk[4]));
    }
    decoded
}

#[test]
fn lsp_server_returns_semantic_tokens_for_source_files() {
    let (root, uri) = sample_package_root("semantic_tokens_source");
    fs::write(
        root.join("src/main.fol"),
        "typ[] Local: rec = {\n    value: int;\n};\n\nfun[] helper(total: int): int = {\n    var value: Local = total;\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(951),
            method: "textDocument/semanticTokens/full".to_string(),
            params: Some(
                serde_json::to_value(LspSemanticTokensParams {
                    text_document: LspTextDocumentIdentifier { uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let tokens: LspSemanticTokens = serde_json::from_value(response.result.unwrap()).unwrap();
    let decoded = decode_semantic_tokens(&tokens.data);

    assert!(decoded.iter().any(|token| *token == (0, 6, 5, 1, 0)));
    assert!(decoded.iter().any(|token| *token == (3, 6, 6, 2, 0)));
    assert!(decoded.iter().any(|token| *token == (3, 13, 5, 3, 0)));
    assert!(decoded.iter().any(|token| *token == (4, 8, 5, 4, 0)));
}

#[test]
fn lsp_server_returns_semantic_tokens_for_build_files() {
    let (root, _uri) = sample_package_root("semantic_tokens_build");
    let build_uri = format!("file://{}", root.join("build.fol").display());
    let build_text = fs::read_to_string(root.join("build.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, build_uri.clone(), &build_text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(952),
            method: "textDocument/semanticTokens/full".to_string(),
            params: Some(
                serde_json::to_value(LspSemanticTokensParams {
                    text_document: LspTextDocumentIdentifier { uri: build_uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let tokens: LspSemanticTokens = serde_json::from_value(response.result.unwrap()).unwrap();
    let decoded = decode_semantic_tokens(&tokens.data);

    assert!(decoded.iter().any(|token| *token == (0, 6, 5, 2, 0)));
    assert!(decoded.iter().any(|token| *token == (0, 12, 5, 3, 0)));
    assert!(
        decoded
            .iter()
            .any(|token| token.0 == 0 && token.3 == 1 && token.2 >= 5)
    );
}

#[test]
fn lsp_server_keeps_build_file_semantic_tokens_for_all_model_declarations() {
    let (root, _uri) = sample_package_root("semantic_tokens_build_models");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_static_lib({ name = \"corelib\", root = \"src/main.fol\", fol_model = \"core\" });\n",
            "    graph.add_static_lib({ name = \"alloclib\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    graph.add_exe({ name = \"tool\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    let build_uri = format!("file://{}", root.join("build.fol").display());
    let build_text = fs::read_to_string(root.join("build.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, build_uri.clone(), &build_text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(953),
            method: "textDocument/semanticTokens/full".to_string(),
            params: Some(
                serde_json::to_value(LspSemanticTokensParams {
                    text_document: LspTextDocumentIdentifier { uri: build_uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let tokens: LspSemanticTokens = serde_json::from_value(response.result.unwrap()).unwrap();
    let decoded = decode_semantic_tokens(&tokens.data);

    assert!(
        decoded.iter().filter(|token| token.0 >= 2 && token.0 <= 4).count() >= 6,
        "build files with core/memo/std declarations should keep semantic tokens on all model lines: {decoded:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_more_specific_semantic_tokens_for_v2_examples() {
    let cases = [
        (
            "semantic_tokens_v2_generic_example",
            "fun pick(T)(value: T): T = {\n    return value;\n};\n\nfun[] main(): int = {\n    return pick(7);\n};\n",
            vec![
                (0, 0, 3, 2, 0),   // fun
                (0, 19, 1, 3, 0),  // T tokenized in generic signature position
                (0, 23, 1, 3, 0),  // T tokenized in generic signature position
                (1, 11, 5, 3, 0),  // value parameter reference
                (4, 0, 3, 2, 0),   // fun
                (5, 11, 4, 2, 0),  // pick call
            ],
        ),
        (
            "semantic_tokens_v2_standards_example",
            "std geo: pro = {\n    fun area(): int;\n};\n\ntyp Rect()(geo): rec = {\n    var width: int;\n};\n\nfun (Rect)area(): int = {\n    return 1;\n};\n",
            vec![
                (4, 0, 3, 1, 0),  // typ
                (8, 0, 3, 2, 0),  // fun
                (8, 5, 4, 1, 0),  // Rect receiver type
            ],
        ),
    ];

    for (label, source, expected_tokens) in cases {
        let (root, uri) = sample_package_root(label);
        fs::write(root.join("src/main.fol"), source).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let response = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(954),
                method: "textDocument/semanticTokens/full".to_string(),
                params: Some(
                    serde_json::to_value(LspSemanticTokensParams {
                        text_document: LspTextDocumentIdentifier { uri },
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let tokens: LspSemanticTokens = serde_json::from_value(response.result.unwrap()).unwrap();
        let decoded = decode_semantic_tokens(&tokens.data);

        for expected in expected_tokens {
            assert!(
                decoded.iter().any(|token| *token == expected),
                "semantic tokens for '{label}' should include {expected:?}, got: {decoded:?}"
            );
        }

        fs::remove_dir_all(root).ok();
    }
}
