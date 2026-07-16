use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_file_boundaries_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn parse_program_from_folder(
    root: &std::path::Path,
) -> Result<AstNode, Vec<fol_diagnostics::Diagnostic>> {
    let mut file_stream = FileStream::from_folder(
        root.to_str()
            .expect("Temporary folder fixture path should be UTF-8"),
    )
    .expect("Should build a parser folder fixture stream");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer)
}

fn parse_decl_package_from_folder(
    root: &std::path::Path,
) -> Result<ParsedPackage, Vec<fol_diagnostics::Diagnostic>> {
    let mut file_stream = FileStream::from_folder(
        root.to_str()
            .expect("Temporary folder fixture path should be UTF-8"),
    )
    .expect("Should build a declaration-package folder fixture stream");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parser.parse_package(&mut lexer)
}

fn parse_decl_package_errors_from_folder(root: &std::path::Path) -> Vec<fol_diagnostics::Diagnostic> {
    parse_decl_package_from_folder(root)
        .expect_err("Folder fixture should fail declaration-only package parsing")
}

fn write_folder_fixture(root: &std::path::Path, files: &[(&str, &str)]) {
    fs::create_dir_all(root).expect("Should create temporary parser folder fixture");
    for (name, source) in files {
        fs::write(root.join(name), source).expect("Should write temporary parser folder file");
    }
}

#[test]
fn test_complete_top_level_declarations_remain_separate_across_folder_boundaries() {
    let temp_root = unique_temp_root("complete_top_level");
    write_folder_fixture(
        &temp_root,
        &[("00_a.fol", "var a = 1;\n"), ("10_b.fol", "var b = 2;\n")],
    );

    let ast = parse_program_from_folder(&temp_root)
        .expect("Complete declarations in separate files should still parse");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(matches!(
                declarations.as_slice(),
                [AstNode::VarDecl { name: first, .. }, AstNode::VarDecl { name: second, .. }]
                    if first == "a" && second == "b"
            ));
        }
        other => panic!("Expected program node, got {other:?}"),
    }
}

#[test]
fn test_binding_initializer_cannot_continue_into_next_file() {
    let temp_root = unique_temp_root("split_binding_value");
    write_folder_fixture(
        &temp_root,
        &[("00_a.fol", "var a =\n"), ("10_b.fol", "1\n")],
    );

    let parse_result = parse_program_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        parse_result.is_err(),
        "A binding initializer should not continue across a file boundary"
    );
}

#[test]
fn test_routine_header_cannot_continue_into_next_file() {
    let temp_root = unique_temp_root("split_routine_header");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_a.fol", "fun add(value: int);\n"),
            ("10_b.fol", ": int = { return value }\n"),
        ],
    );

    let parse_result = parse_program_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        parse_result.is_err(),
        "A routine signature should not continue across a file boundary"
    );
}

#[test]
fn test_use_path_cannot_continue_into_next_file() {
    let temp_root = unique_temp_root("split_use_path");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_a.fol", "use file: loc = {\"std::fs::\n"),
            ("10_b.fol", "File\n"),
        ],
    );

    let parse_result = parse_program_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        parse_result.is_err(),
        "A use path should not continue across a file boundary"
    );
}

#[test]
fn test_block_body_cannot_continue_into_next_file() {
    let temp_root = unique_temp_root("split_block_body");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_a.fol", "fun value(): int = { return 1;\n"),
            ("10_b.fol", "}\n"),
        ],
    );

    let parse_result = parse_program_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        parse_result.is_err(),
        "An open block body should not continue across a file boundary"
    );
}

#[test]
fn test_decl_package_split_binding_reports_boundary_then_second_file_locations() {
    let temp_root = unique_temp_root("decl_split_binding_locations");
    write_folder_fixture(
        &temp_root,
        &[("00_a.fol", "var a =\n"), ("10_b.fol", "1\n")],
    );

    let errors = parse_decl_package_errors_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        !errors.is_empty(),
        "Split bindings should report at least one boundary-token failure"
    );
    assert!(
        errors[0].message.contains("Unsupported expression token"),
        "Expected the first error to anchor at the synthetic file-boundary token, got: {}",
        errors[0].message
    );
    let loc = errors[0].primary_location().expect("diagnostic should have primary location");
    assert!(
        loc.file
            .as_deref()
            .is_some_and(|path| path.ends_with("10_b.fol")),
        "The boundary-token error should identify the incoming second file"
    );
    assert_eq!(loc.line, 1);
    assert_eq!(loc.column, 0);
}

#[test]
fn test_decl_package_split_use_path_reports_boundary_then_second_file_locations() {
    let temp_root = unique_temp_root("decl_split_use_locations");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_a.fol", "use file: loc = {\"std::fs::\n"),
            ("10_b.fol", "File\n"),
        ],
    );

    let errors = parse_decl_package_errors_from_folder(&temp_root);

    fs::remove_dir_all(&temp_root).ok();

    assert!(
        !errors.is_empty(),
        "Split use paths should report at least one boundary-token failure"
    );
    assert!(
        errors[0].message.contains("Expected '}' after quoted import target")
            || errors[0]
                .message
                .contains("Import targets must be quoted string literals inside braces")
            || errors[0]
                .message
                .contains("Expected '}' to close use path"),
        "Expected a quoted-import boundary diagnostic first, got: {}",
        errors[0].message
    );
    let loc = errors[0].primary_location().expect("diagnostic should have primary location");
    assert!(
        loc.file
            .as_deref()
            .is_some_and(|path| path.ends_with("00_a.fol") || path.ends_with("10_b.fol")),
        "The quoted-import boundary error should stay anchored on one of the split files"
    );
    assert_eq!(loc.line, 1);
}

#[test]
fn test_decl_package_boundary_tokens_never_become_source_unit_items() {
    let temp_root = unique_temp_root("decl_boundary_items");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_alpha.fol", "var alpha = 1;\n\n"),
            ("10_beta.fol", "\nvar beta = 2;\n"),
        ],
    );

    let parsed = parse_decl_package_from_folder(&temp_root)
        .expect("Boundary-separated declaration files should still parse cleanly as a package");

    fs::remove_dir_all(&temp_root).ok();

    assert_eq!(
        parsed.source_units.len(),
        2,
        "Explicit file-boundary markers should separate files without creating phantom source units"
    );
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[0]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "alpha"
    ));
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[1]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "beta"
    ));
}
