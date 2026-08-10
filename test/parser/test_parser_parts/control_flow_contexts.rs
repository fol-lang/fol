use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_parser_control_flow_{label}"))
}

fn parse_first_error_from_source(label: &str, source: &str) -> fol_diagnostics::Diagnostic {
    let temp_root = unique_temp_root(label);
    fs::create_dir_all(&temp_root).expect("Should create temporary control-flow fixture dir");
    let fixture = temp_root.join("control_flow_context.fol");
    fs::write(&fixture, source).expect("Should write temporary control-flow fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Temporary control-flow fixture path should be UTF-8"),
    )
    .expect("Should open temporary control-flow fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject out-of-context control flow");

    fs::remove_dir_all(&temp_root).ok();

    errors
        .into_iter()
        .next()
        .expect("First parser error should exist")
}

#[test]
fn test_top_level_return_is_rejected_outside_routine_context() {
    let error = parse_first_error_from_source("top_level_return", "return 1;\n");

    assert!(
        error
            .message
            .contains("'return' is only allowed inside routines"),
        "Top-level return should fail with routine-context wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        1,
        "Top-level return should point at its own line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        1,
        "Top-level return should point at the return keyword itself"
    );
}

#[test]
fn test_branch_body_return_is_rejected_without_routine_context() {
    let error = parse_first_error_from_source(
        "branch_body_return",
        "when(value) {\n    case(ready) {\n        return 1;\n    }\n}\n",
    );

    assert!(
        error
            .message
            .contains("'return' is only allowed inside routines"),
        "Branch-body return should fail with routine-context wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        3,
        "Branch-body return should report the return line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        9,
        "Branch-body return should point at the nested return keyword"
    );
}

#[test]
fn test_top_level_break_is_rejected_outside_loop_context() {
    let error = parse_first_error_from_source("top_level_break", "break;\n");

    assert!(
        error
            .message
            .contains("'break' is only allowed inside loops"),
        "Top-level break should fail with loop-context wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        1,
        "Top-level break should point at its own line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        1,
        "Top-level break should point at the break keyword itself"
    );
}

#[test]
fn test_routine_break_is_rejected_without_loop_context() {
    let error =
        parse_first_error_from_source("routine_break", "fun bad(): int = {\n    break;\n};\n");

    assert!(
        error
            .message
            .contains("'break' is only allowed inside loops"),
        "Routine break should fail with loop-context wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        2,
        "Routine break should report the break line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        5,
        "Routine break should point at the nested break keyword"
    );
}

#[test]
fn test_top_level_yield_is_rejected_without_routine_or_loop_context() {
    let error = parse_first_error_from_source("top_level_yield", "yield 1;\n");

    assert!(
        error
            .message
            .contains("'yield' is only allowed inside routines or loops"),
        "Top-level yield should fail with routine-or-loop wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        1,
        "Top-level yield should point at its own line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        1,
        "Top-level yield should point at the yield keyword itself"
    );
}

#[test]
fn test_branch_body_yield_is_rejected_without_routine_or_loop_context() {
    let error = parse_first_error_from_source(
        "branch_body_yield",
        "when(value) {\n    case(ready) {\n        yield 1;\n    }\n}\n",
    );

    assert!(
        error
            .message
            .contains("'yield' is only allowed inside routines or loops"),
        "Branch-body yield should fail with routine-or-loop wording, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        3,
        "Branch-body yield should report the yield line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        9,
        "Branch-body yield should point at the nested yield keyword"
    );
}
