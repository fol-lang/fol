use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspCompletionContext, LspCompletionList,
    LspCompletionParams, LspPosition, LspTextDocumentIdentifier,
};
use super::helpers::{
    copied_example_package_root, hosted_sample_package_root, open_document,
    sample_loc_workspace_root, sample_package_root,
};
use crate::EditorConfig;
use std::fs;

#[test]
fn lsp_server_handles_completion_requests() {
    let (root, uri) = sample_package_root("completion_request");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: int = 7;\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(30),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
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
    assert!(!completion.is_incomplete);
    assert!(completion.items.iter().any(|item| item.label == "value"));
    assert!(
        completion
            .items
            .iter()
            .find(|item| item.label == "value")
            .and_then(|item| item.detail.as_deref())
            == Some("binding")
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_plain_completion_available_when_typecheck_fails() {
    let (root, uri) = sample_package_root("completion_type_error_fallback");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    var value: int = helper();\n    value = \"oops\";\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(!diagnostics.is_empty());

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(30),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 6,
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
    assert!(completion.items.iter().any(|item| item.label == "value"));
    assert!(completion.items.iter().any(|item| item.label == "helper"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_routine_parameter_completions() {
    let (root, uri) = sample_package_root("completion_params");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(total: int): int = {\n    return total;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(31),
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
    assert!(completion.items.iter().any(|item| item.label == "total"));
    assert!(
        completion
            .items
            .iter()
            .find(|item| item.label == "total")
            .and_then(|item| item.detail.as_deref())
            == Some("parameter")
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_builtin_type_completions_in_type_positions() {
    let (root, uri) = sample_package_root("completion_builtin_types");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: ;\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(36),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 15,
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
    assert!(labels.contains(&"int"));
    assert!(labels.contains(&"str"));
    assert!(labels.contains(&"never"));
    assert!(labels.contains(&"arr"));
    assert!(labels.contains(&"seq"));
    assert!(labels.contains(&"opt"));
    assert!(labels.contains(&"err"));
    assert!(labels.contains(&"ptr"));
    assert!(!labels.contains(&"chn"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_filters_heap_type_surfaces_from_core_type_completion() {
    let (root, uri) = sample_package_root("completion_core_type_surfaces");
    fs::write(
        root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "};\n",
            ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: ;\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(361),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 15,
                    },
                    context: None,
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
    assert!(labels.contains(&"int".to_string()));
    assert!(labels.contains(&"arr".to_string()));
    assert!(labels.contains(&"opt".to_string()));
    assert!(labels.contains(&"err".to_string()));
    assert!(labels.contains(&"ptr".to_string()));
    assert!(!labels.contains(&"chn".to_string()));
    assert!(!labels.contains(&"str".to_string()));
    assert!(!labels.contains(&"vec".to_string()));
    assert!(!labels.contains(&"seq".to_string()));
    assert!(!labels.contains(&"set".to_string()));
    assert!(!labels.contains(&"map".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_offers_hosted_structured_type_surfaces() {
    let (root, uri) = hosted_sample_package_root("completion_hosted_type_surfaces");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: ;\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(362),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 15,
                    },
                    context: None,
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
    assert!(labels.contains(&"ptr".to_string()));
    assert!(labels.contains(&"chn".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_handles_completion_for_single_and_ambiguous_model_package_files() {
    let (root, _) = sample_package_root("completion_mixed_model_package");
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"host\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    graph.add_test({ name = \"suite\", root = \"test/app.fol\", fol_model = \"core\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    var shown: str = std::io::echo_int(7);\n    return .len(shown);\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("test/app.fol"),
        "fun[] main(): int = {\n    var values: arr[int, 2] = {1, 2};\n    return .len(values);\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("notes.fol"),
        "fun[] helper(): int = {\n    return .;\n};\n",
    )
    .unwrap();

    let std_uri = format!("file://{}", root.join("src/main.fol").display());
    let core_uri = format!("file://{}", root.join("test/app.fol").display());
    let notes_uri = format!("file://{}", root.join("notes.fol").display());
    let std_text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let core_text = fs::read_to_string(root.join("test/app.fol")).unwrap();
    let notes_text = fs::read_to_string(root.join("notes.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());

    open_document(&mut server, std_uri.clone(), &std_text);
    open_document(&mut server, core_uri.clone(), &core_text);
    open_document(&mut server, notes_uri.clone(), &notes_text);

    let std_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(390),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: std_uri.clone(),
                    },
                    position: LspPosition {
                        line: 3,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let std_labels = serde_json::from_value::<LspCompletionList>(std_completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(std_labels.iter().any(|label| label == "echo"));

    let core_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(391),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: core_uri.clone(),
                    },
                    position: LspPosition {
                        line: 2,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let core_labels = serde_json::from_value::<LspCompletionList>(core_completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(!core_labels.iter().any(|label| label == "echo"));
    assert!(core_labels.iter().any(|label| label == "len"));

    let notes_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(392),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: notes_uri },
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
    let notes_labels =
        serde_json::from_value::<LspCompletionList>(notes_completion.result.unwrap())
            .unwrap()
            .items
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
    assert!(
        !notes_labels.iter().any(|label| label == "echo"),
        "ambiguous package-local files must not inherit hosted completion: {notes_labels:?}"
    );
    assert!(
        notes_labels.iter().any(|label| label == "len"),
        "ambiguous package-local files should still expose shared root completions: {notes_labels:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_scopes_mixed_artifact_helper_completion() {
    let (root, _) = sample_package_root("completion_mixed_artifact_helpers");
    let core = root.join("core");
    let memo = root.join("memo");
    fs::create_dir_all(&core).unwrap();
    fs::create_dir_all(&memo).unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"core\", root = \"core/main.fol\", fol_model = \"core\" });\n",
            "    graph.add_exe({ name = \"memo\", root = \"memo/main.fol\", fol_model = \"memo\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        core.join("main.fol"),
        "fun[] core_only(): int = { return 1; };\n",
    )
    .unwrap();
    fs::write(
        memo.join("main.fol"),
        "fun[] memo_only(): str = { return \"memo\"; };\n",
    )
    .unwrap();

    let core_helper = core.join("helper.fol");
    let core_uri = format!("file://{}", core_helper.display());
    let core_types = completion_labels_for_path_at_marker(
        &core_helper,
        &core_uri,
        "fun[] helper(): int = {\n    var value: <|>int = 0;\n    return value;\n};\n",
    );
    assert!(core_types.iter().any(|label| label == "int"));
    assert!(
        !core_types.iter().any(|label| label == "str"),
        "core helper inherited memo type completion: {core_types:?}"
    );
    let core_symbols = completion_labels_for_path_at_marker(
        &core_helper,
        &core_uri,
        "fun[] helper(): int = {\n    return <|>core_only();\n};\n",
    );
    assert!(core_symbols.iter().any(|label| label == "core_only"));
    assert!(
        !core_symbols.iter().any(|label| label == "memo_only"),
        "core helper analyzed the sibling memo artifact: {core_symbols:?}"
    );

    let memo_helper = memo.join("helper.fol");
    let memo_uri = format!("file://{}", memo_helper.display());
    let memo_types = completion_labels_for_path_at_marker(
        &memo_helper,
        &memo_uri,
        "fun[] helper(): str = {\n    var value: <|>str = \"ok\";\n    return value;\n};\n",
    );
    assert!(
        memo_types.iter().any(|label| label == "str"),
        "memo helper lost heap type completion: {memo_types:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_build_surface_completions_in_build_files() {
    let (root, _) = sample_package_root("completion_build_surface");
    let build_file = root.join("build.fol");
    fs::write(
        &build_file,
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    build.\n",
            "};\n",
        ),
    )
    .unwrap();
    let text = fs::read_to_string(&build_file).unwrap();
    let uri = format!("file://{}", build_file.display());
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(500),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 10,
                    },
                    context: None,
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
    assert!(labels.contains(&"meta".to_string()));
    assert!(labels.contains(&"add_dep".to_string()));
    assert!(labels.contains(&"export_module".to_string()));
    assert!(labels.contains(&"export_artifact".to_string()));
    assert!(labels.contains(&"export_step".to_string()));
    assert!(labels.contains(&"export_output".to_string()));
    assert!(labels.contains(&"graph".to_string()));
    assert!(!labels.contains(&"add_system_tool_dir".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_graph_path_handle_completions_in_build_files() {
    let (root, _) = sample_package_root("completion_graph_paths");
    let build_file = root.join("build.fol");
    fs::write(
        &build_file,
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.\n",
            "};\n",
        ),
    )
    .unwrap();
    let text = fs::read_to_string(&build_file).unwrap();
    let uri = format!("file://{}", build_file.display());
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(502),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 10,
                    },
                    context: None,
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
    assert!(labels.contains(&"file_from_root".to_string()));
    assert!(labels.contains(&"dir_from_root".to_string()));
    assert!(labels.contains(&"add_system_tool_dir".to_string()));
    assert!(labels.contains(&"add_codegen_dir".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_dependency_handle_completions_in_build_files() {
    let (root, _) = sample_package_root("completion_dependency_surface");
    let build_file = root.join("build.fol");
    fs::write(
        &build_file,
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var dep = build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\" });\n",
            "    dep.\n",
            "};\n",
        ),
    )
    .unwrap();
    let text = fs::read_to_string(&build_file).unwrap();
    let uri = format!("file://{}", build_file.display());
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(501),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 8,
                    },
                    context: None,
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
    assert!(labels.contains(&"module".to_string()));
    assert!(labels.contains(&"artifact".to_string()));
    assert!(labels.contains(&"step".to_string()));
    assert!(labels.contains(&"generated".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_git_dependency_field_completions_in_build_files() {
    let (root, _) = sample_package_root("completion_git_dep_fields");
    let build_file = root.join("build.fol");
    fs::write(
        &build_file,
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://github.com/bresilla/logtiny.git\",  });\n",
            "};\n",
        ),
    )
    .unwrap();
    let text = fs::read_to_string(&build_file).unwrap();
    let uri = format!("file://{}", build_file.display());
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(503),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        // After the quoted git target, at the next config field.
                        character: 111,
                    },
                    context: None,
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
    assert!(labels.contains(&"version".to_string()));
    assert!(labels.contains(&"hash".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_visible_named_type_completions_in_type_positions() {
    let (root, uri) = sample_loc_workspace_root("completion_named_types");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): shared::Status = {\n    var report: shared::Report = ;\n    return shared::Pending;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "typ[exp] Status: ent = {\n    var Pending: int = 1;\n};\n\ntyp[exp] Report: rec = {\n    value: int;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(37),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 31,
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
    assert!(labels.contains(&"Status"));
    assert!(labels.contains(&"Report"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_locks_type_completion_matrix() {
    let (root, uri) = sample_loc_workspace_root("completion_type_matrix");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\ntyp[] Local: rec = {\n    value: int;\n};\n\nfun[] main(): int = {\n    var target: ;\n    return 0;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "typ[exp] Status: ent = {\n    var Pending: int = 1;\n};\n\ntyp[exp] Report: rec = {\n    value: int;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(38),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 7,
                        character: 16,
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
    assert!(labels.contains(&"int"));
    assert!(labels.contains(&"str"));
    assert!(labels.contains(&"Local"));
    assert!(labels.contains(&"Status"));
    assert!(labels.contains(&"Report"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_prefers_builtin_types_ahead_of_named_type_items() {
    let (root, uri) = sample_package_root("completion_type_order");
    fs::write(
        root.join("src/main.fol"),
        "typ[] Aardvark: rec = {\n    value: int;\n};\n\nfun[] main(): int = {\n    var target: ;\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(381),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 16,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let summary = serde_json::from_value::<LspCompletionList>(completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .filter(|item| matches!(item.label.as_str(), "int" | "Aardvark"))
        .map(|item| format!("{}:{}", item.label, item.detail.unwrap_or_default()))
        .collect::<Vec<_>>();
    // The document intentionally contains an incomplete binding, so named
    // types surface through the marked fallback path while builtins stay
    // authoritative.
    assert_eq!(
        summary,
        vec!["int:builtin type", "Aardvark:type (fallback)"]
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_same_package_namespace_members_after_qualification() {
    let (root, uri) = sample_package_root("completion_namespace_local");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return api::;\n};\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src/api")).unwrap();
    fs::write(
        root.join("src/api/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(39),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 16,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList = serde_json::from_value(completion.result.unwrap()).unwrap();
    assert!(completion.items.iter().any(|item| item.label == "helper"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_local_and_imported_namespace_members_separate() {
    let (root, uri) = sample_loc_workspace_root("completion_namespace_separation");
    fs::create_dir_all(root.join("app/src/api")).unwrap();
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): int = {\n    return api::helper() + shared::;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("app/src/api/lib.fol"),
        "fun[exp] local_helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 9;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(40),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 16,
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
    assert!(labels.contains(&"local_helper"));
    assert!(!labels.contains(&"helper"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_locks_loc_and_same_package_namespace_completion() {
    let (root, uri) = sample_loc_workspace_root("completion_namespace_matrix");
    fs::create_dir_all(root.join("app/src/api/tools")).unwrap();
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): int = {\n    return api::;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("app/src/api/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("app/src/api/tools/lib.fol"),
        "fun[exp] leaf(): int = {\n    return 8;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 9;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let local_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(41),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 16,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let local_completion: LspCompletionList =
        serde_json::from_value(local_completion.result.unwrap()).unwrap();
    let local_labels = local_completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(local_labels.contains(&"helper"));
    assert!(local_labels.contains(&"tools"));

    let imported_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(42),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 35,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let imported_completion: LspCompletionList =
        serde_json::from_value(imported_completion.result.unwrap()).unwrap();
    let imported_labels = imported_completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(imported_labels.contains(&"helper"));
    assert!(!imported_labels.contains(&"tools"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_completes_bundled_std_names_only_when_declared() {
    let (root, uri) = copied_example_package_root("examples/std_bundled_fmt");
    let source = "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::;\n};\n";
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), source);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(504),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 21,
                    },
                    context: None,
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
    assert!(labels.contains(&"answer".to_string()));
    assert!(labels.contains(&"double".to_string()));
    assert!(labels.contains(&"math".to_string()));

    fs::remove_dir_all(root).ok();

    let (root, uri) = sample_package_root("completion_std_without_declared_dep");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), source);
    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(505),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 21,
                    },
                    context: None,
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
    assert!(
        !labels.contains(&"answer".to_string()) && !labels.contains(&"double".to_string()),
        "bundled std members should not complete without a declared std dependency: {labels:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_completes_declared_aliases_without_leaking_undeclared_ones() {
    let (root, uri) = copied_example_package_root("examples/std_bundled_fmt");
    fs::create_dir_all(root.join("shared")).unwrap();
    fs::write(
        root.join("shared/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"shared\", version = \"0.1.0\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let source = concat!(
        "use std: pkg = {\"std\"};\n",
        "use shared: loc = {\"../../shared\"};\n",
        "\n",
        "fun[] main(): int = {\n",
        "    return ;\n",
        "};\n",
    );
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), source);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(506),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 11,
                    },
                    context: None,
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
    assert!(labels.contains(&"std".to_string()));
    assert!(labels.contains(&"shared".to_string()));
    assert!(!labels.contains(&"other".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_bundled_std_and_dependency_members_separate_after_qualification() {
    let (root, uri) = copied_example_package_root("examples/std_bundled_fmt");
    fs::create_dir_all(root.join("shared")).unwrap();
    fs::write(
        root.join("shared/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"shared\", version = \"0.1.0\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"std_bundled_fmt\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    build.add_dep({ alias = \"shared\", source = \"loc\", target = \"shared\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"std_bundled_fmt\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    let source = concat!(
        "use std: pkg = {\"std\"};\n",
        "use shared: loc = {\"shared\"};\n",
        "\n",
        "fun[] main(): int = {\n",
        "    return std::fmt::answer() + shared::;\n",
        "};\n",
    );
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), source);

    let std_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(507),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 21,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let std_labels = serde_json::from_value::<LspCompletionList>(std_completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert!(std_labels.contains(&"answer".to_string()));
    assert!(std_labels.contains(&"double".to_string()));
    assert!(!std_labels.contains(&"helper".to_string()));

    let shared_completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(508),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 41,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let shared_labels =
        serde_json::from_value::<LspCompletionList>(shared_completion.result.unwrap())
            .unwrap()
            .items
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
    assert!(!shared_labels.contains(&"answer".to_string()));
    assert!(!shared_labels.contains(&"double".to_string()));

    fs::remove_dir_all(root).ok();
}

fn completion_labels_at_marker(
    root: &std::path::Path,
    uri: &str,
    marked_source: &str,
) -> Vec<String> {
    completion_labels_for_path_at_marker(&root.join("src/main.fol"), uri, marked_source)
}

fn completion_labels_for_path_at_marker(
    path: &std::path::Path,
    uri: &str,
    marked_source: &str,
) -> Vec<String> {
    const MARKER: &str = "<|>";
    let marker = marked_source
        .find(MARKER)
        .expect("completion fixture should contain a cursor marker");
    assert_eq!(
        marked_source[marker + MARKER.len()..].find(MARKER),
        None,
        "completion fixture should contain exactly one cursor marker"
    );
    let before_cursor = &marked_source[..marker];
    let position = LspPosition {
        line: before_cursor.matches('\n').count() as u32,
        character: before_cursor
            .rsplit_once('\n')
            .map_or(before_cursor, |(_, line)| line)
            .chars()
            .count() as u32,
    };
    let source = marked_source.replacen(MARKER, "", 1);
    fs::write(path, &source).unwrap();

    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.to_string(), &source);
    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(700),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.to_string(),
                    },
                    position,
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    serde_json::from_value::<LspCompletionList>(completion.result.unwrap())
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[test]
fn lsp_server_completes_v3_parameter_and_type_contexts() {
    let cases = [
        (
            "parameter",
            "fun[] inspect(item[<|>",
            &["bor", "mux"][..],
            &["var", "tx", "rx"][..],
        ),
        (
            "pointer",
            concat!(
                "typ[] Local: rec = { value: int };\n",
                "fun[] main(): int = {\n",
                "    var pointer: ptr[<|>\n",
            ),
            &["shared", "int", "str", "Local"][..],
            &["chn", "tx", "rx", "return"][..],
        ),
        (
            "channel_element",
            concat!(
                "typ[] Local: rec = { value: int };\n",
                "fun[] main(): int = {\n",
                "    var channel: chn[<|>\n",
            ),
            &["int", "str", "Local"][..],
            &["chn", "shared", "tx", "rx", "return"][..],
        ),
    ];

    for (label, source, expected, excluded) in cases {
        let (root, uri) = hosted_sample_package_root(&format!("completion_v3_{label}"));
        let labels = completion_labels_at_marker(&root, &uri, source);
        for expected in expected {
            assert!(
                labels.iter().any(|label| label == expected),
                "{label} completion should contain {expected:?}: {labels:?}"
            );
        }
        for excluded in excluded {
            assert!(
                !labels.iter().any(|label| label == excluded),
                "{label} completion should exclude {excluded:?}: {labels:?}"
            );
        }
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_completes_v3_nested_owned_and_option_contexts() {
    let cases = [
        (
            "qualified_pointer_target",
            "fun[] main(): int = {\n    var value: ptr[shared, <|>\n",
            &["int", "str"][..],
            &["chn", "shared", "var"][..],
        ),
        (
            "nested_pointer_target",
            "fun[] main(): int = {\n    var value: ptr[vec[<|>\n",
            &["int", "str"][..],
            &["chn", "shared", "var"][..],
        ),
        (
            "nested_channel_target",
            "fun[] main(): int = {\n    var value: chn[opt[<|>\n",
            &["int", "str"][..],
            &["chn", "shared", "var"][..],
        ),
        (
            "owned_target",
            "fun[] main(): int = {\n    var value: @<|>\n",
            &["int", "str"][..],
            &["chn", "shared", "var"][..],
        ),
    ];

    for (label, source, expected, excluded) in cases {
        let (root, uri) = hosted_sample_package_root(&format!("completion_v3_{label}"));
        let labels = completion_labels_at_marker(&root, &uri, source);
        for expected in expected {
            assert!(
                labels.iter().any(|label| label == expected),
                "{label} completion should contain {expected:?}: {labels:?}"
            );
        }
        for excluded in excluded {
            assert!(
                !labels.iter().any(|label| label == excluded),
                "{label} completion should exclude {excluded:?}: {labels:?}"
            );
        }
        fs::remove_dir_all(root).ok();
    }

    let (root, uri) = hosted_sample_package_root("completion_v3_parameter_option_conflict");
    let labels = completion_labels_at_marker(&root, &uri, "fun[] inspect(item[bor, <|>");
    assert!(
        !labels.iter().any(|label| matches!(label.as_str(), "bor" | "mux")),
        "an existing parameter option should suppress duplicate and conflicting options: {labels:?}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_gates_owned_type_completion_by_model() {
    for (label, model, expected) in [("core", "core", false), ("memo", "memo", true)] {
        let (root, uri) = sample_package_root(&format!("completion_v3_owned_{label}"));
        if model == "core" {
            fs::write(
                root.join("build.fol"),
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                    "    return;\n",
                    "};\n",
                ),
            )
            .unwrap();
        }
        let labels = completion_labels_at_marker(
            &root,
            &uri,
            "fun[] main(): int = {\n    var value: @<|>\n",
        );
        assert_eq!(
            labels.iter().any(|label| label == "int"),
            expected,
            "owned type completion availability drifted for {model}: {labels:?}"
        );
        if !expected {
            assert!(
                labels.is_empty(),
                "core owned type context leaked: {labels:?}"
            );
        }
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_completes_qualified_processor_targets() {
    for (label, marked_source, expected) in [
        (
            "spawn",
            "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
                 [>]std::io::<|>;\n\
                 return 0;\n\
             };\n",
            "echo_int",
        ),
        (
            "async",
            "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
                 var pending = std::fmt::<|> | async;\n\
                 return 0;\n\
             };\n",
            "double",
        ),
    ] {
        let (root, uri) = copied_example_package_root("examples/proc_spawn_m1");
        let labels = completion_labels_at_marker(&root, &uri, marked_source);
        assert!(
            labels.iter().any(|item| item == expected),
            "qualified {label} completion should contain '{expected}': {labels:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_gates_v3_nested_type_completion_by_model() {
    for (surface, source, expectations) in [
        (
            "pointer",
            "fun[] main(): int = {\n    var value: ptr[<|>\n",
            [("core", true), ("memo", true), ("hosted", true)],
        ),
        (
            "channel",
            "fun[] main(): int = {\n    var value: chn[<|>\n",
            [("core", false), ("memo", false), ("hosted", true)],
        ),
    ] {
        for (model, expected) in expectations {
            let hosted = model == "hosted";
            let (root, uri) = if hosted {
                hosted_sample_package_root(&format!("completion_v3_{surface}_{model}"))
            } else {
                sample_package_root(&format!("completion_v3_{surface}_{model}"))
            };
            if model == "core" {
                fs::write(
                    root.join("build.fol"),
                    concat!(
                        "pro[] build(): non = {\n",
                        "    var build = .build();\n",
                        "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                        "    var graph = build.graph();\n",
                        "    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                        "    return;\n",
                        "};\n",
                    ),
                )
                .unwrap();
            }
            let labels = completion_labels_at_marker(&root, &uri, source);
            assert_eq!(
                labels.iter().any(|label| label == "int"),
                expected,
                "{surface} target completion availability drifted for {model}: {labels:?}"
            );
            assert_eq!(
                labels.iter().any(|label| label == "shared"),
                surface == "pointer" && expected,
                "pointer qualifier completion availability drifted for {model}: {labels:?}"
            );
            if !expected {
                assert!(
                    labels.is_empty(),
                    "illegal {surface} context should not leak completions in {model}: {labels:?}"
                );
            }
            fs::remove_dir_all(root).ok();
        }
    }
}

#[test]
fn lsp_server_completes_only_mutex_methods_after_mutex_receivers() {
    let (root, uri) = hosted_sample_package_root("completion_v3_mutex_methods");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "typ Counter: rec = { value: int };\n",
            "fun[] update(counter[mux]: Counter): int = {\n",
            "    counter.<|>lock();\n",
            "    counter.unlock();\n",
            "    return 0;\n",
            "};\n",
            "fun[] main(): int = {\n",
            "    var counter: Counter = { value = 0 };\n",
            "    return update(counter);\n",
            "};\n",
        ),
    );
    assert_eq!(labels, vec!["lock".to_string(), "unlock".to_string()]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_respects_nearest_shadow_for_v3_receivers() {
    let (root, uri) = hosted_sample_package_root("completion_v3_mutex_shadow");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "typ Counter: rec = { value: int };\n",
            "fun (Counter)read(): int = { return self.value; };\n",
            "fun[] update(counter[mux]: Counter): int = {\n",
            "    {\n",
            "        var counter: Counter = { value = 4 };\n",
            "        return counter.<|>read();\n",
            "    };\n",
            "};\n",
        ),
    );
    assert!(
        !labels
            .iter()
            .any(|label| matches!(label.as_str(), "lock" | "unlock")),
        "a nearer ordinary binding must shadow an outer mutex parameter: {labels:?}"
    );
    fs::remove_dir_all(root).ok();

    let (root, uri) = hosted_sample_package_root("completion_v3_channel_shadow");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    {\n",
            "        var channel: arr[int, 1] = {4};\n",
            "        return channel[<|>0];\n",
            "    };\n",
            "};\n",
        ),
    );
    assert!(
        !labels
            .iter()
            .any(|label| matches!(label.as_str(), "tx" | "rx")),
        "a nearer ordinary binding must shadow an outer channel: {labels:?}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_suppresses_v3_receiver_completion_in_deferred_blocks() {
    for (label, keyword) in [("dfr", "dfr"), ("edf", "edf")] {
        let (root, uri) = hosted_sample_package_root(&format!("completion_v3_mutex_{label}"));
        let source = format!(
            concat!(
                "typ Counter: rec = {{ value: int }};\n",
                "fun[] update(counter[mux]: Counter): int = {{\n",
                "    {keyword} {{ counter.<|>lock(); }};\n",
                "    return 0;\n",
                "}};\n",
            ),
            keyword = keyword,
        );
        let labels = completion_labels_at_marker(&root, &uri, &source);
        assert!(
            labels.is_empty(),
            "{keyword} must not offer delayed mutex operations: {labels:?}"
        );
        fs::remove_dir_all(root).ok();
    }

    let (root, uri) = hosted_sample_package_root("completion_v3_channel_dfr");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    dfr { var value: int = channel[<|>rx]; };\n",
            "    return 0;\n",
            "};\n",
        ),
    );
    assert!(
        labels.is_empty(),
        "dfr must not offer delayed channel endpoint acquisition: {labels:?}"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_suppresses_completion_inside_comments_and_quotes() {
    let cases = [
        ("slash_line", "    // value.", Some(".")),
        ("slash_block", "    /* value.", Some(".")),
        ("backtick", "    ` value.", Some(".")),
        ("cooked_quote", "    var text = \"value.", Some(".")),
        ("raw_quote", "    var text = 'value.", Some(".")),
        (
            "endpoint_comment",
            "    var channel: chn[int]; channel[ // endpoint",
            None,
        ),
    ];

    for (label, tail, trigger) in cases {
        let (root, uri) = sample_package_root(&format!("completion_protected_{label}"));
        let source = format!("fun[] main(): int = {{\n{tail}");
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &source);

        let completion = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(508),
                method: "textDocument/completion".to_string(),
                params: Some(
                    serde_json::to_value(LspCompletionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: LspPosition {
                            line: 1,
                            character: tail.chars().count() as u32,
                        },
                        context: trigger.map(|trigger| LspCompletionContext {
                            trigger_kind: Some(2),
                            trigger_character: Some(trigger.to_string()),
                        }),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let completion: LspCompletionList =
            serde_json::from_value(completion.result.unwrap()).unwrap();

        assert!(
            completion.items.is_empty(),
            "protected {label} content leaked completion items: {:?}",
            completion.items
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_completes_only_channel_endpoints_in_channel_access() {
    let (root, uri) = hosted_sample_package_root("completion_v3_channel_endpoint");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    return channel[<|>\n",
        ),
    );
    assert_eq!(labels, vec!["tx".to_string(), "rx".to_string()]);
    fs::remove_dir_all(root).ok();

    let (root, uri) = hosted_sample_package_root("completion_v3_array_index");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] helper(): int = { return 1; };\n",
            "fun[] main(): int = {\n",
            "    var values: arr[int, 1] = {1};\n",
            "    return values[<|>\n",
        ),
    );
    assert!(labels.iter().any(|label| label == "helper"));
    assert!(!labels
        .iter()
        .any(|label| matches!(label.as_str(), "tx" | "rx")));
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_tracks_channel_receiver_completion_lifecycle() {
    let (root, uri) = hosted_sample_package_root("completion_v3_channel_receiver_lifecycle");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    4 | channel[tx];\n",
            "    var first: int = channel[rx];\n",
            "    return channel[<|>rx];\n",
            "};\n",
        ),
    );
    assert_eq!(labels, vec!["rx".to_string()]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_tracks_channel_lifecycle_by_resolved_binding() {
    let (root, uri) = hosted_sample_package_root("completion_v3_channel_lifecycle_shadow");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    var outer_value: int = channel[rx];\n",
            "    {\n",
            "        var channel: chn[int];\n",
            "        return channel[<|>rx];\n",
            "    };\n",
            "};\n",
        ),
    );
    assert_eq!(
        labels,
        vec!["tx".to_string(), "rx".to_string()],
        "an outer same-name receive must not consume a shadowing channel"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_fallback_completion_masks_comment_and_quote_payloads() {
    let (root, uri) = hosted_sample_package_root("completion_fallback_masked_non_code");
    let labels = completion_labels_at_marker(
        &root,
        &uri,
        concat!(
            "/*\n",
            "fun[] comment_routine(): int = { return 1; };\n",
            "typ CommentType: int;\n",
            "use comment_pkg: pkg = {\"comment-pkg\"};\n",
            "*/\n",
            "var quoted: str = 'payload\n",
            "fun[] quote_routine(): int = { return 2; };\n",
            "typ QuoteType: int;\n",
            "use quote_pkg: pkg = {\"quote-pkg\"};\n",
            "var quote_binding: int = 3;\n",
            "';\n",
            "fun[] main(): int = {\n",
            "    var real_binding: int = 7;\n",
            "    var broken: ;\n",
            "    return <|>real_binding;\n",
            "};\n",
        ),
    );
    assert!(labels.contains(&"real_binding".to_string()));
    for fake in [
        "comment_routine",
        "CommentType",
        "comment_pkg",
        "quote_routine",
        "QuoteType",
        "quote_pkg",
        "quote_binding",
    ] {
        assert!(
            !labels.iter().any(|label| label == fake),
            "fallback completion must ignore non-code declaration '{fake}': {labels:?}"
        );
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_gates_heap_binding_completion_by_model() {
    for (label, model, hosted, expected) in [
        ("core", "core", false, false),
        ("memo", "memo", false, true),
        ("hosted", "memo", true, true),
    ] {
        let (root, uri) = if hosted {
            hosted_sample_package_root(&format!("completion_v3_heap_{label}"))
        } else {
            sample_package_root(&format!("completion_v3_heap_{label}"))
        };
        if model == "core" {
            fs::write(
                root.join("build.fol"),
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                    "    return;\n",
                    "};\n",
                ),
            )
            .unwrap();
        }
        let labels = completion_labels_at_marker(&root, &uri, "fun[] main(): int = {\n    @<|>\n");
        assert_eq!(
            labels.iter().any(|label| label == "var"),
            expected,
            "@var completion availability drifted for {label}: {labels:?}"
        );
        assert!(
            labels.iter().all(|item| item == "var"),
            "the @ binding context should not leak unrelated completions: {labels:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}
