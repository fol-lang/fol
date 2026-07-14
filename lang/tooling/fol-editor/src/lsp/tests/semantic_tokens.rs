use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspSemanticTokens, LspSemanticTokensParams,
    LspTextDocumentIdentifier,
};
use super::helpers::{copied_example_package_root, open_document, sample_package_root};
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

fn request_semantic_tokens(
    server: &mut EditorLspServer,
    uri: String,
    id: i64,
) -> Vec<(u32, u32, u32, u32, u32)> {
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(id),
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
    decode_semantic_tokens(&tokens.data)
}

fn nth_text_position(text: &str, needle: &str, ordinal: usize) -> (u32, u32) {
    let mut search_offset = 0;
    let mut byte_index = None;
    for _ in 0..ordinal {
        let relative = text[search_offset..]
            .find(needle)
            .expect("semantic-token needle should exist");
        byte_index = Some(search_offset + relative);
        search_offset += relative + needle.len();
    }
    let byte_index = byte_index.expect("semantic-token ordinal should be positive");
    let prefix = &text[..byte_index];
    (
        prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
        prefix
            .rsplit('\n')
            .next()
            .expect("split keeps one segment")
            .encode_utf16()
            .count() as u32,
    )
}

fn token_covers_position(token: &(u32, u32, u32, u32, u32), position: (u32, u32)) -> bool {
    token.0 == position.0 && token.1 <= position.1 && position.1 < token.1 + token.2
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

    // Routine declarations anchor at their NAME (helper at 4:6); type
    // declarations still anchor at the `typ` keyword until TypeDecl carries
    // a name syntax id. Explicit parameters and locals retain their precise
    // declaration spans, and their resolved uses are tokenized too.
    assert!(decoded.iter().any(|token| *token == (0, 0, 3, 1, 0)));
    assert!(decoded.iter().any(|token| *token == (4, 6, 6, 2, 0)));
    assert!(decoded.iter().any(|token| *token == (4, 13, 5, 3, 0)));
    assert!(decoded.iter().any(|token| *token == (5, 15, 5, 1, 0)));
    assert!(decoded.iter().any(|token| *token == (5, 23, 5, 3, 0)));
    assert!(decoded.iter().any(|token| *token == (6, 11, 5, 4, 0)));
}

#[test]
fn lsp_semantic_tokens_use_utf16_columns_after_astral_text() {
    let (root, uri) = sample_package_root("semantic_tokens_utf16");
    let text = concat!(
        "fun[] helper(): int = { return 7; };\n",
        "fun[] main(): int = { var icon: str = \"😀\"; return helper(); };\n",
    );
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), text);
    assert!(diagnostics[0].diagnostics.is_empty());

    let decoded = request_semantic_tokens(&mut server, uri, 6004);
    let helper_position = nth_text_position(text, "helper", 2);
    assert!(decoded
        .iter()
        .any(|token| token_covers_position(token, helper_position)));

    let line = text.lines().nth(1).unwrap();
    let helper_byte = line.rfind("helper").unwrap();
    assert_eq!(
        helper_position.1,
        line[..helper_byte].chars().count() as u32 + 1,
        "the astral character must add one extra UTF-16 code unit"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_tokens_explicit_v3_parameter_declarations_in_real_examples() {
    let cases = [
        (
            "examples/mem_borrow_param_m2",
            vec![
                (4, 14, 4, 3, 0), // item[bor] declaration
                (5, 11, 4, 3, 0), // item use
            ],
        ),
        (
            "examples/proc_mutex_m3",
            vec![
                (6, 13, 7, 3, 0),  // worker counter[mux] declaration
                (12, 17, 7, 3, 0), // coordinate counter[mux] declaration
            ],
        ),
    ];

    for (index, (example, expected)) in cases.into_iter().enumerate() {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);
        let decoded = request_semantic_tokens(&mut server, uri, 955 + index as i64);

        for token in expected {
            assert!(
                decoded.contains(&token),
                "semantic tokens for real V3 example '{example}' should include {token:?}, got: {decoded:?}"
            );
        }

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_tokens_v3_memory_and_processor_references_in_real_examples() {
    let cases = [
        ("examples/mem_move_stack_vs_heap_m1", "heap_a", 2),
        ("examples/mem_ptr_unique_m3", "outer", 2),
        ("examples/proc_spawn_m1", "worker", 2),
        ("examples/proc_spawn_m1", "echo_int", 1),
        ("examples/proc_channel_m2", "channel", 6),
        ("examples/proc_async_await_m4", "transferred", 2),
        ("examples/proc_async_await_m4", "double", 1),
    ];

    for (index, (example, needle, ordinal)) in cases.into_iter().enumerate() {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);
        let decoded = request_semantic_tokens(&mut server, uri, 965 + index as i64);
        let position = nth_text_position(&text, needle, ordinal);

        assert!(
            decoded
                .iter()
                .any(|token| token_covers_position(token, position)),
            "real V3 example '{example}' should retain a semantic token over {needle} occurrence {ordinal} at {position:?}: {decoded:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_does_not_tokenize_synthesized_self_at_receiver_routine_name() {
    let (root, uri) = copied_example_package_root("examples/core_records");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let decoded = request_semantic_tokens(&mut server, uri, 957);

    assert!(
        decoded.contains(&(5, 13, 4, 2, 0)),
        "receiver routine name should remain a function token: {decoded:?}"
    );
    assert!(
        decoded.contains(&(5, 18, 2, 3, 0)),
        "explicit receiver-routine parameter declaration should be a parameter token: {decoded:?}"
    );
    assert!(
        !decoded.contains(&(5, 13, 4, 3, 0)),
        "synthesized self must not repaint the receiver routine name as a parameter: {decoded:?}"
    );

    fs::remove_dir_all(root).ok();
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

    // The `build` routine anchors at its name; the untyped `build`/`graph`
    // locals tokenize at their resolved reference sites.
    assert!(decoded.iter().any(|token| *token == (0, 6, 5, 2, 0)));
    assert!(decoded.iter().any(|token| *token == (2, 4, 5, 4, 0)));
    assert!(decoded.iter().any(|token| token.3 == 4));
}

#[test]
fn lsp_server_keeps_build_file_semantic_tokens_for_all_model_declarations() {
    let (root, _uri) = sample_package_root("semantic_tokens_build_models");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
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

    for line in [5, 6, 7] {
        assert!(
            decoded.iter().any(|token| token.0 == line),
            "build files with core/memo/std declarations should keep semantic tokens on artifact line {line}: {decoded:?}"
        );
    }

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_more_specific_semantic_tokens_for_v2_examples() {
    let cases = [
        (
            "semantic_tokens_v2_generic_example",
            "fun pick(T)(value: T): T = {\n    return value;\n};\n\nfun[] main(): int = {\n    return pick(7);\n};\n",
            vec![
                (0, 4, 4, 2, 0),   // pick declaration name
                (0, 19, 1, 3, 0),  // T tokenized in generic signature position
                (0, 23, 1, 3, 0),  // T tokenized in generic signature position
                (1, 11, 5, 3, 0),  // value parameter reference
                (4, 6, 4, 2, 0),   // main declaration name
                (5, 11, 4, 2, 0),  // pick call
            ],
        ),
        (
            "semantic_tokens_v2_standards_example",
            "std geo: pro = {\n    fun area(): int;\n};\n\ntyp Rect()(geo): rec = {\n    var width: int;\n};\n\nfun (Rect)area(): int = {\n    return 1;\n};\n",
            vec![
                (4, 0, 3, 1, 0),  // typ (type decls still anchor at the keyword)
                (8, 10, 4, 2, 0), // area declaration name
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
