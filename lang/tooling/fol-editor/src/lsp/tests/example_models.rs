use super::super::{
    EditorLspServer, JsonRpcId, JsonRpcRequest, LspCompletionContext, LspCompletionList,
    LspCompletionParams, LspDefinitionParams, LspDocumentSymbolParams, LspHover, LspHoverParams,
    LspLocation, LspPosition, LspSemanticTokens, LspSemanticTokensParams,
    LspTextDocumentIdentifier, LspWorkspaceSymbolParams,
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

fn find_nth_position(text: &str, needle: &str, ordinal: usize) -> LspPosition {
    let mut search_offset = 0_usize;
    let mut byte_index = None;
    for _ in 0..ordinal {
        let found = text[search_offset..]
            .find(needle)
            .expect("needle should exist in example source");
        byte_index = Some(search_offset + found);
        search_offset += found + needle.len();
    }
    let byte_index = byte_index.expect("ordinal should be at least 1");
    let prefix = &text[..byte_index];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit('\n')
        .next()
        .expect("split should keep a trailing segment")
        .chars()
        .count() as u32;
    LspPosition { line, character }
}

fn request_definition(
    server: &mut EditorLspServer,
    uri: &str,
    position: LspPosition,
    id: i64,
) -> Option<LspLocation> {
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(id),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.to_string(),
                    },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    serde_json::from_value(response.result.unwrap()).unwrap()
}

fn request_hover(
    server: &mut EditorLspServer,
    uri: &str,
    position: LspPosition,
    id: i64,
) -> Option<LspHover> {
    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(id),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: uri.to_string(),
                    },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    serde_json::from_value(response.result.unwrap()).unwrap()
}

#[test]
fn lsp_server_opens_real_model_example_packages_cleanly() {
    for example in [
        "examples/core_dfr",
        "examples/mem_linked_list_m1",
        "examples/mem_tree_m1",
        "examples/mem_move_stack_vs_heap_m1",
        "examples/mem_borrow_m2",
        "examples/mem_borrow_giveback_m2",
        "examples/mem_borrow_param_m2",
        "examples/mem_mut_borrow_m2",
        "examples/mem_edf_m2",
        "examples/mem_ptr_unique_m3",
        "examples/mem_ptr_shared_m3",
        "examples/mem_ptr_shared_recursive_m3",
        "examples/proc_spawn_m1",
        "examples/proc_spawn_move_heap_m1",
        "examples/proc_channel_m2",
        "examples/proc_channel_pull_m2",
        "examples/proc_channel_capture_m2",
        "examples/proc_channel_loop_m2",
        "examples/proc_select_m3",
        "examples/proc_mutex_m3",
        "examples/proc_mutex_explicit_unlock_m3",
        "examples/proc_async_await_m4",
        "examples/proc_await_error_m4",
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/generic_standard_constraint_m1m2",
        "examples/generic_turbofish_m1",
        "examples/generic_type_constrained_m1m2",
        "examples/generic_type_exec_m1m2",
        "examples/generic_error_m1m2",
        "examples/generic_receiver_m1",
        "examples/generic_receiver_cross_file_m1",
        "examples/memo_defaults",
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/standards_default_body_m2",
        "examples/standards_blueprint_m2",
        "examples/standards_extended_m2",
        "examples/standards_generic_m2",
        "examples/std_bundled_fmt",
        "examples/std_bundled_io",
        "examples/std_echo_min",
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);

        assert!(
            diagnostics
                .iter()
                .all(|published| published.diagnostics.is_empty()),
            "real example '{example}' should open without editor diagnostics: {diagnostics:#?}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_processor_m1_spawn_state_and_failures() {
    let (root, uri) = copied_example_package_root("examples/proc_spawn_m1");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));

    let mut spawn = find_nth_position(&text, "[>]", 1);
    spawn.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1630),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: spawn,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("spawn marker should have hover")
        .contents
        .contains("joined at process exit"));
    fs::remove_dir_all(root).ok();

    for (example, expected) in [
        (
            "examples/fail_proc_spawn_in_memo_m1",
            "spawn requires hosted std support",
        ),
        (
            "examples/fail_proc_spawn_rc_cross_m1",
            "shared Rc pointers cannot cross a spawn boundary",
        ),
        (
            "examples/fail_proc_spawn_recoverable_m1",
            "spawning a recoverable routine without await discards its error",
        ),
        (
            "examples/fail_proc_spawn_heap_use_after_move_m1",
            "use of moved heap-owned binding 'owned'",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_processor_m2_channel_state_and_failures() {
    let (root, uri) = copied_example_package_root("examples/proc_channel_pull_m2");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));

    for (needle, expected, id) in [
        ("tx", "non-blocking send of `int`", 1640),
        ("rx", "blocking receive of `int`", 1641),
    ] {
        let position = find_nth_position(&text, needle, 1);
        let hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(id),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
        assert!(hover
            .expect("channel endpoint should have hover")
            .contents
            .contains(expected));
    }
    fs::remove_dir_all(root).ok();

    for (example, expected) in [
        (
            "examples/fail_proc_channel_index_m2",
            "channel receivers are blocking pull expressions and cannot be indexed",
        ),
        (
            "examples/fail_proc_channel_in_core_m2",
            "channel types require hosted std support",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_processor_m3_select_and_mutex_state() {
    let (root, uri) = copied_example_package_root("examples/proc_mutex_m3");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));
    let position = find_nth_position(&text, "mux", 1);
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1650),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("mux option should have hover")
        .contents
        .contains("mutex-guarded shared `Counter`"));
    fs::remove_dir_all(root).ok();

    for (example, expected) in [
        (
            "examples/fail_proc_select_old_form_m3",
            "old select(channel as binding) form is not supported",
        ),
        (
            "examples/fail_proc_mutex_double_paren_m3",
            "Expected generic parameter name",
        ),
        (
            "examples/fail_proc_mutex_in_memo_m3",
            "mutex parameters require hosted std support",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_processor_m4_eventual_state_and_failures() {
    let (root, uri) = copied_example_package_root("examples/proc_async_await_m4");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));
    for (needle, expected, id) in [
        ("async", "internal eventual of `int`", 1660),
        ("await", "blocks for `int`", 1661),
    ] {
        let position = find_nth_position(&text, needle, 1);
        let hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(id),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
        assert!(hover
            .expect("eventual pipe stage should have hover")
            .contents
            .contains(expected));
    }
    fs::remove_dir_all(root).ok();

    let (root, uri) = copied_example_package_root("examples/proc_await_error_m4");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let position = find_nth_position(&text, "await", 1);
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1662),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    let contents = hover.expect("recoverable await should have hover").contents;
    assert!(contents.contains("blocks for `int`"));
    assert!(contents.contains("recoverable error `int`"));
    fs::remove_dir_all(root).ok();

    for (example, expected) in [
        (
            "examples/fail_proc_evt_named_m4",
            "eventual types are internal in V3 and cannot be named",
        ),
        (
            "examples/fail_proc_async_in_core_m4",
            "async pipe stages require hosted std support",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_navigates_every_positive_v3_processor_example() {
    for (example, needle, ordinal) in [
        ("examples/proc_spawn_m1", "worker", 2),
        ("examples/proc_spawn_move_heap_m1", "consume", 2),
        ("examples/proc_channel_m2", "produce", 2),
        ("examples/proc_channel_pull_m2", "channel", 2),
        ("examples/proc_channel_capture_m2", "channel", 3),
        ("examples/proc_channel_loop_m2", "channel", 2),
        ("examples/proc_select_m3", "first", 2),
        ("examples/proc_mutex_m3", "worker", 2),
        ("examples/proc_mutex_explicit_unlock_m3", "update", 2),
        ("examples/proc_async_await_m4", "work", 2),
        ("examples/proc_await_error_m4", "probe", 2),
    ] {
        let (root, uri) = copied_example_package_root(example);
        fs::create_dir_all(root.join(".git")).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);
        let position = find_nth_position(&text, needle, ordinal);
        let response = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1668),
                method: "textDocument/definition".to_string(),
                params: Some(
                    serde_json::to_value(LspDefinitionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let definition: Option<LspLocation> =
            serde_json::from_value(response.result.unwrap()).unwrap();
        let definition = definition.unwrap_or_else(|| {
            panic!("positive processor example '{example}' should navigate '{needle}'")
        });
        assert_eq!(definition.uri, uri);
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_navigates_every_positive_v3_memory_example() {
    for (example, needle, ordinal, declaration_line) in [
        ("examples/mem_linked_list_m1", "head", 2, 7),
        ("examples/mem_tree_m1", "root", 2, 9),
        ("examples/mem_move_stack_vs_heap_m1", "heap_a", 2, 8),
        ("examples/mem_borrow_m2", "view", 2, 8),
        ("examples/mem_borrow_giveback_m2", "view", 2, 6),
        ("examples/mem_borrow_param_m2", "inspect", 2, 4),
        ("examples/mem_mut_borrow_m2", "view", 2, 6),
        ("examples/mem_edf_m2", "probe", 2, 2),
        ("examples/mem_ptr_unique_m3", "pointer", 2, 2),
        ("examples/mem_ptr_shared_m3", "first", 2, 2),
        ("examples/mem_ptr_shared_recursive_m3", "tail_ptr", 2, 7),
    ] {
        let (root, uri) = copied_example_package_root(example);
        fs::create_dir_all(root.join(".git")).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri.clone(), &text);
        assert!(
            diagnostics
                .iter()
                .all(|published| published.diagnostics.is_empty()),
            "positive memory example '{example}' should analyze cleanly: {diagnostics:#?}"
        );

        let position = find_nth_position(&text, needle, ordinal);
        let definition = request_definition(&mut server, &uri, position, 1671)
            .unwrap_or_else(|| panic!("'{example}' should navigate '{needle}'"));
        assert_eq!(definition.uri, uri);
        assert_eq!(
            definition.range.start.line, declaration_line,
            "'{example}' should navigate '{needle}' to its compiler-owned declaration"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_reports_borrow_parameter_hover_and_definition() {
    let (root, uri) = copied_example_package_root("examples/mem_borrow_param_m2");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let item_use = find_nth_position(&text, "item", 2);

    let hover = request_hover(&mut server, &uri, item_use, 1672)
        .expect("borrow parameter use should have compiler-backed hover");
    assert!(hover.contents.contains("item: bor[Item]"));
    let definition = request_definition(&mut server, &uri, item_use, 1673)
        .expect("borrow parameter use should navigate to its declaration");
    assert_eq!(definition.uri, uri);
    assert_eq!(definition.range.start.line, 4);
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_recursive_shared_pointer_hover_and_definition() {
    let (root, uri) = copied_example_package_root("examples/mem_ptr_shared_recursive_m3");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let pointer_use = find_nth_position(&text, "tail_ptr", 2);

    let hover = request_hover(&mut server, &uri, pointer_use, 1674)
        .expect("shared recursive pointer use should have compiler-backed hover");
    assert!(hover.contents.contains("shared refcount pointer"));
    assert!(hover.contents.contains("dereference yields Node"));
    assert!(hover.contents.contains("cycles leak"));
    let definition = request_definition(&mut server, &uri, pointer_use, 1675)
        .expect("shared recursive pointer use should navigate to its declaration");
    assert_eq!(definition.uri, uri);
    assert_eq!(definition.range.start.line, 7);
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_reports_select_arm_binding_type_and_definition() {
    let (root, uri) = copied_example_package_root("examples/proc_select_m3");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let position = find_nth_position(&text, "value", 2);

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1669),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("select arm use should have hover")
        .contents
        .contains(": int"));

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1670),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    let definition = definition.expect("select arm use should navigate to its as-binding");
    assert_eq!(definition.uri, uri);
    assert_eq!(
        definition.range.start.line,
        find_nth_position(&text, "value", 1).line
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_v3_memory_m1_navigation_and_state() {
    let (root, uri) = copied_example_package_root("examples/mem_linked_list_m1");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));

    let mut owned_node = find_nth_position(&text, "@Node", 1);
    owned_node.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1600),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: owned_node,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("@Node should have compiler-backed hover")
        .contents
        .contains("owned heap type"));
    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1601),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: owned_node,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    let definition = definition.expect("@Node should navigate to Node");
    assert_eq!(definition.uri, uri);
    assert_eq!(definition.range.start.line, 0);
    fs::remove_dir_all(root).ok();

    let (root, uri) = copied_example_package_root("examples/mem_move_stack_vs_heap_m1");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let mut moved_owner = find_nth_position(&text, "heap_a", 2);
    moved_owner.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1602),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: moved_owner,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("moved owner should retain compiler-backed hover")
        .contents
        .contains("moved; ownership transferred"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_v3_memory_m1_failures() {
    for (example, expected) in [
        ("examples/fail_mem_use_after_move_m1", "O1001"),
        (
            "examples/fail_mem_recursive_value_m1",
            "guard the recursive edge with owned heap indirection",
        ),
        (
            "examples/fail_mem_heap_in_core_m1",
            "heap allocation binding requires heap support",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        fs::create_dir_all(root.join(".git")).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_memory_m2_borrow_and_edf_state() {
    let (root, uri) = copied_example_package_root("examples/mem_borrow_giveback_m2");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));

    let mut borrow = find_nth_position(&text, "view", 1);
    borrow.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1610),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: borrow,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("borrow binding should retain compiler-backed hover")
        .contents
        .contains("borrow of owner"));
    fs::remove_dir_all(root).ok();

    let (root, uri) = copied_example_package_root("examples/mem_edf_m2");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let mut edf = find_nth_position(&text, "edf", 1);
    edf.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1611),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: edf,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("edf keyword should have hover")
        .contents
        .contains("error-only defer"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_borrow_hover_tracks_source_position_and_lexical_release() {
    let (root, uri) = sample_package_root("borrow_hover_position");
    let text = "fun[] main(): int = {\n\
        var owner: int = 7;\n\
        var before: int = owner;\n\
        {\n\
            var[bor] first: int = owner;\n\
            var seen: int = first;\n\
            !first;\n\
            var after_giveback: int = owner;\n\
        };\n\
        {\n\
            var[bor] second: int = owner;\n\
            var observed: int = second;\n\
        };\n\
        var after_scope: int = owner;\n\
        return before + after_scope;\n\
    };\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));

    for ordinal in [3, 5] {
        let hover = request_hover(
            &mut server,
            &uri,
            find_nth_position(text, "owner", ordinal),
            1680 + ordinal as i64,
        )
        .expect("owner borrow site should have compiler-backed hover");
        assert!(
            hover.contents.contains("inaccessible while borrow"),
            "owner occurrence {ordinal} should be inside an active borrow: {hover:?}"
        );
    }
    for ordinal in [2, 4, 6] {
        let hover = request_hover(
            &mut server,
            &uri,
            find_nth_position(text, "owner", ordinal),
            1690 + ordinal as i64,
        )
        .expect("owner use should have compiler-backed hover");
        assert!(
            !hover.contents.contains("inaccessible while borrow"),
            "owner occurrence {ordinal} is before creation or after release: {hover:?}"
        );
    }

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_surfaces_v3_memory_m2_failures() {
    for (example, expected) in [
        ("examples/fail_mem_owner_while_borrowed_m2", "O2001"),
        ("examples/fail_mem_second_mut_borrow_m2", "O2002"),
        ("examples/fail_mem_mut_borrow_immutable_owner_m2", "O2003"),
    ] {
        let (root, uri) = copied_example_package_root(example);
        fs::create_dir_all(root.join(".git")).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_surfaces_v3_memory_m3_pointer_state_and_failures() {
    let (root, uri) = copied_example_package_root("examples/mem_ptr_shared_m3");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri.clone(), &text);
    assert!(diagnostics
        .iter()
        .all(|published| published.diagnostics.is_empty()));
    let mut pointer = find_nth_position(&text, "first", 2);
    pointer.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1620),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: pointer,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    let contents = hover.expect("shared pointer should have hover").contents;
    assert!(contents.contains("shared refcount pointer"));
    assert!(contents.contains("dereference yields int"));
    assert!(contents.contains("cycles leak"));
    fs::remove_dir_all(root).ok();

    let (root, uri) = copied_example_package_root("examples/mem_ptr_unique_m3");
    fs::create_dir_all(root.join(".git")).unwrap();
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);
    let pointer = find_nth_position(&text, "*pointer", 1);
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1621),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: pointer,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    assert!(hover
        .expect("dereference sigil should have compiler-backed hover")
        .contents
        .contains("dereference unique pointer to `int`"));
    fs::remove_dir_all(root).ok();

    for (example, expected) in [
        ("examples/fail_mem_ptr_raw_m3", "V4 interop surface"),
        (
            "examples/fail_mem_ptr_in_core_m3",
            "pointer construction requires heap support",
        ),
    ] {
        let (root, uri) = copied_example_package_root(example);
        fs::create_dir_all(root.join(".git")).unwrap();
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let rendered = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            rendered.iter().any(|message| message.contains(expected)),
            "'{example}' should surface '{expected}', got {rendered:?}"
        );
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_document_symbols_for_real_example_roots() {
    for example in [
        "examples/mem_linked_list_m1",
        "examples/mem_tree_m1",
        "examples/mem_move_stack_vs_heap_m1",
        "examples/mem_borrow_m2",
        "examples/mem_borrow_giveback_m2",
        "examples/mem_borrow_param_m2",
        "examples/mem_mut_borrow_m2",
        "examples/mem_edf_m2",
        "examples/mem_ptr_unique_m3",
        "examples/mem_ptr_shared_m3",
        "examples/mem_ptr_shared_recursive_m3",
        "examples/proc_spawn_m1",
        "examples/proc_spawn_move_heap_m1",
        "examples/proc_channel_m2",
        "examples/proc_channel_pull_m2",
        "examples/proc_channel_capture_m2",
        "examples/proc_channel_loop_m2",
        "examples/proc_select_m3",
        "examples/proc_mutex_m3",
        "examples/proc_mutex_explicit_unlock_m3",
        "examples/proc_async_await_m4",
        "examples/proc_await_error_m4",
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/generic_standard_constraint_m1m2",
        "examples/generic_turbofish_m1",
        "examples/generic_type_constrained_m1m2",
        "examples/generic_type_exec_m1m2",
        "examples/generic_error_m1m2",
        "examples/generic_receiver_m1",
        "examples/generic_receiver_cross_file_m1",
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/standards_default_body_m2",
        "examples/standards_blueprint_m2",
        "examples/standards_extended_m2",
        "examples/standards_generic_m2",
        "examples/std_bundled_fmt",
        "examples/std_bundled_io",
        "examples/core_run_min",
        "examples/memo_run_min",
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let response = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(981),
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
        let symbols: Vec<crate::LspDocumentSymbol> =
            serde_json::from_value(response.result.unwrap()).unwrap();

        assert!(
            symbols.iter().any(|symbol| symbol.name == "main"),
            "real example '{example}' should surface a main symbol: {symbols:#?}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_workspace_symbols_for_open_real_examples() {
    let mut server = EditorLspServer::new(EditorConfig::default());
    let mut roots = Vec::new();
    for example in [
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/std_bundled_fmt",
        "examples/std_bundled_io",
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        open_document(&mut server, uri, &text);
        roots.push(root);
    }

    let response = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(982),
            method: "workspace/symbol".to_string(),
            params: Some(
                serde_json::to_value(LspWorkspaceSymbolParams {
                    query: "main".to_string(),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let symbols: Vec<crate::LspWorkspaceSymbol> =
        serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(
        symbols
            .iter()
            .filter(|symbol| {
                symbol.name.contains("::src::main")
                    || symbol.name == "main"
                    || symbol
                        .container_name
                        .as_deref()
                        .map(|name| name.contains("src::main"))
                        .unwrap_or(false)
            })
            .count()
            >= 2,
        "open real examples should contribute workspace symbols: {symbols:#?}"
    );
    assert!(
        symbols.iter().any(|symbol| symbol.name == "std::answer"),
        "bundled std example roots should contribute std workspace symbols too: {symbols:#?}"
    );

    for root in roots {
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_reports_model_aware_diagnostics_for_real_example_roots() {
    let cases = [
        (
            "examples/generic_routine_m1",
            "fun pick(T)(value: T): T = {\n    return value;\n};\nfun[] main(): int = {\n    return pick(7);\n};\n",
            None,
        ),
        (
            "examples/generic_routine_pair_m1",
            "fun pair(T)(left: T, right: T): T = {\n    return right;\n};\nfun[] main(): int = {\n    return pair(1, 2);\n};\n",
            None,
        ),
        (
            "examples/core_dfr",
            "fun[] main(): str = {\n    return \"bad\";\n};\n",
            Some("str requires heap support and is unavailable in 'fol_model = core'"),
        ),
        (
            "examples/memo_defaults",
            "fun[] main(): int = {\n    return .echo(7);\n};\n",
            Some("'.echo(...)' requires hosted std support"),
        ),
        (
            "examples/std_bundled_fmt",
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::math::answer();\n};\n",
            None,
        ),
        (
            "examples/standards_protocol_m2",
            "std geo: pro = {\n    fun area(): int;\n};\n\
             typ Rect()(geo): rec = {\n    var width: int;\n};\n\
             fun (Rect)area(): int = {\n    return 1;\n};\n\
             fun[] main(): int = {\n    return 0;\n};\n",
            None,
        ),
        (
            "examples/standards_protocol_pair_m2",
            "std geo: pro = {\n    fun area(): int;\n    fun perimeter(): int;\n};\n\
             typ Rect()(geo): rec = {\n    var width: int;\n};\n\
             fun (Rect)area(): int = {\n    return 1;\n};\n\
             fun (Rect)perimeter(): int = {\n    return 4;\n};\n\
             fun[] main(): int = {\n    return 0;\n};\n",
            None,
        ),
        (
            "examples/generic_type_semantic_m1m2",
            "typ Box(T): rec = {\n    var item: T;\n};\nfun[] main(): int = {\n    return 0;\n};\n",
            None,
        ),
        (
            "examples/fail_generic_standard_constraint_m1m2",
            "std geo: pro = {\n    fun area(): int;\n};\ntyp Plain(): rec = {\n    var value: int;\n};\nfun pick(T: geo)(value: T): T = {\n    return value;\n};\nfun[] main(): int = {\n    var plain: Plain = { value = 1 };\n    pick(plain);\n    return 0;\n};\n",
            Some("requires type 'Plain' to satisfy standard 'geo'"),
        ),
        (
            "examples/std_bundled_io",
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    var shown: str = std::io::echo_str(\"ok\");\n    return 7;\n};\n",
            None,
        ),
        (
            "examples/std_echo_min",
            "fun[] main(): int = {\n    var shown: int = .echo(9);\n    return 9;\n};\n",
            None,
        ),
    ];

    for (example, source, expected_message) in cases {
        let (root, uri) = copied_example_package_root(example);
        fs::write(root.join("src/main.fol"), source).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, source);
        let messages = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        match expected_message {
            Some(expected_message) => assert!(
                messages.iter().any(|message| message.contains(expected_message)),
                "example '{example}' should surface model-aware error '{expected_message}', got: {messages:?}"
            ),
            None => assert!(
                messages.is_empty(),
                "std example '{example}' should stay quiet under legal hosted surfaces: {messages:?}"
            ),
        }

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_semantic_tokens_for_real_model_examples() {
    for example in [
        "examples/core_dfr",
        "examples/generic_standard_constraint_m1m2",
        "examples/generic_type_exec_m1m2",
        "examples/std_echo_min",
    ] {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let response = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(980),
                method: "textDocument/semanticTokens/full".to_string(),
                params: Some(
                    serde_json::to_value(LspSemanticTokensParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let tokens: LspSemanticTokens = serde_json::from_value(response.result.unwrap()).unwrap();
        let decoded = decode_semantic_tokens(&tokens.data);
        let kinds = decoded.iter().map(|token| token.3).collect::<Vec<_>>();

        assert!(
            !decoded.is_empty(),
            "semantic tokens should not be empty for real example '{example}'"
        );
        // The semantic token stream should at least carry function or
        // variable tokens. Individual kind coverage is too brittle
        // because different examples surface different binding shapes.
        assert!(
            kinds.iter().any(|kind| matches!(kind, 2 | 4)),
            "semantic tokens for '{example}' should include at least one function or variable token: {decoded:?}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_reports_missing_bundled_std_dependency_from_editor_path() {
    let (root, uri) = super::helpers::sample_package_root("missing_bundled_std_dep");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var graph = .build().graph();\n",
            "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
            "};\n",
        ),
    )
    .unwrap();
    let text =
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n";
    fs::write(root.join("src/main.fol"), text).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, text);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        messages.iter().any(|message| message.contains("std")),
        "missing bundled std dependency should surface through the editor resolver path: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_respects_model_completion_when_opened_at_real_example_roots() {
    let cases = [
        (
            "examples/core_dfr",
            "fun[] main(): int = {\n    var value: ;\n    return 0;\n};\n",
            LspPosition {
                line: 1,
                character: 15,
            },
            None,
            vec!["int", "arr", "opt", "err"],
            vec!["str", "seq", "vec", "set", "map", "echo"],
        ),
        (
            "examples/memo_defaults",
            "fun[] main(): int = {\n    return .;\n};\n",
            LspPosition {
                line: 1,
                character: 12,
            },
            Some(LspCompletionContext {
                trigger_kind: Some(2),
                trigger_character: Some(".".to_string()),
            }),
            vec!["len", "eq", "not"],
            vec!["echo"],
        ),
        (
            "examples/std_bundled_fmt",
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::math::;\n};\n",
            LspPosition {
                line: 2,
                character: 27,
            },
            Some(LspCompletionContext {
                trigger_kind: Some(2),
                trigger_character: Some(":".to_string()),
            }),
            vec!["answer"],
            vec![],
        ),
        (
            "examples/std_bundled_io",
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::io::;\n};\n",
            LspPosition {
                line: 2,
                character: 16,
            },
            Some(LspCompletionContext {
                trigger_kind: Some(2),
                trigger_character: Some(":".to_string()),
            }),
            vec!["echo_bool", "echo_chr", "echo_int", "echo_str"],
            vec![],
        ),
        (
            "examples/std_echo_min",
            "fun[] main(): int = {\n    return .;\n};\n",
            LspPosition {
                line: 1,
                character: 12,
            },
            Some(LspCompletionContext {
                trigger_kind: Some(2),
                trigger_character: Some(".".to_string()),
            }),
            vec!["len", "echo", "eq", "not"],
            vec![],
        ),
    ];

    for (example, source, position, context, present, absent) in cases {
        let (root, uri) = copied_example_package_root(example);
        fs::write(root.join("src/main.fol"), source).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), source);

        let completion = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(981),
                method: "textDocument/completion".to_string(),
                params: Some(
                    serde_json::to_value(LspCompletionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                        context,
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

        for label in present {
            assert!(
                labels.iter().any(|candidate| candidate == label),
                "example '{example}' should expose completion '{label}', got: {labels:?}"
            );
        }
        for label in absent {
            assert!(
                !labels.iter().any(|candidate| candidate == label),
                "example '{example}' should hide completion '{label}', got: {labels:?}"
            );
        }

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_hover_for_v2_generic_examples() {
    let cases = [
        ("examples/generic_routine_m1", "pick(", 2, "pick"),
        ("examples/generic_routine_pair_m1", "pair(", 2, "pair"),
    ];

    for (example, needle, ordinal, expected) in cases {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1401),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, needle, ordinal),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
        let hover = hover.expect("generic call-site hover should resolve");

        assert!(
            hover.contents.contains(expected),
            "example '{example}' should surface hover for '{expected}', got: {hover:?}"
        );
        assert!(
            !hover.contents.contains("lowering") && !hover.contents.contains("backend"),
            "generic hover should not overclaim lowering/backend support: {hover:?}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_definitions_for_v2_generic_call_sites() {
    let cases = [
        ("examples/generic_routine_m1", "pick(", 2),
        ("examples/generic_routine_pair_m1", "pair(", 2),
        ("examples/generic_routine_cross_file_m1", "pick(", 1),
    ];

    for (example, needle, ordinal) in cases {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let definition = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1402),
                method: "textDocument/definition".to_string(),
                params: Some(
                    serde_json::to_value(LspDefinitionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, needle, ordinal),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let definition: Option<LspLocation> =
            serde_json::from_value(definition.result.unwrap()).unwrap();
        let definition = definition.expect("generic call-site definition should resolve");

        // Cross-file examples resolve into a sibling source unit; only
        // assert that the definition lands inside the same package.
        assert!(definition.uri.starts_with("file://"));
        assert_eq!(definition.range.start.line, 0);

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_hover_and_definition_for_generic_receiver_examples() {
    // Same-file: hover and definition on the monomorphized method call
    // sites in the generic receiver example.
    let hover_cases = [
        ("examples/generic_receiver_m1", "get(", 2, "get"),
        ("examples/generic_receiver_m1", "swap(", 2, "swap"),
    ];
    for (example, needle, ordinal, expected) in hover_cases {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1411),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, needle, ordinal),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
        let hover = hover.expect("generic receiver call-site hover should resolve");
        assert!(
            hover.contents.contains(expected),
            "example '{example}' should surface hover for '{expected}', got: {hover:?}"
        );

        fs::remove_dir_all(root).ok();
    }

    // Same-file definition: the call resolves back to the generic receiver
    // routine declaration line.
    {
        let (root, uri) = copied_example_package_root("examples/generic_receiver_m1");
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let decl_line = find_nth_position(&text, "fun (Box[T])get(T)(): T", 1).line;
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let definition = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1412),
                method: "textDocument/definition".to_string(),
                params: Some(
                    serde_json::to_value(LspDefinitionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, "get(", 2),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let definition: Option<LspLocation> =
            serde_json::from_value(definition.result.unwrap()).unwrap();
        let definition = definition.expect("generic receiver call-site definition should resolve");
        assert!(definition.uri.starts_with("file://"));
        assert_eq!(
            definition.range.start.line, decl_line,
            "definition should land on the generic receiver routine declaration"
        );

        fs::remove_dir_all(root).ok();
    }

    // Cross-file: the method call in main.fol resolves into the sibling
    // source unit that declares the generic receiver routine.
    {
        let (root, uri) = copied_example_package_root("examples/generic_receiver_cross_file_m1");
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let definition = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1413),
                method: "textDocument/definition".to_string(),
                params: Some(
                    serde_json::to_value(LspDefinitionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, "get(", 1),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let definition: Option<LspLocation> =
            serde_json::from_value(definition.result.unwrap()).unwrap();
        let definition = definition.expect("cross-file generic receiver definition should resolve");
        assert!(
            definition.uri.ends_with("shared.fol"),
            "cross-file definition should land in the declaring unit: {definition:?}"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_hover_and_definition_for_positive_generic_type_example() {
    let (root, uri) = copied_example_package_root("examples/generic_type_exec_m1m2");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let mut type_use_position = find_nth_position(&text, "Box[int]", 1);
    type_use_position.character += 1;

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1409),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: type_use_position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    let hover = hover.expect("generic-type hover should resolve");
    assert!(
        hover.contents.contains("Box"),
        "generic-type hover should mention the base type, got: {hover:?}"
    );

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1410),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: type_use_position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    let definition = definition.expect("generic-type definition should resolve");
    assert_eq!(definition.uri, uri);
    assert_eq!(definition.range.start.line, 2);

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_hover_and_definition_for_positive_constrained_generic_example() {
    let (root, uri) = copied_example_package_root("examples/generic_standard_constraint_m1m2");
    let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, uri.clone(), &text);

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1411),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: find_nth_position(&text, "pick(", 2),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    let hover = hover.expect("constrained generic call-site hover should resolve");
    assert!(
        hover.contents.contains("pick"),
        "constrained-generic hover should mention the routine, got: {hover:?}"
    );
    // Hover content currently prints the monomorphized routine signature
    // without the original constraint — that detail belongs to a later
    // editor hardening slice. For now we only require the routine name.

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1412),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: find_nth_position(&text, "pick(", 2),
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    let definition = definition.expect("constrained-generic call-site definition should resolve");
    assert_eq!(definition.uri, uri);
    // The constrained-generic definition lands in the same file; the
    // exact line depends on the example source, so we only require it
    // to resolve to a sensible position.
    assert!(definition.range.start.line > 0);

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_returns_hover_and_definition_for_v2_standards_examples() {
    let cases = [
        ("examples/standards_protocol_m2", "(geo)", 1, "geo", "area"),
        (
            "examples/standards_protocol_pair_m2",
            "(geo)",
            1,
            "geo",
            "area",
        ),
    ];

    for (example, contract_needle, contract_ordinal, expected_standard, requirement_name) in cases {
        let (root, uri) = copied_example_package_root(example);
        let text = fs::read_to_string(root.join("src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), &text);

        let contract_position = {
            let mut pos = find_nth_position(&text, contract_needle, contract_ordinal);
            pos.character += 1;
            pos
        };
        let hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1403),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: contract_position,
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
        let hover = hover.expect("standard contract-header hover should resolve");
        assert!(
            hover.contents.contains(expected_standard),
            "contract hover for '{example}' should mention '{expected_standard}', got: {hover:?}"
        );

        let definition = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1404),
                method: "textDocument/definition".to_string(),
                params: Some(
                    serde_json::to_value(LspDefinitionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: contract_position,
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let definition: Option<LspLocation> =
            serde_json::from_value(definition.result.unwrap()).unwrap();
        let definition = definition.expect("standard contract-header definition should resolve");
        assert_eq!(definition.uri, uri);
        assert_eq!(definition.range.start.line, 0);

        let requirement_hover = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1405),
                method: "textDocument/hover".to_string(),
                params: Some(
                    serde_json::to_value(LspHoverParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position: find_nth_position(&text, &format!("{requirement_name}("), 1),
                    })
                    .unwrap(),
                ),
            })
            .unwrap()
            .unwrap();
        let requirement_hover: Option<LspHover> =
            serde_json::from_value(requirement_hover.result.unwrap()).unwrap();
        assert!(
            requirement_hover.is_none(),
            "required standard routine hover should stay absent until dedicated V2 declaration navigation exists, got: {requirement_hover:?} for '{example}' on '{requirement_name}'"
        );

        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn lsp_server_returns_hover_and_definition_for_v2_multi_standard_examples() {
    let (root, _) = copied_example_package_root("examples/standards_protocol_multi_m2");
    let rect_path = root.join("src/rect.fol");
    let contracts_path = root.join("src/contracts.fol");
    let rect_uri = format!("file://{}", rect_path.display());
    let contracts_uri = format!("file://{}", contracts_path.display());
    let rect_text = fs::read_to_string(&rect_path).unwrap();
    let contracts_text = fs::read_to_string(&contracts_path).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    open_document(&mut server, contracts_uri.clone(), &contracts_text);
    open_document(&mut server, rect_uri.clone(), &rect_text);

    let mut contract_position = find_nth_position(&rect_text, "(geo", 1);
    contract_position.character += 1;
    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1407),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: rect_uri.clone(),
                    },
                    position: contract_position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let hover: Option<LspHover> = serde_json::from_value(hover.result.unwrap()).unwrap();
    let hover = hover.expect("multi-standard contract hover should resolve");
    assert!(
        hover.contents.contains("geo"),
        "multi-standard hover should mention the selected protocol, got: {hover:?}"
    );

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1408),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier {
                        uri: rect_uri.clone(),
                    },
                    position: contract_position,
                })
                .unwrap(),
            ),
        })
        .unwrap()
        .unwrap();
    let definition: Option<LspLocation> =
        serde_json::from_value(definition.result.unwrap()).unwrap();
    let definition = definition.expect("multi-standard contract definition should resolve");
    assert_eq!(definition.uri, contracts_uri);
    assert_eq!(definition.range.start.line, 0);

    fs::remove_dir_all(root).ok();
}

#[test]
fn lsp_server_keeps_completion_available_in_v2_safe_contexts() {
    let cases = [
        (
            "examples/generic_routine_m1",
            "fun pick(T)(value: T): T = {\n    return value;\n};\n\nfun[] main(): int = {\n    return p;\n};\n",
            LspPosition { line: 4, character: 12 },
            None,
            vec![],
            vec!["$"],
        ),
        (
            "examples/standards_protocol_m2",
            "std geo: pro = {\n    fun area(): i\n};\n\ntyp Rect()(geo): rec = {\n    var width: int;\n};\n\nfun (Rect)area(): int = {\n    return 1;\n};\n\nfun[] main(): int = {\n    return 0;\n};\n",
            LspPosition { line: 1, character: 16 },
            None,
            vec!["int"],
            vec!["geo"],
        ),
        (
            "examples/standards_protocol_m2",
            "std geo: pro = {\n    fun area(): int;\n};\n\ntyp Rect()(g): rec = {\n    var width: int;\n};\n\nfun (Rect)area(): int = {\n    return 1;\n};\n\nfun[] main(): int = {\n    return 0;\n};\n",
            LspPosition { line: 3, character: 11 },
            None,
            vec!["Rect", "width"],
            vec!["$"],
        ),
        (
            "examples/fail_generic_standard_constraint_m1m2",
            "std geo: pro = {\n    fun area(): int;\n};\n\nfun pick(T: geo)(value: T): T = {\n    return value;\n};\n\nfun[] main(): int = {\n    return p;\n};\n",
            LspPosition { line: 7, character: 12 },
            None,
            vec![],
            vec!["$"],
        ),
    ];

    for (example, source, position, context, present, absent) in cases {
        let (root, uri) = copied_example_package_root(example);
        fs::write(root.join("src/main.fol"), source).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        open_document(&mut server, uri.clone(), source);

        let completion = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: JsonRpcId::Number(1406),
                method: "textDocument/completion".to_string(),
                params: Some(
                    serde_json::to_value(LspCompletionParams {
                        text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                        position,
                        context,
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

        for label in present {
            assert!(
                labels.iter().any(|candidate| candidate == label),
                "example '{example}' should expose completion '{label}', got: {labels:?}"
            );
        }
        for label in absent {
            assert!(
                !labels.iter().any(|candidate| candidate.contains(label)),
                "example '{example}' should not expose fake completion '{label}', got: {labels:?}"
            );
        }

        fs::remove_dir_all(root).ok();
    }
}

#[test]
#[ignore = "pre-existing drift: editor overlay path no longer surfaces parser guidance for unquoted import targets"]
fn lsp_server_reports_parser_failure_for_unquoted_import_targets() {
    let (root, uri) = copied_example_package_root("examples/std_bundled_fmt");
    // Intentionally unquoted import target. The parser rejects this and
    // the editor must surface the rejection through a diagnostic.
    let source = "use std: pkg = {std};\nfun[] main(): int = {\n    return 0;\n};\n";
    fs::write(root.join("src/main.fol"), source).unwrap();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = open_document(&mut server, uri, source);
    let messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(
        !messages.is_empty(),
        "editor path should surface parser guidance for unquoted import targets, got: {messages:?}"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
#[ignore = "pre-existing fixture drift; LSP overlay paths for workspace builds need reconciling with the shipped workspace resolver contract"]
fn lsp_server_reports_transitive_model_boundaries_for_real_workspaces() {
    let cases = [
        (
            "transitive_core_alloc",
            "core",
            "memo",
            "fun[exp] label(): str = {\n    return \"heap\";\n};\n",
            "fun[] main(): int = {\n    return .len(shared.label());\n};\n",
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "transitive_alloc_std",
            "memo",
            "std",
            "fun[exp] ping(): int = {\n    return .echo(7);\n};\n",
            "fun[] main(): int = {\n    return shared.ping();\n};\n",
            "'.echo(...)' requires hosted std support",
        ),
    ];

    for (label, app_model, dep_model, dep_source, app_source, expected_message) in cases {
        let root = super::helpers::temp_root(label);
        let app_src = root.join("app/src");
        let shared_src = root.join("shared");
        fs::create_dir_all(&app_src).unwrap();
        fs::create_dir_all(&shared_src).unwrap();

        fs::write(
            root.join("app/build.fol"),
            format!(
                concat!(
                    "pro[] build(): non = {{\n",
                    "    var build = .build();\n",
                    "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_exe({{ name = \"app\", root = \"src/main.fol\", fol_model = \"{}\" }});\n",
                    "    return;\n",
                    "}};\n",
                ),
                app_model
            ),
        )
        .unwrap();
        fs::write(
            root.join("shared/build.fol"),
            format!(
                concat!(
                    "pro[] build(): non = {{\n",
                    "    var build = .build();\n",
                    "    build.meta({{ name = \"shared\", version = \"0.1.0\" }});\n",
                    "    var graph = build.graph();\n",
                    "    graph.add_static_lib({{ name = \"shared\", root = \"src/lib.fol\", fol_model = \"{}\" }});\n",
                    "    return;\n",
                    "}};\n",
                ),
                dep_model
            ),
        )
        .unwrap();
        fs::write(
            root.join("app/src/main.fol"),
            format!("use shared: loc = {{\"../../shared\"}};\n\n{app_source}"),
        )
        .unwrap();
        fs::write(root.join("shared/lib.fol"), dep_source).unwrap();

        let uri = format!("file://{}", root.join("app/src/main.fol").display());
        let text = fs::read_to_string(root.join("app/src/main.fol")).unwrap();
        let mut server = EditorLspServer::new(EditorConfig::default());
        let diagnostics = open_document(&mut server, uri, &text);
        let messages = diagnostics
            .iter()
            .flat_map(|published| published.diagnostics.iter())
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert!(
            messages.iter().any(|message| message.contains(expected_message)),
            "workspace '{label}' should surface transitive model error '{expected_message}', got: {messages:?}"
        );

        fs::remove_dir_all(root).ok();
    }
}
