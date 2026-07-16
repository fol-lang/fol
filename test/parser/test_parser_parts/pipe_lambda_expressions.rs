use super::*;

#[test]
fn test_pipe_lambda_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pipe_lambda_expr.fol")
        .expect("Should read pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse expression-bodied pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.len() == 1 && params[0].name == "x")
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_typed_pipe_lambda_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_typed_expr.fol")
            .expect("Should read typed pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typed pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 2
                            && matches!(params[0].param_type, FolType::Int { .. })
                            && fol_type_has_qualified_segments(&params[1].param_type, &["pkg", "Score"])
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_zero_and_multi_param_pipe_lambda_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_multi_expr.fol")
            .expect("Should read zero/multi pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse zero and multi-param pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().filter(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.is_empty())
                )).count() == 1
                    && body.iter().filter(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.len() == 2)
                    )).count() == 1
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_block_bodied_pipe_lambda_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_block.fol")
            .expect("Should read block-bodied pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block-bodied pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { body, .. } if body.iter().any(|node| matches!(node, AstNode::Return { .. })))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_flow_bodied_pipe_lambda_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_lambda_flow_expr.fol")
        .expect("Should read flow-bodied pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow-bodied pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { body, .. } if !body.is_empty())
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_flow_bodied_pipe_lambda_inquiry_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pipe_lambda_flow_inquiry_expr.fol")
            .expect("Should read flow-bodied pipe lambda inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiries on flow-bodied pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { inquiries, .. } if inquiries.len() == 1)
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_capture_lists_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pipe_lambda_capture_expr.fol")
        .expect("Should read pipe lambda capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse capture lists on pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { captures, params, .. }
                        if captures == &vec!["left".to_string(), "right".to_string()]
                            && params.len() == 1
                            && params[0].name == "x"
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_capture_lists_accept_semicolon_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_capture_semicolon.fol")
            .expect("Should read semicolon pipe lambda capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated captures on pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { captures, params, .. }
                        if captures == &vec!["left".to_string(), "right".to_string()]
                            && params.len() == 1
                            && params[0].name == "x"
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_inquiry_clauses_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pipe_lambda_inquiry_expr.fol")
        .expect("Should read pipe lambda inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiry clauses on block pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { inquiries, .. }
                        if inquiries.len() == 1
                            && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 1)
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_expression_pipe_lambda_inquiry_clauses_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pipe_lambda_expr_inquiry.fol")
        .expect("Should read expression pipe lambda inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiry clauses on expression pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, body, .. }
                if name == "make"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousFun { inquiries, .. }
                            if inquiries.len() == 1
                                && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 1)
                        )
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_expression_pipe_lambda_rejects_canonical_duplicate_inquiries() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_pipe_lambda_duplicate_inquiry_expr_canonical.fol",
    )
    .expect("Should read canonical duplicate pipe lambda inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate inquiry clauses on expression pipe lambdas");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate inquiry clause for 'CacheName'"),
        "Expected canonical duplicate inquiry error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_pipe_lambda_rejects_duplicate_parameters() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_duplicate_param.fol")
            .expect("Should read duplicate pipe lambda parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate pipe lambda parameters");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate parameter name 'x'"),
        "Expected duplicate pipe lambda parameter error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_pipe_lambda_supports_grouped_parameters() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_grouped_params.fol")
            .expect("Should read grouped pipe lambda parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse grouped pipe lambda parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 3
                            && matches!(params[0].param_type, FolType::Int { .. })
                            && matches!(params[1].param_type, FolType::Int { .. })
                            && matches!(params[2].param_type, FolType::Named { ref name , ..} if name == "str")
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_semicolon_parameter_groups() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_semicolon_params.fol")
            .expect("Should read semicolon pipe lambda parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated pipe lambda parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 3
                            && matches!(params[0].param_type, FolType::Int { .. })
                            && matches!(params[1].param_type, FolType::Int { .. })
                            && matches!(params[2].param_type, FolType::Named { ref name , ..} if name == "str")
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_pipe_lambda_parameters_in_initializers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_semicolon_params_initializer.fol")
            .expect("Should read semicolon pipe lambda initializer fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated pipe lambda parameters in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { name, value: Some(value), .. }
                    if name == "lambda"
                        && matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.len() == 2)
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_pipe_lambda_parameters_in_call_args() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_semicolon_params_call_arg.fol")
            .expect("Should read semicolon pipe lambda call-arg fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated pipe lambda parameters in call args");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::FunctionCall { name, args, .. }
                        if name == "emit"
                            && matches!(args.as_slice(), [AstNode::AnonymousFun { params, .. }] if params.len() == 2)
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_trailing_parameter_separator() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_trailing_separator.fol")
            .expect("Should read trailing-separator pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing parameter separators on pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.len() == 2)
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_pipe_lambda_separator_in_initializers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_trailing_separator_initializer.fol")
            .expect("Should read trailing-separator pipe lambda initializer fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing separators on pipe lambda initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { name, value: Some(value), .. }
                    if name == "lambda"
                        && matches!(value.as_ref(), AstNode::AnonymousFun { params, .. } if params.len() == 2)
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_pipe_lambda_separator_in_call_args() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_trailing_separator_call_arg.fol")
            .expect("Should read trailing-separator pipe lambda call-arg fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing separators on pipe lambda call args");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::FunctionCall { name, args, .. }
                        if name == "emit"
                            && matches!(args.as_slice(), [AstNode::AnonymousFun { params, .. }] if params.len() == 2)
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_pipe_lambda_parameters_with_return_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_semicolon_params_return_type.fol")
            .expect("Should read semicolon typed pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated typed pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun {
                            params,
                            return_type: Some(FolType::Int { .. }),
                            ..
                        }
                        if params.len() == 2
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_pipe_lambda_separator_with_return_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_trailing_separator_return_type.fol")
            .expect("Should read trailing typed pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing separators on typed pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun {
                            params,
                            return_type: Some(FolType::Int { .. }),
                            ..
                        }
                        if params.len() == 2
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_default_parameters() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_default_params.fol")
            .expect("Should read default pipe lambda parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default pipe lambda parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 3
                            && params[0].default.is_some()
                            && params[1].default.is_none()
                            && params[2].default.is_some()
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_variadic_parameters() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_variadic_params.fol")
            .expect("Should read variadic pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse variadic pipe lambda parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 2
                            && matches!(params[0].param_type, FolType::Int { .. })
                            && matches!(params[1].param_type, FolType::Sequence { .. })
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_rejects_non_last_variadic_parameter() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_variadic_not_last.fol")
            .expect("Should read non-last variadic pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject non-last variadic pipe lambda parameters");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Variadic parameter must be the last parameter"),
        "Expected non-last variadic pipe lambda error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_pipe_lambda_rejects_variadic_default_values() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_variadic_default_forbidden.fol")
            .expect("Should read variadic default pipe lambda fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject default values on variadic pipe lambda parameters");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Variadic parameters cannot have default values"),
        "Expected variadic-default pipe lambda error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_pipe_lambda_parameter_casing_has_no_borrow_semantics() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_borrowable_params.fol")
            .expect("Should read uppercase pipe lambda parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse uppercase pipe lambda parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { params, .. }
                        if params.len() == 2
                            && params[0].name == "BUF"
                            && !params[0].is_borrowable
                            && !params[1].is_borrowable
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_explicit_return_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_return_type.fol")
            .expect("Should read pipe lambda return-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse explicit return types on pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { return_type: Some(FolType::Int { .. }), .. }
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_lambda_supports_explicit_error_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pipe_lambda_error_type.fol")
            .expect("Should read pipe lambda error-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse explicit error types on pipe lambdas");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::AnonymousFun { error_type: Some(FolType::Error { .. }), .. }
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
