use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_source_origins_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_parse_package_retains_successful_top_level_origins() {
    let temp_root = unique_temp_root("top_level");
    fs::create_dir_all(&temp_root).expect("Should create temporary source-origin fixture dir");
    let fixture = temp_root.join("origins.fol");
    fs::write(&fixture, "`doc`\nvar alpha = 1;\nfun beta(): int = { return alpha };\n")
        .expect("Should write temporary source-origin fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary source-origin fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Source-origin fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let source_unit = parsed
        .source_units
        .first()
        .expect("Single-file parse package should expose one source unit");
    assert_eq!(
        source_unit.items.len(),
        3,
        "Comment, binding, and routine should all retain successful origins"
    );

    let comment_origin = parsed_top_level_origin(&parsed, &source_unit.items[0]);
    let var_origin = parsed_top_level_origin(&parsed, &source_unit.items[1]);
    let fun_origin = parsed_top_level_origin(&parsed, &source_unit.items[2]);

    assert_eq!(comment_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(comment_origin.line, 1);
    assert_eq!(comment_origin.column, 1);

    assert_eq!(var_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(var_origin.line, 2);
    assert_eq!(var_origin.column, 1);

    assert_eq!(fun_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(fun_origin.line, 3);
    // Routine items anchor at the declared NAME (`beta`), not the keyword.
    assert_eq!(fun_origin.column, 5);
}

#[test]
fn test_parse_package_retains_item_origins_across_multiple_files() {
    let temp_root = unique_temp_root("multi_file");
    fs::create_dir_all(&temp_root).expect("Should create temporary multi-file origin fixture dir");
    let alpha = temp_root.join("00_alpha.fol");
    let beta = temp_root.join("10_beta.fol");
    fs::write(&alpha, "var alpha = 1;\n").expect("Should write first multi-file origin fixture");
    fs::write(&beta, "var beta = 2;\n").expect("Should write second multi-file origin fixture");

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary multi-file origin fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    let first_origin = parsed_top_level_origin(&parsed, &parsed.source_units[0].items[0]);
    let second_origin = parsed_top_level_origin(&parsed, &parsed.source_units[1].items[0]);

    assert_ne!(
        first_origin.file,
        second_origin.file,
        "Parsed top-level origins should keep the physical file identity of each source unit"
    );
    assert_eq!(first_origin.line, 1);
    assert_eq!(second_origin.line, 1);
}

#[test]
fn test_parse_package_retains_nested_routine_origins() {
    let temp_root = unique_temp_root("nested_routine");
    fs::create_dir_all(&temp_root).expect("Should create temporary nested-routine fixture dir");
    let fixture = temp_root.join("nested_routines.fol");
    fs::write(
        &fixture,
        "fun outer(): int = {\n    fun inner(): int = { return 1 };\n    return inner();\n};\n",
    )
    .expect("Should write temporary nested-routine fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary nested-routine fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Nested-routine fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let outer_body = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl { body, .. } => body,
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let inner_origin = ast_node_origin(&parsed, &outer_body[0]);
    assert_eq!(inner_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(inner_origin.line, 2);
    // Routine declarations anchor their origin at the declared NAME, so
    // symbols, diagnostics, and editor tokens point at `inner`, not `fun`.
    assert_eq!(inner_origin.column, 9);
}

#[test]
fn test_parse_package_retains_nested_use_decl_origins() {
    let temp_root = unique_temp_root("nested_use");
    fs::create_dir_all(&temp_root).expect("Should create temporary nested-use fixture dir");
    let fixture = temp_root.join("nested_use.fol");
    fs::write(
        &fixture,
        "fun outer(): int = {\n    use warn: loc = {\"pkg::warn\"};\n    return 0;\n};\n",
    )
    .expect("Should write temporary nested-use fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary nested-use fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Nested-use fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let outer_body = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl { body, .. } => body,
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let use_origin = ast_node_origin(&parsed, &outer_body[0]);
    assert_eq!(use_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(use_origin.line, 2);
    assert_eq!(use_origin.column, 5);
}

#[test]
fn test_parse_package_retains_qualified_reference_origins() {
    let temp_root = unique_temp_root("qualified_refs");
    fs::create_dir_all(&temp_root).expect("Should create temporary qualified-ref fixture dir");
    let fixture = temp_root.join("qualified_refs.fol");
    fs::write(
        &fixture,
        "fun outer(): int = {\n    fun inner(): pkg::Value = {\n        return pkg::value;\n    };\n    return 0;\n};\n",
    )
    .expect("Should write temporary qualified-ref fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary qualified-ref fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Qualified-ref fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let outer_body = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl { body, .. } => body,
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let inner = match &outer_body[0] {
        AstNode::FunDecl {
            return_type: Some(FolType::QualifiedNamed { path }),
            body,
            ..
        } => (path, body),
        other => panic!("Expected nested routine with qualified return type, got {other:?}"),
    };

    let return_type_origin = qualified_path_origin(&parsed, inner.0);
    assert_eq!(return_type_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(return_type_origin.line, 2);
    assert_eq!(return_type_origin.column, 18);

    let value_path = match &inner.1[0] {
        AstNode::Return { value } => match value.as_deref() {
            Some(AstNode::QualifiedIdentifier { path }) => path,
            other => panic!("Expected qualified identifier return value, got {other:?}"),
        },
        other => panic!("Expected return statement, got {other:?}"),
    };

    let value_origin = qualified_path_origin(&parsed, value_path);
    assert_eq!(value_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(value_origin.line, 3);
    assert_eq!(value_origin.column, 16);
}

#[test]
fn test_parse_package_retains_plain_identifier_origins() {
    let temp_root = unique_temp_root("plain_identifier");
    fs::create_dir_all(&temp_root).expect("Should create temporary identifier-origin fixture dir");
    let fixture = temp_root.join("identifier_refs.fol");
    fs::write(
        &fixture,
        "fun outer(): int = {\n    return missing;\n};\n",
    )
    .expect("Should write temporary identifier-origin fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary identifier-origin fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Identifier-origin fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let routine_body = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl { body, .. } => body,
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let identifier = match &routine_body[0] {
        AstNode::Return { value } => match value.as_deref() {
            Some(identifier @ AstNode::Identifier { .. }) => identifier,
            other => panic!("Expected identifier return value, got {other:?}"),
        },
        other => panic!("Expected return statement, got {other:?}"),
    };

    let identifier_origin = ast_node_origin(&parsed, identifier);
    assert_eq!(identifier_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(identifier_origin.line, 2);
    assert_eq!(identifier_origin.column, 12);
}

#[test]
fn test_parse_package_retains_plain_free_call_origins() {
    let temp_root = unique_temp_root("plain_free_call");
    fs::create_dir_all(&temp_root).expect("Should create temporary call-origin fixture dir");
    let fixture = temp_root.join("call_refs.fol");
    fs::write(
        &fixture,
        "fun outer(): int = {\n    return helper(1);\n};\n",
    )
    .expect("Should write temporary call-origin fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary call-origin fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Call-origin fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let routine_body = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl { body, .. } => body,
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let call = match &routine_body[0] {
        AstNode::Return { value } => match value.as_deref() {
            Some(call @ AstNode::FunctionCall { .. }) => call,
            other => panic!("Expected free-call return value, got {other:?}"),
        },
        other => panic!("Expected return statement, got {other:?}"),
    };

    let call_origin = ast_node_origin(&parsed, call);
    assert_eq!(call_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(call_origin.line, 2);
    assert_eq!(call_origin.column, 12);
}

#[test]
fn test_parse_package_retains_plain_named_type_origins() {
    let temp_root = unique_temp_root("plain_named_type");
    fs::create_dir_all(&temp_root).expect("Should create temporary type-origin fixture dir");
    let fixture = temp_root.join("type_refs.fol");
    fs::write(&fixture, "fun outer(value: Missing): Missing = {\n    return value;\n};\n")
        .expect("Should write temporary type-origin fixture");

    let parsed = parse_package_from_file(
        fixture
            .to_str()
            .expect("Temporary type-origin fixture path should be UTF-8"),
    );

    let expected_path = std::fs::canonicalize(&fixture)
        .expect("Type-origin fixture path should canonicalize")
        .to_string_lossy()
        .to_string();

    fs::remove_dir_all(&temp_root).ok();

    let (parameter_type, return_type) = match &parsed.source_units[0].items[0].node {
        AstNode::FunDecl {
            params,
            return_type: Some(return_type),
            ..
        } => (&params[0].param_type, return_type),
        other => panic!("Expected top-level outer routine, got {other:?}"),
    };

    let parameter_origin = fol_type_origin(&parsed, parameter_type);
    assert_eq!(parameter_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(parameter_origin.line, 1);
    assert_eq!(parameter_origin.column, 18);

    let return_origin = fol_type_origin(&parsed, return_type);
    assert_eq!(return_origin.file.as_deref(), Some(expected_path.as_str()));
    assert_eq!(return_origin.line, 1);
    assert_eq!(return_origin.column, 28);
}
