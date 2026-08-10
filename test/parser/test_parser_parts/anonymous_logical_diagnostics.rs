use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_parser_anon_log_{label}"))
}

fn parse_first_error_from_source(source: &str) -> fol_diagnostics::Diagnostic {
    let temp_root = unique_temp_root("diag");
    fs::create_dir_all(&temp_root).expect("Should create temporary diagnostic fixture folder");
    let fixture = temp_root.join("anon_log_diag.fol");
    fs::write(&fixture, source).expect("Should write temporary anonymous logical fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Temporary diagnostic fixture path should be UTF-8"),
    )
    .expect("Should open temporary anonymous logical fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject malformed anonymous logical fixture");

    fs::remove_dir_all(&temp_root).ok();

    errors
        .into_iter()
        .next()
        .expect("First parser error should exist")
}

#[test]
fn test_anonymous_logical_missing_open_paren_uses_logical_wording() {
    let error = parse_first_error_from_source(
        "fun host(): bool = { return log[] value: bool => value; };\n",
    );

    assert!(
        error
            .message
            .contains("Expected '(' after anonymous logical"),
        "Anonymous logical diagnostics should no longer inherit function wording, got: {}",
        error.message
    );
}

#[test]
fn test_anonymous_logical_untyped_parameter_uses_logical_wording() {
    let error = parse_first_error_from_source(
        "fun host(): bool = { return log[] (value): bool => value; };\n",
    );

    assert!(
        error
            .message
            .contains("Expected ':' after logical parameter name"),
        "Anonymous logical parameter diagnostics should use logical wording, got: {}",
        error.message
    );
}

#[test]
fn test_anonymous_logical_missing_body_separator_uses_logical_wording() {
    let error = parse_first_error_from_source(
        "fun host(): bool = { return log[] (value: bool): bool value; };\n",
    );

    assert!(
        error
            .message
            .contains("Expected '=' or '=>' before anonymous logical body"),
        "Anonymous logical body diagnostics should use logical wording, got: {}",
        error.message
    );
}

#[test]
fn unique_temp_root_produces_distinct_paths_for_rapid_calls() {
    let first = unique_temp_root("collision_check");
    let second = unique_temp_root("collision_check");

    assert_ne!(first.path(), second.path());
}
