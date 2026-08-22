use super::*;

#[test]
fn test_routine_variadic_parameter_lowers_to_sequence_type() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_variadic_param.fol")
        .expect("Should read variadic parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept variadic parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, params, .. }
                    if name == "calc"
                        && params.len() == 2
                        && matches!(
                            params[1].param_type,
                            FolType::Sequence { ref element_type }
                                if matches!(element_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_variadic_parameter_must_be_last() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_variadic_param_not_last.fol")
            .expect("Should read malformed variadic ordering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject non-final variadic parameters");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Variadic parameter must be the last parameter"),
        "Non-final variadic parameter should report placement error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_variadic_parameter_cannot_have_default_value() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_variadic_param_default.fol")
            .expect("Should read malformed variadic default fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject default values on variadic parameters");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Variadic parameters cannot have default values"),
        "Variadic default value should report explicit error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_variadic_parameter_is_rejected_in_generic_headers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_variadic_generic_header.fol")
            .expect("Should read malformed variadic generic-header fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject variadic parameters in generic headers");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Variadic parameters are not allowed in routine generic headers"),
        "Variadic generic header should report explicit error, got: {}",
        parse_error.message
    );
}
