use super::*;

#[test]
fn test_unary_minus_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_unary_minus_missing_operand.fol")
            .expect("Should read unary-minus missing operand test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when unary minus is missing its operand");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
        first_message.contains("Expected expression after unary '-'"),
        "Unary minus without operand should report explicit unary-minus operand error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Unary minus missing-operand parse error should point to return line"
    );
}

#[test]
fn test_unary_not_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_unary_not_missing_operand.fol")
            .expect("Should read unary-not missing operand test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when unary not is missing its operand");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
        first_message.contains("Expected expression after unary 'not'"),
        "Unary not without operand should report explicit unary-not operand error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Unary not missing-operand parse error should point to return line"
    );
}

#[test]
fn test_top_level_call_argument_unary_minus_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unary_minus_missing_operand.fol")
            .expect("Should read top-level unary-minus missing operand call-arg test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level call arg unary minus is missing an operand");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
            first_message.contains("Expected expression after unary '-'"),
            "Top-level unary minus without operand should report explicit unary-minus operand error, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Top-level call-arg unary minus missing-operand parse error should point to call line"
    );
}

#[test]
fn test_top_level_call_argument_unary_not_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unary_not_missing_operand.fol")
            .expect("Should read top-level unary-not missing operand call-arg test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level call arg unary not is missing an operand");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
            first_message.contains("Expected expression after unary 'not'"),
            "Top-level unary not without operand should report explicit unary-not operand error, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Top-level call-arg unary not missing-operand parse error should point to call line"
    );
}

#[test]
fn test_top_level_call_argument_unary_plus_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unary_plus_missing_operand.fol")
            .expect("Should read top-level unary-plus missing operand call-arg test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level call arg unary plus is missing an operand");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
            first_message.contains("Expected expression after unary '+'"),
            "Top-level unary plus without operand should report explicit unary-plus operand error, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Top-level call-arg unary plus missing-operand parse error should point to call line"
    );
}

#[test]
fn test_unary_missing_operand_at_eof_reports_explicit_errors() {
    let cases = [
        (
            "test/parser/simple_fun_unary_plus_eof_operand.fol",
            "Expected expression after unary '+'",
        ),
        (
            "test/parser/simple_fun_unary_not_eof_operand.fol",
            "Expected expression after unary 'not'",
        ),
        (
            "test/parser/simple_fun_unary_minus_eof_operand.fol",
            "Expected expression after unary '-'",
        ),
    ];

    for (path, expected_message) in cases {
        let mut file_stream =
            FileStream::from_file(path).unwrap_or_else(|_| panic!("Should read fixture: {}", path));

        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse(&mut lexer)
            .expect_err("Parser should fail when unary operand is missing at EOF");

        let parse_error = errors
            .first()
            .expect("First parser error should be ParseError");

        let first_message = parse_error.message.clone();
        assert!(
            first_message.contains(expected_message),
            "Fixture {} should report '{}', got: {}",
            path,
            expected_message,
            first_message
        );
        assert_eq!(
            parse_error.primary_location().unwrap().line,
            2,
            "Fixture {} should report unary EOF error on second line",
            path
        );
    }
}

#[test]
fn test_missing_call_closing_paren_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_missing_paren.fol")
        .expect("Should read missing call paren test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when a call is missing a closing ')' ");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    assert!(
        first_message.contains("Expected ',', ';', or ')' in call arguments")
            || first_message.contains("Unsupported expression token '; '"),
        "Missing call ')' should report a call-argument parse error, got: {}",
        first_message
    );
}

#[test]
fn test_missing_call_argument_separator_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_bad_separator.fol")
        .expect("Should read bad call separator test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when call arguments are missing a separator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Expected ',', ';', or ')' in call arguments"),
        "Missing call separator should report argument-separator parse error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Missing call separator parse error should point to the next argument token line"
    );
}

#[test]
fn test_top_level_call_with_leading_comma_argument_reports_location() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_leading_comma_arg.fol")
            .expect("Should read top-level malformed call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when call arguments start with a comma");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    assert!(
        first_message.contains("Unsupported expression token"),
        "Leading comma argument should report unsupported expression token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Leading comma parse error should point to the comma line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Leading comma parse error should include a non-zero column"
    );
}

#[test]
fn test_method_call_missing_argument_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_bad_separator.fol")
            .expect("Should read malformed method call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when method call arguments are missing a separator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Expected ',', ';', or ')' in call arguments"),
        "Method call with missing separator should report argument-separator parse error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Method missing-separator parse error should point to the next argument token line"
    );
}

#[test]
fn test_nested_call_missing_argument_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_nested_bad_separator.fol")
            .expect("Should read malformed nested call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when nested call arguments are missing a separator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    assert!(
        first_message.contains("Expected ',', ';', or ')' in call arguments"),
        "Nested call with missing separator should report argument-separator parse error, got: {}",
        first_message
    );
}

#[test]
fn test_top_level_call_with_double_comma_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_double_comma_arg.fol")
            .expect("Should read malformed top-level double-comma call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level call has an empty argument slot");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token"),
        "Top-level call with double comma should report unsupported expression token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Top-level double-comma parse error should point to empty argument slot line"
    );
}

#[test]
fn test_method_call_with_empty_argument_slot_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_empty_argument_slot.fol")
            .expect("Should read malformed method empty-slot call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when method call starts argument list with comma");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token"),
        "Method call with empty argument slot should report unsupported expression token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Method empty-slot parse error should point to the comma line"
    );
}

#[test]
fn test_nested_call_with_empty_argument_slot_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_nested_empty_slot.fol")
            .expect("Should read malformed nested empty-slot call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when nested call has an empty argument slot");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token"),
        "Nested call with empty argument slot should report unsupported expression token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        5,
        "Nested empty-slot parse error should point to the comma line"
    );
}

#[test]
fn test_method_call_with_nested_empty_argument_slot_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_nested_empty_slot.fol")
            .expect("Should read malformed method nested empty-slot call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when method call has nested empty argument slot");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
            first_message.contains("Unsupported expression token"),
            "Method call with nested empty argument slot should report unsupported expression token, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        5,
        "Method nested empty-slot parse error should point to the comma line"
    );
}

#[test]
fn test_call_argument_with_dangling_operator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_dangling_operator.fol")
            .expect("Should read malformed dangling-operator call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when a call argument expression has a dangling operator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token ','"),
        "Dangling operator in call argument should report unsupported comma token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Dangling operator parse error should point to the expression line"
    );
}

#[test]
fn test_method_call_argument_with_dangling_operator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_dangling_operator.fol")
            .expect("Should read malformed method dangling-operator call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when a method call argument has a dangling operator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token ','"),
        "Dangling operator in method call argument should report unsupported comma token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Method dangling-operator parse error should point to the expression line"
    );
}

#[test]
fn test_method_call_nested_dangling_operator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_nested_dangling_operator.fol")
            .expect("Should read malformed method nested dangling-operator call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when nested method call argument has a dangling operator");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token ','"),
        "Nested dangling operator in method call should report unsupported comma token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Nested method dangling-operator parse error should point to inner expression line"
    );
}

#[test]
fn test_function_call_nested_dangling_operator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_nested_dangling_operator.fol")
            .expect("Should read malformed function nested dangling-operator call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should fail when nested function call argument has a dangling operator",
    );

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token ','"),
        "Nested dangling operator in function call should report unsupported comma token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Nested function dangling-operator parse error should point to inner expression line"
    );
}

#[test]
fn test_top_level_nested_call_with_empty_argument_slot_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_nested_empty_slot.fol")
            .expect("Should read malformed top-level nested empty-slot call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when top-level nested call has an empty argument slot");

    let first_message = errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| "<no error message>".to_string());

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        first_message.contains("Unsupported expression token"),
        "Top-level nested empty-slot call should report unsupported expression token, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Top-level nested empty-slot parse error should point to inner empty-slot comma line"
    );
}
