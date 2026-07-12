use crate::{
    require_discovered_root, FrontendCommandResult, FrontendConfig, FrontendError,
    FrontendErrorKind, FrontendResult,
};
use std::path::Path;

fn lower_editor_error(error: fol_editor::EditorError) -> FrontendError {
    let mut frontend = FrontendError::new(FrontendErrorKind::CommandFailed, error.message);
    for note in error.notes {
        frontend = frontend.with_note(note);
    }
    frontend
}

fn editor_summary_to_result(summary: fol_editor::EditorCommandSummary) -> FrontendCommandResult {
    let mut details = summary.details;
    details.sort();
    let rendered = if details.is_empty() {
        summary.summary
    } else {
        format!("{} ({})", summary.summary, details.join(", "))
    };
    FrontendCommandResult::new(summary.command, rendered)
}

pub fn editor_lsp_command(config: &FrontendConfig) -> FrontendResult<FrontendCommandResult> {
    require_discovered_root(&config.working_directory).map_err(|error| {
        if error.kind() == FrontendErrorKind::WorkspaceNotFound {
            error
                .with_note("start the editor inside a FOL package or workspace root")
                .with_note("or open a package directory before launching `fol tool lsp`")
        } else {
            error
        }
    })?;
    fol_editor::editor_lsp_entrypoint()
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_lsp_stdio(config: &FrontendConfig) -> FrontendResult<()> {
    let _ = config;
    fol_editor::run_lsp_stdio(fol_editor::EditorConfig::default()).map_err(lower_editor_error)
}

pub fn editor_completion_command(
    path: &str,
    line: u32,
    character: u32,
) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_completion_file(
        Path::new(path),
        fol_editor::LspPosition { line, character },
    )
    .map(editor_summary_to_result)
    .map_err(lower_editor_error)
}

pub fn editor_format_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_format_file(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_parse_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_parse_file(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_highlight_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_highlight_file(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_symbols_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_symbols_file(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_references_command(
    path: &str,
    line: u32,
    character: u32,
    include_declaration: bool,
) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_references_file(
        Path::new(path),
        fol_editor::LspPosition { line, character },
        include_declaration,
    )
    .map(editor_summary_to_result)
    .map_err(lower_editor_error)
}

pub fn editor_rename_command(
    path: &str,
    line: u32,
    character: u32,
    new_name: &str,
) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_rename_file(
        Path::new(path),
        fol_editor::LspPosition { line, character },
        new_name,
    )
    .map(editor_summary_to_result)
    .map_err(lower_editor_error)
}

pub fn editor_semantic_tokens_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_semantic_tokens_file(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

pub fn editor_tree_generate_command(path: &str) -> FrontendResult<FrontendCommandResult> {
    fol_editor::editor_tree_generate_bundle(Path::new(path))
        .map(editor_summary_to_result)
        .map_err(lower_editor_error)
}

#[cfg(test)]
mod tests {
    use super::{
        editor_format_command, editor_highlight_command, editor_lsp_command, editor_parse_command,
        editor_symbols_command, editor_references_command, editor_rename_command,
        editor_semantic_tokens_command, editor_tree_generate_command,
    };
    use crate::{FrontendConfig, FrontendErrorKind};
    use std::path::{Path, PathBuf};

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

    fn editor_fixture_path() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root should exist")
            .join("test/apps/fixtures/record_flow/main.fol")
            .to_string_lossy()
            .to_string()
    }

    fn editor_config_with_root() -> (PathBuf, FrontendConfig) {
        let temp_root = std::env::temp_dir().join(format!(
            "fol_frontend_editor_roundtrip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let src = temp_root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            temp_root.join("build.fol"),
            "name: editor_test\nversion: 0.1.0\n",
        )
        .unwrap();
        std::fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0\n};\n",
        )
        .unwrap();
        (
            temp_root.clone(),
            FrontendConfig {
                working_directory: temp_root,
                ..FrontendConfig::default()
            },
        )
    }

    #[test]
    fn editor_lsp_command_keeps_public_summary_shape() {
        let (temp_root, config) = editor_config_with_root();
        let lsp = editor_lsp_command(&config).expect("lsp command should succeed");
        assert_eq!(lsp.command, "lsp");
        assert!(lsp.summary.contains("fol tool lsp"));
        assert!(lsp.summary.contains("transport=stdio"));
        assert!(lsp.summary.contains("features="));

        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn editor_format_command_keeps_public_summary_shape() {
        let format_root = temp_root("format_summary");
        std::fs::create_dir_all(&format_root).unwrap();
        let format_path = format_root.join("sample.fol");
        std::fs::write(&format_path, "fun[] main(): int = {\nreturn 0;\n};\n").unwrap();
        let format = editor_format_command(format_path.to_str().unwrap())
            .expect("format command should succeed");

        assert_eq!(format.command, "format");
        assert!(format.summary.contains("changed=true"));
        assert!(format.summary.contains("path="));
        assert!(format.summary.contains("changed_lines="));
        assert!(format.summary.contains("style=hybrid-line"));

        std::fs::remove_dir_all(format_root).ok();
    }

    #[test]
    fn editor_parse_command_keeps_public_summary_shape() {
        let path = editor_fixture_path();
        let parse = editor_parse_command(&path).expect("parse command should succeed");

        assert_eq!(parse.command, "parse");
        assert!(parse.summary.contains("record_flow/main.fol"));
        assert!(parse.summary.contains("grammar_bytes="));
        assert!(parse.summary.contains("lines="));
    }

    #[test]
    fn editor_highlight_command_keeps_public_summary_shape() {
        let path = editor_fixture_path();
        let highlight =
            editor_highlight_command(&path).expect("highlight command should succeed");

        assert_eq!(highlight.command, "highlight");
        assert!(highlight.summary.contains("capture_count="));
        assert!(highlight.summary.contains("captures="));
        assert!(highlight.summary.contains("intrinsic_names="));
    }

    #[test]
    fn editor_symbols_command_keeps_public_summary_shape() {
        let path = editor_fixture_path();
        let symbols = editor_symbols_command(&path).expect("symbols command should succeed");

        assert_eq!(symbols.command, "symbols");
        assert!(symbols.summary.contains("symbol_candidates="));
        assert!(symbols.summary.contains("query_snapshots="));
    }

    #[test]
    fn editor_references_command_keeps_public_summary_shape() {
        let path = editor_fixture_path();
        let references = editor_references_command(&path, 5, 11, true)
            .expect("references command should succeed");

        assert_eq!(references.command, "references");
        assert!(references.summary.contains("reference_count="));
        assert!(references.summary.contains("include_declaration=true"));
        assert!(references.summary.contains("path="));
    }

    #[test]
    fn editor_rename_command_keeps_public_summary_shape() {
        let root = temp_root("rename_summary_shape");
        std::fs::create_dir_all(root.join("src")).expect("should create rename fixture src dir");
        std::fs::write(
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
        std::fs::write(
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
        let path = root.join("src/main.fol").to_string_lossy().to_string();
        let rename = editor_rename_command(&path, 5, 11, "count")
            .expect("rename command should succeed");

        assert_eq!(rename.command, "rename");
        assert!(rename.summary.contains("edit_count="));
        assert!(rename.summary.contains("new_name=count"));
        assert!(rename.summary.contains("path="));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn editor_semantic_tokens_command_keeps_public_summary_shape() {
        let path = editor_fixture_path();
        let semantic_tokens = editor_semantic_tokens_command(&path)
            .expect("semantic-tokens command should succeed");

        assert_eq!(semantic_tokens.command, "semantic-tokens");
        assert!(semantic_tokens.summary.contains("token_count="));
        assert!(semantic_tokens.summary.contains("legend="));
        assert!(semantic_tokens.summary.contains("path="));
    }

    #[test]
    fn editor_tree_generate_command_keeps_public_summary_shape() {
        let tree_root = temp_root("tree_summary");
        let tree = editor_tree_generate_command(tree_root.to_str().unwrap())
            .expect("tree generate command should succeed");

        assert_eq!(tree.command, "tree generate");
        assert!(tree.summary.contains("tree-sitter bundle ready"));
        assert!(tree.summary.contains("query_files="));
        assert!(tree.summary.contains("corpus_files="));

        std::fs::remove_dir_all(tree_root).ok();
    }

    #[test]
    fn editor_file_commands_wrap_missing_path_failures_as_frontend_errors() {
        let error = editor_parse_command(
            "test/apps/fixtures/record_flow/missing.fol",
        )
        .expect_err("missing files should fail");

        assert_eq!(error.kind(), FrontendErrorKind::CommandFailed);
        assert!(error.message().contains("failed to read"));
    }

    #[test]
    fn editor_lsp_command_adds_editor_specific_workspace_guidance() {
        let config = FrontendConfig {
            working_directory: std::env::temp_dir().join(format!(
                "fol_frontend_editor_missing_root_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be after epoch")
                    .as_nanos()
            )),
            ..FrontendConfig::default()
        };
        std::fs::create_dir_all(&config.working_directory).unwrap();

        let error = editor_lsp_command(&config).expect_err("missing roots should fail");

        assert_eq!(error.kind(), FrontendErrorKind::WorkspaceNotFound);
        assert!(error
            .notes()
            .iter()
            .any(|note| note.contains("start the editor inside a FOL package or workspace root")));
        assert!(error
            .notes()
            .iter()
            .any(|note| note.contains("fol tool lsp")));

        std::fs::remove_dir_all(&config.working_directory).ok();
    }
}
