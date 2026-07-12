use fol_frontend::run_command_from_args_in_dir;
use std::fs;
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_editor_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}


fn write_rename_fixture_package(root: &PathBuf) {
    fs::create_dir_all(root.join("src")).expect("should create rename fixture src dir");
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"core\" });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write rename fixture build file");
    fs::write(
        root.join("src/main.fol"),
        concat!(
            "fun[] helper(): int = {\n",
            "    return 7;\n",
            "};\n",
            "\n",
            "fun[] main(): int = {\n",
            "    return helper();\n",
            "};\n",
        ),
    )
    .expect("should write rename fixture entry");
}


fn write_cross_package_rename_fixture(root: &PathBuf) {
    fs::create_dir_all(root.join("app/src")).expect("should create app root");
    fs::create_dir_all(root.join("shared")).expect("should create shared root");
    fs::write(root.join("fol.work.yaml"), "members:\n  - app\n")
        .expect("should write workspace manifest");
    fs::write(
        root.join("app/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"core\" });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write app build source");
    fs::write(
        root.join("app/src/main.fol"),
        concat!(
            "use shared: loc = {\"../../shared\"};\n",
            "\n",
            "fun[] main(): int = {\n",
            "    return shared::helper();\n",
            "};\n",
        ),
    )
    .expect("should write main source");
    fs::write(
        root.join("shared/lib.fol"),
        concat!(
            "fun[exp] helper(): int = {\n",
            "    return 7;\n",
            "};\n",
        ),
    )
    .expect("should write shared source");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should canonicalize")
}

fn lsp_format_text(path: &std::path::Path, text: &str) -> String {
    let canonical = path.canonicalize().expect("path should canonicalize");
    let uri = fol_editor::EditorDocumentUri::from_file_path(canonical.clone())
        .expect("uri should serialize");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri: uri.as_str().to_string(),
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.to_string(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    let response = server
        .handle_request(fol_editor::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: fol_editor::JsonRpcId::Number(1),
            method: "textDocument/formatting".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDocumentFormattingParams {
                    text_document: fol_editor::LspTextDocumentIdentifier {
                        uri: uri.as_str().to_string(),
                    },
                })
                .expect("formatting params should serialize"),
            ),
        })
        .expect("formatting request should succeed")
        .expect("formatting request should produce a response");
    let edits: Vec<fol_editor::LspTextEdit> =
        serde_json::from_value(response.result.expect("formatting result should exist"))
            .expect("formatting edits should deserialize");
    edits
        .into_iter()
        .next()
        .map(|edit| edit.new_text)
        .unwrap_or_else(|| text.to_string())
}

#[test]
fn editor_lsp_command_is_publicly_dispatchable() {
    let root = repo_root();
    let (_, result) =
        run_command_from_args_in_dir(["fol", "tool", "lsp"], root.join("xtra/logtiny"))
            .expect("editor lsp should dispatch");

    assert_eq!(result.command, "lsp");
    assert!(result.summary.contains("fol tool lsp"));
    assert!(result.summary.contains("diagnostics"));
    assert!(result.summary.contains("hover"));
    assert!(result.summary.contains("definition"));
    assert!(result.summary.contains("formatting"));
    assert!(result.summary.contains("references"));
    assert!(result.summary.contains("rename"));
    assert!(result.summary.contains("semantic tokens"));
    assert!(result.summary.contains("symbols"));
    assert!(result.summary.contains("completion"));
    assert!(result
        .summary
        .contains("features=diagnostics,hover,definition,formatting,codeAction,signatureHelp,references,rename,semanticTokens,symbols,completion"));
}

#[test]
fn editor_surface_stays_under_tool_not_a_parallel_editor_group() {
    let root = repo_root();
    let error = run_command_from_args_in_dir(["fol", "editor", "lsp"], root.join("xtra/logtiny"))
        .expect_err("`fol editor` should not exist as a parallel public surface");
    let json = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");
    assert!(json.contains("\"code\": \"F1004\""));
    assert!(json.contains("File not found"));
    assert!(json.contains("editor"));
}

#[test]
fn editor_tool_surface_rejects_placeholder_future_commands() {
    let root = repo_root();

    for command in [["fol", "tool", "semanticTokens"]] {
        let error = run_command_from_args_in_dir(command, root.join("xtra/logtiny"))
            .expect_err("unsupported future tool command should stay off the public surface");
        let json = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
            mode: fol_frontend::OutputMode::Json,
        })
        .render_error(&error)
        .expect("json render should succeed");

        assert!(json.contains("\"code\": \"F1001\""));
        assert!(json.contains("unknown tool subcommand"));
        assert!(json.contains(command[2]));
    }
}

#[test]
fn editor_format_command_dispatches_and_rewrites_files() {
    let root = temp_root("format");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    fs::write(&file, "fun[] main(): int = {\nreturn 0;\n};\n").expect("should write sample source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should dispatch");

    assert_eq!(result.command, "format");
    assert!(result.summary.contains("changed=true"));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "fun[] main(): int = {\n    return 0;\n};\n"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_command_reports_noop_for_already_formatted_files() {
    let root = temp_root("format_noop");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    let text = "fun[] main(): int = {\n    return 0;\n};\n";
    fs::write(&file, text).expect("should write sample source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should dispatch");

    assert_eq!(result.command, "format");
    assert!(result.summary.contains("changed=false"));
    assert_eq!(fs::read_to_string(&file).unwrap(), text);

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_command_rewrites_parse_broken_files_without_failing() {
    let root = temp_root("format_broken");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    fs::write(
        &file,
        "fun[] main(): int = {\nwhen(true) {\ncase(true) {\nreturn 7;\n}\n}\n",
    )
    .expect("should write broken source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should handle broken source");

    assert_eq!(result.command, "format");
    assert!(result.summary.contains("changed=true"));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "fun[] main(): int = {\n    when(true) {\n        case(true) {\n            return 7;\n        }\n    }\n"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_cli_matches_lsp_for_source_files() {
    let root = temp_root("format_parity_src");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    let source = "use shared: loc = {\"../shared\"};\n\nfun[] main(): int = {\nwhen(.eq(7, 7)) {\ncase(true) { return 7; }\n* { return 0; }\n}\n};\n";
    fs::write(&file, source).expect("should write sample source");

    let lsp_formatted = lsp_format_text(&file, source);
    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should dispatch");

    assert_eq!(result.command, "format");
    assert_eq!(fs::read_to_string(&file).unwrap(), lsp_formatted);

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_cli_matches_lsp_for_build_files() {
    let root = temp_root("format_parity_build");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("build.fol");
    let source = "pro[] build(): non = {\nvar graph = .graph();\nvar target = graph.standard_target();\nvar app = graph.add_exe({\nname = \"demo\",\nroot = \"src/main.fol\",\n});\ngraph.install(app);\n};\n";
    fs::write(&file, source).expect("should write build source");

    let lsp_formatted = lsp_format_text(&file, source);
    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should dispatch");

    assert_eq!(result.command, "format");
    assert_eq!(fs::read_to_string(&file).unwrap(), lsp_formatted);

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_file_commands_dispatch_against_real_fol_fixtures() {
    let root = repo_root();
    let fixture_path = root.join("test/apps/fixtures/record_flow/main.fol");
    let fixture = fixture_path.to_string_lossy();
    let fixture = fixture.as_ref();

    let (_, parse) = run_command_from_args_in_dir(["fol", "tool", "parse", fixture], &root)
        .expect("editor parse should dispatch");
    let (_, highlight) = run_command_from_args_in_dir(["fol", "tool", "highlight", fixture], &root)
        .expect("editor highlight should dispatch");
    let (_, symbols) = run_command_from_args_in_dir(["fol", "tool", "symbols", fixture], &root)
        .expect("editor symbols should dispatch");
    let (_, references) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "references",
            fixture,
            "--line",
            "5",
            "--character",
            "11",
        ],
        &root,
    )
    .expect("editor references should dispatch");
    let rename_root = temp_root("editor_dispatch_rename");
    write_rename_fixture_package(&rename_root);
    let rename_path = rename_root.join("src/main.fol");
    let (_, rename) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "rename",
            rename_path.to_string_lossy().as_ref(),
            "--line",
            "5",
            "--character",
            "11",
            "count",
        ],
        &rename_root,
    )
    .expect("editor rename should dispatch");
    fs::remove_dir_all(&rename_root).ok();
    let (_, semantic_tokens) = run_command_from_args_in_dir(
        ["fol", "tool", "semantic-tokens", fixture],
        &root,
    )
    .expect("editor semantic-tokens should dispatch");

    assert_eq!(parse.command, "parse");
    assert!(parse.summary.contains("grammar_bytes="));
    assert_eq!(highlight.command, "highlight");
    assert!(highlight.summary.contains("capture_count="));
    assert!(highlight.summary.contains("captures="));
    assert!(highlight.summary.contains("intrinsic_names="));
    assert_eq!(symbols.command, "symbols");
    assert!(symbols.summary.contains("query_snapshots=3"));
    assert_eq!(references.command, "references");
    assert!(references.summary.contains("reference_count="));
    assert!(references.summary.contains("include_declaration=true"));
    assert_eq!(rename.command, "rename");
    assert!(rename.summary.contains("edit_count="));
    assert!(rename.summary.contains("new_name=count"));
    assert_eq!(semantic_tokens.command, "semantic-tokens");
    assert!(semantic_tokens.summary.contains("token_count="));
    assert!(semantic_tokens.summary.contains("legend="));
}

#[test]
fn editor_build_file_commands_dispatch_through_public_cli() {
    let root = temp_root("build_cli_surface");
    write_rename_fixture_package(&root);
    let build = root.join("build.fol");

    // `graph` is declared on line 3 and used on lines 4 and 5; the resolver
    // now records local binding declaration origins, so references cover the
    // declaration plus both use sites.
    let (_, references) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "references",
            build.to_string_lossy().as_ref(),
            "--line",
            "4",
            "--character",
            "14",
        ],
        &root,
    )
    .expect("editor references should dispatch on build files");
    let (_, semantic_tokens) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "semantic-tokens",
            build.to_string_lossy().as_ref(),
        ],
        &root,
    )
    .expect("editor semantic-tokens should dispatch on build files");

    assert_eq!(references.command, "references");
    assert!(references.summary.contains("reference_count=3"));
    assert_eq!(semantic_tokens.command, "semantic-tokens");
    assert!(semantic_tokens.summary.contains("token_count="));

    let error = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "rename",
            build.to_string_lossy().as_ref(),
            "--line",
            "0",
            "--character",
            "7",
            "bundle",
        ],
        &root,
    )
    .expect_err("build entry rename should stay outside the first safe boundary");
    let rendered = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("stderr should be json");

    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], "F1004");
    assert!(diagnostic["message"]
        .as_str()
        .expect("message should be a string")
        .contains("does not support build entry symbols"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_references_command_can_exclude_declarations() {
    let root = repo_root();
    let fixture_path = root.join("test/apps/fixtures/record_flow/main.fol");
    let fixture = fixture_path.to_string_lossy();

    let (_, references) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "references",
            fixture.as_ref(),
            "--line",
            "5",
            "--character",
            "11",
            "--exclude-declaration",
        ],
        &root,
    )
    .expect("editor references should dispatch with declaration exclusion");

    assert_eq!(references.command, "references");
    assert!(references.summary.contains("include_declaration=false"));
}

#[test]
fn editor_rename_command_surfaces_safe_boundary_failures() {
    let root = temp_root("rename_multipackage_boundary");
    write_cross_package_rename_fixture(&root);
    let error = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "rename",
            root.join("app/src/main.fol").to_string_lossy().as_ref(),
            "--line",
            "3",
            "--character",
            "19",
            "entry",
        ],
        &root,
    )
    .expect_err("cross-package rename should stay outside the safe boundary");
    let json = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");

    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("rendered error should be json");
    assert_eq!(parsed["diagnostics"][0]["code"], "F1004");
    assert!(json.contains("multi-package symbols"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_rename_command_supports_same_package_namespaced_symbols() {
    let root = temp_root("rename_same_package_namespace_cli");
    write_rename_fixture_package(&root);
    // `api` is a package-root directory, so it is a namespace of the same
    // package; the qualified call site anchors rename at the final segment.
    fs::create_dir_all(root.join("api")).expect("should create api root");
    fs::write(
        root.join("src/main.fol"),
        concat!(
            "fun[] main(): int = {\n",
            "    return api::helper();\n",
            "};\n",
        ),
    )
    .expect("should write main source");
    fs::write(
        root.join("api/lib.fol"),
        concat!(
            "fun[exp] helper(): int = {\n",
            "    return 7;\n",
            "};\n",
        ),
    )
    .expect("should write namespaced helper");

    let (_, result) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "rename",
            root.join("src/main.fol").to_string_lossy().as_ref(),
            "--line",
            "1",
            "--character",
            "16",
            "assist",
        ],
        &root,
    )
    .expect("same-package namespace rename should stay inside the safe boundary");

    assert_eq!(result.command, "rename");
    assert!(result.summary.contains("edit_count=2"));
    assert!(result.summary.contains("touched_files=2"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_commands_respect_requested_output_mode() {
    let root = repo_root();
    let fixture_path = root.join("test/apps/fixtures/record_flow/main.fol");
    let fixture = fixture_path.to_string_lossy();
    let (output, result) = run_command_from_args_in_dir(
        ["fol", "tool", "--output", "plain", "parse", fixture.as_ref()],
        &root,
    )
    .expect("editor parse should support output mode");
    let rendered = output
        .render_command_summary(&result)
        .expect("plain output should render");

    assert!(rendered.contains("command: parse"));
    assert!(rendered.contains("summary: loaded"));
    assert!(rendered.contains("bytes="));
}

#[test]
fn editor_commands_do_not_require_workspace_discovery() {
    let root = temp_root("no_workspace");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    fs::write(&file, "fun[] main(): int = {\n    return 0\n};\n")
        .expect("should write sample source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "parse", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor parse should not need a workspace root");

    assert_eq!(result.command, "parse");
    assert!(result.summary.contains("path="));

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_command_does_not_require_workspace_discovery() {
    let root = temp_root("format_no_workspace");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    fs::write(&file, "fun[] main(): int = {\nreturn 0;\n};\n")
        .expect("should write sample source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", file.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should not need a workspace root");

    assert_eq!(result.command, "format");
    assert!(result.summary.contains("changed=true"));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "fun[] main(): int = {\n    return 0;\n};\n"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_format_command_does_not_mutate_unrelated_files() {
    let root = temp_root("format_isolated_write");
    fs::create_dir_all(&root).expect("should create temp root");
    let target = root.join("target.fol");
    let sibling = root.join("sibling.fol");
    fs::write(&target, "fun[] main(): int = {\nreturn 0;\n};\n")
        .expect("should write target source");
    fs::write(
        &sibling,
        "fun[] keep(): int = {\n    return 7\n}\n",
    )
    .expect("should write sibling source");

    let (_, result) = run_command_from_args_in_dir(
        ["fol", "tool", "format", target.to_string_lossy().as_ref()],
        &root,
    )
    .expect("editor format should dispatch");

    assert_eq!(result.command, "format");
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "fun[] main(): int = {\n    return 0;\n};\n"
    );
    assert_eq!(
        fs::read_to_string(&sibling).unwrap(),
        "fun[] keep(): int = {\n    return 7\n}\n"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_command_plain_output_stays_snapshot_stable_for_real_fixtures() {
    let root = repo_root();
    let fixture_path = root.join("xtra/logtiny/src/log.fol");
    let fixture = fixture_path.to_string_lossy();
    let (output, result) = run_command_from_args_in_dir(
        ["fol", "tool", "--output", "plain", "symbols", fixture.as_ref()],
        &root,
    )
    .expect("editor symbols should support plain output");
    let rendered = output
        .render_command_summary(&result)
        .expect("plain output should render");

    assert_eq!(
        rendered,
        format!(
            "command: symbols\nsummary: symbol query ready with 912 bytes (lines=64, path={}, query_snapshots=3, symbol_candidates=1)",
            fixture_path.display()
        )
    );
}

#[test]
fn editor_format_plain_output_stays_snapshot_stable() {
    let root = temp_root("format_plain_snapshot");
    fs::create_dir_all(&root).expect("should create temp root");
    let file = root.join("sample.fol");
    fs::write(&file, "fun[] main(): int = {\nreturn 0;\n};\n")
        .expect("should write sample source");

    let (output, result) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "--output",
            "plain",
            "format",
            file.to_string_lossy().as_ref(),
        ],
        &root,
    )
    .expect("editor format should support plain output");
    let rendered = output
        .render_command_summary(&result)
        .expect("plain output should render");

    assert_eq!(
        rendered,
        format!(
            "command: format\nsummary: formatted {} (changed=true, changed_lines=1, lines=3, path={}, style=hybrid-line)",
            file.display(),
            file.display()
        )
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_command_json_errors_keep_stable_shapes() {
    let error = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "--output",
            "json",
            "parse",
            "missing-editor-file.fol",
        ],
        repo_root(),
    )
    .expect_err("missing file should fail");
    let rendered = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("stderr should be json");
    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], "F1004");
    assert!(diagnostic["message"]
        .as_str()
        .expect("message should be a string")
        .contains("failed to read"));
    assert!(diagnostic["notes"].is_array());
}

#[test]
fn editor_lsp_reports_workspace_guidance_when_no_root_is_present() {
    let root = temp_root("missing_lsp_root");
    fs::create_dir_all(&root).expect("should create temp root");
    let error = run_command_from_args_in_dir(["fol", "tool", "lsp"], &root)
        .expect_err("editor lsp should require a discovered root");
    let rendered = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("stderr should be json");

    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], "F1002");
    let notes = diagnostic["notes"]
        .as_array()
        .expect("notes should be an array");
    assert!(notes.iter().any(|note| note
        .as_str()
        .unwrap_or("")
        .contains("start the editor inside a FOL package or workspace root")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn editor_rename_json_error_stays_snapshot_stable() {
    let root = temp_root("rename_json_error_snapshot");
    write_cross_package_rename_fixture(&root);
    let error = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "--output",
            "json",
            "rename",
            root.join("app/src/main.fol").to_string_lossy().as_ref(),
            "--line",
            "3",
            "--character",
            "19",
            "entry",
        ],
        &root,
    )
    .expect_err("cross-package rename should stay outside the safe boundary");
    let rendered = fol_frontend::FrontendOutput::new(fol_frontend::FrontendOutputConfig {
        mode: fol_frontend::OutputMode::Json,
    })
    .render_error(&error)
    .expect("json render should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("stderr should be json");

    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], "F1004");
    assert_eq!(
        diagnostic["message"],
        "rename currently refuses multi-package symbols"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn tree_generate_command_writes_bundle_layout() {
    let root = temp_root("tree_generate");
    let output = root.join("bundle");

    let (_, result) = run_command_from_args_in_dir(
        [
            "fol",
            "tool",
            "tree",
            "generate",
            output.to_string_lossy().as_ref(),
        ],
        repo_root(),
    )
    .expect("tree generate should dispatch");

    assert_eq!(result.command, "tree generate");
    assert!(output.join("grammar.js").is_file());
    assert!(output.join("queries/fol/highlights.scm").is_file());
    assert!(output.join("queries/fol/locals.scm").is_file());
    assert!(output.join("queries/fol/symbols.scm").is_file());
    assert!(output.join("test/corpus/declarations.txt").is_file());

    fs::remove_dir_all(root).ok();
}
