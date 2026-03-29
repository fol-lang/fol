use super::*;
use fol_editor::{LspDefinitionParams, LspHover, LspHoverParams, LspLocation};

fn strip_ansi(value: &str) -> String {
    let mut stripped = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }

        stripped.push(ch);
    }

    stripped
}

fn find_file_by_name(root: &std::path::Path, target_name: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_by_name(&path, target_name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some(target_name) {
            return Some(path);
        }
    }
    None
}

fn collect_rust_source_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(collect_rust_source_files(&path));
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            found.push(path);
        }
    }
    found
}

fn emitted_crate_root(output: &std::process::Output) -> std::path::PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let root = stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if let Some(tail) = plain.split("crate_root=").nth(1) {
                return tail.split(" binary=").next().map(str::trim).map(str::to_string);
            }
            if plain.contains("emitted-rust") {
                return plain.split_whitespace().last().map(str::to_string);
            }
            None
        })
        .expect("compile success should report a crate root");
    std::path::PathBuf::from(root)
}

fn built_binary_path(output: &std::process::Output) -> std::path::PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let binary = stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if let Some(tail) = plain.split("binary=").nth(1) {
                return Some(tail.trim().to_string());
            }
            if plain.contains("binary") {
                return plain.split_whitespace().last().map(str::to_string);
            }
            None
        })
        .expect("compile success should report a binary path");
    std::path::PathBuf::from(binary)
}

fn example_declares_bundled_std(root: &std::path::Path) -> bool {
    std::fs::read_to_string(root.join("build.fol"))
        .map(|source| source.contains("source = \"internal\"") && source.contains("target = \"standard\""))
        .unwrap_or(false)
}

fn run_example_compile(root: &std::path::Path, keep_build_dir: bool) -> std::process::Output {
    if example_declares_bundled_std(root) {
        if keep_build_dir {
            run_fol_with_store_in_dir(
                root,
                &repo_root().join("lang/library"),
                &["code", "build", "--keep-build-dir"],
            )
        } else {
            run_fol_with_store_in_dir(root, &repo_root().join("lang/library"), &["code", "build"])
        }
    } else if keep_build_dir {
        run_fol_in_dir(root, &["code", "build", "--keep-build-dir"])
    } else {
        run_fol_in_dir(root, &["code", "build"])
    }
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("should create destination directory");
    for entry in std::fs::read_dir(src).expect("should read source directory") {
        let entry = entry.expect("should read source entry");
        let file_type = entry.file_type().expect("should read source file type");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("should copy source file");
        }
    }
}

fn temp_example_root(example_path: &str) -> std::path::PathBuf {
    let source = repo_root().join(example_path);
    let temp_root = unique_temp_root(&format!("example_copy_{}", example_path.replace('/', "_")));
    let target = temp_root.join("workspace");
    copy_dir_all(&source, &target);
    target
}

fn materialize_local_bundled_std_alias_as(root: &std::path::Path, alias: &str) {
    let bundled_std_root =
        fol_package::available_bundled_std_root().expect("bundled std root should exist");
    copy_dir_all(&bundled_std_root, &root.join(".fol/pkg").join(alias));
    std::fs::write(root.join("fol.work.yaml"), "package_store_root: .fol/pkg\n")
        .expect("should write workspace package-store override");
}

fn materialize_local_bundled_std_alias(root: &std::path::Path) {
    materialize_local_bundled_std_alias_as(root, "std");
}

fn expected_runtime_import_for_model(model: &str) -> String {
    let runtime_module = match model {
        "memo" => "memo",
        other => other,
    };
    format!("use fol_runtime::{runtime_module} as rt;")
}

fn positive_runtime_model_examples() -> &'static [(&'static str, &'static str)] {
    &[
        ("examples/core_run_min", "core"),
        ("examples/core_blink_shape", "core"),
        ("examples/core_defer", "core"),
        ("examples/core_records", "core"),
        ("examples/core_surface_showcase", "core"),
        ("examples/memo_run_min", "memo"),
        ("examples/memo_defaults", "memo"),
        ("examples/memo_containers", "memo"),
        ("examples/memo_collections", "memo"),
        ("examples/memo_surface_showcase", "memo"),
        ("examples/std_bundled_fmt", "std"),
        ("examples/std_bundled_io", "std"),
        ("examples/std_explicit_pkg", "std"),
        ("examples/std_cli", "std"),
        ("examples/std_echo_min", "std"),
        ("examples/std_named_calls", "std"),
        ("examples/std_substrate_echo", "std"),
        ("examples/std_surface_showcase", "std"),
        ("examples/mixed_models_workspace", "std"),
    ]
}

#[test]
fn test_generic_routine_m1_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/generic_routine_m1");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text = std::fs::read_to_string(&main).expect("generic routine example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "generic routine M1 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        !build.status.success(),
        "generic routine M1 example should stop before lowering/backend: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
        "generic routine M1 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_generic_routine_pair_m1_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/generic_routine_pair_m1");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text =
        std::fs::read_to_string(&main).expect("generic pair routine example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "generic pair M1 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!build.status.success());
    assert!(
        combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
        "generic pair M1 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_generic_routine_cross_file_m1_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/generic_routine_cross_file_m1");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text = std::fs::read_to_string(&main)
        .expect("cross-file generic routine example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "cross-file generic M1 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_example_compile(&root, false);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!build.status.success());
    assert!(
        combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
        "cross-file generic M1 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_fail_generic_type_m1_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_generic_type_m1");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        !check.status.success(),
        "generic type M1 boundary example should fail semantic checking: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        combined.contains("generic types are not yet supported"),
        "generic type M1 boundary example should keep the explicit generic-type message: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_generic_misuse_m1_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_generic_misuse_m1");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        !check.status.success(),
        "generic misuse M1 example should fail semantic checking: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        combined.contains("generic routine constraints are not yet supported in V2 Milestone 1")
            || combined.contains("template instantiation is not yet supported"),
        "generic misuse M1 example should keep explicit misuse boundaries: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_generic_cross_file_m1_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_generic_cross_file_m1");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text =
        std::fs::read_to_string(&main).expect("cross-file generic misuse example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    let all_messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();
    assert!(
        all_messages
            .iter()
            .any(|message| message.contains("leaves generic parameter 'T' underconstrained")),
        "cross-file generic misuse example should surface the explicit underconstrained diagnostic in editor: {all_messages:#?}"
    );

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("leaves generic parameter 'T' underconstrained"),
        "cross-file generic misuse example should keep the explicit underconstrained boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_standards_protocol_m2_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/standards_protocol_m2");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text = std::fs::read_to_string(&main).expect("standards example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "standards M2 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        !build.status.success(),
        "standards M2 example should stop before lowering/backend: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
        "standards M2 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_standards_protocol_pair_m2_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/standards_protocol_pair_m2");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text =
        std::fs::read_to_string(&main).expect("multi-routine standards example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "multi-routine standards M2 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!build.status.success());
    assert!(
        combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
        "multi-routine standards M2 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_standards_protocol_multi_m2_example_opens_cleanly_but_stops_before_lowering() {
    let root = temp_example_root("examples/standards_protocol_multi_m2");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text =
        std::fs::read_to_string(&main).expect("multi-standard standards example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "multi-standard standards M2 example should stay editor-clean before lowering: {diagnostics:#?}"
    );

    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&build.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&build.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!build.status.success());
    assert!(
        combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
        "multi-standard standards M2 example should surface the explicit lowering boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_fail_standard_missing_routine_m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_standard_missing_routine_m2");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("missing required routine 'perimeter'"),
        "missing-routine standards example should keep the explicit conformance diagnostic: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_standard_signature_m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_standard_signature_m2");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("routine 'area' has incompatible signature"),
        "signature-mismatch standards example should keep the explicit conformance diagnostic: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_standard_import_ambiguity_m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_standard_import_ambiguity_m2");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text =
        std::fs::read_to_string(&main).expect("imported-standard ambiguity example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    let all_messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();
    assert!(
        all_messages
            .iter()
            .any(|message| message.contains("standard 'geo' is ambiguous in lexical scope")),
        "imported-standard ambiguity example should surface the resolver ambiguity in editor: {all_messages:#?}"
    );

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("standard 'geo' is ambiguous in lexical scope"),
        "imported-standard ambiguity example should keep the resolver ambiguity boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_standard_blueprint_m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_standard_blueprint_m2");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        !check.status.success(),
        "negative standards M2 example should fail semantic checking: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(
        combined.contains(
            "type 'Rect' claims unsupported standard 'shape'; only protocol standards are supported in V2 Milestone 2",
        ),
        "negative standards M2 example should keep the explicit unsupported-standard message: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_generic_standard_constraint_m1m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_generic_standard_constraint_m1m2");

    let main = root.join("src/main.fol");
    let uri = format!("file://{}", main.display());
    let text = std::fs::read_to_string(&main)
        .expect("generic-standard seam example source should load");
    let mut server = fol_editor::EditorLspServer::new(fol_editor::EditorConfig::default());
    let diagnostics = server
        .handle_notification(fol_editor::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(fol_editor::LspDidOpenTextDocumentParams {
                    text_document: fol_editor::LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.clone(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen should succeed");
    let all_messages = diagnostics
        .iter()
        .flat_map(|published| published.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();
    assert!(
        all_messages.iter().any(|message| {
            message.contains("generic routine constraints are not yet supported in V2 Milestone 1")
        }),
        "generic-standard seam example should surface the explicit M1 constraint boundary in editor: {all_messages:#?}"
    );

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("generic routine constraints are not yet supported in V2 Milestone 1"),
        "generic-standard seam example should keep the explicit M1 constraint boundary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_fail_standard_as_type_m2_example_rejects_cleanly() {
    let root = temp_example_root("examples/fail_standard_as_type_m2");

    let check = run_fol_in_dir(&root, &["code", "check"]);
    let stdout = strip_ansi(&String::from_utf8_lossy(&check.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&check.stderr));
    let combined = format!("{stdout}\n{stderr}");
    assert!(!check.status.success());
    assert!(
        combined.contains("standard 'geo' cannot be used as an ordinary type in V2 Milestone 2"),
        "negative standards-as-type example should keep the explicit unsupported-type message: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

fn init_git_repo(root: &std::path::Path) {
    for args in [
        vec!["init"],
        vec!["config", "user.name", "FOL"],
        vec!["config", "user.email", "fol@example.com"],
        vec!["add", "."],
        vec!["commit", "-m", "init"],
    ] {
        let status = std::process::Command::new("git")
            .args(&args)
            .current_dir(root)
            .status()
            .expect("should run git command");
        assert!(status.success(), "git {:?} should succeed", args);
    }
}

fn git_output(root: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("should run git command");
    assert!(output.status.success(), "git {:?} should succeed", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn create_git_remote_from_logtiny_fixture(root: &std::path::Path) {
    let source = repo_root().join("xtra/logtiny");
    copy_dir_all(&source, root);
    std::fs::remove_dir_all(root.join(".git")).ok();
    std::fs::remove_dir_all(root.join(".fol")).ok();
    init_git_repo(root);
    let rename = std::process::Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(root)
        .status()
        .expect("should rename default branch");
    assert!(rename.success(), "git branch -M main should succeed");
    let tag = std::process::Command::new("git")
        .args(["tag", "v0.1.0"])
        .current_dir(root)
        .status()
        .expect("should create initial tag");
    assert!(tag.success(), "git tag v0.1.0 should succeed");
}

fn run_fol_with_store_in_dir(
    dir: &std::path::Path,
    store_root: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_fol"))
        .args(["--package-store-root"])
        .arg(store_root)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("should run fol CLI with explicit package-store root")
}

fn write_formal_model_package(
    root: &std::path::Path,
    name: &str,
    fol_model: &str,
    source_name: &str,
    source: &str,
) {
    let (fol_model, std_dep) = if fol_model == "std" {
        (
            "memo",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
        )
    } else {
        (fol_model, "")
    };
    std::fs::create_dir_all(root.join("src")).expect("should create package source root");
    std::fs::write(
        root.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
                "{std_dep}",
                "    var graph = build.graph();\n",
                "    var lib = graph.add_static_lib({{ name = \"{name}\", root = \"src/{source_name}\", fol_model = \"{fol_model}\" }});\n",
                "    graph.install(lib);\n",
                "}};\n",
            ),
            name = name,
            source_name = source_name,
            fol_model = fol_model,
            std_dep = std_dep,
        ),
    )
    .expect("should write package build");
    std::fs::write(root.join("src").join(source_name), source).expect("should write package source");
}

fn write_model_app_package(
    root: &std::path::Path,
    name: &str,
    fol_model: &str,
    source: &str,
    add_run: bool,
) {
    let (fol_model, std_dep) = if fol_model == "std" {
        (
            "memo",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
        )
    } else {
        (fol_model, "")
    };
    std::fs::create_dir_all(root.join("src")).expect("should create app source root");
    let run_line = if add_run {
        "    graph.add_run(app);\n"
    } else {
        ""
    };
    std::fs::write(
        root.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
                "{std_dep}",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({{ name = \"{name}\", root = \"src/main.fol\", fol_model = \"{fol_model}\" }});\n",
                "    graph.install(app);\n",
                "{run_line}",
                "}};\n",
            ),
            name = name,
            fol_model = fol_model,
            std_dep = std_dep,
            run_line = run_line,
        ),
    )
    .expect("should write app build");
    std::fs::write(root.join("src/main.fol"), source).expect("should write app source");
}

#[test]
fn test_editor_file_commands_cover_build_fol_entry_files() {
    let parse = run_fol(&[
        "tool",
        "--output",
        "json",
        "parse",
        "xtra/logtiny/build.fol",
    ]);
    assert!(
        parse.status.success(),
        "build.fol parse should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&parse.stdout),
        String::from_utf8_lossy(&parse.stderr)
    );
    let parse_json = parse_cli_json(&parse);
    assert_eq!(parse_json["command"], "parse");
    assert!(parse_json["summary"]
        .as_str()
        .expect("parse summary should be a string")
        .contains("xtra/logtiny/build.fol"));

    let highlight = run_fol(&[
        "tool",
        "--output",
        "json",
        "highlight",
        "xtra/logtiny/build.fol",
    ]);
    assert!(
        highlight.status.success(),
        "build.fol highlight should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&highlight.stdout),
        String::from_utf8_lossy(&highlight.stderr)
    );
    let highlight_json = parse_cli_json(&highlight);
    assert_eq!(highlight_json["command"], "highlight");
    assert!(highlight_json["summary"]
        .as_str()
        .expect("highlight summary should be a string")
        .contains("capture_count="));
    assert!(highlight_json["summary"]
        .as_str()
        .expect("highlight summary should be a string")
        .contains("xtra/logtiny/build.fol"));

    let symbols = run_fol(&[
        "tool",
        "--output",
        "json",
        "symbols",
        "xtra/logtiny/build.fol",
    ]);
    assert!(
        symbols.status.success(),
        "build.fol symbols should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&symbols.stdout),
        String::from_utf8_lossy(&symbols.stderr)
    );
    let symbols_json = parse_cli_json(&symbols);
    assert_eq!(symbols_json["command"], "symbols");
    assert!(symbols_json["summary"]
        .as_str()
        .expect("symbols summary should be a string")
        .contains("query_snapshots="));
    assert!(symbols_json["summary"]
        .as_str()
        .expect("symbols summary should be a string")
        .contains("xtra/logtiny/build.fol"));
}

#[test]
fn test_editor_file_commands_cover_standards_m2_example_sources() {
    let parse = run_fol(&[
        "tool",
        "--output",
        "json",
        "parse",
        "examples/standards_protocol_m2/src/main.fol",
    ]);
    assert!(
        parse.status.success(),
        "standards M2 example parse should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&parse.stdout),
        String::from_utf8_lossy(&parse.stderr)
    );

    let highlight = run_fol(&[
        "tool",
        "--output",
        "json",
        "highlight",
        "examples/standards_protocol_m2/src/main.fol",
    ]);
    assert!(
        highlight.status.success(),
        "standards M2 example highlight should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&highlight.stdout),
        String::from_utf8_lossy(&highlight.stderr)
    );
    let highlight_json = parse_cli_json(&highlight);
    assert_eq!(highlight_json["command"], "highlight");
    assert!(highlight_json["summary"]
        .as_str()
        .expect("highlight summary should be a string")
        .contains("capture_count="));
    assert!(highlight_json["summary"]
        .as_str()
        .expect("highlight summary should be a string")
        .contains("examples/standards_protocol_m2/src/main.fol"));
}

#[test]
fn test_lsp_covers_build_fol_symbols_hover_definition_and_completion() {
    let temp_root = unique_temp_root("lsp_build_fol");
    std::fs::create_dir_all(temp_root.join("src")).expect("should create source root");
    let build_text = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({{ name = \"demo\", version = \"0.1.0\" }});\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    graph.\n",
        "};\n",
    );
    std::fs::write(temp_root.join("build.fol"), build_text).expect("should write build file");
    let uri = format!("file://{}", temp_root.join("build.fol").display());

    let mut server = EditorLspServer::new(EditorConfig::default());
    open_lsp_document(&mut server, uri.clone(), build_text);

    let symbols = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(1),
            method: "textDocument/documentSymbol".to_string(),
            params: Some(
                serde_json::to_value(LspDocumentSymbolParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                })
                .expect("documentSymbol params should serialize"),
            ),
        })
        .expect("documentSymbol request should succeed")
        .expect("documentSymbol should produce a response");
    let symbols: Vec<LspDocumentSymbol> =
        serde_json::from_value(symbols.result.expect("symbols should have a result"))
            .expect("document symbols should deserialize");

    assert!(
            !symbols.is_empty(),
            "build.fol document symbols should include the build routine after resolver processes build units"
        );
    assert!(
        symbols.iter().any(|s| s.name == "build"),
        "build.fol document symbols should include the 'build' entry routine"
    );

    let hover = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(2),
            method: "textDocument/hover".to_string(),
            params: Some(
                serde_json::to_value(LspHoverParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 8,
                    },
                })
                .expect("hover params should serialize"),
            ),
        })
        .expect("hover request should succeed")
        .expect("hover should produce a response");
    let _hover: Option<LspHover> =
        serde_json::from_value(hover.result.expect("hover should have a result"))
            .expect("hover result should deserialize");

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 1,
                        character: 8,
                    },
                })
                .expect("definition params should serialize"),
            ),
        })
        .expect("definition request should succeed")
        .expect("definition should produce a response");
    let _definition: Option<LspLocation> =
        serde_json::from_value(definition.result.expect("definition should have a result"))
            .expect("definition result should deserialize");

    let completion = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(4),
            method: "textDocument/completion".to_string(),
            params: Some(
                serde_json::to_value(LspCompletionParams {
                    text_document: LspTextDocumentIdentifier { uri: uri.clone() },
                    position: LspPosition {
                        line: 2,
                        character: 10,
                    },
                    context: None,
                })
                .expect("completion params should serialize"),
            ),
        })
        .expect("completion request should succeed")
        .expect("completion should produce a response");
    let completion: LspCompletionList =
        serde_json::from_value(completion.result.expect("completion should have a result"))
            .expect("completion result should deserialize");

    assert!(
        !completion.items.is_empty(),
        "build.fol completion should still return a non-empty list"
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_examples_tree_contains_discoverable_formal_packages() {
    for root in example_package_roots() {
        assert!(
            root.join("build.fol").is_file(),
            "missing build.fol in {}",
            root.display()
        );
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("example package name should be utf-8");
        let syntax =
            parse_directory_package_syntax(&root, display_name, PackageSourceKind::Package)
                .expect("formal example package syntax should parse");
        let discovered =
            infer_package_root(&syntax).expect("formal example package should be discoverable");
        assert_eq!(
            discovered,
            root.canonicalize()
                .expect("example package root should canonicalize")
        );
    }
}

#[test]
fn test_examples_build_files_parse_cleanly() {
    for root in example_package_roots() {
        let build = parse_package_build(&root.join("build.fol"))
            .expect("checked-in example build.fol should parse cleanly");
        assert_eq!(
            build.mode(),
            PackageBuildMode::ModernOnly,
            "example build should stay on the semantic build surface: {}",
            root.display()
        );
    }
}

#[test]
fn test_examples_formal_packages_keep_build_source_units_in_syntax() {
    for root in example_package_roots() {
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .expect("example package name should be utf-8");
        let syntax =
            parse_directory_package_syntax(&root, display_name, PackageSourceKind::Package)
                .expect("formal example package syntax should parse");

        assert_eq!(
            syntax
                .source_units
                .iter()
                .filter(|unit| unit.kind == fol_parser::ast::ParsedSourceUnitKind::Build)
                .count(),
            1,
            "expected exactly one build source unit in {}",
            root.display()
        );
    }
}

#[test]
fn test_build_fixture_local_root_package_builds_and_runs() {
    let root = build_fixture_root("exe_object_config");
    let build_source = std::fs::read_to_string(root.join("build.fol"))
        .expect("build fixture should keep a checked-in build file");
    assert!(
        build_source.starts_with("pro[] build(): non"),
        "fixture should exercise the new build entry: {}",
        build_source
    );

    let build = run_fol_in_dir(&root, &["code", "build", "--keep-build-dir"]);
    assert!(
        build.status.success(),
        "local-root build fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("built 1 workspace package(s)"),
        "local-root build fixture should report a build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = run_fol_in_dir(&root, &["code", "run"]);
    assert!(
        run.status.success(),
        "local-root build fixture should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("ran "),
        "local-root build fixture should report a run summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_build_fixture_named_bundle_step_builds_from_new_semantic_entry() {
    let root = build_fixture_root("hybrid_bundle_step");
    let app_root = root.join("app");
    let build_source = std::fs::read_to_string(app_root.join("build.fol"))
        .expect("bundle fixture should keep a checked-in build file");
    assert!(
        build_source.starts_with("pro[] build(): non"),
        "bundle fixture should exercise the new build entry: {}",
        build_source
    );

    let build = run_fol_in_dir(&app_root, &["code", "build"]);
    assert!(
            build.status.success(),
            "bundle fixture should build through the new semantic build route: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("built 1 workspace package(s)"),
        "bundle fixture should keep the routed build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let check = run_fol_in_dir(&app_root, &["code", "build", "--step", "bundle"]);
    assert!(
        check.status.success(),
        "bundle fixture should execute the named bundle step: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn test_build_fixture_nested_local_library_executes_default_run_route() {
    let root = build_fixture_root("run_step_chain");
    let build_source = std::fs::read_to_string(root.join("build.fol"))
        .expect("run-step fixture should keep a checked-in build file");
    assert!(
        build_source.starts_with("pro[] build(): non"),
        "run-step fixture should exercise the new build entry: {}",
        build_source
    );

    let run = run_fol_in_dir(&root, &["code", "run"]);
    assert!(
        run.status.success(),
        "nested local-library fixture should run successfully: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("ran "),
        "nested local-library fixture should report a run summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_build_fixture_second_local_package_runs_with_new_semantic_entry() {
    let root = build_fixture_root("pkg_dependency_run");
    let app_root = root.join("app");
    let build_source = std::fs::read_to_string(app_root.join("build.fol"))
        .expect("secondary run fixture should keep a checked-in build file");
    assert!(
        build_source.starts_with("pro[] build(): non"),
        "secondary run fixture should exercise the new build entry: {}",
        build_source
    );

    let run = run_fol_in_dir(&app_root, &["code", "run"]);
    assert!(
        run.status.success(),
        "secondary run fixture should run with the new semantic entry: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("ran "),
        "secondary run fixture should report a run summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_build_fixture_conditional_step_evaluates_when_condition() {
    let root = build_fixture_root("conditional_step");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path)
        .expect("conditional_step fixture should keep a build.fol");

    let request_release = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            optimize: Some(BuildOptimizeMode::ReleaseFast),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated_release = evaluate_build_source(&request_release, &build_path, &source)
        .expect("conditional_step should evaluate with release-fast")
        .expect("release-fast evaluation should produce operations");
    assert!(
        evaluated_release
            .result
            .graph
            .steps()
            .iter()
            .any(|s| s.name == "strip"),
        "release-fast should add the strip step"
    );

    let request_debug = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated_debug = evaluate_build_source(&request_debug, &build_path, &source)
        .expect("conditional_step should evaluate without optimize")
        .expect("debug evaluation should produce operations");
    assert!(
        !evaluated_debug
            .result
            .graph
            .steps()
            .iter()
            .any(|s| s.name == "strip"),
        "debug build should not add the strip step"
    );
}

#[test]
fn test_build_fixture_helper_routine_evaluates_correctly() {
    let root = build_fixture_root("helper_routine");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path)
        .expect("helper_routine fixture should keep a build.fol");

    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("helper_routine should evaluate")
        .expect("helper_routine should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    assert!(
        artifacts.iter().any(|a| a.name == "core"),
        "helper_routine should produce a core static lib"
    );
    assert!(
        artifacts.iter().any(|a| a.name == "io"),
        "helper_routine should produce an io static lib"
    );
    assert!(
        artifacts.iter().any(|a| a.name == "app"),
        "helper_routine should produce an app executable"
    );
}

#[test]
fn test_build_fixture_loop_libs_produces_multiple_artifacts() {
    let root = build_fixture_root("loop_libs");
    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("loop_libs fixture should keep a build.fol");

    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("loop_libs should evaluate")
        .expect("loop_libs should produce operations for each iteration");

    let artifacts = evaluated.result.graph.artifacts();
    assert!(
        artifacts.iter().any(|a| a.name == "core"),
        "loop should produce core artifact"
    );
    assert!(
        artifacts.iter().any(|a| a.name == "io"),
        "loop should produce io artifact"
    );
    assert!(
        artifacts.iter().any(|a| a.name == "utils"),
        "loop should produce utils artifact"
    );
}

#[test]
fn test_build_fixture_d_options_accepts_option_overrides() {
    let root = build_fixture_root("d_options");
    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("d_options fixture should keep a build.fol");

    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("d_options should evaluate")
        .expect("d_options should produce operations");

    let options = evaluated.result.graph.options();
    assert!(
        options.iter().any(|o| o.name == "root"),
        "d_options should declare the root user option"
    );
}

#[test]
fn test_build_fixture_mem_model_supports_string_values() {
    let root = build_fixture_root("model_memo_str");

    let build = run_fol_in_dir(&root, &["code", "build"]);
    assert!(
        build.status.success(),
        "memo string fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("built 1 workspace package(s)"),
        "memo string fixture should report a build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
            String::from_utf8_lossy(&build.stdout).contains("capability_mode=memo"),
            "memo string fixture should surface its capability mode in the build summary: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
}

#[test]
fn test_build_fixture_mem_model_supports_sequences() {
    let root = build_fixture_root("model_memo_seq");

    let build = run_fol_in_dir(&root, &["code", "build"]);
    assert!(
        build.status.success(),
        "memo sequence fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("built 1 workspace package(s)"),
        "memo sequence fixture should report a build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_build_fixture_mem_model_supports_full_heap_surface() {
    let root = build_fixture_root("model_memo_surface_full");

    let build = run_fol_in_dir(&root, &["code", "build", "--keep-build-dir"]);
    assert!(
        build.status.success(),
        "memo full-surface fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("capability_mode=memo"),
        "memo full-surface fixture should surface its model in the build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = find_file_by_name(&root.join(".fol/build"), "main.rs")
        .expect("memo full-surface fixture should emit main.rs");
    let emitted = std::fs::read_to_string(&generated).expect("generated main should load");
    assert!(emitted.contains("use fol_runtime::memo as rt;"));
}

#[test]
fn test_build_fixture_std_model_runs_echo_programs() {
    let root = build_fixture_root("model_std_echo");

    let build = run_fol_with_store_in_dir(
        &root,
        &repo_root().join("lang/library"),
        &["code", "build", "--keep-build-dir"],
    );
    assert!(
        build.status.success(),
        "std echo fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    let binary = build_stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if plain.contains("binary") {
                plain.split_whitespace().last().map(str::to_string)
            } else {
                None
            }
        })
        .expect("std echo build should report a binary path")
        .trim()
        .to_string();

    let run = Command::new(&binary)
        .output()
        .expect("std echo fixture binary should execute");
    assert!(
        run.status.success(),
        "std echo fixture binary should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("std-ready"),
        "std echo fixture should print through the hosted std runtime path: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_build_fixture_std_model_supports_hosted_mem_surfaces() {
    let root = build_fixture_root("model_std_hosted_alloc");

    let build = run_fol_with_store_in_dir(
        &root,
        &repo_root().join("lang/library"),
        &["code", "build", "--keep-build-dir"],
    );
    assert!(
        build.status.success(),
        "std hosted-memo fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    let binary = build_stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if plain.contains("binary") {
                plain.split_whitespace().last().map(str::to_string)
            } else {
                None
            }
        })
        .expect("std hosted-memo build should report a binary path")
        .trim()
        .to_string();

    let run = Command::new(&binary)
        .output()
        .expect("std hosted-memo fixture binary should execute");
    assert!(
        run.status.success(),
        "std hosted-memo fixture binary should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("std-ready"),
        "std hosted-memo fixture should print through std runtime: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_build_fixture_core_model_rejects_heap_backed_surfaces() {
    let root = build_fixture_root("model_core_heap_reject");

    let build = run_fol_in_dir(&root, &["code", "build"]);
    assert!(
        !build.status.success(),
        "core heap fixture should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        stderr.contains("seq[...] requires heap support and is unavailable in 'fol_model = core'"),
        "core heap fixture should keep the model diagnostic: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_build_fixture_core_model_supports_full_foundation_surface() {
    let root = build_fixture_root("model_core_surface_full");
    let build = run_fol_in_dir(&root, &["code", "build", "--keep-build-dir"]);
    assert!(
        build.status.success(),
        "core full-surface fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = find_file_by_name(&root.join(".fol/build"), "main.rs")
        .expect("core full-surface fixture should emit main.rs");
    let emitted = std::fs::read_to_string(&generated).expect("generated main should load");
    assert!(emitted.contains("use fol_runtime::core as rt;"));
}

#[test]
fn test_build_fixture_core_model_supports_foundation_surface() {
    let root = build_fixture_root("model_core_foundation");

    let build = run_fol_in_dir(&root, &["code", "build"]);
    assert!(
        build.status.success(),
        "core foundation fixture should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        String::from_utf8_lossy(&build.stdout).contains("built 1 workspace package(s)"),
        "core foundation fixture should report a build summary: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
            String::from_utf8_lossy(&build.stdout).contains("capability_mode=core"),
            "core foundation fixture should surface its fol_model in the build summary: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
}

#[test]
fn test_build_fixture_mixed_models_workspace_keeps_per_artifact_models() {
    let root = build_fixture_root("mixed_models_workspace");
    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("mixed-model fixture should keep a build.fol");

    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("mixed-model fixture should evaluate")
        .expect("mixed-model fixture should produce a graph");

    let artifacts = &evaluated.evaluated.artifacts;
    let core = artifacts
        .iter()
        .find(|a| a.name == "corelib")
        .expect("corelib");
    let mem = artifacts
        .iter()
        .find(|a| a.name == "memolib")
        .expect("memolib");
    let tool = artifacts.iter().find(|a| a.name == "tool").expect("tool");

    assert_eq!(
        core.fol_model,
        fol_package::build_artifact::BuildArtifactFolModel::Core
    );
    assert_eq!(
        mem.fol_model,
        fol_package::build_artifact::BuildArtifactFolModel::Memo
    );
    assert_eq!(tool.fol_model, fol_package::build_artifact::BuildArtifactFolModel::Memo);

    let run = run_fol_with_store_in_dir(&root, &repo_root().join("lang/library"), &["code", "run"]);
    assert!(
        run.status.success(),
        "mixed-model fixture should still run its std tool: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("ran "));
}

#[test]
fn test_core_artifact_accepts_transitive_core_pkg_dependency() {
    let temp_root = unique_temp_root("model_core_dep_core");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("corelib"),
        "corelib",
        "core",
        "lib.fol",
        "fun[exp] answer(): int = {\n    return 7;\n};\n",
    );
    write_model_app_package(
        &app_root,
        "app",
        "core",
        concat!(
            "use corelib: pkg = {\"corelib\"};\n",
            "fun[] main(): int = {\n",
            "    return corelib::src::answer();\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    assert!(
        build.status.success(),
        "core->core pkg dependency should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(String::from_utf8_lossy(&build.stdout).contains("capability_mode=core"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_core_artifact_rejects_transitive_mem_pkg_dependency() {
    let temp_root = unique_temp_root("model_core_dep_mem");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("memolib"),
        "memolib",
        "memo",
        "lib.fol",
        "fun[exp] helper(): str = {\n    return \"ok\";\n};\n",
    );
    write_model_app_package(
        &app_root,
        "app",
        "core",
        concat!(
            "use memolib: pkg = {\"memolib\"};\n",
            "fun[] main(): int = {\n",
            "    var value: str = memolib::src::helper();\n",
            "    return 0;\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !build.status.success(),
        "core->memo pkg dependency should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(stderr.contains("str requires heap support and is unavailable in 'fol_model = core'"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_core_artifact_rejects_transitive_std_pkg_dependency() {
    let temp_root = unique_temp_root("model_core_dep_std");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("stdlib"),
        "stdlib",
        "std",
        "lib.fol",
        "fun[exp] helper(): int = {\n    return .echo(1);\n};\n",
    );
    write_model_app_package(
        &app_root,
        "app",
        "core",
        concat!(
            "use stdlib: pkg = {\"stdlib\"};\n",
            "fun[] main(): int = {\n",
            "    return stdlib::src::helper();\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !build.status.success(),
        "core->std pkg dependency should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(stderr.contains("'.echo(...)' requires hosted std support"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_mem_artifact_accepts_transitive_mem_pkg_dependency() {
    let temp_root = unique_temp_root("model_memo_dep_mem");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("memolib"),
        "memolib",
        "memo",
        "lib.fol",
        concat!(
            "fun[exp] size(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .len(values);\n",
            "};\n",
        ),
    );
    write_model_app_package(
        &app_root,
        "app",
        "memo",
        concat!(
            "use memolib: pkg = {\"memolib\"};\n",
            "fun[] main(): int = {\n",
            "    return memolib::src::size();\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    assert!(
        build.status.success(),
        "memo->memo pkg dependency should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(String::from_utf8_lossy(&build.stdout).contains("capability_mode=memo"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_mem_artifact_rejects_transitive_std_echo_dependency() {
    let temp_root = unique_temp_root("model_memo_dep_std");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("stdlib"),
        "stdlib",
        "std",
        "lib.fol",
        "fun[exp] helper(): int = {\n    return .echo(1);\n};\n",
    );
    write_model_app_package(
        &app_root,
        "app",
        "memo",
        concat!(
            "use stdlib: pkg = {\"stdlib\"};\n",
            "fun[] main(): int = {\n",
            "    return stdlib::src::helper();\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !build.status.success(),
        "memo should reject transitive std echo dependency: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(stderr.contains("'.echo(...)' requires hosted std support"));
    assert!(stderr.contains("current artifact model is 'memo'"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_std_artifact_accepts_mixed_core_and_mem_pkg_dependencies() {
    let temp_root = unique_temp_root("model_std_dep_core_mem");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("corelib"),
        "corelib",
        "core",
        "lib.fol",
        "fun[exp] answer(): int = {\n    return 5;\n};\n",
    );
    write_formal_model_package(
        &store_root.join("memolib"),
        "memolib",
        "memo",
        "lib.fol",
        concat!(
            "fun[exp] size(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .len(values);\n",
            "};\n",
        ),
    );
    write_model_app_package(
        &app_root,
        "app",
        "std",
        concat!(
            "use corelib: pkg = {\"corelib\"};\n",
            "use memolib: pkg = {\"memolib\"};\n",
            "use std: pkg = {\"std\"};\n",
            "fun[] main(): int = {\n",
            "    return std::io::echo_int(corelib::src::answer() + memolib::src::size());\n",
            "};\n",
        ),
        true,
    );

    let bundled_std_root =
        fol_package::available_bundled_std_root().expect("bundled std root should exist");
    copy_dir_all(&bundled_std_root, &store_root.join("std"));
    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build", "--keep-build-dir"]);
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    assert!(
        build.status.success(),
        "std mixed pkg dependency graph should build: stdout=\n{}\nstderr=\n{}",
        build_stdout,
        String::from_utf8_lossy(&build.stderr)
    );
    let binary = build_stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if plain.contains("binary") {
                plain.split_whitespace().last().map(str::to_string)
            } else {
                None
            }
        })
        .expect("std mixed dependency build should report a binary path")
        .trim()
        .to_string();
    let run = Command::new(&binary)
        .output()
        .expect("std mixed dependency binary should execute");
    assert!(
        run.status.success(),
        "std mixed dependency binary should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("8"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_std_consumer_of_mem_pkg_dependency_emits_std_runtime_only() {
    let temp_root = unique_temp_root("model_std_dep_mem_emit");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("memolib"),
        "memolib",
        "memo",
        "lib.fol",
        concat!(
            "fun[exp] size(): int = {\n",
            "    var values: seq[int] = {1, 2, 3};\n",
            "    return .len(values);\n",
            "};\n",
        ),
    );
    write_model_app_package(
        &app_root,
        "app",
        "std",
        concat!(
            "use memolib: pkg = {\"memolib\"};\n",
            "use std: pkg = {\"std\"};\n",
            "fun[] main(): int = {\n",
            "    return std::io::echo_int(memolib::src::size());\n",
            "};\n",
        ),
        true,
    );

    let bundled_std_root =
        fol_package::available_bundled_std_root().expect("bundled std root should exist");
    copy_dir_all(&bundled_std_root, &store_root.join("std"));
    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build", "--keep-build-dir"]);
    assert!(
        build.status.success(),
        "std memo-consumer build should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let emitted = find_file_by_name(&app_root.join(".fol/build"), "main.rs")
        .expect("std memo-consumer should emit main.rs");
    let source = std::fs::read_to_string(&emitted).expect("generated main should load");
    assert!(source.contains("use fol_runtime::std as rt;"));
    assert!(!source.contains("use fol_runtime::memo as rt;"));
    assert!(!source.contains("use fol_runtime::core as rt;"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_core_illegal_dependency_failure_happens_before_emission() {
    let temp_root = unique_temp_root("model_core_dep_mem_no_emit");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    write_formal_model_package(
        &store_root.join("memolib"),
        "memolib",
        "memo",
        "lib.fol",
        "fun[exp] helper(): str = {\n    return \"ok\";\n};\n",
    );
    write_model_app_package(
        &app_root,
        "app",
        "core",
        concat!(
            "use memolib: pkg = {\"memolib\"};\n",
            "fun[] main(): int = {\n",
            "    var value: str = memolib::src::helper();\n",
            "    return 0;\n",
            "};\n",
        ),
        false,
    );

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build", "--keep-build-dir"]);
    assert!(
        !build.status.success(),
        "illegal core memo-consumer should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        find_file_by_name(&app_root.join(".fol/build"), "main.rs").is_none(),
        "illegal core memo-consumer should fail before Rust emission"
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_build_fixtures_emit_runtime_imports_for_each_model() {
    let cases = [
        ( "core", false, "fun[] main(): int = {\n    return 7;\n};\n", "core"),
        (
            "memo",
            false,
            "fun[] main(): str = {\n    return \"memo-ready\";\n};\n",
            "memo",
        ),
        (
            "memo_std",
            true,
            concat!(
                "use std: pkg = {\"std\"};\n",
                "fun[] main(): int = {\n",
                "    return std::io::echo_int(7);\n",
                "};\n",
            ),
            "std",
        ),
    ];

    for (label, add_std, main_source, expected_model) in cases {
        let temp_root = unique_temp_root(&format!("build_runtime_import_{label}"));
        let root = temp_root.join("demo");
        std::fs::create_dir_all(root.join("src")).expect("should create source root");
        std::fs::write(
            root.join("build.fol"),
            format!(
                concat!(
                    "pro[] build(): non = {{\n",
                    "    var build = .build();\n",
                    "    build.meta({{ name = \"demo\", version = \"0.1.0\" }});\n",
                    "{}",
                    "    var graph = build.graph();\n",
                    "    var app = graph.add_exe({{\n",
                    "        name = \"demo\",\n",
                    "        root = \"src/main.fol\",\n",
                    "        fol_model = \"{}\",\n",
                    "    }});\n",
                    "    graph.install(app);\n",
                    "    return;\n",
                    "}};\n",
                ),
                if add_std {
                    "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n"
                } else {
                    ""
                },
                if expected_model == "core" { "core" } else { "memo" }
            ),
        )
        .expect("should write build file");
        std::fs::write(root.join("src/main.fol"), main_source).expect("should write source");

        let build = if add_std {
            run_fol_with_store_in_dir(
                &root,
                &repo_root().join("lang/library"),
                &["code", "build", "--keep-build-dir"],
            )
        } else {
            run_fol_in_dir(&root, &["code", "build", "--keep-build-dir"])
        };
        assert!(
            build.status.success(),
            "model '{label}' should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let generated = find_file_by_name(&root.join(".fol/build"), "main.rs")
            .expect("generated backend source should exist");
        let source =
            std::fs::read_to_string(&generated).expect("generated backend source should load");
        let expected_import = expected_runtime_import_for_model(expected_model);

        assert!(
            source.contains(&expected_import),
            "model '{label}' should emit '{expected_import}' in {:?}:\n{}",
            generated,
            source
        );

        std::fs::remove_dir_all(&temp_root).ok();
    }
}

#[test]
fn test_cli_build_emits_rust_for_model_examples() {
    let cases = [
        ("examples/build_dep_handles", "use fol_runtime::memo as rt;"),
        ("examples/build_dep_args", "use fol_runtime::memo as rt;"),
        ("examples/build_dep_exports", "use fol_runtime::memo as rt;"),
        ("examples/build_dep_paths", "use fol_runtime::memo as rt;"),
        ("examples/build_dep_modes", "use fol_runtime::memo as rt;"),
        (
            "examples/build_described_steps",
            "use fol_runtime::memo as rt;",
        ),
        (
            "examples/build_generated_dirs",
            "use fol_runtime::memo as rt;",
        ),
        (
            "examples/build_install_prefix",
            "use fol_runtime::memo as rt;",
        ),
        (
            "examples/build_output_handles",
            "use fol_runtime::memo as rt;",
        ),
        ("examples/build_system_lib", "use fol_runtime::memo as rt;"),
        ("examples/build_system_tool", "use fol_runtime::memo as rt;"),
        ("examples/build_source_paths", "use fol_runtime::memo as rt;"),
        ("examples/core_blink_shape", "use fol_runtime::core as rt;"),
        ("examples/core_defer", "use fol_runtime::core as rt;"),
        ("examples/core_records", "use fol_runtime::core as rt;"),
        ("examples/core_surface_showcase", "use fol_runtime::core as rt;"),
        ("examples/memo_defaults", "use fol_runtime::memo as rt;"),
        ("examples/memo_containers", "use fol_runtime::memo as rt;"),
        (
            "examples/memo_collections",
            "use fol_runtime::memo as rt;",
        ),
        (
            "examples/memo_surface_showcase",
            "use fol_runtime::memo as rt;",
        ),
        ("examples/std_cli", "use fol_runtime::std as rt;"),
        ("examples/std_bundled_fmt", "use fol_runtime::std as rt;"),
        ("examples/std_explicit_pkg", "use fol_runtime::std as rt;"),
        ("examples/std_echo_min", "use fol_runtime::std as rt;"),
        ("examples/std_named_calls", "use fol_runtime::std as rt;"),
        ("examples/std_surface_showcase", "use fol_runtime::std as rt;"),
    ];

    for (path, expected_import) in cases {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        assert!(
            build.status.success(),
            "example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
            .expect("generated example backend source should exist");
        let source =
            std::fs::read_to_string(&generated).expect("generated example source should load");

        assert!(
            source.contains(expected_import),
            "example '{path}' should emit '{expected_import}' in {:?}:\n{}",
            generated,
            source
        );
    }
}

#[test]
fn test_cli_example_build_summaries_surface_expected_models() {
    let cases = [
        ("examples/build_dep_handles", "memo"),
        ("examples/build_dep_args", "memo"),
        ("examples/build_dep_exports", "memo"),
        ("examples/build_dep_paths", "memo"),
        ("examples/build_dep_modes", "memo"),
        ("examples/build_described_steps", "memo"),
        ("examples/build_generated_dirs", "memo"),
        ("examples/build_install_prefix", "memo"),
        ("examples/build_output_handles", "memo"),
        ("examples/build_system_lib", "memo"),
        ("examples/build_system_tool", "memo"),
        ("examples/build_source_paths", "memo"),
        ("examples/core_blink_shape", "core"),
        ("examples/core_defer", "core"),
        ("examples/core_records", "core"),
        ("examples/core_surface_showcase", "core"),
        ("examples/memo_defaults", "memo"),
        ("examples/memo_containers", "memo"),
        ("examples/memo_collections", "memo"),
        ("examples/memo_surface_showcase", "memo"),
        ("examples/std_bundled_fmt", "std"),
        ("examples/std_explicit_pkg", "std"),
        ("examples/std_cli", "std"),
        ("examples/std_echo_min", "std"),
        ("examples/std_named_calls", "std"),
        ("examples/std_surface_showcase", "std"),
    ];

    for (path, expected_model) in cases {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        let stdout = String::from_utf8_lossy(&build.stdout);
        assert!(
            build.status.success(),
            "example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            stdout,
            String::from_utf8_lossy(&build.stderr)
        );
        let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
            .expect("generated example backend source should exist");
        let source =
            std::fs::read_to_string(&generated).expect("generated example source should load");
        let expected_import = expected_runtime_import_for_model(expected_model);
        assert!(
            source.contains(&expected_import),
            "example '{path}' should map to runtime import '{expected_import}' in {:?}:\n{}",
            generated,
            source
        );
    }
}

#[test]
fn test_cli_std_examples_run_and_print_expected_output() {
    let cases = [
        ("examples/std_bundled_fmt", "13"),
        ("examples/std_bundled_io", "std-io"),
        ("examples/std_explicit_pkg", "std-explicit-pkg"),
        ("examples/std_cli", "std-ready"),
        ("examples/std_echo_min", "9"),
        ("examples/std_named_calls", "host-ok-ready"),
        ("examples/std_substrate_echo", "11"),
        ("examples/std_surface_showcase", "std-hosted-full"),
    ];

    for (path, expected_text) in cases {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        let build_stdout = String::from_utf8_lossy(&build.stdout);
        assert!(
            build.status.success(),
            "std example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            build_stdout,
            String::from_utf8_lossy(&build.stderr)
        );
        let run = std::process::Command::new(built_binary_path(&build))
            .output()
            .expect("built std example should execute");
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert!(
            run.status.success(),
            "std example '{path}' binary should run: stdout=\n{}\nstderr=\n{}",
            stdout,
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            stdout.contains(expected_text),
            "std example '{path}' should print '{expected_text}': stdout=\n{}\nstderr=\n{}",
            stdout,
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn test_cli_core_and_memo_examples_run_without_bundled_std_dependency() {
    for path in ["examples/core_run_min", "examples/memo_run_min"] {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        let build_stdout = String::from_utf8_lossy(&build.stdout);
        assert!(
            build.status.success(),
            "no-std runnable example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            build_stdout,
            String::from_utf8_lossy(&build.stderr)
        );

        let run = run_fol_in_dir(&root, &["code", "run"]);
        let run_stdout = String::from_utf8_lossy(&run.stdout);
        assert!(
            run.status.success(),
            "no-std runnable example '{path}' should run: stdout=\n{}\nstderr=\n{}",
            run_stdout,
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            run_stdout.contains("ran "),
            "no-std runnable example '{path}' should report a run summary: stdout=\n{}\nstderr=\n{}",
            run_stdout,
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn test_bundled_std_discovery_stays_coherent_across_cli_and_editor() {
    use fol_editor::{
        EditorConfig, EditorDocumentUri, EditorLspServer, JsonRpcId, JsonRpcNotification,
        JsonRpcRequest, LspDefinitionParams, LspDidOpenTextDocumentParams, LspPosition,
        LspTextDocumentIdentifier, LspTextDocumentItem,
    };

    let root = temp_example_root("examples/std_bundled_fmt");
    let store_root = repo_root().join("lang/library");
    let build = run_fol_with_store_in_dir(
        &root,
        &store_root,
        &[root.to_str().expect("example root should be valid UTF-8")],
    );
    assert!(
        build.status.success(),
        "bundled std example should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let binary = String::from_utf8_lossy(&build.stdout)
        .lines()
        .find_map(|line| line.split("binary=").nth(1))
        .expect("build should report a binary path")
        .trim()
        .to_string();
    let run = std::process::Command::new(&binary)
        .output()
        .expect("bundled std example binary should execute");
    assert!(
        run.status.success(),
        "bundled std example should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("13"),
        "bundled std example should print through hosted std flow: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    materialize_local_bundled_std_alias(&root);
    let source_path = root.join("src/main.fol");
    let source = std::fs::read_to_string(&source_path).expect("example source should load");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = server
        .handle_notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(LspDidOpenTextDocumentParams {
                    text_document: LspTextDocumentItem {
                        uri: uri.clone(),
                        language_id: "fol".to_string(),
                        version: 1,
                        text: source,
                    },
                })
                .unwrap(),
            ),
        })
        .expect("didOpen should succeed");
    assert!(
        diagnostics
            .iter()
            .all(|published| published.diagnostics.is_empty()),
        "bundled std example should stay editor-clean without override: {diagnostics:#?}"
    );

    let definition = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(3991),
            method: "textDocument/definition".to_string(),
            params: Some(
                serde_json::to_value(LspDefinitionParams {
                    text_document: LspTextDocumentIdentifier { uri },
                    position: LspPosition {
                        line: 2,
                        character: 27,
                    },
                })
                .unwrap(),
            ),
        })
        .expect("definition request should succeed");
    assert!(definition.is_some(), "bundled std definition request should complete");
}

#[test]
fn test_bundled_std_io_example_builds_and_runs_without_override() {
    let root = temp_example_root("examples/std_bundled_io");
    let build = run_example_compile(&root, true);
    let stdout = String::from_utf8_lossy(&build.stdout);
    assert!(
        build.status.success(),
        "bundled std.io example should build: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(built_binary_path(&build))
        .output()
        .expect("bundled std.io example should run");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "bundled std.io example should execute");
    assert!(
        stdout.contains("std-io"),
        "bundled std.io example should print through the std.io wrapper: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.contains("true") && stdout.contains("z"),
        "bundled std.io example should exercise the new bool/char wrappers too: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_bundled_std_alias_example_builds_with_materialized_alias_root() {
    let root = temp_example_root("examples/std_alias_pkg");
    materialize_local_bundled_std_alias_as(&root, "standard_lib");
    std::fs::remove_file(root.join("fol.work.yaml"))
        .expect("alias build test should remove workspace override before CLI build");

    let build = run_fol_with_store_in_dir(
        &root,
        &root.join(".fol/pkg"),
        &["code", "build", "--keep-build-dir"],
    );
    assert!(
        build.status.success(),
        "bundled std alias example should build with a materialized alias root: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
        .expect("bundled std alias example should emit main.rs");
    let source = std::fs::read_to_string(&generated).expect("generated main should load");
    assert!(source.contains("use fol_runtime::std as rt;"));

    let binary = built_binary_path(&build);
    let run = std::process::Command::new(&binary)
        .output()
        .expect("bundled std alias example should run");
    assert!(run.status.success(), "bundled std alias example should execute");
    assert!(String::from_utf8_lossy(&run.stdout).contains("10"));
}

#[test]
fn test_bundled_std_package_root_builds_under_the_current_model() {
    let root = repo_root().join("lang/library/std");
    let build = run_fol_in_dir(&root, &["code", "check"]);
    assert!(
        build.status.success(),
        "bundled std package root should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

#[test]
fn test_bundled_std_tree_stays_source_only_and_bootstrap_honest() {
    let root = repo_root().join("lang/library/std");

    assert!(
        root.join("build.fol").exists(),
        "bundled std should ship a real build.fol package root"
    );
    assert!(
        root.join("lib.fol").exists(),
        "bundled std should ship a real root source file"
    );
    assert!(
        root.join("fmt/root.fol").exists(),
        "bundled std bootstrap should ship std.fmt"
    );
    assert!(
        root.join("fmt/math/lib.fol").exists(),
        "bundled std bootstrap should ship std.fmt.math"
    );
    assert!(
        root.join("io/lib.fol").exists(),
        "bundled std bootstrap should ship std.io once it has honest public source"
    );
    assert!(
        !root.join("os/lib.fol").exists(),
        "bundled std should not ship a public std.os module before it has honest source"
    );
    assert!(
        !root.join(".fol").exists(),
        "bundled std tree should stay source-only in the repo"
    );
}

#[test]
fn test_build_rejects_std_imports_under_core_model() {
    let temp_root = unique_temp_root("build_core_use_std_reject");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join("pkg");
    std::fs::create_dir_all(app_root.join("src")).expect("should create app source root");
    std::fs::create_dir_all(store_root.join("std/fmt")).expect("should create std source root");
    std::fs::write(
        store_root.join("std/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
    )
    .expect("should write std build");
    std::fs::write(
        store_root.join("std/fmt/root.fol"),
        "fun[exp] answer(): int = {\n    return 42;\n};\n",
    )
    .expect("should write std source");
    std::fs::write(
        app_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"core\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("should write app build");
    std::fs::write(
        app_root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");

    let build = run_fol_with_store_in_dir(&app_root, &store_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(!build.status.success(), "core std-import app should fail");
    assert!(
        stderr.contains("bundled std imports require 'fol_model = memo'; current artifact model is 'core'"),
        "core std-import app should keep the capability diagnostic: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_build_rejects_std_imports_without_declared_dependency() {
    let temp_root = unique_temp_root("build_memo_use_std_missing_dep");
    let app_root = temp_root.join("app");
    std::fs::create_dir_all(app_root.join("src")).expect("should create app source root");
    std::fs::write(
        app_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("should write app build");
    std::fs::write(
        app_root.join("src/main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .expect("should write app source");

    let build = run_fol_in_dir(&app_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(!build.status.success(), "memo std-import app should fail without declared dependency");
    assert!(
        stderr.contains(".fol/pkg/std") || stderr.contains("resolver pkg import target"),
        "memo std-import app should fail because bundled std is dependency-backed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_final_std_contract_negative_paths_fail_cleanly() {
    let temp_root = unique_temp_root("final_std_contract_negatives");

    let old_std_mode = temp_root.join("old_std_mode");
    std::fs::create_dir_all(old_std_mode.join("src")).expect("should create std-mode app source root");
    std::fs::write(
        old_std_mode.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"std\" });\n",
            "};\n",
        ),
    )
    .expect("should write std-mode build");
    std::fs::write(old_std_mode.join("src/main.fol"), "fun[] main(): int = { return 0; };\n")
        .expect("should write std-mode main");
    let old_std_mode_output = run_fol_in_dir(&old_std_mode, &["code", "build"]);
    let old_std_mode_stderr = String::from_utf8_lossy(&old_std_mode_output.stderr);
    assert!(
        !old_std_mode_output.status.success(),
        "old std-mode spelling should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&old_std_mode_output.stdout),
        old_std_mode_stderr
    );
    assert!(
        old_std_mode_stderr.contains("artifact fol_model no longer accepts 'std'"),
        "old std-mode spelling should keep the exact guidance: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&old_std_mode_output.stdout),
        old_std_mode_stderr
    );

    let bad_internal = temp_root.join("bad_internal_target");
    std::fs::create_dir_all(bad_internal.join("src")).expect("should create bad-internal app source root");
    std::fs::write(
        bad_internal.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"stdlib\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "};\n",
        ),
    )
    .expect("should write bad-internal build");
    std::fs::write(bad_internal.join("src/main.fol"), "fun[] main(): int = { return 0; };\n")
        .expect("should write bad-internal main");
    let bad_internal_output = run_fol_in_dir(&bad_internal, &["code", "build"]);
    let bad_internal_stderr = String::from_utf8_lossy(&bad_internal_output.stderr);
    assert!(
        !bad_internal_output.status.success(),
        "bad internal target should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&bad_internal_output.stdout),
        bad_internal_stderr
    );
    assert!(
        bad_internal_stderr.contains("may use internal source only with target 'standard'"),
        "bad internal target should keep the exact bundled-std target diagnostic: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&bad_internal_output.stdout),
        bad_internal_stderr
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_build_install_prefix_moves_without_changing_build_source() {
    let root = temp_example_root("examples/build_install_prefix");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path).expect("example build.fol");

    let request_a = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            install_prefix: root.join(".out-a").display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let request_b = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            install_prefix: root.join(".out-b").display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated_a = evaluate_build_source(&request_a, &build_path, &source)
        .expect("example should evaluate")
        .expect("example should produce a graph");
    let evaluated_b = evaluate_build_source(&request_b, &build_path, &source)
        .expect("example should evaluate")
        .expect("example should produce a graph");

    let install_a = evaluated_a
        .result
        .graph
        .installs()
        .iter()
        .find(|install| install.name == "install")
        .expect("example install should exist")
        .projected_destination
        .clone();
    let install_b = evaluated_b
        .result
        .graph
        .installs()
        .iter()
        .find(|install| install.name == "install")
        .expect("example install should exist")
        .projected_destination
        .clone();

    assert_ne!(install_a, install_b);
    assert!(install_a.ends_with("/bin/demo"));
    assert!(install_b.ends_with("/bin/demo"));
}

#[test]
fn test_cli_build_summary_surfaces_install_prefix_and_outputs() {
    let root = temp_example_root("examples/build_install_prefix");
    let install_prefix = root.join(".custom-install");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_fol"))
        .args(["code", "build"])
        .env("FOL_INSTALL_PREFIX", &install_prefix)
        .current_dir(&root)
        .output()
        .expect("should run fol CLI in directory");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "install-prefix example should build: stdout=\n{}\nstderr=\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains(&format!("install_prefix={}", install_prefix.display())),
        "build summary should surface install prefix: stdout=\n{}\nstderr=\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("outputs="),
        "build summary should surface output count: stdout=\n{}\nstderr=\n{}",
        stdout,
        stderr
    );
}

#[test]
fn test_build_output_handle_example_keeps_generated_handles_composed() {
    let root = temp_example_root("examples/build_output_handles");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path).expect("example build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("output-handle example should evaluate")
        .expect("output-handle example should produce a graph");

    assert_eq!(evaluated.result.graph.generated_files().len(), 2);
    assert!(evaluated
        .result
        .graph
        .installs()
        .iter()
        .any(|install| install.name == "generated-cfg"));
    assert!(evaluated
        .result
        .graph
        .installs()
        .iter()
        .any(|install| install.name == "copied-banner"));
}

#[test]
fn test_build_dep_args_example_keeps_forwarded_dependency_args() {
    let root = temp_example_root("examples/build_dep_args");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path).expect("example build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            options: std::collections::BTreeMap::from([
                ("jobs".to_string(), "6".to_string()),
                ("flavor".to_string(), "strict".to_string()),
            ]),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("dep-arg example should evaluate")
        .expect("dep-arg example should produce a graph");
    let dep = evaluated
        .evaluated
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "shared")
        .expect("shared dependency should be recorded");
    let assets = evaluated
        .evaluated
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "assets")
        .expect("assets dependency should be recorded");
    let logtiny = evaluated
        .evaluated
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "logtiny")
        .expect("logtiny dependency should be recorded");

    assert_eq!(dep.args.get("jobs").map(String::as_str), Some("6"));
    assert_eq!(dep.args.get("flavor").map(String::as_str), Some("strict"));
    assert_eq!(
        dep.evaluation_mode,
        Some(fol_package::DependencyBuildEvaluationMode::Lazy)
    );
    assert_eq!(
        assets.evaluation_mode,
        Some(fol_package::DependencyBuildEvaluationMode::OnDemand)
    );
    assert_eq!(
        logtiny.evaluation_mode,
        Some(fol_package::DependencyBuildEvaluationMode::Eager)
    );
}

#[test]
fn test_build_dep_exports_example_keeps_only_explicit_build_surfaces() {
    let root = temp_example_root("examples/build_dep_exports");
    let shared_root = root.join("deps/shared");
    let mut stream = fol_stream::FileStream::from_folder(
        shared_root
            .to_str()
            .expect("shared example path should be utf-8"),
    )
    .expect("shared package stream should open");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = fol_parser::ast::AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("shared package syntax should parse");

    let surface =
        fol_package::build_dependency::project_dependency_surface("shared", &shared_root, &syntax)
            .expect("explicit export surface should project");

    assert_eq!(
        surface
            .modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        vec!["api"]
    );
    assert_eq!(
        surface
            .artifacts
            .iter()
            .map(|artifact| artifact.name.as_str())
            .collect::<Vec<_>>(),
        vec!["runtime"]
    );
    assert_eq!(
        surface
            .steps
            .iter()
            .map(|step| step.name.as_str())
            .collect::<Vec<_>>(),
        vec!["install"]
    );
    assert_eq!(
        surface
            .generated_outputs
            .iter()
            .map(|output| output.name.as_str())
            .collect::<Vec<_>>(),
        vec!["schema"]
    );

    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("dep export example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("dep export example should evaluate")
        .expect("dep export example should produce a graph");

    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "api"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Module));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "runtime"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Artifact));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "install"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Step));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "schema"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::GeneratedOutput));
}

#[test]
fn test_build_dep_paths_example_keeps_explicit_path_surfaces_and_queries() {
    let root = temp_example_root("examples/build_dep_paths");
    let shared_root = root.join("deps/shared");
    let mut stream = fol_stream::FileStream::from_folder(
        shared_root
            .to_str()
            .expect("shared example path should be utf-8"),
    )
    .expect("shared package stream should open");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = fol_parser::ast::AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("shared package syntax should parse");

    let surface =
        fol_package::build_dependency::project_dependency_surface("shared", &shared_root, &syntax)
            .expect("explicit path export surface should project");

    assert_eq!(
        surface
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        vec!["defaults"]
    );
    assert_eq!(
        surface
            .dirs
            .iter()
            .map(|dir| dir.name.as_str())
            .collect::<Vec<_>>(),
        vec!["public"]
    );
    assert_eq!(
        surface
            .paths
            .iter()
            .map(|path| path.name.as_str())
            .collect::<Vec<_>>(),
        vec!["schema-path"]
    );

    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("dep path example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("dep path example should evaluate")
        .expect("dep path example should produce a graph");

    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "defaults"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::File));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "public"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Dir));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "schema-path"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Path));
}

#[test]
fn test_build_generated_dirs_example_keeps_generated_dir_surfaces_and_queries() {
    let root = temp_example_root("examples/build_generated_dirs");
    let shared_root = root.join("deps/shared");
    let mut stream = fol_stream::FileStream::from_folder(
        shared_root
            .to_str()
            .expect("shared example path should be utf-8"),
    )
    .expect("shared package stream should open");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = fol_parser::ast::AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("shared package syntax should parse");

    let surface =
        fol_package::build_dependency::project_dependency_surface("shared", &shared_root, &syntax)
            .expect("generated dir export surface should project");

    assert_eq!(
        surface
            .dirs
            .iter()
            .map(|dir| dir.name.as_str())
            .collect::<Vec<_>>(),
        vec!["assets"]
    );

    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path)
        .expect("generated dir example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("generated dir example should evaluate")
        .expect("generated dir example should produce a graph");

    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "assets"
            && query.kind == fol_package::BuildRuntimeDependencyQueryKind::Dir));
    assert!(evaluated
        .result
        .graph
        .installs()
        .iter()
        .any(|install| install.name == "assets"
            && install.kind == fol_package::BuildInstallKind::Directory));
}

#[test]
fn test_build_dependency_surfaces_stay_empty_without_explicit_exports() {
    let root = unique_temp_root("dep_surface_requires_export");
    std::fs::create_dir_all(root.join("deps/shared/src"))
        .expect("should create dependency source root");
    std::fs::write(
            root.join("deps/shared/build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"shared\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var lib = graph.add_static_lib({ name = \"shared\", root = \"src/root.fol\", fol_model = \"memo\" });\n",
                "    graph.install(lib);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("should write dependency build");
    std::fs::write(
        root.join("deps/shared/src/root.fol"),
        "fun[exp] helper(): int = {\n    return 7;\n};\n",
    )
    .expect("should write dependency source");

    let shared_root = root.join("deps/shared");
    let mut stream = fol_stream::FileStream::from_folder(
        shared_root
            .to_str()
            .expect("shared dependency root should be utf-8"),
    )
    .expect("shared package stream should open");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = fol_parser::ast::AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("shared package syntax should parse");

    let surface =
        fol_package::build_dependency::project_dependency_surface("shared", &shared_root, &syntax)
            .expect("surface should project");

    let roots = surface
        .source_roots
        .iter()
        .map(|root| root.relative_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    assert!(
        roots[0].ends_with("deps/shared/src"),
        "dependency source roots should stay available for ordinary imports: {roots:?}"
    );
    assert!(surface.modules.is_empty());
    assert!(surface.artifacts.is_empty());
    assert!(surface.steps.is_empty());
    assert!(
        surface.generated_outputs.is_empty(),
        "non-exported dependency should not expose build-facing outputs"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_build_described_steps_example_surfaces_known_step_descriptions() {
    let root = temp_example_root("examples/build_described_steps");
    let build_path = root.join("build.fol");
    let source = std::fs::read_to_string(&build_path)
        .expect("described step example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("described step example should evaluate")
        .expect("described step example should produce a graph");

    assert!(evaluated
        .result
        .graph
        .steps()
        .iter()
        .any(|step| step.name == "compile"
            && step.description.as_deref() == Some("Compile the executable")));
    assert!(evaluated
        .result
        .graph
        .steps()
        .iter()
        .any(|step| step.name == "docs"
            && step.description.as_deref() == Some("Generate package documentation")));
    assert!(evaluated
        .result
        .graph
        .steps()
        .iter()
        .any(|step| step.name == "bundle"
            && step.description.as_deref() == Some("Assemble the release bundle")));
}

#[test]
fn test_build_system_tool_example_keeps_typed_tool_inputs() {
    let root = temp_example_root("examples/build_system_tool");
    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("system tool example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("system tool example should evaluate")
        .expect("system tool example should produce a graph");

    let generated = evaluated
        .result
        .graph
        .generated_files()
        .iter()
        .find(|generated| {
            generated.kind == fol_package::BuildGeneratedFileKind::CaptureOutput
                && generated.name == "gen/schema.fol"
        })
        .expect("system tool example should keep the generated output");
    assert_eq!(generated.name, "gen/schema.fol");
    assert!(evaluated
        .result
        .graph
        .steps()
        .iter()
        .any(|step| step.name == "codegen"
            && step.description.as_deref() == Some("Generate schema bindings")));
}

#[test]
fn test_cli_build_rejects_negative_build_surface_examples() {
    let cases = [
            (
                "fail_dep_unknown_surface",
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                    "    var dep = build.add_dep({ alias = \"shared\", source = \"loc\", target = \"deps/shared\" });\n",
                    "    var graph = build.graph();\n",
                    "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                    "    var bindings = dep.generated(\"bindings\");\n",
                    "    app.add_generated(bindings);\n",
                    "    return;\n",
                    "};\n",
                ),
                Some(concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"shared\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    var lib = graph.add_static_lib({ name = \"shared\", root = \"src/root.fol\", fol_model = \"memo\" });\n",
                    "    graph.install(lib);\n",
                    "    return;\n",
                    "};\n",
                )),
                "requires a local generated-output handle, not a dependency path handle",
            ),
            (
                "fail_dep_invalid_args",
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\", args = { target = graph } });\n",
                    "    return;\n",
                    "};\n",
                ),
                None,
                "dependency arg 'target' must evaluate to bool, int, str, or an option handle",
            ),
            (
                "fail_output_handle_usage",
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    graph.install_file({ name = \"bad\", source = 1 });\n",
                    "    return;\n",
                    "};\n",
                ),
                None,
                "source-file handle or generated-output handle",
            ),
            (
                "fail_install_projection",
                concat!(
                    "pro[] build(): non = {\n",
                    "    var build = .build();\n",
                    "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                    "    var graph = build.graph();\n",
                    "    var assets = graph.dir_from_root(\"\");\n",
                    "    graph.install_dir({ name = \"assets\", source = assets });\n",
                    "    return;\n",
                    "};\n",
                ),
                None,
                "dir_from_root requires a non-empty relative path",
            ),
        ];

    for (label, build_source, dep_build_source, expected) in cases {
        let root = unique_temp_root(label);
        std::fs::create_dir_all(root.join("src")).expect("should create source root");
        std::fs::write(root.join("build.fol"), build_source).expect("should write build file");
        std::fs::write(
            root.join("src/main.fol"),
            "fun[] main(): int = {\n    return 7;\n};\n",
        )
        .expect("should write source");
        if let Some(dep_source) = dep_build_source {
            std::fs::create_dir_all(root.join("deps/shared/src"))
                .expect("should create dependency root");
            std::fs::write(root.join("deps/shared/build.fol"), dep_source)
                .expect("should write dependency build");
            std::fs::write(
                root.join("deps/shared/src/root.fol"),
                "fun[exp] helper(): int = {\n    return 1;\n};\n",
            )
            .expect("should write dependency source");
        }

        let output = run_fol_in_dir(&root, &["code", "build"]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "negative build example '{label}' should fail: stdout=\n{}\nstderr=\n{}",
            stdout,
            stderr
        );
        assert!(
                stdout.contains(expected) || stderr.contains(expected),
                "negative build example '{label}' should mention '{expected}': stdout=\n{}\nstderr=\n{}",
                stdout,
                stderr
            );

        std::fs::remove_dir_all(&root).ok();
    }
}

#[test]
fn test_cli_build_and_run_mixed_model_example_workspace() {
    let root = temp_example_root("examples/mixed_models_workspace");

    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("mixed example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("mixed example should evaluate")
        .expect("mixed example should produce a graph");
    let artifacts = &evaluated.evaluated.artifacts;
    assert!(artifacts.iter().any(|a| {
        a.name == "corelib"
            && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Core
    }));
    assert!(artifacts.iter().any(|a| {
        a.name == "memolib"
            && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Memo
    }));
    assert!(artifacts.iter().any(|a| {
        a.name == "tool" && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Memo
    }));

    let build = run_fol_with_store_in_dir(&root, &repo_root().join("lang/library"), &["code", "build"]);
    assert!(
        build.status.success(),
        "mixed-model example should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = run_fol_with_store_in_dir(&root, &repo_root().join("lang/library"), &["code", "run"]);
    assert!(
        run.status.success(),
        "mixed-model example should run its std tool: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("ran "));
}

#[test]
fn test_cli_run_rejects_core_example_route() {
    let temp_root = unique_temp_root("run_core_route_reject");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"core\",\n",
            "    });\n",
            "    graph.add_run(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 7;\n};\n",
    )
    .expect("should write source");

    let run = run_fol_in_dir(&root, &["code", "run"]);
    assert!(
        run.status.success(),
        "core route should remain runnable: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_run_rejects_mem_example_route() {
    let temp_root = unique_temp_root("run_mem_route_reject");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"memo\",\n",
            "    });\n",
            "    graph.add_run(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): str = {\n    return \"memo\";\n};\n",
    )
    .expect("should write source");

    let run = run_fol_in_dir(&root, &["code", "run"]);
    assert!(
        run.status.success(),
        "memo route should remain runnable: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("ran "));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_test_rejects_memo_example_route() {
    let temp_root = unique_temp_root("test_memo_route_reject");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var tests = graph.add_test({\n",
            "        name = \"demo-tests\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"memo\",\n",
            "    });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): str = {\n    return \"memo\";\n};\n",
    )
    .expect("should write source");

    let run = run_fol_in_dir(&root, &["code", "test"]);
    assert!(
        run.status.success(),
        "memo test route should remain runnable: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_run_rejects_ambiguous_non_std_models_with_resolved_models() {
    let temp_root = unique_temp_root("run_mixed_route_reject");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var blink = graph.add_exe({ name = \"blink\", root = \"src/blink.fol\", fol_model = \"core\" });\n",
            "    var heap = graph.add_exe({ name = \"heap\", root = \"src/heap.fol\", fol_model = \"memo\" });\n",
            "    graph.install(blink);\n",
            "    graph.install(heap);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/blink.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write core source");
    std::fs::write(
        root.join("src/heap.fol"),
        "fun[] main(): int = {\n    var shown: str = \"ok\";\n    return 0;\n};\n",
    )
    .expect("should write memo source");

    let run = run_fol_in_dir(&root, &["code", "run"]);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "mixed non-std route should be rejected");
    assert!(stderr.contains("requires an explicit named step"));
    assert!(stderr.contains("resolved model(s): core, memo"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_run_rejects_explicit_core_run_step_with_step_model_detail() {
    let temp_root = unique_temp_root("run_explicit_core_step_reject");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var blink = graph.add_exe({ name = \"blink\", root = \"src/blink.fol\", fol_model = \"core\" });\n",
            "    graph.add_run(blink);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/blink.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write core source");

    let run = run_fol_in_dir(&root, &["code", "run", "--step", "run"]);
    assert!(
        run.status.success(),
        "explicit core run step route should remain runnable: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("ran "));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_run_reports_missing_entry_step_separately_from_model_rejection() {
    let temp_root = unique_temp_root("run_missing_entry_step");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.step(\"docs\", \"Generate docs\");\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] helper(): int = {\n    return 0;\n};\n",
    )
    .expect("should write source");

    let run = run_fol_in_dir(&root, &["code", "run"]);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success(), "missing entry route should be rejected");
    assert!(stderr.contains("workspace build execution does not define step 'run'"));
    assert!(stderr.contains("known steps:"));
    assert!(stderr.contains("docs"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_examples_emit_runtime_imports_in_generated_package_sources() {
    let cases = [
        ("examples/core_blink_shape", "use fol_runtime::core as rt;"),
        ("examples/core_defer", "use fol_runtime::core as rt;"),
        (
            "examples/memo_collections",
            "use fol_runtime::memo as rt;",
        ),
        ("examples/memo_defaults", "use fol_runtime::memo as rt;"),
        (
            "examples/build_generated_dirs",
            "use fol_runtime::memo as rt;",
        ),
        ("examples/build_system_lib", "use fol_runtime::memo as rt;"),
        ("examples/build_system_tool", "use fol_runtime::memo as rt;"),
        ("examples/std_cli", "use fol_runtime::std as rt;"),
        ("examples/std_bundled_fmt", "use fol_runtime::std as rt;"),
        ("examples/std_echo_min", "use fol_runtime::std as rt;"),
    ];

    for (path, expected_import) in cases {
        let root = temp_example_root(path);

        let build = run_example_compile(&root, true);
        assert!(
            build.status.success(),
            "example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let generated = find_file_by_name(&emitted_crate_root(&build), "src.rs")
            .expect("generated package source should exist");
        let source =
            std::fs::read_to_string(&generated).expect("generated package source should load");

        assert!(
            source.contains(expected_import),
            "example '{path}' package source should emit '{expected_import}' in {:?}:\n{}",
            generated,
            source
        );
    }
}

#[test]
fn test_model_examples_keep_runtime_imports_clean_across_emitted_rust_trees() {
    let cases = [
        (
            "examples/core_records",
            "use fol_runtime::core as rt;",
            Some("use fol_runtime::memo"),
            Some("use fol_runtime::std"),
        ),
        (
            "examples/memo_collections",
            "use fol_runtime::memo as rt;",
            None,
            Some("use fol_runtime::std"),
        ),
        (
            "examples/std_cli",
            "use fol_runtime::std as rt;",
            None,
            None,
        ),
    ];

    for (path, required, forbid_a, forbid_b) in cases {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        assert!(
            build.status.success(),
            "example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );

        let rust_files = collect_rust_source_files(&emitted_crate_root(&build));
        assert!(
            !rust_files.is_empty(),
            "example '{path}' should emit Rust source files"
        );
        let joined = rust_files
            .iter()
            .map(|file| std::fs::read_to_string(file).expect("rust source should load"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains(required),
            "example '{path}' emitted tree should contain '{required}'"
        );
        if let Some(forbidden) = forbid_a {
            assert!(
                !joined.contains(forbidden),
                "example '{path}' emitted tree should not contain '{forbidden}'"
            );
        }
        if let Some(forbidden) = forbid_b {
            assert!(
                !joined.contains(forbidden),
                "example '{path}' emitted tree should not contain '{forbidden}'"
            );
        }
    }
}

#[test]
fn test_std_logtiny_git_example_builds_against_local_git_remote() {
    let example_root = temp_example_root("examples/std_logtiny_git");
    let remote_root = example_root
        .parent()
        .expect("example temp root should have a parent")
        .join("logtiny-remote");
    create_git_remote_from_logtiny_fixture(&remote_root);
    let selected_revision = git_output(&remote_root, &["rev-parse", "HEAD"]);
    let short_hash = &selected_revision[..12];

    let build_path = example_root.join("build.fol");
    let build_source =
        std::fs::read_to_string(&build_path).expect("git example build file should load");
    std::fs::write(
        &build_path,
        build_source
            .replace(
                "target = \"git+https://github.com/bresilla/logtiny.git\",",
                &format!("target = \"git+file://{}\",", remote_root.display()),
            )
            .replace("version = \"tag:v0.1.3\",", "version = \"tag:v0.1.0\",")
            .replace("hash = \"b242d319644a\",", &format!("hash = \"{short_hash}\",")),
    )
    .expect("git example build file should rewrite");

    let fetch = run_fol_in_dir(&example_root, &["pack", "fetch"]);
    assert!(
        fetch.status.success(),
        "git dependency example should fetch: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&fetch.stdout),
        String::from_utf8_lossy(&fetch.stderr)
    );

    let build = run_fol_in_dir(&example_root, &["code", "build", "--keep-build-dir"]);
    assert!(
        build.status.success(),
        "git dependency example should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let build_stdout = String::from_utf8_lossy(&build.stdout);
    let binary = build_stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if plain.contains("binary") {
                plain.split_whitespace().last().map(str::to_string)
            } else {
                None
            }
        })
        .expect("git dependency example build should report a binary path")
        .trim()
        .to_string();
    let run = std::process::Command::new(&binary)
        .output()
        .expect("git dependency example binary should execute");
    assert!(
        run.status.success(),
        "git dependency example should run: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn test_std_logtiny_git_example_supports_branch_tag_commit_and_hash_fields() {
    let remote_root = unique_temp_root("logtiny_selector_remote");
    create_git_remote_from_logtiny_fixture(&remote_root);
    let selected_revision = git_output(&remote_root, &["rev-parse", "HEAD"]);
    let short_hash = &selected_revision[..12];
    let remote_locator = format!("git+file://{}", remote_root.display());
    let cases = [
        (
            format!("target = \"{remote_locator}\","),
            "version = \"branch:main\",".to_string(),
            None,
        ),
        (
            format!("target = \"{remote_locator}\","),
            "version = \"tag:v0.1.0\",".to_string(),
            None,
        ),
        (
            format!("target = \"{remote_locator}\","),
            format!("version = \"commit:{selected_revision}\","),
            None,
        ),
        (
            format!("target = \"{remote_locator}\","),
            "version = \"branch:main\",".to_string(),
            Some(format!("hash = \"{short_hash}\",")),
        ),
    ];

    for (target_line, version_line, hash_line) in cases {
        let example_root = temp_example_root("examples/std_logtiny_git");
        let build_path = example_root.join("build.fol");
        let build_source =
            std::fs::read_to_string(&build_path).expect("git example build file should load");
        let build_source = build_source
            .replace(
                "target = \"git+https://github.com/bresilla/logtiny.git\",",
                &target_line,
            )
            .replace("version = \"tag:v0.1.3\",", &version_line);
        let build_source = match hash_line {
            Some(hash_line) => build_source.replace("hash = \"b242d319644a\",", &hash_line),
            None => build_source.replace("        hash = \"b242d319644a\",\n", ""),
        };
        std::fs::write(
            &build_path,
            build_source,
        )
        .expect("git example build file should rewrite");

        let fetch = run_fol_in_dir(&example_root, &["pack", "fetch"]);
        assert!(
            fetch.status.success(),
            "git selector example should fetch for '{target_line} {version_line}': stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&fetch.stdout),
            String::from_utf8_lossy(&fetch.stderr)
        );

        let build = run_fol_in_dir(&example_root, &["code", "build"]);
        assert!(
            build.status.success(),
            "git selector example should build for '{target_line} {version_line}': stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
    }
}

#[test]
fn test_mixed_model_example_keeps_graph_models_and_std_emission() {
    let root = temp_example_root("examples/mixed_models_workspace");

    let build_path = root.join("build.fol");
    let source =
        std::fs::read_to_string(&build_path).expect("mixed example should keep a build.fol");
    let request = BuildEvaluationRequest {
        package_root: root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let evaluated = evaluate_build_source(&request, &build_path, &source)
        .expect("mixed example should evaluate")
        .expect("mixed example should produce a graph");
    let artifacts = &evaluated.evaluated.artifacts;
    assert!(artifacts.iter().any(|a| {
        a.name == "corelib"
            && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Core
    }));
    assert!(artifacts.iter().any(|a| {
        a.name == "memolib"
            && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Memo
    }));
    assert!(artifacts.iter().any(|a| {
        a.name == "tool" && a.fol_model == fol_package::build_artifact::BuildArtifactFolModel::Memo
    }));

    let build = run_fol_with_store_in_dir(
        &root,
        &repo_root().join("lang/library"),
        &["code", "build", "--keep-build-dir"],
    );
    assert!(
        build.status.success(),
        "mixed-model example should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let generated = find_file_by_name(&root.join(".fol/build"), "main.rs")
        .expect("mixed example should emit main.rs");
    let emitted = std::fs::read_to_string(&generated).expect("generated main should load");
    assert!(emitted.contains("use fol_runtime::std as rt;"));
}

#[test]
fn test_work_info_surfaces_model_distribution_for_mixed_model_example() {
    let root = temp_example_root("examples/mixed_models_workspace");
    let info = run_fol_with_store_in_dir(&root, &repo_root().join("lang/library"), &["work", "info"]);
    let stdout = String::from_utf8_lossy(&info.stdout);
    assert!(
        info.status.success(),
        "work info should succeed for mixed-model example: stdout=\n{}\nstderr=\n{}",
        stdout,
        String::from_utf8_lossy(&info.stderr)
    );
    assert!(stdout.contains("artifact_models=core=1,memo=2"));
    assert!(stdout.contains("bundled_std_members=1/1"));
}

#[test]
fn test_docs_reference_real_example_packages() {
    let runtime_docs = std::fs::read_to_string(repo_root().join("docs/runtime-models.md"))
        .expect("runtime model docs should exist");
    let build_docs = std::fs::read_to_string(repo_root().join("book/src/055_build/_index.md"))
        .expect("build docs index should exist");
    let runtime_examples = [
        "examples/core_run_min",
        "examples/core_blink_shape",
        "examples/core_defer",
        "examples/core_records",
        "examples/core_surface_showcase",
        "examples/memo_run_min",
        "examples/memo_defaults",
        "examples/memo_containers",
        "examples/memo_collections",
        "examples/memo_surface_showcase",
        "examples/std_bundled_fmt",
        "examples/std_bundled_io",
        "examples/std_explicit_pkg",
        "examples/std_alias_pkg",
        "examples/std_cli",
        "examples/std_echo_min",
        "examples/std_logtiny_git",
        "examples/std_named_calls",
        "examples/std_substrate_echo",
        "examples/std_surface_showcase",
        "examples/mixed_models_workspace",
    ];
    let build_examples = [
        "examples/build_dep_exports",
        "examples/build_dep_modes",
        "examples/build_described_steps",
        "examples/build_generated_dirs",
        "examples/build_system_lib",
        "examples/build_system_tool",
        "examples/build_source_paths",
        "examples/build_dep_handles",
        "examples/build_output_handles",
        "examples/build_install_prefix",
    ];

    for example in runtime_examples.iter().chain(build_examples.iter()) {
        assert!(
            repo_root().join(example).exists(),
            "documented example path should exist: {example}"
        );
    }

    for example in runtime_examples {
        assert!(
            runtime_docs.contains(example),
            "runtime docs should mention {example}"
        );
    }
    let actual_runtime_examples = std::fs::read_dir(repo_root().join("examples"))
        .expect("examples root should exist")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            let is_runtime_model_example = name.starts_with("core_")
                || name.starts_with("memo_")
                || name.starts_with("std_")
                || name == "mixed_models_workspace";
            is_runtime_model_example.then(|| format!("examples/{name}"))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let documented_runtime_examples = runtime_examples
        .iter()
        .map(|example| (*example).to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual_runtime_examples, documented_runtime_examples,
        "runtime model docs should track the full current set of runtime-model example packages"
    );
    for example in build_examples {
        assert!(
            build_docs.contains(example),
            "build docs should mention {example}"
        );
    }
}

#[test]
fn test_book_summary_includes_the_build_direction_page() {
    let summary = std::fs::read_to_string(repo_root().join("book/src/SUMMARY.md"))
        .expect("book summary should exist");

    assert!(
        summary.contains("./055_build/900_direction.md"),
        "book summary should include the build direction page"
    );
}

#[test]
fn test_v2_current_subset_inventory_stays_honest() {
    let generics_note =
        std::fs::read_to_string(repo_root().join("docs/v2-generics-m1.md"))
            .expect("generic milestone note should load");
    let standards_note =
        std::fs::read_to_string(repo_root().join("docs/v2-standards-m2.md"))
            .expect("standards milestone note should load");
    let versions =
        std::fs::read_to_string(repo_root().join("plan/VERSIONS.md"))
            .expect("version note should load");

    assert!(generics_note.contains("examples/generic_routine_m1"));
    assert!(generics_note.contains("examples/generic_routine_pair_m1"));
    assert!(generics_note.contains("examples/generic_routine_cross_file_m1"));
    assert!(generics_note.contains("examples/fail_generic_type_m1"));
    assert!(generics_note.contains("examples/fail_generic_misuse_m1"));
    assert!(generics_note.contains("examples/fail_generic_cross_file_m1"));
    assert!(generics_note.contains("examples/fail_generic_standard_constraint_m1m2"));
    assert!(generics_note.contains("generic routine lowering is still explicitly unsupported"));
    assert!(generics_note.contains("receiver-qualified generic routines"));
    assert!(generics_note.contains("imported and cross-file generic routine calls"));

    assert!(standards_note.contains("examples/standards_protocol_m2"));
    assert!(standards_note.contains("examples/standards_protocol_pair_m2"));
    assert!(standards_note.contains("examples/standards_protocol_multi_m2"));
    assert!(standards_note.contains("examples/fail_standard_blueprint_m2"));
    assert!(standards_note.contains("examples/fail_standard_as_type_m2"));
    assert!(standards_note.contains("examples/fail_standard_missing_routine_m2"));
    assert!(standards_note.contains("examples/fail_standard_signature_m2"));
    assert!(standards_note.contains("examples/fail_standard_import_ambiguity_m2"));
    assert!(standards_note.contains("lowering/backend still stop at an explicit Milestone 2 boundary"));
    assert!(standards_note.contains("multi-standard conformance on one type"));
    assert!(standards_note.contains("imported-standard conformance truth"));

    assert!(versions.contains("Milestone 1"));
    assert!(versions.contains("generic routine core"));
    assert!(versions.contains("Milestone 2"));
    assert!(versions.contains("protocol standards only"));
}

#[test]
fn test_v2_m1_example_matrix_stays_honest() {
    let actual_examples = [
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/fail_generic_type_m1",
        "examples/fail_generic_misuse_m1",
        "examples/fail_generic_cross_file_m1",
        "examples/fail_generic_standard_constraint_m1m2",
    ]
    .into_iter()
    .map(|path| path.to_string())
    .collect::<std::collections::BTreeSet<_>>();
    let docs = std::fs::read_to_string(repo_root().join("docs/v2-generics-m1.md"))
        .expect("generic milestone note should load");

    for example in &actual_examples {
        assert!(repo_root().join(example).is_dir(), "generic example should exist: {example}");
        assert!(docs.contains(example), "generic docs should mention {example}");
    }
}

#[test]
fn test_v2_m2_example_matrix_stays_honest() {
    let actual_examples = [
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/fail_standard_blueprint_m2",
        "examples/fail_standard_as_type_m2",
        "examples/fail_standard_missing_routine_m2",
        "examples/fail_standard_signature_m2",
        "examples/fail_standard_import_ambiguity_m2",
    ]
    .into_iter()
    .map(|path| path.to_string())
    .collect::<std::collections::BTreeSet<_>>();
    let docs = std::fs::read_to_string(repo_root().join("docs/v2-standards-m2.md"))
        .expect("standards milestone note should load");

    for example in &actual_examples {
        assert!(repo_root().join(example).is_dir(), "standards example should exist: {example}");
        assert!(docs.contains(example), "standards docs should mention {example}");
    }
}

#[test]
fn test_v2_example_inventory_by_naming_convention_stays_visible() {
    let examples_root = repo_root().join("examples");
    let actual_generics = std::fs::read_dir(&examples_root)
        .expect("examples directory should load")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            ((name.starts_with("generic_") && name.ends_with("_m1"))
                || (name.starts_with("fail_generic_")
                    && (name.ends_with("_m1") || name.ends_with("_m1m2"))))
            .then(|| format!("examples/{name}"))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let actual_standards = std::fs::read_dir(&examples_root)
        .expect("examples directory should load")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            ((name.starts_with("standards_") || name.starts_with("fail_standard_"))
                && name.ends_with("_m2"))
            .then(|| format!("examples/{name}"))
        })
        .collect::<std::collections::BTreeSet<_>>();

    let documented_generics = [
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/fail_generic_type_m1",
        "examples/fail_generic_misuse_m1",
        "examples/fail_generic_cross_file_m1",
        "examples/fail_generic_standard_constraint_m1m2",
    ]
    .into_iter()
    .map(|path| path.to_string())
    .collect::<std::collections::BTreeSet<_>>();
    let documented_standards = [
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/fail_standard_blueprint_m2",
        "examples/fail_standard_as_type_m2",
        "examples/fail_standard_missing_routine_m2",
        "examples/fail_standard_signature_m2",
        "examples/fail_standard_import_ambiguity_m2",
    ]
    .into_iter()
    .map(|path| path.to_string())
    .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(actual_generics, documented_generics);
    assert_eq!(actual_standards, documented_standards);
}

#[test]
fn test_v2_docs_and_book_track_the_current_example_matrix() {
    let generics_note =
        std::fs::read_to_string(repo_root().join("docs/v2-generics-m1.md"))
            .expect("generic milestone note should load");
    let standards_note =
        std::fs::read_to_string(repo_root().join("docs/v2-standards-m2.md"))
            .expect("standards milestone note should load");
    let generics_book =
        std::fs::read_to_string(repo_root().join("book/src/500_items/500_generics.md"))
            .expect("generics book chapter should load");
    let standards_book =
        std::fs::read_to_string(repo_root().join("book/src/500_items/400_standards.md"))
            .expect("standards book chapter should load");

    for example in [
        "examples/generic_routine_m1",
        "examples/generic_routine_pair_m1",
        "examples/generic_routine_cross_file_m1",
        "examples/fail_generic_type_m1",
        "examples/fail_generic_misuse_m1",
        "examples/fail_generic_cross_file_m1",
        "examples/fail_generic_standard_constraint_m1m2",
    ] {
        assert!(generics_note.contains(example), "generics milestone note should mention {example}");
        assert!(generics_book.contains(example), "generics book chapter should mention {example}");
    }

    for example in [
        "examples/standards_protocol_m2",
        "examples/standards_protocol_pair_m2",
        "examples/standards_protocol_multi_m2",
        "examples/fail_standard_blueprint_m2",
        "examples/fail_standard_as_type_m2",
        "examples/fail_standard_missing_routine_m2",
        "examples/fail_standard_signature_m2",
        "examples/fail_standard_import_ambiguity_m2",
    ] {
        assert!(standards_note.contains(example), "standards milestone note should mention {example}");
        assert!(standards_book.contains(example), "standards book chapter should mention {example}");
    }
}

#[test]
fn test_bundled_std_docs_and_readme_keep_the_shipped_surface_honest() {
    let bundled_std_docs = std::fs::read_to_string(repo_root().join("docs/bundled-std.md"))
        .expect("bundled std docs should exist");
    let bundled_std_readme = std::fs::read_to_string(repo_root().join("lang/library/std/README.md"))
        .expect("bundled std readme should exist");

    for path in [
        "lang/library/std/lib.fol",
        "lang/library/std/fmt/root.fol",
        "lang/library/std/fmt/math/lib.fol",
        "lang/library/std/io/lib.fol",
        "examples/std_bundled_fmt/build.fol",
        "examples/std_bundled_io/build.fol",
        "examples/std_alias_pkg/build.fol",
        "examples/std_substrate_echo/build.fol",
    ] {
        assert!(
            repo_root().join(path).is_file(),
            "bundled std shipped-surface contract should keep file '{}'",
            path
        );
    }
    assert!(
        !repo_root().join("lang/library/std/os").exists(),
        "bundled std should not claim std.os before it ships honest source"
    );

    for needle in [
        "std.fmt",
        "std.fmt.math",
        "std.io",
        "fmt::answer(): int",
        "fmt::double(int): int",
        "fmt::triple(int): int",
        "fmt::sum2(int, int): int",
        "fmt::math::answer(): int",
        "io::echo_int(int): int",
        "io::echo_str(str): str",
        "io::echo_bool(bol): bol",
        "io::echo_chr(chr): chr",
        "examples/std_bundled_fmt",
        "examples/std_bundled_io",
        "examples/std_alias_pkg",
        "examples/std_substrate_echo",
    ] {
        assert!(
            bundled_std_docs.contains(needle),
            "bundled std docs should mention shipped surface item '{needle}'"
        );
        assert!(
            bundled_std_readme.contains(needle),
            "bundled std readme should mention shipped surface item '{needle}'"
        );
    }

    for forbidden in ["std.os/lib.fol", "std.memo", "std.fs", "std.net"] {
        assert!(
            !bundled_std_docs.contains(forbidden),
            "bundled std docs should not claim unshipped surface '{forbidden}'"
        );
        assert!(
            !bundled_std_readme.contains(forbidden),
            "bundled std readme should not claim unshipped surface '{forbidden}'"
        );
    }
}

#[test]
fn test_active_repo_surface_keeps_runtime_memo_name_and_no_stale_alloc_refs() {
    let tracked_roots = [
        "AGENTS.md",
        "docs",
        "book",
        "examples",
        "lang/library/std",
        "lang/execution/fol-runtime/src",
        "lang/execution/fol-backend/src",
        "lang/tooling/fol-frontend/src",
        "lang/tooling/fol-editor/src",
        "test/integration_tests",
    ];

    let stale_needles = [
        "fol_runtime::alloc",
        "use fol_runtime::alloc",
        "alloc as rt",
        "runtime_module=fol_runtime::alloc",
        "base_alloc_tier",
        "pub mod alloc;",
        "include_str!(\"alloc.rs\")",
    ];

    for root in tracked_roots {
        let path = repo_root().join(root);
        let mut command = std::process::Command::new("rg");
        command.current_dir(repo_root()).args(["-n", "--fixed-strings"]);
        command.arg("-g").arg("!test/integration_tests/integration_editor_and_build.rs");
        for needle in stale_needles {
            command.arg("-e").arg(needle);
        }
        let output = command
            .arg(&path)
            .output()
            .expect("repo stale-name scan should run");

        assert!(
            !output.status.success(),
            "active repo surface '{}' should not keep stale alloc runtime refs:\n{}",
            root,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let runtime_models = std::fs::read_to_string(repo_root().join("docs/runtime-models.md"))
        .expect("runtime-model docs should exist");
    assert!(
        runtime_models.contains("fol_runtime::memo"),
        "runtime-model docs should name the internal memo seam"
    );
}

#[test]
fn test_standard_dependency_contract_matrix_holds() {
    let temp_root = unique_temp_root("standard_dependency_contract");
    let store_root = temp_root.join("pkg");
    let core_root = temp_root.join("core_app");
    let core_fail_root = temp_root.join("core_fail");
    let memo_root = temp_root.join("memo_app");
    let std_root = temp_root.join("std_app");
    let missing_root = temp_root.join("missing_std_app");

    write_model_app_package(
        &core_root,
        "core_app",
        "core",
        "fun[] main(): int = {\n    return 0;\n};\n",
        false,
    );
    write_model_app_package(
        &core_fail_root,
        "core_fail",
        "core",
        "fun[] main(): int = {\n    var bad: str = \"nope\";\n    return .len(bad);\n};\n",
        false,
    );
    write_model_app_package(
        &memo_root,
        "memo_app",
        "memo",
        "fun[] main(): int = {\n    var value: str = \"memo\";\n    return .len(value);\n};\n",
        false,
    );
    write_model_app_package(
        &std_root,
        "std_app",
        "memo",
        concat!(
            "use std: pkg = {\"std\"};\n",
            "fun[] main(): int = {\n",
            "    return std::fmt::double(21);\n",
            "};\n",
        ),
        false,
    );
    let std_build = std::fs::read_to_string(std_root.join("build.fol"))
        .expect("std build should exist");
    std::fs::write(
        std_root.join("build.fol"),
        std_build.replace(
            "    var graph = build.graph();\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n    var graph = build.graph();\n",
        ),
    )
    .expect("std build should add bundled standard dependency");

    write_model_app_package(
        &missing_root,
        "missing_std_app",
        "memo",
        concat!(
            "use std: pkg = {\"std\"};\n",
            "fun[] main(): int = {\n",
            "    return std::fmt::answer();\n",
            "};\n",
        ),
        false,
    );

    let core_build = run_fol_in_dir(&core_root, &["code", "build"]);
    assert!(
        core_build.status.success(),
        "core app should build without std dependency: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&core_build.stdout),
        String::from_utf8_lossy(&core_build.stderr)
    );

    let memo_build = run_fol_in_dir(&memo_root, &["code", "build"]);
    assert!(
        memo_build.status.success(),
        "memo app should build without std dependency: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&memo_build.stdout),
        String::from_utf8_lossy(&memo_build.stderr)
    );

    let core_fail = run_fol_in_dir(&core_fail_root, &["code", "build"]);
    assert!(
        !core_fail.status.success(),
        "core app should reject heap-backed families: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&core_fail.stdout),
        String::from_utf8_lossy(&core_fail.stderr)
    );
    assert!(String::from_utf8_lossy(&core_fail.stderr).contains("fol_model = core"));

    let missing_build = run_fol_with_store_in_dir(&missing_root, &store_root, &["code", "build"]);
    assert!(
        !missing_build.status.success(),
        "pkg std import without declared dependency materialization should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&missing_build.stdout),
        String::from_utf8_lossy(&missing_build.stderr)
    );

    let workspace = fol_frontend::FrontendWorkspace {
        root: fol_frontend::WorkspaceRoot::new(std_root.clone()),
        members: vec![fol_frontend::PackageRoot::new(std_root.clone())],
        std_root_override: None,
        package_store_root_override: Some(store_root.clone()),
        build_root: std_root.join(".fol/build"),
        cache_root: std_root.join(".fol/cache"),
        git_cache_root: std_root.join(".fol/cache/git"),
        install_prefix: std_root.join(".fol/out"),
    };
    fol_frontend::fetch_workspace(&workspace)
        .expect("internal standard dependency should materialize");

    let std_build_output = run_fol_with_store_in_dir(&std_root, &store_root, &["code", "build"]);
    let std_stdout = String::from_utf8_lossy(&std_build_output.stdout);
    assert!(
        std_build_output.status.success(),
        "declared internal standard dependency should unlock pkg std imports: stdout=\n{}\nstderr=\n{}",
        std_stdout,
        String::from_utf8_lossy(&std_build_output.stderr)
    );
    assert!(std_stdout.contains("capability_mode=memo"));
    assert!(std_stdout.contains("bundled_std=1/1"));

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_bundled_std_bootstrap_contract_matrix_stays_coherent() {
    use fol_editor::{
        EditorConfig, EditorDocumentUri, EditorLspServer, JsonRpcId, JsonRpcNotification,
        JsonRpcRequest, LspDidOpenTextDocumentParams, LspSemanticTokens,
        LspSemanticTokensParams, LspTextDocumentIdentifier, LspTextDocumentItem,
    };

    let root = temp_example_root("examples/std_bundled_io");
    let store_root = repo_root().join("lang/library");
    let build = run_fol_with_store_in_dir(
        &root,
        &store_root,
        &["--keep-build-dir", root.to_str().expect("example root should be valid UTF-8")],
    );
    assert!(
        build.status.success(),
        "bundled std contract matrix example should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let crate_root = emitted_crate_root(&build);
    let generated = find_file_by_name(&crate_root, "main.rs")
        .expect("bundled std contract matrix should emit backend source");
    let source = std::fs::read_to_string(&generated).expect("generated source should load");
    assert!(source.contains("use fol_runtime::std as rt;"));

    let binary = built_binary_path(&build);
    let run = std::process::Command::new(&binary)
        .output()
        .expect("bundled std contract matrix binary should run");
    assert!(run.status.success());
    assert!(String::from_utf8_lossy(&run.stdout).contains("std-io"));

    materialize_local_bundled_std_alias(&root);
    let source_path = root.join("src/main.fol");
    let text = std::fs::read_to_string(&source_path).expect("example source should load");
    let uri = EditorDocumentUri::from_file_path(source_path)
        .expect("example uri should build")
        .as_str()
        .to_string();
    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = server
        .handle_notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(LspDidOpenTextDocumentParams {
                    text_document: LspTextDocumentItem {
                        uri: uri.clone(),
                        language_id: "fol".to_string(),
                        version: 1,
                        text,
                    },
                })
                .unwrap(),
            ),
        })
        .expect("didOpen should succeed");
    assert!(diagnostics.iter().all(|published| published.diagnostics.is_empty()));

    let semantic = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: JsonRpcId::Number(4991),
            method: "textDocument/semanticTokens/full".to_string(),
            params: Some(
                serde_json::to_value(LspSemanticTokensParams {
                    text_document: LspTextDocumentIdentifier { uri },
                })
                .unwrap(),
            ),
        })
        .expect("semantic token request should succeed")
        .expect("semantic token response should exist");
    let tokens: LspSemanticTokens = serde_json::from_value(semantic.result.unwrap()).unwrap();
    assert!(
        !tokens.data.is_empty(),
        "bundled std contract matrix should keep editor readability"
    );
}

#[test]
fn test_positive_runtime_model_examples_build_with_expected_models_and_runtime_imports() {
    for (path, expected_model) in positive_runtime_model_examples() {
        let root = temp_example_root(path);
        let build = run_example_compile(&root, true);
        let stdout = String::from_utf8_lossy(&build.stdout);
        assert!(
            build.status.success(),
            "positive runtime model example '{path}' should build: stdout=\n{}\nstderr=\n{}",
            stdout,
            String::from_utf8_lossy(&build.stderr)
        );
        if *path != "examples/mixed_models_workspace" {
            let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
                .expect("generated backend source should exist");
            let source =
                std::fs::read_to_string(&generated).expect("generated backend source should load");
            let expected_import = expected_runtime_import_for_model(expected_model);
            assert!(
                source.contains(&expected_import),
                "positive runtime model example '{path}' should emit '{expected_import}' in {:?}:\n{}",
                generated,
                source
            );
        }
    }
}

#[test]
fn test_negative_runtime_model_examples_fail_with_expected_boundary_class() {
    let cases = [
        (
            "examples/fail_core_heap_reject",
            None,
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "examples/fail_memo_echo",
            None,
            "'.echo(...)' requires hosted std support",
        ),
        (
            "examples/fail_core_alloc_boundary",
            Some("app"),
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "examples/fail_core_std_import",
            None,
            "bundled std imports require 'fol_model = memo'; current artifact model is 'core'",
        ),
        (
            "examples/fail_memo_std_missing_dep",
            None,
            "std",
        ),
    ];

    for (path, subdir, expected_message) in cases {
        let root = temp_example_root(path);
        let working_root = subdir.map(|value| root.join(value)).unwrap_or(root.clone());
        let store_root = if path == "examples/fail_core_std_import" {
            let root = unique_temp_root("fail_core_std_import_matrix_store");
            std::fs::create_dir_all(root.join("std/fmt")).expect("should create std source root");
            std::fs::write(
                root.join("std/build.fol"),
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
            )
            .expect("should write std build");
            std::fs::write(
                root.join("std/fmt/root.fol"),
                "fun[exp] answer(): int = {\n    return 42;\n};\n",
            )
            .expect("should write std source");
            Some(root)
        } else {
            None
        };
        let build = if let Some(store_root) = store_root.as_ref() {
            run_fol_with_store_in_dir(&working_root, store_root, &["code", "build"])
        } else {
            run_fol_in_dir(&working_root, &["code", "build"])
        };
        let stderr = String::from_utf8_lossy(&build.stderr);
        assert!(
            !build.status.success(),
            "negative runtime model example '{path}' should fail: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            stderr
        );
        assert!(
            stderr.contains(expected_message),
            "negative runtime model example '{path}' should report '{expected_message}': stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&build.stdout),
            stderr
        );
        if let Some(store_root) = store_root {
            std::fs::remove_dir_all(store_root).ok();
        }
    }
}

#[test]
fn test_runtime_model_regression_matrix_stays_coherent_across_layers() {
    let direct_cases = [
        (
            "test/app/build/model_core_surface_full",
            true,
            Some("use fol_runtime::core as rt;"),
        ),
        (
            "test/app/build/model_memo_surface_full",
            true,
            Some("use fol_runtime::memo as rt;"),
        ),
        (
            "examples/fail_core_heap_reject",
            false,
            Some("str requires heap support and is unavailable in 'fol_model = core'"),
        ),
    ];

    for (path, should_succeed, expected) in direct_cases {
        let root = if path.starts_with("examples/") {
            temp_example_root(path)
        } else {
            repo_root().join(path)
        };
        let build = if path.starts_with("examples/") {
            run_example_compile(&root, true)
        } else {
            run_fol_in_dir(&root, &["code", "build", "--keep-build-dir"])
        };
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&build.stdout),
            String::from_utf8_lossy(&build.stderr)
        );
        assert_eq!(
            build.status.success(),
            should_succeed,
            "runtime matrix direct case '{path}' should have success={should_succeed}: output=\n{combined}"
        );
        if let Some(expected) = expected {
            if should_succeed {
                let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
                    .expect("generated backend source should exist");
                let source = std::fs::read_to_string(&generated)
                    .expect("generated backend source should load");
                assert!(
                    source.contains(expected),
                    "runtime matrix direct case '{path}' should emit '{expected}' in {:?}:\n{}",
                    generated,
                    source
                );
            } else {
                assert!(
                    combined.contains(expected),
                    "runtime matrix direct case '{path}' should mention '{expected}': output=\n{combined}"
                );
            }
        }
    }

    let mixed_root = temp_example_root("examples/mixed_models_workspace");
    let run = run_fol_with_store_in_dir(
        &mixed_root,
        &repo_root().join("lang/library"),
        &["code", "run"],
    );
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "mixed-model std run should succeed");
    assert!(run_stdout.contains("7"));

    let emitted_root = temp_example_root("examples/std_surface_showcase");
    let build = run_example_compile(&emitted_root, true);
    assert!(build.status.success(), "std emitted-import matrix should build");
    let generated = find_file_by_name(&emitted_crate_root(&build), "main.rs")
        .expect("generated std source should exist");
    let source = std::fs::read_to_string(generated).expect("generated std source should load");
    assert!(source.contains("use fol_runtime::std as rt;"));
}

#[test]
fn test_example_directories_stay_source_only_without_checked_in_build_artifacts() {
    let examples_root = repo_root().join("examples");
    for forbidden in [".fol", "package.yaml", "out.txt", "err.txt"] {
        let output = std::process::Command::new("find")
            .arg(&examples_root)
            .args(["-name", forbidden])
            .output()
            .expect("find should succeed");
        assert!(output.status.success(), "find should succeed for {forbidden}");
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(
            stdout.is_empty(),
            "examples should not keep checked-in '{forbidden}' artifacts:\n{stdout}"
        );
    }
}

#[test]
fn test_build_fixtures_core_model_reject_forbidden_surfaces() {
    let cases = [
        (
            "model_core_reject_str",
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_seq",
            "seq[...] requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_vec",
            "vec[...] requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_set",
            "set[...] requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_map",
            "map[...] requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_inferred_str",
            "string literals require heap support and are unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_heap_mix",
            "requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_return_string",
            "str requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_body_vec",
            "vec[...] requires heap support and is unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_dynamic_len_string",
            "string literals require heap support and are unavailable in 'fol_model = core'",
        ),
        (
            "model_core_reject_echo",
            "'.echo(...)' requires hosted std support",
        ),
    ];

    for (fixture, needle) in cases {
        let root = build_fixture_root(fixture);
        let build = run_fol_in_dir(&root, &["code", "build"]);
        let stderr = String::from_utf8_lossy(&build.stderr);
        assert!(
            !build.status.success(),
            "core forbidden-surface fixture should fail for {}: stdout=\n{}\nstderr=\n{}",
            fixture,
            String::from_utf8_lossy(&build.stdout),
            stderr
        );
        assert!(
                stderr.contains(needle),
                "core forbidden-surface fixture should report the expected diagnostic for {}: stdout=\n{}\nstderr=\n{}",
                fixture,
                String::from_utf8_lossy(&build.stdout),
                stderr
            );
    }
}

#[test]
fn test_build_fixture_mem_model_rejects_echo() {
    let root = build_fixture_root("model_memo_reject_echo");
    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !build.status.success(),
        "memo echo fixture should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(stderr.contains("'.echo(...)' requires hosted std support"));
    assert!(stderr.contains("current artifact model is 'memo'"));
}

#[test]
fn test_negative_core_model_example_fails_with_heap_boundary_diagnostic() {
    let root = temp_example_root("examples/fail_core_heap_reject");
    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);

    assert!(
        !build.status.success(),
        "negative core model example should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(
        stderr.contains("str requires heap support and is unavailable in 'fol_model = core'"),
        "negative core model example should keep the heap-boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_negative_mem_model_example_fails_with_hosted_boundary_diagnostic() {
    let root = temp_example_root("examples/fail_memo_echo");
    let build = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);

    assert!(
        !build.status.success(),
        "negative memo model example should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(
        stderr.contains("'.echo(...)' requires hosted std support"),
        "negative memo model example should keep the hosted-boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_negative_transitive_core_mem_boundary_example_fails_cleanly() {
    let root = temp_example_root("examples/fail_core_alloc_boundary");
    let build = run_fol_in_dir(&root.join("app"), &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);

    assert!(
        !build.status.success(),
        "negative transitive core/memo example should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(
        stderr.contains("str requires heap support and is unavailable in 'fol_model = core'"),
        "negative transitive core/memo example should keep the heap-boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
}

#[test]
fn test_negative_core_std_import_example_fails_with_std_boundary_diagnostic() {
    let root = temp_example_root("examples/fail_core_std_import");
    let store_root = unique_temp_root("fail_core_std_import_store");
    std::fs::create_dir_all(store_root.join("std/fmt")).expect("should create std source root");
    std::fs::write(
        store_root.join("std/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
    )
    .expect("should write std build");
    std::fs::write(
        store_root.join("std/fmt/root.fol"),
        "fun[exp] answer(): int = {\n    return 42;\n};\n",
    )
    .expect("should write std source");
    let build = run_fol_with_store_in_dir(&root, &store_root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&build.stderr);

    assert!(
        !build.status.success(),
        "negative core std-import example should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    assert!(
        stderr.contains("bundled std imports require 'fol_model = memo'; current artifact model is 'core'"),
        "negative core std-import example should keep the bundled std boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        stderr
    );
    std::fs::remove_dir_all(store_root).ok();
}

#[test]
fn test_cli_code_build_rejects_old_root_build_syntax() {
    let root = unique_temp_root("old_root_build_syntax");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(root.join("build.fol"), "def root: loc = \"src\";\n")
        .expect("should write old build syntax");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "old root build syntax should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("canonical `pro[] build(): non` entry"),
        "old root build syntax should point at the canonical build entry: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_cli_code_build_rejects_plain_pro_build_headers() {
    let root = unique_temp_root("plain_pro_build_header");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        "pro build(): non = {\n    return;\n};\n",
    )
    .expect("should write non-canonical build header");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "plain pro build header should fail: stdout=;\n{};\nstderr=;\n{};",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
            stderr.contains("canonical `pro[] build(): non` entry")
                || stderr.contains("missing required field 'name'"),
            "plain pro build header should fail through the build.fol contract: stdout=;\n{};\nstderr=;\n{};",
            String::from_utf8_lossy(&output.stdout),
            stderr
        );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_cli_code_build_rejects_empty_build_file() {
    let root = unique_temp_root("empty_build_file");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(root.join("build.fol"), "").expect("should write empty build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);

    assert!(
        !output.status.success(),
        "empty build.fol should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_cli_code_build_rejects_missing_build_metadata() {
    let root = unique_temp_root("missing_build_metadata");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);

    assert!(
        !output.status.success(),
        "missing build metadata should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing required field 'name'"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_cli_code_build_rejects_missing_source_root() {
    let root = unique_temp_root("missing_source_root");
    std::fs::create_dir_all(&root).expect("should create root dir");
    std::fs::write(root.join("build.fol"), semantic_bin_build("demo"))
        .expect("should write build file");
    // Intentionally no src/main.fol

    let output = run_fol_in_dir(&root, &["code", "build"]);

    assert!(
        !output.status.success(),
        "missing source root should fail: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn test_cli_code_build_keeps_core_string_boundary_diagnostic() {
    let temp_root = unique_temp_root("build_core_string_boundary");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"core\",\n",
            "    });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): str = {\n    return \"ok\";\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "core string boundary should fail");
    assert!(
        stderr.contains("str requires heap support and is unavailable in 'fol_model = core'"),
        "CLI should preserve the core string boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_code_build_keeps_memo_echo_boundary_diagnostic() {
    let temp_root = unique_temp_root("build_memo_echo_boundary");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"memo\",\n",
            "    });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return .echo(1);\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "memo echo boundary should fail");
    assert!(
        stderr
            .contains("'.echo(...)' requires hosted std support"),
        "CLI should preserve the memo echo boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_code_build_keeps_core_dynamic_len_boundary_diagnostic() {
    let temp_root = unique_temp_root("build_core_len_boundary");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"core\",\n",
            "    });\n",
            "    graph.install(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return .len(\"Ada\");\n};\n",
    )
    .expect("should write app source");

    let output = run_fol_in_dir(&root, &["code", "build"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "core dynamic len boundary should fail"
    );
    assert!(
        stderr.contains(
            "string literals require heap support and are unavailable in 'fol_model = core'"
        ),
        "CLI should preserve the core dynamic len boundary wording: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_code_build_and_run_keep_std_model_runtime_path() {
    let temp_root = unique_temp_root("build_std_model_runtime");
    let root = temp_root.join("demo");
    std::fs::create_dir_all(root.join("src")).expect("should create source root");
    std::fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/main.fol\",\n",
            "        fol_model = \"memo\",\n",
            "    });\n",
            "    graph.install(app);\n",
            "    graph.add_run(app);\n",
            "    return;\n",
            "};\n",
        ),
    )
    .expect("should write build file");
    std::fs::write(
        root.join("src/main.fol"),
        concat!(
            "use std: pkg = {\"std\"};\n",
            "fun[] main(): int = {\n",
            "    return std::io::echo_int(7);\n",
            "};\n",
        ),
    )
    .expect("should write app source");

    let build = run_fol_with_store_in_dir(
        &root,
        &repo_root().join("lang/library"),
        &["code", "build", "--keep-build-dir"],
    );
    let build_stdout = String::from_utf8_lossy(&build.stdout);
    assert!(
        build.status.success(),
        "memo+standard build should succeed: stdout=\n{}\nstderr=\n{}",
        build_stdout,
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        build_stdout.contains("built 1 workspace package(s)"),
        "memo+standard build should report a build summary: stdout=\n{}\nstderr=\n{}",
        build_stdout,
        String::from_utf8_lossy(&build.stderr)
    );

    let run = run_fol_with_store_in_dir(&root, &repo_root().join("lang/library"), &["code", "run"]);
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "memo+standard run should succeed: stdout=\n{}\nstderr=\n{}",
        run_stdout,
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run_stdout.contains("7"),
        "memo+standard run should execute through runtime std path: stdout=\n{}\nstderr=\n{}",
        run_stdout,
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run_stdout.contains("ran "),
        "memo+standard run should report a run summary: stdout=\n{}\nstderr=\n{}",
        run_stdout,
        String::from_utf8_lossy(&run.stderr)
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_lsp_unknown_method_returns_method_not_found_error() {
    let mut server = EditorLspServer::new(EditorConfig::default());

    let result = server.handle_request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(999),
        method: "textDocument/nonExistentMethod".to_string(),
        params: None,
    });

    match result {
        Ok(Some(response)) => {
            assert!(
                response.error.is_some(),
                "Unknown LSP method should return an error response"
            );
        }
        Ok(None) => {
            // Also acceptable: server ignores unknown methods
        }
        Err(_) => {
            // Also acceptable: server rejects at dispatch level
        }
    }
}

#[test]
fn test_lsp_document_with_syntax_errors_returns_diagnostics() {
    let temp_root = unique_temp_root("lsp_syntax_errors");
    std::fs::create_dir_all(&temp_root).expect("should create temp root");
    let uri = format!("file://{}", temp_root.join("bad.fol").display());
    let bad_source = "fun[] main( = {\n    return;\n};\n";

    let mut server = EditorLspServer::new(EditorConfig::default());
    let diagnostics = server
        .handle_notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(LspDidOpenTextDocumentParams {
                    text_document: LspTextDocumentItem {
                        uri: uri.clone(),
                        language_id: "fol".to_string(),
                        version: 1,
                        text: bad_source.to_string(),
                    },
                })
                .expect("didOpen params should serialize"),
            ),
        })
        .expect("didOpen with syntax errors should not crash the server");

    assert!(
        !diagnostics.is_empty(),
        "Documents with syntax errors should still produce diagnostic notifications"
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_lsp_hover_on_empty_document_does_not_crash() {
    let temp_root = unique_temp_root("lsp_empty_hover");
    std::fs::create_dir_all(&temp_root).expect("should create temp root");
    let uri = format!("file://{}", temp_root.join("empty.fol").display());

    let mut server = EditorLspServer::new(EditorConfig::default());
    open_lsp_document(&mut server, uri.clone(), "");

    let hover = server.handle_request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(1),
        method: "textDocument/hover".to_string(),
        params: Some(
            serde_json::to_value(LspHoverParams {
                text_document: LspTextDocumentIdentifier { uri },
                position: LspPosition {
                    line: 0,
                    character: 0,
                },
            })
            .expect("hover params should serialize"),
        ),
    });

    assert!(
        hover.is_ok(),
        "Hover on empty document should not crash: {:?}",
        hover
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
#[ignore = "exercises git fetch through a local fixture remote"]
fn test_frontend_fetches_public_logtiny_from_github() {
    let temp_root = unique_temp_root("frontend_fetch_public_logtiny");
    let app_root = temp_root.join("app");
    create_app_with_git_dependency_from_url(
        &app_root,
        "git+https://github.com/bresilla/logtiny.git",
        Some("tag:v0.1.4"),
        Some("7b8fe6b033e4"),
    );

    let output = run_fol_in_dir(&app_root, &["pack", "fetch"]);

    assert!(
        output.status.success(),
        "public git fetch should succeed: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        app_root.join("fol.lock").is_file(),
        "public fetch should write fol.lock"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("prepared 1 workspace package"),
        "public fetch should keep the fetch summary"
    );

    std::fs::remove_dir_all(&temp_root).ok();
}

#[test]
#[ignore = "exercises real github branch/tag/commit/hash git fetches"]
fn test_frontend_fetches_public_logtiny_version_matrix_from_github() {
    let cases = [
        (
            "branch",
            "git+https://github.com/bresilla/logtiny.git",
            Some("branch:develop"),
            None,
        ),
        (
            "tag",
            "git+https://github.com/bresilla/logtiny.git",
            Some("tag:v0.1.4"),
            None,
        ),
        (
            "commit",
            "git+https://github.com/bresilla/logtiny.git",
            Some("commit:7b8fe6b033e4ea1f1d5337d5f141353597db7a07"),
            None,
        ),
        (
            "hash",
            "git+https://github.com/bresilla/logtiny.git",
            Some("branch:develop"),
            Some("7b8fe6b033e4"),
        ),
    ];

    for (label, remote_url, version, hash) in cases {
        let temp_root = unique_temp_root(&format!("frontend_fetch_public_logtiny_{label}"));
        let app_root = temp_root.join("app");
        create_app_with_git_dependency_from_url(&app_root, remote_url, version, hash);

        let output = run_fol_in_dir(&app_root, &["pack", "fetch"]);
        assert!(
            output.status.success(),
            "public git fetch should succeed for {label}: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            app_root.join("fol.lock").is_file(),
            "public fetch should write fol.lock for {label}"
        );

        std::fs::remove_dir_all(&temp_root).ok();
    }
}

fn create_app_with_git_dependency_from_url(
    app_root: &Path,
    remote_url: &str,
    version: Option<&str>,
    hash: Option<&str>,
) {
    std::fs::create_dir_all(app_root.join("src")).expect("Should create app source dir");
    let name = app_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    let version_field = version
        .map(|value| format!("        version = \"{value}\",\n"))
        .unwrap_or_default();
    let hash_field = hash
        .map(|value| format!("        hash = \"{value}\",\n"))
        .unwrap_or_default();
    std::fs::write(
            app_root.join("build.fol"),
            format!(
                concat!(
                    "pro[] build(): non = {{\n",
                    "    var build = .build();\n",
                    "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
                    "    build.add_dep({{\n",
                    "        alias = \"logtiny\",\n",
                    "        source = \"git\",\n",
                    "        target = \"{remote}\",\n",
                    "{version_field}",
                    "{hash_field}",
                    "    }});\n",
                    "    var graph = build.graph();\n",
                    "    var app = graph.add_exe({{ name = \"{name}\", root = \"src/main.fol\" }});\n",
                    "    graph.install(app);\n",
                    "    graph.add_run(app);\n",
                    "}};\n",
                ),
                name = name,
                remote = remote_url,
                version_field = version_field,
                hash_field = hash_field,
            ),
        )
            .expect("Should write app build");
    std::fs::write(
        app_root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write app source");
}
