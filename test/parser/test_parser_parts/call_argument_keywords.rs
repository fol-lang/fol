use super::*;

#[test]
fn test_function_calls_support_keyword_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_keyword_call_args.fol")
        .expect("Should read keyword call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword call arguments");

    let has_keyword_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 3
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_call,
        "Function call should preserve keyword arguments structurally"
    );
}

#[test]
fn test_function_calls_support_mixed_keyword_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_mixed_keyword_call_args.fol")
        .expect("Should read mixed keyword call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept mixed positional and keyword call arguments");

    let has_mixed_keyword_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 5
                        && !matches!(&args[0], AstNode::NamedArgument { .. })
                        && !matches!(&args[1], AstNode::NamedArgument { .. })
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el5")
                        && matches!(&args[3], AstNode::NamedArgument { name, .. } if name == "el4")
                        && matches!(&args[4], AstNode::NamedArgument { name, .. } if name == "el3")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_mixed_keyword_call,
        "Function call should preserve positional arguments before keyword arguments"
    );
}

#[test]
fn test_function_calls_reject_positional_arguments_after_keyword_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_call_positional_after_named.fol")
            .expect("Should read malformed mixed call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let error = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject positional arguments after keyword arguments");

    let first_message = error
        .first()
        .map(|problem| problem.message.clone())
        .unwrap_or_default();

    assert!(
        first_message.contains("Positional call arguments are not allowed after named arguments"),
        "Expected positional-after-keyword diagnostic, got: {}",
        first_message
    );
}

#[test]
fn test_method_calls_support_keyword_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_keyword_method_call_args.fol")
        .expect("Should read keyword method-call fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword method call arguments");

    let has_keyword_method_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::MethodCall { method, args, .. }
                    if method == "calc"
                        && args.len() == 3
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_method_call,
        "Method call should preserve keyword arguments structurally"
    );
}

#[test]
fn test_method_calls_support_unpack_after_keyword_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_unpack_after_named.fol")
            .expect("Should read keyword-plus-unpack method-call fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept method call unpack arguments after keyword arguments");

    let has_keyword_unpack_method_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::MethodCall { method, args, .. }
                    if method == "calc"
                        && args.len() == 2
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "flag")
                        && matches!(&args[1], AstNode::Unpack { .. })
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_unpack_method_call,
        "Method call should preserve unpack arguments after keyword arguments structurally"
    );
}

#[test]
fn test_invoke_expressions_support_keyword_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_keyword_invoke_args.fol")
        .expect("Should read keyword invoke-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword invoke arguments");

    let has_keyword_invoke = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::Invoke { args, .. }
                    if args.len() == 2
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "left")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "right")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_invoke,
        "Invoke expressions should preserve keyword arguments structurally"
    );
}

#[test]
fn test_keyword_call_arguments_accept_semicolon_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_call_args_semicolon.fol")
            .expect("Should read semicolon keyword call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept semicolon-separated keyword arguments");

    let has_semicolon_keyword_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 3
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_semicolon_keyword_call,
        "Keyword call arguments should accept semicolon separators"
    );
}

#[test]
fn test_keyword_call_arguments_accept_trailing_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_call_args_trailing.fol")
            .expect("Should read trailing-separator keyword call-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept trailing separators after keyword arguments");

    let has_trailing_keyword_call = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Assignment { value, .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 3
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
                )
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_trailing_keyword_call,
        "Keyword call arguments should accept trailing separators"
    );
}

#[test]
fn test_keyword_function_call_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_call_stmt.fol")
            .expect("Should read keyword call-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword function call statements");

    let has_keyword_call_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::FunctionCall { name, args, .. }
                if name == "calc"
                    && args.len() == 3
                    && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                    && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                    && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_call_stmt,
        "Statement call path should preserve keyword arguments structurally"
    );
}

#[test]
fn test_keyword_method_call_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_method_call_stmt.fol")
            .expect("Should read keyword method call-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword method call statements");

    let has_keyword_method_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::MethodCall { method, args, .. }
                if method == "calc"
                    && args.len() == 3
                    && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                    && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                    && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_method_stmt,
        "Statement method-call path should preserve keyword arguments structurally"
    );
}

#[test]
fn test_keyword_invoke_statements_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_keyword_invoke_stmt.fol")
            .expect("Should read keyword invoke-statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword invoke statements");

    let has_keyword_invoke_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
            matches!(
                node,
                AstNode::Invoke { args, .. }
                if args.len() == 2
                    && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "left")
                    && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "right")
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_invoke_stmt,
        "Statement invoke path should preserve keyword arguments structurally"
    );
}
