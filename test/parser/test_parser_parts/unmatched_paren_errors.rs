use super::*;

#[test]
fn test_function_call_with_unmatched_close_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unmatched_close_paren_arg.fol")
            .expect("Should read malformed unmatched-close-paren call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should fail when function call argument list contains unmatched ')' token",
    );

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token ')'"),
        "Unmatched ')' argument should report unsupported close-paren token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Unmatched close-paren argument parse error should point to the malformed expression line"
    );
}

#[test]
fn test_method_call_with_unmatched_close_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_unmatched_close_paren_arg.fol")
            .expect("Should read malformed unmatched-close-paren method call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should fail when method call argument list contains unmatched ')' token",
    );

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
            first_message.contains("Unsupported expression token ')'"),
            "Unmatched ')' in method call argument should report unsupported close-paren token, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Method unmatched close-paren parse error should point to malformed expression line"
    );
}

#[test]
fn test_top_level_call_with_unmatched_close_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unmatched_close_paren_arg.fol")
            .expect("Should read malformed unmatched-close-paren top-level call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should fail when top-level call argument list contains unmatched ')' token",
    );

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
            first_message.contains("Unsupported expression token ')'"),
            "Unmatched ')' in top-level call argument should report unsupported close-paren token, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Top-level unmatched close-paren parse error should point to malformed expression line"
    );
}

#[test]
fn test_function_call_with_unmatched_open_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unmatched_open_paren_arg.fol")
            .expect("Should read malformed unmatched-open-paren call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when function call argument has unmatched '(' token");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Expected closing ')' for parenthesized expression"),
        "Unmatched '(' in function call argument should report missing close-paren error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Function unmatched open-paren parse error should point to malformed expression line"
    );
}

#[test]
fn test_method_call_with_unmatched_open_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_unmatched_open_paren_arg.fol")
            .expect("Should read malformed unmatched-open-paren method call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when method call argument has unmatched '(' token");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Expected closing ')' for parenthesized expression"),
        "Unmatched '(' in method call argument should report missing close-paren error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Method unmatched open-paren parse error should point to malformed expression line"
    );
}

#[test]
fn test_top_level_call_with_unmatched_open_paren_argument_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unmatched_open_paren_arg.fol")
            .expect("Should read malformed unmatched-open-paren top-level call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level call argument has unmatched '(' token");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Expected closing ')' for parenthesized expression"),
        "Unmatched '(' in top-level call argument should report missing close-paren error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Top-level unmatched open-paren parse error should point to malformed expression line"
    );
}

fn assert_first_parse_error(path: &str, expected_message_substring: &str, expected_line: usize) {
    let mut file_stream =
        FileStream::from_file(path).unwrap_or_else(|_| panic!("Should read fixture: {}", path));

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(&format!(
        "Parser should fail for malformed fixture: {}",
        path
    ));

    let parse_error = errors.first().unwrap_or_else(|| {
        panic!(
            "First parser error should be ParseError for fixture: {}",
            path
        )
    });

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains(expected_message_substring),
        "Fixture {} should report '{}', got: {}",
        path,
        expected_message_substring,
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        expected_line,
        "Fixture {} should report expected error line",
        path
    );
}

const EXPECT_MISSING_CLOSE_PAREN: &str = "Expected closing ')' for parenthesized expression";
const EXPECT_UNSUPPORTED_CLOSE_PAREN_TOKEN: &str = "Unsupported expression token ')'";

#[test]
fn test_mixed_unmatched_open_error_matrix_representative_cases() {
    let cases = [
        (
            "test/parser/simple_fun_call_mixed_unmatched_open_and_trailing_comma.fol",
            4usize,
        ),
        (
            "test/parser/simple_fun_method_call_mixed_unmatched_open_and_trailing_comma.fol",
            4usize,
        ),
        (
            "test/parser/simple_call_top_level_mixed_unmatched_open_and_trailing_comma.fol",
            3usize,
        ),
        (
            "test/parser/simple_fun_call_mixed_unmatched_open_sixth_arg.fol",
            9usize,
        ),
        (
            "test/parser/simple_fun_method_call_mixed_unmatched_open_sixth_arg.fol",
            9usize,
        ),
        (
            "test/parser/simple_call_top_level_mixed_unmatched_open_sixth_arg.fol",
            8usize,
        ),
    ];

    for (path, line) in cases {
        assert_first_parse_error(path, EXPECT_MISSING_CLOSE_PAREN, line);
    }
}

#[test]
fn test_mixed_unmatched_close_error_matrix_representative_cases() {
    let cases = [
        (
            "test/parser/simple_fun_call_mixed_unmatched_close_first_arg.fol",
            4usize,
        ),
        (
            "test/parser/simple_fun_method_call_mixed_unmatched_close_first_arg.fol",
            4usize,
        ),
        (
            "test/parser/simple_call_top_level_mixed_unmatched_close_first_arg.fol",
            3usize,
        ),
        (
            "test/parser/simple_fun_call_mixed_unmatched_close_fifth_arg.fol",
            8usize,
        ),
        (
            "test/parser/simple_fun_method_call_mixed_unmatched_close_fifth_arg.fol",
            8usize,
        ),
        (
            "test/parser/simple_call_top_level_mixed_unmatched_close_fifth_arg.fol",
            7usize,
        ),
    ];

    for (path, line) in cases {
        assert_first_parse_error(path, EXPECT_UNSUPPORTED_CLOSE_PAREN_TOKEN, line);
    }
}
