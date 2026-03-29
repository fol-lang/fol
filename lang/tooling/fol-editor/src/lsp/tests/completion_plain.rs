use super::helpers::{open_document, sample_loc_workspace_root, sample_package_root};
use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspCompletionList, LspCompletionParams,
    LspPosition, LspTextDocumentIdentifier,
};
use crate::EditorConfig;
use std::fs;

#[test]
fn lsp_server_returns_current_package_top_level_completions() {
    let (root, uri) = sample_package_root("completion_top_level");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(32),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    assert!(completion.items.iter().any(|item| item.label == "helper"));
    assert!(
        completion
            .items
            .iter()
            .find(|item| item.label == "helper")
            .and_then(|item| item.detail.as_deref())
            == Some("routine")
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_import_alias_completions() {
    let (root, uri) = sample_loc_workspace_root("completion_import_alias");
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(33),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
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

    let completion: LspCompletionList =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    assert!(completion.items.iter().any(|item| item.label == "shared"));
    assert!(
        completion
            .items
            .iter()
            .find(|item| item.label == "shared")
            .and_then(|item| item.detail.as_deref())
            == Some("namespace")
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_prefers_nearer_symbols_when_completion_names_conflict() {
    let (root, uri) = sample_package_root("completion_shadowing");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    var helper: int = 9;\n    return helper;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(34),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 5,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    let helpers = completion
        .items
        .iter()
        .filter(|item| item.label == "helper")
        .collect::<Vec<_>>();
    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].detail.as_deref(), Some("binding"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_locks_plain_completion_to_local_package_and_import_alias_symbols() {
    let (root, uri) = sample_loc_workspace_root("completion_symbol_matrix");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] local_helper(): int = {\n    return 4;\n};\n\nfun[] main(total: int): int = {\n    var value: int = 7;\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(35),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 7,
                        character: 12,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    let labels = completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"value"));
    assert!(labels.contains(&"total"));
    assert!(labels.contains(&"local_helper"));
    assert!(labels.contains(&"shared"));
    assert!(!labels.contains(&"helper"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_plain_completion_free_of_child_namespace_noise() {
    let (root, uri) = sample_package_root("completion_plain_namespace_filter");
    fs::create_dir_all(root.join("src/api")).unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return \n};\n",
    )
    .unwrap();
    fs::write(
        root.join("src/api/lib.fol"),
        "fun[exp] child_helper(): int = {\n    return 9;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(47),
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
    assert!(labels.contains(&"helper".to_string()));
    assert!(!labels.contains(&"child_helper".to_string()));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_locks_completion_item_labels_kinds_and_order() {
    let (root, uri) = sample_loc_workspace_root("completion_item_shape_matrix");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nali[] LocalAlias = int;\n\ntyp[] LocalRec: rec = {\n    value: int;\n};\n\nfun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(total: int): int = {\n    var value: int = 9;\n    return \n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 8;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
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
                        line: 11,
                        character: 11,
                    },
                    context: None,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();

    let completion: LspCompletionList =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    let summary = completion
        .items
        .iter()
        .map(|item| {
            format!(
                "{}:{}:{}",
                item.label,
                item.kind,
                item.detail.clone().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        vec![
            "total:6:parameter",
            "value:6:binding",
            "helper:3:routine",
            "LocalAlias:22:type alias",
            "LocalRec:22:type",
            "shared:9:namespace",
        ]
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_prefers_authoritative_plain_completion_over_text_fallbacks() {
    let (root, uri) = sample_package_root("completion_authoritative_plain");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return \n};\n\nfun[] phantom(\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(!diagnostics.is_empty());

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(49),
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
    assert!(labels.contains(&"helper".to_string()));
    assert!(!labels.contains(&"phantom".to_string()));
    assert!(
        !labels.contains(&"main".to_string()),
        "authoritative completion should not degrade to text-scan top-level noise when resolver data exists"
    );

    fs::remove_dir_all(root).ok();
}
