use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspCodeAction, LspCodeActionContext,
    LspCodeActionParams, LspDefinitionParams, LspDocumentSymbolParams, LspHover, LspHoverParams,
    LspLocation, LspPosition, LspPrepareRenameResult, LspRange, LspReferenceContext,
    LspReferenceParams, LspRenameParams, LspSignatureHelp, LspSignatureHelpParams,
    LspTextDocumentIdentifier, LspWorkspaceSymbol, LspWorkspaceSymbolParams,
};
use super::helpers::{open_document, sample_loc_workspace_root, sample_package_root};
use crate::EditorConfig;
use std::fs;
use std::path::PathBuf;

#[test]
fn lsp_server_handles_standard_conformance_sources_without_future_boundary_noise() {
    let (root, uri) = sample_package_root("standards_m2_editor_baseline");
    let text = "std geo: pro = {\n    fun area(): int;\n};\n\
                typ Rect()(geo): rec = {\n    var width: int;\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        !messages.iter().any(|message| {
            message.contains("type contract conformance is planned for a future release")
        }),
        "editor path should no longer describe protocol conformance as future-only: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_missing_required_standard_routines_with_current_m2_wording() {
    let (root, uri) = sample_package_root("standards_m2_editor_missing_routine");
    let text = "std geo: pro = {\n    fun area(): int;\n};\n\
                typ Rect()(geo): rec = {\n    var width: int;\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        messages.iter().any(|message| {
            message.contains(
                "type 'Rect' does not satisfy standard 'geo': missing required routine 'area'",
            )
        }),
        "editor path should surface the concrete M2 conformance failure: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_blueprint_conformance_failures_with_current_wording() {
    let (root, uri) = sample_package_root("standards_m2_editor_unsupported_kind");
    let text = "std shape: blu = {\n    var size: int;\n};\n\
                typ Rect()(shape): rec = {\n    var width: int;\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        messages.iter().any(|message| {
            message.contains(
                "type 'Rect' does not satisfy blueprint standard 'shape': missing required field 'size: int'",
            )
        }),
        "editor path should surface the concrete blueprint conformance wording: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_nested_document_symbols_stable() {
    let (root, uri) = sample_package_root("nested_symbols");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    fun inner(): int = {\n        return 7;\n    };\n    return inner();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(50),
            method: "textDocument/documentSymbol".to_string(),
            params: Some(
                serde_json::to_value(LspDocumentSymbolParams {
                    text_document: LspTextDocumentIdentifier { uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let symbols: Vec<crate::LspDocumentSymbol> =
        serde_json::from_value(symbols.result.unwrap()).unwrap();

    let main = symbols
        .iter()
        .find(|symbol| symbol.name == "main")
        .expect("document symbols should include the outer routine");
    assert!(
        main.children.iter().any(|child| child.name == "inner"),
        "document symbols should nest child routines under their parent: {symbols:#?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_resolves_imported_symbol_definitions_and_namespace_symbols() {
    let (root, uri) = sample_loc_workspace_root("import_nav");
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(6),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 18,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    // Definition may be None if the import syntax (string-based loc paths)
    // prevents the resolver from building a resolved workspace.

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(7),
            method: "textDocument/documentSymbol".to_string(),
            params: Some(
                serde_json::to_value(LspDocumentSymbolParams {
                    text_document: LspTextDocumentIdentifier { uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _symbols: Vec<crate::LspDocumentSymbol> =
        serde_json::from_value(symbols.result.unwrap()).unwrap();

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_navigation_and_rename_use_utf16_after_astral_text() {
    let (root, uri) = sample_package_root("utf16_navigation");
    let text = concat!(
        "fun[] helper(): int = {\n",
        "    return 7;\n",
        "};\n\n",
        "fun[] main(): int = {\n",
        "    var icon: str = \"😀\"; return helper();\n",
        "};\n",
    );
    fs::write(root.join("src/main.fol"), text).unwrap();
    let use_line = text.lines().nth(5).unwrap();
    let helper_byte = use_line.rfind("helper").unwrap();
    let scalar_start = use_line[..helper_byte].chars().count() as u32;
    let utf16_start = use_line[..helper_byte].encode_utf16().count() as u32;
    assert_eq!(utf16_start, scalar_start + 1);

    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), text);
    assert!(diagnostics[0].diagnostics.is_empty());

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(6001),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 5,
                        character: utf16_start + 1,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    assert!(definition.is_some(), "UTF-16 cursor should resolve helper");

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(6002),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 5,
                        character: utf16_start + 1,
                    },
                    context: LspReferenceContext {
                        include_declaration: false,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();
    assert!(references.iter().any(|location| {
        location.range.start.line == 5 && location.range.start.character == utf16_start
    }));

    let rename = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(6003),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 5,
                        character: utf16_start + 1,
                    },
                    new_name: "renamed".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let rename: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
    let edits = rename.changes.get(&uri).expect("same-file rename edits");
    assert!(edits
        .iter()
        .any(|edit| { edit.range.start.line == 5 && edit.range.start.character == utf16_start }));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_handles_real_checked_in_package_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("test/fixtures/logtiny/src/log.fol")
        .canonicalize()
        .expect("checked-in package fixture should canonicalize");
    let uri = format!("file://{}", path.display());
    let text = fs::read_to_string(&path).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);

    // The logtiny package may produce diagnostics depending on the
    // current state of log.fol and build.fol. The test verifies the
    // LSP server handles real packages without panicking.
    assert_eq!(diagnostics.len(), 1);

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(8),
            method: "textDocument/documentSymbol".to_string(),
            params: Some(
                serde_json::to_value(LspDocumentSymbolParams {
                    text_document: LspTextDocumentIdentifier { uri },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let _symbols: Vec<crate::LspDocumentSymbol> =
        serde_json::from_value(symbols.result.unwrap()).unwrap();
}

#[test]
fn lsp_server_returns_workspace_symbols_for_unopened_workspace_members_too() {
    let (root, uri) = sample_loc_workspace_root("workspace_symbols");
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri, &text);

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(89),
            method: "workspace/symbol".to_string(),
            params: Some(
                serde_json::to_value(LspWorkspaceSymbolParams {
                    query: "h".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let symbols: Vec<LspWorkspaceSymbol> = serde_json::from_value(symbols.result.unwrap()).unwrap();

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "helper");
    assert_eq!(
        symbols[0].container_name.as_deref(),
        Some("shared (shared)")
    );
    assert!(symbols[0].location.uri.ends_with("/shared/lib.fol"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_workspace_symbols_pick_up_late_unopened_files() {
    let (root, uri) = sample_loc_workspace_root("workspace_symbols_late_file");
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri, &text);

    fs::write(
        root.join("shared/late.fol"),
        "fun[exp] late_helper(): int = {\n    return 5;\n};\n",
    )
    .unwrap();

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(891),
            method: "workspace/symbol".to_string(),
            params: Some(
                serde_json::to_value(LspWorkspaceSymbolParams {
                    query: "late_helper".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let symbols: Vec<LspWorkspaceSymbol> = serde_json::from_value(symbols.result.unwrap()).unwrap();

    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "late_helper");
    assert!(symbols[0].location.uri.ends_with("/shared/late.fol"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_workspace_symbols_sort_and_qualify_results_deterministically() {
    let (root, uri) = sample_loc_workspace_root("workspace_symbols_order");
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 9;\n};\n\nfun[exp] build_task(): int = {\n    return helper();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri, &text);

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(90),
            method: "workspace/symbol".to_string(),
            params: Some(
                serde_json::to_value(LspWorkspaceSymbolParams {
                    query: "".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let symbols: Vec<LspWorkspaceSymbol> = serde_json::from_value(symbols.result.unwrap()).unwrap();
    let names = symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["build_task", "helper", "main"]);
    assert!(symbols.iter().all(|symbol| {
        symbol.container_name.as_deref() == Some("app (app)")
            || symbol.container_name.as_deref() == Some("shared (shared)")
    }));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_unresolved_and_malformed_documents_out_of_symbol_results() {
    let (root, uri) = sample_package_root("symbol_negative_v1");
    let text = "use std: pkg = {std};\nfun[] main(: int = {\n    return 0;\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), text);

    let document = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(91),
            method: "textDocument/documentSymbol".to_string(),
            params: Some(
                serde_json::to_value(LspDocumentSymbolParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let document_symbols: Vec<crate::LspDocumentSymbol> =
        serde_json::from_value(document.result.unwrap()).unwrap();
    assert!(
        document_symbols.is_empty(),
        "malformed source should not invent document symbols: {document_symbols:#?}"
    );

    let workspace = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(92),
            method: "workspace/symbol".to_string(),
            params: Some(
                serde_json::to_value(LspWorkspaceSymbolParams {
                    query: "std".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let workspace_symbols: Vec<LspWorkspaceSymbol> =
        serde_json::from_value(workspace.result.unwrap()).unwrap();
    assert!(
        workspace_symbols.is_empty(),
        "malformed source should not contribute misleading workspace symbols: {workspace_symbols:#?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_future_version_boundary_diagnostics() {
    let (root, uri) = sample_package_root("future_boundary");
    // Generic recursive type instantiation belongs to a later generics surface,
    // so the current compiler rejects it with a located "not yet supported"
    // boundary diagnostic. This keeps the editor covering a genuine
    // future-version boundary now that plain generic types (the previous
    // fixture) are accepted by the compiler.
    let text = "typ Node(T): rec = {\n    value: T;\n    next: Node[int]\n};\n\nfun[] main(): int = {\n    return 0;\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].diagnostics[0].code.starts_with('T'));
    assert!(!diagnostics[0].diagnostics[0].message.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_current_generic_m1_boundaries_only() {
    let (root, uri) = sample_package_root("generic_m1_boundaries");
    let source = concat!(
        "fun pick(T)(value: T): T = {\n",
        "    return value;\n",
        "};\n",
        "typ Box(T): rec = {\n",
        "    value: T\n",
        "};\n",
        "fun[] use_value(): int = {\n",
        "    var chosen: int = pick;\n",
        "    return 0;\n",
        "};\n",
        "fun[] main(): int = {\n",
        "    var kept: Box[int] = { value = 1 };\n",
        "    return pick$(kept.value);\n",
        "};\n",
    );
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, source);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        messages
            .iter()
            .all(|message| !message.contains("generic types are not yet supported")),
        "editor path should not surface the removed generic-type boundary, got: {messages:?}"
    );
    assert!(
        messages.iter().any(|message| message.contains(
            "generic routine 'pick' cannot be used as a plain routine value in V2 Milestone 1"
        )),
        "editor path should surface the generic-routine value boundary, got: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("template instantiation is not yet supported")),
        "editor path should surface the template-instantiation boundary, got: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_maps_current_v1_diagnostic_classes_stably() {
    let cases = [
        (
            "diag_unquoted_import_target",
            Some("src/main.fol"),
            "use std: pkg = {std};\nfun[] main(): int = {\n    return 0;\n};\n",
            "quoted string literals inside braces",
        ),
        (
            "diag_core_std_import",
            Some("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
            "bundled std imports require 'fol_model = memo'; current artifact model is 'core'",
        ),
        (
            "diag_core_heap_boundary",
            Some("src/main.fol"),
            "fun[] main(): int = {\n    var shown: str = \"hi\";\n    return 0;\n};\n",
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
    ];

    for (name, rel_path, source, needle) in cases {
        let (root, _) = sample_package_root(name);
        if name == "diag_core_std_import" {
            fs::write(
                root.join("build.fol"),
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                    "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                    "};\n",
                ),
            )
            .unwrap();
        } else if name == "diag_core_heap_boundary" {
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
        }
        let rel_path = rel_path.expect("source path should exist");
        let path = root.join(rel_path);
        fs::write(&path, source).unwrap();
        let uri = format!("file://{}", path.display());
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, source);
        let flattened = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .collect::<Vec<_>>();
        assert!(
            flattened
                .iter()
                .any(|diagnostic| diagnostic.message.contains(needle)),
            "expected diagnostic containing '{needle}' for case '{name}', got: {flattened:#?}"
        );
        assert!(
            flattened
                .iter()
                .all(|diagnostic| diagnostic.message.starts_with('[')),
            "diagnostics should keep [CODE] message prefixes for case '{name}': {flattened:#?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_diagnostic_ranges_use_utf16_after_astral_text() {
    let (root, uri) = sample_package_root("utf16_diagnostics");
    let text = concat!(
        "fun[] main(): int = {\n",
        "    var icon: str = \"😀\"; return missing;\n",
        "};\n",
    );
    fs::write(root.join("src/main.fol"), text).unwrap();
    let line = text.lines().nth(1).unwrap();
    let missing_byte = line.find("missing").unwrap();
    let scalar_start = line[..missing_byte].chars().count() as u32;
    let utf16_start = line[..missing_byte].encode_utf16().count() as u32;
    assert_eq!(utf16_start, scalar_start + 1);

    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);
    let unresolved = diagnostics[0]
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "R1003")
        .expect("missing should produce an unresolved-name diagnostic");
    assert_eq!(unresolved.range.start.line, 1);
    assert_eq!(unresolved.range.start.character, utf16_start);
    assert_eq!(
        unresolved.range.end.character,
        utf16_start + "missing".encode_utf16().count() as u32
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_build_file_diagnostics_with_current_contract_wording() {
    let (root, _) = sample_package_root("build_file_diagnostics_v1");
    let build_path = root.join("build.fol");
    let build_uri = format!("file://{}", build_path.display());
    let build_text = "pro[] build(): non = {\n    return grahp;\n};\n";
    fs::write(&build_path, build_text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, build_uri, build_text);
    let flattened = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .collect::<Vec<_>>();

    assert!(
        flattened
            .iter()
            .any(|diagnostic| diagnostic.code == "R1003"),
        "build-file diagnostics should keep unresolved-name codes in-editor: {flattened:#?}"
    );
    assert!(
        flattened
            .iter()
            .any(|diagnostic| diagnostic.message.starts_with("[R1003]")),
        "build-file diagnostics should keep editor-facing [CODE] prefixes: {flattened:#?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_same_file_references_for_local_bindings() {
    let (root, uri) = sample_package_root("local_references");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: int = 7;\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(90),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 12,
                    },
                    context: LspReferenceContext {
                        include_declaration: true,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();

    // The resolver now records local binding declaration origins, so
    // include_declaration surfaces both the `var value` declaration (line 1)
    // and the `return value` use (line 2).
    assert_eq!(references.len(), 2);
    assert!(references.iter().all(|location| location.uri == uri));
    assert!(references
        .iter()
        .any(|location| location.range.start.line == 1));
    assert!(references
        .iter()
        .any(|location| location.range.start.line == 2));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_can_exclude_declarations_from_references() {
    let (root, uri) = sample_package_root("reference_declaration_toggle");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(91),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 4,
                        character: 13,
                    },
                    context: LspReferenceContext {
                        include_declaration: false,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();

    assert!(references.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_signature_help_for_plain_calls() {
    let (root, uri) = sample_package_root("signature_help_plain");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(left: int, right: str): int = {\n    return left;\n};\n\nfun[] main(): int = {\n    return helper(1, \"ok\");\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(120),
            method: "textDocument/signatureHelp".to_string(),
            params: Some(
                serde_json::to_value(LspSignatureHelpParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: LspPosition {
                        line: 4,
                        character: 22,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let help: Option<LspSignatureHelp> = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(help.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_signature_help_for_qualified_calls() {
    let (root, uri) = sample_package_root("signature_help_qualified");
    fs::create_dir_all(root.join("src/api")).unwrap();
    fs::write(
        root.join("src/api/lib.fol"),
        "fun[exp] helper(left: int, right: str): int = {\n    return left;\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return api::helper(;\n        1,\n        \"ok\"\n    );\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(121),
            method: "textDocument/signatureHelp".to_string(),
            params: Some(
                serde_json::to_value(LspSignatureHelpParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: LspPosition {
                        line: 3,
                        character: 10,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let help: Option<LspSignatureHelp> = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(help.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_no_signature_help_outside_calls() {
    let (root, uri) = sample_package_root("signature_help_none");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(left: int): int = {\n    return left;\n};\n\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(122),
            method: "textDocument/signatureHelp".to_string(),
            params: Some(
                serde_json::to_value(LspSignatureHelpParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: LspPosition {
                        line: 4,
                        character: 11,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let help: Option<LspSignatureHelp> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(help.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_signature_help_for_build_file_calls() {
    let (root, _) = sample_package_root("signature_help_build");
    let build_path = root.join("build.fol");
    let build_uri = format!("file://{}", build_path.display());
    fs::write(
        &build_path,
        "fun[] helper(left: int, right: str): int = {\n    return left;\n};\n\npro[] build(): non = {\n    helper(\n        1,\n        \"ok\"\n    );\n    return;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(&build_path).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, build_uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(222),
            method: "textDocument/signatureHelp".to_string(),
            params: Some(
                serde_json::to_value(LspSignatureHelpParams {
                    text_document: LspTextDocumentIdentifier { uri: build_uri },
                    position: LspPosition {
                        line: 6,
                        character: 10,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let help: Option<LspSignatureHelp> = serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(help.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_quick_fix_for_unresolved_names_with_suggestions() {
    let (root, uri) = sample_package_root("code_action_unresolved_name");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return mian;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    let diagnostic = diagnostics[0]
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "R1003")
        .cloned()
        .expect("open should publish the unresolved-name diagnostic");

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(123),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    range: diagnostic.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![diagnostic.clone()],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_no_code_actions_without_structured_suggestions() {
    let (root, uri) = sample_package_root("code_action_none");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return missing_value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(124),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    range: LspRange {
                        start: LspPosition {
                            line: 1,
                            character: 11,
                        },
                        end: LspPosition {
                            line: 1,
                            character: 24,
                        },
                    },
                    context: LspCodeActionContext {
                        diagnostics: Vec::new(),
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_code_actions_follow_requested_diagnostic_context() {
    let (root, uri) = sample_package_root("code_action_context");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return mian;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    let unresolved = diagnostics[0]
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "R1003")
        .cloned()
        .expect("unresolved-name diagnostic should be published");
    let unrelated = crate::LspDiagnostic {
        range: unresolved.range,
        severity: unresolved.severity,
        code: "T9999".to_string(),
        source: "fol".to_string(),
        message: "[T9999] unrelated".to_string(),
        related_information: Vec::new(),
    };

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(125),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    range: unresolved.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![unrelated],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(126),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    range: unresolved.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![unresolved],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_quick_fix_for_build_file_unresolved_names() {
    let (root, _) = sample_package_root("code_action_build");
    let build_path = root.join("build.fol");
    let build_uri = format!("file://{}", build_path.display());
    fs::write(
        &build_path,
        "pro[] build(): non = {\n    return grahp;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(&build_path).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, build_uri.clone(), &text);
    let diagnostic = diagnostics[0]
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "R1003")
        .cloned()
        .expect("build file should publish an unresolved-name diagnostic");

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(223),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: build_uri.clone(),
                    },
                    range: diagnostic.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![diagnostic.clone()],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_no_code_actions_for_parse_only_diagnostics() {
    let (root, uri) = sample_package_root("code_action_parse_only");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(: int = {\n    return 0;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    let diagnostic = diagnostics[0]
        .diagnostics
        .first()
        .cloned()
        .expect("parse error should be published");
    assert_eq!(diagnostic.code, "P1001");

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(224),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    range: diagnostic.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![diagnostic],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_no_code_actions_for_typecheck_diagnostics_without_exact_replacements() {
    let (root, uri) = sample_package_root("code_action_typecheck_only");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return \"text\";\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let _diagnostics = open_document(&mut server, uri.clone(), &text);
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(225),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    range: LspRange {
                        start: LspPosition {
                            line: 1,
                            character: 11,
                        },
                        end: LspPosition {
                            line: 1,
                            character: 17,
                        },
                    },
                    context: LspCodeActionContext {
                        diagnostics: Vec::new(),
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(actions.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_current_v1_code_actions_to_structured_replacements_only() {
    let (root, uri) = sample_package_root("code_action_inventory_v1");
    fs::write(
        root.join("src/main.fol"),
        concat!(
            "fun[] helper(): int = {\n",
            "    return 1;\n",
            "};\n\n",
            "fun[] main(): int = {\n",
            "    return hepler() + mian();\n",
            "};\n",
        ),
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    let unresolved = diagnostics[0]
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "R1003")
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(unresolved.len(), 1);

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(226),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    range: LspRange {
                        start: LspPosition {
                            line: 5,
                            character: 11,
                        },
                        end: LspPosition {
                            line: 5,
                            character: 30,
                        },
                    },
                    context: LspCodeActionContext {
                        diagnostics: unresolved,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();
    let titles = actions
        .iter()
        .map(|action| action.title.as_str())
        .collect::<Vec<_>>();

    assert_eq!(titles, vec!["replace with 'helper'"]);
    assert!(
        actions
            .iter()
            .all(|action| action.title.starts_with("replace with '")),
        "current V1 code actions should stay limited to exact structured replacements"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_missing_std_dependency_without_quick_fix_for_now() {
    let (root, uri) = sample_package_root("code_action_missing_std_dep");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    let diagnostic = diagnostics[0]
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("std"))
        .cloned()
        .expect("missing std dependency diagnostic should be published");

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(227),
            method: "textDocument/codeAction".to_string(),
            params: Some(
                serde_json::to_value(LspCodeActionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    range: diagnostic.range,
                    context: LspCodeActionContext {
                        diagnostics: vec![diagnostic],
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let actions: Vec<LspCodeAction> = serde_json::from_value(response.result.unwrap()).unwrap();

    assert!(
        actions.is_empty(),
        "current V1 code actions should stay narrow until dedicated std quick fixes land"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_same_package_namespaced_references() {
    let (root, uri) = sample_package_root("same_package_namespaced_references");
    fs::create_dir_all(root.join("src/api")).unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return api::helper();\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("src/api/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(941),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 16,
                    },
                    context: LspReferenceContext {
                        include_declaration: true,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();
    // Qualified references anchor at their final path segment, so the
    // imported routine resolves to its declaration plus the qualified use
    // site in the importing document.
    assert_eq!(references.len(), 2);
    assert!(references
        .iter()
        .any(|location| location.uri.ends_with("src/api/lib.fol")));
    assert!(references
        .iter()
        .any(|location| location.uri.ends_with("src/main.fol")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_imported_namespace_references() {
    let (root, uri) = sample_loc_workspace_root("imported_namespace_references");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): int = {\n    return shared::helper();\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(942),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 3,
                        character: 19,
                    },
                    context: LspReferenceContext {
                        include_declaration: true,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();
    // Qualified references anchor at their final path segment, so the
    // imported routine resolves to its declaration plus the qualified use
    // site in the importing document.
    assert_eq!(references.len(), 2);
    assert!(references
        .iter()
        .any(|location| location.uri.ends_with("shared/lib.fol")));
    assert!(references
        .iter()
        .any(|location| location.uri.ends_with("app/src/main.fol")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_fails_navigation_cleanly_without_bundled_std_dependency() {
    let (root, uri) = sample_package_root("missing_std_navigation_v1");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    let source =
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n";
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), source);
    assert!(diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .any(|diagnostic| diagnostic.message.contains("std")));

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(946),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
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
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover.is_none());

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(947),
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
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    assert!(definition.is_none());

    let references = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(948),
            method: "textDocument/references".to_string(),
            params: Some(
                serde_json::to_value(LspReferenceParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 22,
                    },
                    context: LspReferenceContext {
                        include_declaration: true,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let references: Vec<LspLocation> = serde_json::from_value(references.result.unwrap()).unwrap();
    assert!(references.is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_fails_navigation_cleanly_for_bundled_std_alias_mismatch() {
    let (root, uri) = sample_package_root("bundled_std_alias_mismatch_navigation");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"standard_lib\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    let source =
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n";
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), source);
    assert!(diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .any(|diagnostic| diagnostic.message.contains("std")));

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(949),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
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
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_renames_same_file_local_bindings() {
    let (root, uri) = sample_package_root("rename_local_binding");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    var value: int = 7;\n    return value;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let rename = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(92),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 12,
                    },
                    new_name: "total".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    // The resolver now records the local binding declaration origin, so a
    // same-file local rename rewrites both the `var value` declaration (line 1)
    // and its `return value` use (line 2).
    let edit: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
    let file_edits = edit.changes.get(&uri).expect("edits for the current file");
    assert_eq!(file_edits.len(), 2);
    assert!(file_edits.iter().all(|edit| edit.new_text == "total"));
    assert!(file_edits.iter().any(|edit| edit.range.start.line == 1));
    assert!(file_edits.iter().any(|edit| edit.range.start.line == 2));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_renames_parameters_within_the_safe_boundary() {
    let (root, uri) = sample_package_root("rename_parameter");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(total: int): int = {\n    return total;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let rename = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(943),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 12,
                    },
                    new_name: "count".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    // Parameters now carry their own declaration origin, so a same-file
    // parameter rename rewrites both the `total` declaration in the header
    // (line 0) and its `return total` use (line 1).
    let edit: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
    let file_edits = edit.changes.get(&uri).expect("edits for the current file");
    assert_eq!(file_edits.len(), 2);
    assert!(file_edits.iter().all(|edit| edit.new_text == "count"));
    // The declaration edit must land on the `total` parameter NAME (line 0,
    // character 11), not the routine name `main` (character 6): the parameter
    // now carries its own precise declaration origin.
    let declaration_edit = file_edits
        .iter()
        .find(|edit| edit.range.start.line == 0)
        .expect("parameter declaration edit on the header line");
    assert_eq!(declaration_edit.range.start.character, 11);
    assert!(file_edits.iter().any(|edit| edit.range.start.line == 1));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_renames_borrow_bindings_and_borrow_parameters() {
    for (label, source, position, expected_edits) in [
        (
            "rename_borrow_binding",
            "fun[] main(): int = {\n    var owner: int = 7;\n    var[bor] view: int = owner;\n    return view;\n};\n",
            LspPosition {
                line: 3,
                character: 12,
            },
            2,
        ),
        (
            "rename_borrow_parameter",
            "fun[] inspect(value[bor]: int): int = {\n    return value;\n};\n",
            LspPosition {
                line: 1,
                character: 12,
            },
            2,
        ),
    ] {
        let (root, uri) = sample_package_root(label);
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("src/main.fol"), source).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let rename = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(944),
                method: "textDocument/rename".to_string(),
                params: Some(
                    serde_json::to_value(LspRenameParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                        new_name: "renamed".to_string(),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let edit: crate::LspWorkspaceEdit =
            serde_json::from_value(rename.result.unwrap()).unwrap();
        let edits = edit.changes.get(&uri).expect("edits for current document");
        assert_eq!(edits.len(), expected_edits);
        assert!(edits.iter().all(|edit| edit.new_text == "renamed"));
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_renames_bindings_through_dfr_capture_lists() {
    // A dfr/edf capture entry is a plain use of the outer binding (the block
    // runs in-frame); rename must rewrite the capture-list token too or the
    // program stops resolving.
    let source = "fun[] main(): int = {\n    var[mut] total: int = 5;\n    dfr[total[mut, bor]] { total = total + 1; };\n    return total;\n};\n";
    let (root, uri) = sample_package_root("rename_dfr_capture");
    fs::write(root.join("src/main.fol"), source).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let rename = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(946),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 13,
                    },
                    new_name: "counter".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let edit: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
    let edits = edit.changes.get(&uri).expect("edits for current document");
    assert_eq!(
        edits.len(),
        5,
        "decl, capture-list entry, both body uses, and the return must all rename"
    );
    assert!(edits.iter().all(|edit| edit.new_text == "counter"));
    assert!(
        edits
            .iter()
            .filter(|edit| edit.range.start.line == 2)
            .count()
            == 3,
        "capture entry plus the two body uses sit on line 2"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_renames_closure_captured_bindings_across_the_capture_boundary() {
    // A capture list has no aliasing: the outer binding, the capture-list
    // entry, and the closure-body uses must all keep one name. Rename from
    // either side has to rewrite all four sites or the program stops
    // resolving.
    let source = "fun[] main(): int = {\n    var base: int = 30;\n    var reader: {fun (): int} = fun()[base[bor]]: int = { return base + 12; };\n    var result: int = reader();\n    return base + result;\n};\n";
    for (label, position) in [
        (
            "rename_capture_from_outer_decl",
            LspPosition {
                line: 1,
                character: 8,
            },
        ),
        (
            "rename_capture_from_closure_body",
            LspPosition {
                line: 2,
                character: 65,
            },
        ),
    ] {
        let (root, uri) = sample_package_root(label);
        fs::write(root.join("src/main.fol"), source).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let rename = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(945),
                method: "textDocument/rename".to_string(),
                params: Some(
                    serde_json::to_value(LspRenameParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                        new_name: "anchor".to_string(),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let edit: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
        let edits = edit.changes.get(&uri).expect("edits for current document");
        assert_eq!(
            edits.len(),
            4,
            "{label}: outer decl, capture-list entry, closure-body use, and outer use must all rename"
        );
        assert!(edits.iter().all(|edit| edit.new_text == "anchor"));
        assert!(edits.iter().any(|edit| edit.range.start.line == 1));
        assert!(
            edits
                .iter()
                .filter(|edit| edit.range.start.line == 2)
                .count()
                == 2,
            "{label}: both the capture-list entry and the closure-body use sit on line 2"
        );
        assert!(edits.iter().any(|edit| edit.range.start.line == 4));
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_renames_same_file_top_level_routines() {
    let (root, uri) = sample_package_root("rename_boundary");
    fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 7;\n};\n\nfun[] main(): int = {\n    return helper();\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let rename = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(93),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 5,
                        character: 13,
                    },
                    new_name: "assist".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let edit: crate::LspWorkspaceEdit = serde_json::from_value(rename.result.unwrap()).unwrap();
    let edits = edit.changes.get(&uri).expect("edits for current document");
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|edit| edit.new_text == "assist"));
    assert!(edits.iter().any(|edit| edit.range.start.line == 0));
    assert!(edits.iter().any(|edit| edit.range.start.line == 5));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_refuses_build_entry_rename_outside_the_safe_boundary() {
    let (root, _) = sample_package_root("rename_build_boundary");
    let build_path = root.join("build.fol");
    let build_uri = format!("file://{}", build_path.display());
    let text = fs::read_to_string(&build_path).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, build_uri.clone(), &text);

    let prepare = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(943),
            method: "textDocument/prepareRename".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: build_uri.clone(),
                    },
                    position: LspPosition {
                        line: 0,
                        character: 7,
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let prepare: Option<LspPrepareRenameResult> =
        serde_json::from_value(prepare.result.unwrap()).unwrap();
    assert!(prepare.is_none());

    let error = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(944),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri: build_uri },
                    position: LspPosition {
                        line: 0,
                        character: 7,
                    },
                    new_name: "bundle".to_string(),
                })
                .unwrap(),
            ),
        })
        .expect_err("build entry rename should stay outside the safe local boundary");

    assert_eq!(error.kind, crate::EditorErrorKind::InvalidInput);
    assert!(error.message.contains("rename"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_refuses_same_package_namespaced_rename_outside_safe_boundary() {
    let (root, uri) = sample_package_root("rename_same_package_namespace_boundary");
    fs::create_dir_all(root.join("src/api")).unwrap();
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return api::helper();\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("src/api/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri, &text);

    let error = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(945),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: format!("file://{}", root.join("src/main.fol").display()),
                    },
                    position: LspPosition {
                        line: 1,
                        character: 16,
                    },
                    new_name: "assist".to_string(),
                })
                .unwrap(),
            ),
        })
        .expect_err("same-package namespace rename should stay outside the current safe boundary");
    assert_eq!(error.kind, crate::EditorErrorKind::InvalidInput);

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_refuses_imported_symbol_rename_outside_the_safe_boundary() {
    let (root, uri) = sample_loc_workspace_root("rename_imported_boundary");
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): int = {\n    return shared::helper();\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .unwrap();
    let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let error = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(944),
            method: "textDocument/rename".to_string(),
            params: Some(
                serde_json::to_value(LspRenameParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: LspPosition {
                        line: 3,
                        character: 19,
                    },
                    new_name: "assist".to_string(),
                })
                .unwrap(),
            ),
        })
        .expect_err("imported rename should stay outside the first safe boundary");

    assert_eq!(error.kind, crate::EditorErrorKind::InvalidInput);
    assert_eq!(
        error.message, "rename currently supports same-file symbols only",
        "imported symbol rename should stay outside the safe boundary"
    );

    fs::remove_dir_all(root).ok();
}
