use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspCompletionContext, LspCompletionList,
    LspCompletionParams, LspPosition, LspTextDocumentIdentifier,
};
use super::helpers::{hosted_sample_package_root, open_document, sample_package_root, temp_root};
use crate::EditorConfig;
use std::fs;

#[test]
fn lsp_server_returns_supported_v1_dot_intrinsics() {
    let (root, uri) = sample_package_root("completion_dot_intrinsics");
    // `.echo` is only a legal completion when the package declares the
    // bundled internal std dependency (hosted capability).
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return .;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(44),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
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

    let completion: LspCompletionList = serde_json::from_value(completion.result.unwrap()).unwrap();
    let labels = completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"len"));
    assert!(labels.contains(&"echo"));
    assert!(labels.contains(&"eq"));
    assert!(labels.contains(&"not"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_uses_conservative_dot_fallback_for_incomplete_contexts() {
    let (root, uri) = hosted_sample_package_root("completion_dot_fallback");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return .;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(45),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList = serde_json::from_value(completion.result.unwrap()).unwrap();
    let labels = completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"len"));
    assert!(labels.contains(&"echo"));
    assert!(labels.contains(&"eq"));
    assert!(labels.contains(&"not"));
    assert!(!labels.contains(&"panic"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_uses_lsp_dot_trigger_when_document_text_is_not_yet_synced() {
    let (root, uri) = hosted_sample_package_root("completion_dot_lsp_context");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return \n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(48),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
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

    let labels = serde_json::from_value::<LspCompletionList>(completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(labels.contains(&"len".to_string()));
    assert!(labels.contains(&"echo".to_string()));
    assert!(!labels.contains(&"main".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_locks_dot_intrinsic_completion_matrix() {
    let (root, uri) = hosted_sample_package_root("completion_dot_matrix");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return .;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(46),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
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

    let completion: LspCompletionList = serde_json::from_value(completion.result.unwrap()).unwrap();
    let labels = completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"len"));
    assert!(labels.contains(&"echo"));
    assert!(labels.contains(&"eq"));
    assert!(labels.contains(&"nq"));
    assert!(labels.contains(&"lt"));
    assert!(labels.contains(&"le"));
    assert!(labels.contains(&"gt"));
    assert!(labels.contains(&"ge"));
    assert!(labels.contains(&"not"));
    assert!(!labels.contains(&"panic"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_filters_echo_from_core_and_memo_without_and_with_std() {
    for (label, model, declare_std, expected_echo) in [
        ("core", "core", false, false),
        ("memo", "memo", false, false),
        ("memo_with_std", "memo", true, true),
    ] {
        let (root, uri) = sample_package_root(&format!("completion_dot_model_{label}"));
        fs::write(
            root.join("build.fol"),
            if declare_std {
                format!(
                    concat!(
                        "pro[] build(): non = {{\n",
                        "    var build = .build();\n",
                        "    build.meta({{ name = \"sample\", version = \"0.1.0\" }});\n",
                        "    build.add_dep({{ alias = \"std\", source = \"internal\", target = \"standard\" }});\n",
                        "    var graph = build.graph();\n",
                        "    graph.add_exe({{ name = \"demo\", root = \"src/main.fol\", fol_model = \"{}\" }});\n",
                        "}};\n",
                    ),
                    model
                )
            } else {
                format!(
                    concat!(
                        "pro[] build(): non = {{\n",
                        "    var build = .build();\n",
                        "    build.meta({{ name = \"sample\", version = \"0.1.0\" }});\n",
                        "    var graph = build.graph();\n",
                        "    graph.add_exe({{ name = \"demo\", root = \"src/main.fol\", fol_model = \"{}\" }});\n",
                        "}};\n",
                    ),
                    model
                )
            },
        )
        .unwrap();
        fs::write(
            root.join("src/main.fol"),
            "fun[] main(): int = {\n    return .;\n};\n",
        )
        .unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let completion = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(461),
                method: "textDocument/completion".to_string(),
                params: Some(
                    serde_json::to_value(LspCompletionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
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

        let labels = serde_json::from_value::<LspCompletionList>(completion.result.unwrap())
            .unwrap()
            .items
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels.contains(&"echo".to_string()),
            expected_echo,
            "dot completion echo visibility should track fol_model={model}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_keeps_model_completion_context_isolated_across_workspace_members() {
    let root = temp_root("completion_mixed_workspace_models");
    let app_src = root.join("app/src");
    let tool_src = root.join("tool/src");
    fs::create_dir_all(&app_src).unwrap();
    fs::create_dir_all(&tool_src).unwrap();
    fs::write(root.join("fol.work.yaml"), "members:\n  - app\n  - tool\n").unwrap();

    fs::write(root.join("app/build.fol"), "name: app\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("app/build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
    )
    .unwrap();
    fs::write(
        root.join("app/src/main.fol"),
        "fun[] main(): int = {\n    return .;\n};\n",
    )
    .unwrap();

    fs::write(root.join("tool/build.fol"), "name: tool\nversion: 0.1.0\n").unwrap();
    fs::write(
        root.join("tool/build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"tool\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "};\n",
            ),
    )
    .unwrap();
    fs::write(
        root.join("tool/src/main.fol"),
        "fun[] main(): int = {\n    return .;\n};\n",
    )
    .unwrap();

    let app_uri = format!("file://{}", root.join("app/src/main.fol").display());
    let tool_uri = format!("file://{}", root.join("tool/src/main.fol").display());
    let app_text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let tool_text = fs::read_to_string(root.join("tool/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, app_uri.clone(), &app_text);
    open_document(&mut server, tool_uri.clone(), &tool_text);

    let app_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(462),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: app_uri.clone(),
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
    let tool_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(463),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: tool_uri.clone(),
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

    let app_labels = serde_json::from_value::<LspCompletionList>(app_completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    let tool_labels = serde_json::from_value::<LspCompletionList>(tool_completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();

    assert!(!app_labels.contains(&"echo".to_string()));
    assert!(tool_labels.contains(&"echo".to_string()));

    fs::remove_dir_all(root).ok();
}
