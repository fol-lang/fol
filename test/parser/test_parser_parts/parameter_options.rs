use super::*;

#[test]
fn parameter_options_set_explicit_borrow_and_mutex_flags() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_parameter_options.fol")
        .expect("Should read parameter-option fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept name[options]: type parameters");

    let AstNode::Program { declarations } = ast else {
        panic!("Expected program node");
    };
    assert!(declarations.iter().any(|node| matches!(
        node,
        AstNode::FunDecl { params, .. }
            if params.len() == 2
                && params[0].name == "value"
                && params[0].is_borrowable
                && !params[0].is_mutex
                && params[1].name == "lock"
                && !params[1].is_borrowable
                && params[1].is_mutex
    )));
}

#[test]
fn unknown_parameter_options_fail_at_parse_time() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_unknown_parameter_option.fol")
            .expect("Should read unknown parameter-option fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Unknown parameter options must fail parsing");
    assert!(errors.iter().any(|error| error
        .message
        .contains("Unknown parameter option 'legacy'; expected 'bor'")));
}
