use super::*;

#[test]
fn test_anonymous_function_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_anonymous_expr.fol")
        .expect("Should read anonymous function fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous function expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousFun { params, return_type, body, .. }
                            if params.len() == 2
                                && matches!(return_type, Some(FolType::Int { .. }))
                                && !body.is_empty()
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_function_immediate_invocation_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_anonymous_invoke_expr.fol")
        .expect("Should read anonymous invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse immediate anonymous invocation");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::Invoke { callee, args }
                            if args.len() == 2
                                && matches!(callee.as_ref(), AstNode::AnonymousFun { .. })
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_procedure_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pro_anonymous_expr.fol")
        .expect("Should read anonymous procedure fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous procedure expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousPro { params, return_type, body, .. }
                            if params.len() == 1
                                && matches!(return_type, Some(FolType::Int { .. }))
                                && !body.is_empty()
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_procedure_immediate_invocation_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pro_anonymous_invoke_expr.fol")
        .expect("Should read anonymous procedure invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse immediate anonymous procedure invocation");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::Invoke { callee, args }
                            if args.len() == 1
                                && matches!(callee.as_ref(), AstNode::AnonymousPro { .. })
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_expr.fol")
            .expect("Should read shorthand anonymous function fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand anonymous function expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AnonymousFun { params, return_type, .. }
                            if params.len() == 2 && return_type.is_none())
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_immediate_invocation_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_invoke_expr.fol")
            .expect("Should read shorthand anonymous invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand anonymous invocation");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::Invoke { callee, args }
                            if args.len() == 2
                                && matches!(callee.as_ref(), AstNode::AnonymousFun { .. }))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_logical_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_log_anonymous_expr.fol")
        .expect("Should read anonymous logical expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous logical expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousLog { params, return_type: Some(FolType::Bool), .. }
                            if params.len() == 1 && params[0].name == "a"
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_logical_immediate_invocation_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_log_anonymous_invoke_expr.fol")
        .expect("Should read anonymous logical invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse invoked anonymous logical expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::Invoke { callee, args }
                            if args.len() == 1
                                && matches!(
                                    callee.as_ref(),
                                    AstNode::AnonymousLog { return_type: Some(FolType::Bool), .. }
                                )
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_function_flow_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_anonymous_flow_expr.fol")
        .expect("Should read anonymous function flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous function flow bodies");

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
fn test_anonymous_procedure_flow_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pro_anonymous_flow_expr.fol")
        .expect("Should read anonymous procedure flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous procedure flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::AnonymousPro { body, .. } if !body.is_empty())
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_anonymous_logical_flow_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_log_anonymous_flow_expr.fol")
        .expect("Should read anonymous logical flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse anonymous logical flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::AnonymousLog { body, .. } if !body.is_empty())
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_body_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_flow_expr.fol")
            .expect("Should read shorthand anonymous flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand anonymous function flow bodies");

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
fn test_anonymous_function_flow_body_inquiry_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_anonymous_flow_inquiry_expr.fol")
            .expect("Should read anonymous flow-body inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiries on anonymous function flow bodies");

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
fn test_shorthand_anonymous_function_flow_body_inquiry_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_flow_inquiry_expr.fol")
            .expect("Should read shorthand anonymous flow-body inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiries on shorthand anonymous flow bodies");

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
fn test_anonymous_routine_inquiry_clauses_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_anonymous_routine_inquiries.fol")
            .expect("Should read anonymous routine inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiry clauses on anonymous routines");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "make"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousFun { inquiries, .. }
                                if inquiries.len() == 1
                                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 1)
                            )
                        ))
                )
            }));

            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { name, body, .. }
                    if name == "build"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousPro { inquiries, .. }
                                if inquiries.len() == 1
                                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "this") && body.len() == 1)
                            )
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_inquiry_clauses_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_inquiry_expr.fol")
            .expect("Should read shorthand anonymous inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inquiry clauses on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousFun { inquiries, .. }
                                if inquiries.len() == 1
                                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 1)
                            )
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_return_type_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_return_type.fol")
            .expect("Should read shorthand anonymous return-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse explicit return types on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun { return_type: Some(FolType::Int { .. }), .. })
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_error_type_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_error_type.fol")
            .expect("Should read shorthand anonymous error-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse explicit error types on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun { error_type: Some(FolType::Named { name, .. }), .. } if name == "Failure")
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_return_type_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_flow_return_type.fol")
            .expect("Should read shorthand anonymous flow return-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow-bodied return types on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                return_type: Some(FolType::Int { .. }),
                                body,
                                ..
                            } if !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_error_type_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_flow_error_type.fol")
            .expect("Should read shorthand anonymous flow error-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow-bodied error types on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                error_type: Some(FolType::Named { name, .. }),
                                body,
                                ..
                            } if name == "Failure" && !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_capture_return_type_parsing() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_capture_return_type.fol",
    )
    .expect("Should read shorthand anonymous flow capture+return fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse captures and return types on shorthand flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                captures,
                                return_type: Some(FolType::Int { .. }),
                                body,
                                ..
                            } if captures == &vec!["left".to_string()] && !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_capture_inquiry_parsing() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_capture_inquiry.fol",
    )
    .expect("Should read shorthand anonymous flow capture+inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse captures and inquiries on shorthand flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                captures,
                                inquiries,
                                body,
                                ..
                            } if captures == &vec!["left".to_string()]
                                && inquiries.len() == 1
                                && !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
#[test]
fn test_shorthand_anonymous_function_flow_return_type_in_initializer() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_return_type_initializer.fol",
    )
    .expect("Should read shorthand anonymous flow return initializer fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow return types in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::VarDecl { name, value: Some(value), .. }
                            if name == "f"
                                && matches!(value.as_ref(), AstNode::AnonymousFun {
                                return_type: Some(FolType::Int { .. }),
                                body,
                                ..
                            } if !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_error_type_in_initializer() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_error_type_initializer.fol",
    )
    .expect("Should read shorthand anonymous flow error initializer fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow error types in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::VarDecl { name, value: Some(value), .. }
                            if name == "f"
                                && matches!(value.as_ref(), AstNode::AnonymousFun {
                                    error_type: Some(FolType::Named { name, .. }),
                                    body,
                                    ..
                                } if name == "Failure" && !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_return_type_in_call_args() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_return_type_call_arg.fol",
    )
    .expect("Should read shorthand anonymous flow return call-arg fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow return types in call arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. }
                                if name == "emit"
                                    && matches!(args.as_slice(), [AstNode::AnonymousFun {
                                        return_type: Some(FolType::Int { .. }),
                                        body,
                                        ..
                                    }] if !body.is_empty()))
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_error_type_in_call_args() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_shorthand_anonymous_flow_error_type_call_arg.fol",
    )
    .expect("Should read shorthand anonymous flow error call-arg fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow error types in call arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. }
                                if name == "emit"
                                    && matches!(args.as_slice(), [AstNode::AnonymousFun {
                                        error_type: Some(FolType::Named { name, .. }),
                                        body,
                                        ..
                                    }] if name == "Failure" && !body.is_empty()))
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_return_type_in_procedure_bodies() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_pro_shorthand_anonymous_flow_return_type_expr.fol",
    )
    .expect("Should read shorthand anonymous flow return fixture in procedure body");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow return types in procedure bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                return_type: Some(FolType::Int { .. }),
                                body,
                                ..
                            } if !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_flow_error_type_in_logical_bodies() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_log_shorthand_anonymous_flow_error_type_expr.fol",
    )
    .expect("Should read shorthand anonymous flow error fixture in logical body");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse shorthand flow error types in logical bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::LogDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::AnonymousFun {
                                error_type: Some(FolType::Named { name, .. }),
                                body,
                                ..
                            } if name == "Failure" && !body.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
