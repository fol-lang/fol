use super::*;

fn parse_error_snapshot(path: &str) -> (String, usize, usize) {
    let mut file_stream = FileStream::from_file(path).expect("Should read parser error fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject malformed tokens in the fixture");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    (
        parse_error.message.clone(),
        parse_error.primary_location().unwrap().line,
        parse_error.primary_location().unwrap().column,
    )
}

#[test]
fn test_call_argument_illegal_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_call_illegal_string_arg.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal call-argument token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal call-argument token should report the call site line"
    );
    assert!(
        column > 0,
        "Illegal call-argument token should retain a concrete source column"
    );
}

#[test]
fn test_call_argument_illegal_raw_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_call_illegal_raw_string_arg.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal raw call-argument token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal raw call-argument token should report the call site line"
    );
    assert!(
        column > 0,
        "Illegal raw call-argument token should retain a concrete source column"
    );
}

#[test]
fn test_call_argument_illegal_numeric_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_call_illegal_numeric_arg.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal numeric call-argument token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal numeric call-argument token should report the call site line"
    );
    assert!(
        column > 0,
        "Illegal numeric call-argument token should retain a concrete source column"
    );
}

#[test]
fn test_type_reference_illegal_token_reports_offending_token_location() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_param_illegal_type_ref.fol")
        .expect("Should read illegal type-reference fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject illegal tokens inside type references");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Illegal type-reference token should report an explicit illegal-token diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Illegal type-reference token should report the signature line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Illegal type-reference token should retain a concrete source column"
    );
}

#[test]
fn test_container_element_illegal_token_reports_offending_token_location() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_container_illegal_element.fol")
        .expect("Should read illegal container-element fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject illegal tokens inside container literals");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Illegal container-element token should report an explicit illegal-token diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Illegal container-element token should report the container literal line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Illegal container-element token should retain a concrete source column"
    );
}

#[test]
fn test_record_initializer_value_illegal_token_reports_offending_token_location() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_record_init_illegal_value.fol")
            .expect("Should read illegal record-init fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject illegal tokens inside record initializer values");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Illegal record-initializer token should report an explicit illegal-token diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Illegal record-initializer token should report the record literal line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Illegal record-initializer token should retain a concrete source column"
    );
}

#[test]
fn test_return_expression_illegal_token_reports_offending_token_location() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_return_illegal_value.fol")
        .expect("Should read illegal return-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject illegal tokens inside return expressions");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Illegal return-expression token should report an explicit illegal-token diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Illegal return-expression token should report the return line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Illegal return-expression token should retain a concrete source column"
    );
}

#[test]
fn test_return_expression_illegal_raw_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_return_illegal_raw_value.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal raw return-expression token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal raw return-expression token should report the return line"
    );
    assert!(
        column > 0,
        "Illegal raw return-expression token should retain a concrete source column"
    );
}

#[test]
fn test_return_expression_illegal_numeric_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_return_illegal_numeric_value.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal numeric return-expression token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal numeric return-expression token should report the return line"
    );
    assert!(
        column > 0,
        "Illegal numeric return-expression token should retain a concrete source column"
    );
}

#[test]
fn test_parameter_default_illegal_token_reports_offending_token_location() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_param_default_illegal_value.fol")
            .expect("Should read illegal parameter-default fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject illegal tokens inside parameter default values");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Illegal parameter-default token should report an explicit illegal-token diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Illegal parameter-default token should report the signature line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Illegal parameter-default token should retain a concrete source column"
    );
}

#[test]
fn test_parameter_default_illegal_raw_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_param_default_illegal_raw_value.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal raw parameter-default token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 1,
        "Illegal raw parameter-default token should report the signature line"
    );
    assert!(
        column > 0,
        "Illegal raw parameter-default token should retain a concrete source column"
    );
}

#[test]
fn test_parameter_default_illegal_numeric_token_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_param_default_illegal_numeric_value.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal numeric parameter-default token should report an explicit illegal-token diagnostic, got: {}",
        message
    );
    assert_eq!(
        line, 1,
        "Illegal numeric parameter-default token should report the signature line"
    );
    assert!(
        column > 0,
        "Illegal numeric parameter-default token should retain a concrete source column"
    );
}

#[test]
fn test_unterminated_backtick_comment_in_call_reports_offending_token_location() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unterminated_backtick_comment.fol")
            .expect("Should read unterminated backtick-comment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unterminated backtick comments inside call arguments");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Parser encountered illegal token"),
        "Malformed backtick comments should surface as explicit illegal-token diagnostics, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Malformed backtick comments should anchor to the offending comment start line"
    );
    assert!(
        parse_error.primary_location().unwrap().column > 0,
        "Malformed backtick comments should retain a concrete source column"
    );
}

#[test]
fn test_unterminated_doc_comment_in_call_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_call_unterminated_doc_comment.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Malformed doc comments should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(
        line, 4,
        "Malformed doc comments should anchor to the offending comment start line"
    );
    assert!(
        column > 0,
        "Malformed doc comments should retain a concrete source column"
    );
}

#[test]
fn test_unterminated_slash_block_comment_in_call_reports_offending_token_location() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_call_unterminated_block_comment.fol",
    )
    .expect("Should read parser error fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unterminated block comment in fixture");

    let first_error = errors.first().expect("Should have at least one error");
    let message = first_error.message.clone();
    assert!(
        message.contains("unterminated block comment"),
        "Unterminated slash block comment should produce a specific lexer diagnostic, got: {}",
        message
    );
}

#[test]
fn test_dot_builtin_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_dot_builtin_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal builtin-call names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal builtin-call names should report the builtin call line"
    );
    assert!(
        column > 0,
        "Illegal builtin-call names should retain a concrete source column"
    );
}

#[test]
fn test_postfix_member_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_field_access_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal postfix member names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(
        line, 2,
        "Illegal postfix member names should report the member access line"
    );
    assert!(
        column > 0,
        "Illegal postfix member names should retain a concrete source column"
    );
}

#[test]
fn test_alias_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_alias_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal alias names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal alias names should report the declaration line");
    assert!(column > 0, "Illegal alias names should retain a concrete source column");
}

#[test]
fn test_type_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_typ_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal type names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal type names should report the declaration line");
    assert!(column > 0, "Illegal type names should retain a concrete source column");
}

#[test]
fn test_type_declaration_illegal_option_reports_offending_token_location() {
    let (message, line, column) = parse_error_snapshot("test/parser/simple_typ_illegal_option.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal type options should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal type options should report the declaration line");
    assert!(column > 0, "Illegal type options should retain a concrete source column");
}

#[test]
fn test_segment_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_seg_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal segment names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal segment names should report the declaration line");
    assert!(column > 0, "Illegal segment names should retain a concrete source column");
}

#[test]
fn test_standard_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_std_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal standard names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal standard names should report the declaration line");
    assert!(
        column > 0,
        "Illegal standard names should retain a concrete source column"
    );
}

#[test]
fn test_definition_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_def_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal definition names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(
        line, 1,
        "Illegal definition names should report the declaration line"
    );
    assert!(
        column > 0,
        "Illegal definition names should retain a concrete source column"
    );
}

#[test]
fn test_definition_declaration_illegal_option_reports_offending_token_location() {
    let (message, line, column) = parse_error_snapshot("test/parser/simple_def_illegal_option.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal definition options should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(
        line, 1,
        "Illegal definition options should report the declaration line"
    );
    assert!(
        column > 0,
        "Illegal definition options should retain a concrete source column"
    );
}

#[test]
fn test_function_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_fun_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal function names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal function names should report the declaration line");
    assert!(
        column > 0,
        "Illegal function names should retain a concrete source column"
    );
}

#[test]
fn test_routine_illegal_option_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_routine_illegal_option.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal routine options should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal routine options should anchor to the header line");
    assert!(
        column > 0,
        "Illegal routine options should retain a concrete source column"
    );
}

#[test]
fn test_logical_declaration_illegal_name_reports_offending_token_location() {
    let (message, line, column) =
        parse_error_snapshot("test/parser/simple_log_illegal_name.fol");

    assert!(
        message.contains("Parser encountered illegal token"),
        "Illegal logical names should surface as explicit illegal-token diagnostics, got: {}",
        message
    );
    assert_eq!(line, 1, "Illegal logical names should report the declaration line");
    assert!(
        column > 0,
        "Illegal logical names should retain a concrete source column"
    );
}
