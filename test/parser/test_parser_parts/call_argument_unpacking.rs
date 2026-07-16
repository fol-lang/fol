use super::*;

#[test]
fn test_function_calls_support_unpack_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_unpack_args.fol")
        .expect("Should read unpack call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept call-site unpack arguments");

    let has_unpack_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_call,
        "Function call should preserve call-site unpack arguments structurally"
    );
}

#[test]
fn test_method_calls_support_unpack_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_unpack_args.fol")
            .expect("Should read unpack method-call fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack method call arguments");

    let has_unpack_method_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::MethodCall { method, args, .. }
                    if method == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_method_call,
        "Method call should preserve call-site unpack arguments structurally"
    );
}

#[test]
fn test_invoke_expressions_support_unpack_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_invoke_unpack_args.fol")
            .expect("Should read unpack invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack invoke arguments");

    let has_unpack_invoke = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::Invoke { args, .. }
                    if args.len() == 2
                        && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_invoke,
        "Invoke expressions should preserve call-site unpack arguments structurally"
    );
}

#[test]
fn test_invoke_expressions_support_unpack_after_named_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_invoke_unpack_after_named.fol")
            .expect("Should read invoke unpack-after-keyword fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept invoke unpack arguments after keyword arguments");

    let has_named_unpack_invoke = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::Invoke { args, .. }
                    if args.len() == 2
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "left")
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_named_unpack_invoke,
        "Invoke expressions should preserve unpack arguments after named arguments structurally"
    );
}

#[test]
fn test_unpack_arguments_accept_semicolon_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unpack_args_semicolon.fol")
            .expect("Should read semicolon unpack fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept semicolon-separated unpack arguments");

    let has_semicolon_unpack_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_semicolon_unpack_call,
        "Unpack call arguments should accept semicolon separators"
    );
}

#[test]
fn test_unpack_arguments_accept_trailing_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unpack_args_trailing.fol")
            .expect("Should read trailing unpack fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept trailing separators after unpack arguments");

    let has_trailing_unpack_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_trailing_unpack_call,
        "Unpack call arguments should accept trailing separators"
    );
}

#[test]
fn test_unpack_function_call_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unpack_stmt.fol")
            .expect("Should read unpack call-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack function call statements");

    let has_unpack_call_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::FunctionCall { name, args, .. }
                if name == "calc"
                    && args.len() == 2
                    && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                    && matches!(&args[1], AstNode::Unpack { .. })
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_call_stmt,
        "Statement call path should preserve unpack arguments structurally"
    );
}

#[test]
fn test_unpack_method_call_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_unpack_stmt.fol")
            .expect("Should read unpack method call-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack method call statements");

    let has_unpack_method_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::MethodCall { method, args, .. }
                if method == "calc"
                    && args.len() == 2
                    && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                    && matches!(&args[1], AstNode::Unpack { .. })
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_method_stmt,
        "Statement method-call path should preserve unpack arguments structurally"
    );
}

#[test]
fn test_unpack_invoke_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_invoke_unpack_stmt.fol")
            .expect("Should read unpack invoke-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack invoke statements");

    let has_unpack_invoke_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Invoke { args, .. }
                if args.len() == 2
                    && matches!(&args[0], AstNode::Literal(Literal::Boolean(true)))
                    && matches!(&args[1], AstNode::Unpack { .. })
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_unpack_invoke_stmt,
        "Statement invoke path should preserve unpack arguments structurally"
    );
}

#[test]
fn test_unpack_arguments_reject_missing_operand() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unpack_missing_operand.fol")
            .expect("Should read malformed unpack call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let error = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unpack arguments without an operand");

    let first_message = error
        .first()
        .map(|problem| problem.message.clone())
        .unwrap_or_default();

    assert!(
        first_message.contains("Expected expression after '...' in call arguments"),
        "Expected unpack-missing-operand diagnostic, got: {}",
        first_message
    );
}

#[test]
fn test_unpack_arguments_accept_after_named_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unpack_after_named.fol")
            .expect("Should read unpack-after-keyword fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept unpack arguments after keyword arguments");

    let has_named_unpack_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "flag")
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_named_unpack_call,
        "Function call should preserve unpack arguments after named arguments structurally"
    );
}
