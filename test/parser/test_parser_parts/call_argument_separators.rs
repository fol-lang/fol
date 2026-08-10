use super::*;

#[test]
fn test_call_argument_lists_accept_trailing_commas() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_trailing_comma.fol")
        .expect("Should read trailing comma call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse call arguments with trailing commas");

    let (has_ping_two_args, has_run_one_arg, has_emit_one_arg) = match ast {
        AstNode::Program { declarations } => {
            let has_ping_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunctionCall { name, args, .. }
                    if name == "ping" && args.len() == 2
                )
            });

            let has_run_one_arg = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { method, args, .. }
                    if method == "run" && args.len() == 1
                )
            });

            let has_emit_one_arg = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 1)
                )
            });

            (has_ping_two_args, has_run_one_arg, has_emit_one_arg)
        }
        _ => panic!("Expected program node"),
    };

    assert!(has_ping_two_args);
    assert!(has_run_one_arg);
    assert!(has_emit_one_arg);
}

#[test]
fn test_call_argument_lists_accept_semicolons() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_semicolon.fol")
        .expect("Should read semicolon call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse call arguments with semicolons");

    let (has_ping_two_args, has_run_one_arg, has_emit_two_args) = match ast {
        AstNode::Program { declarations } => {
            let has_ping_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunctionCall { name, args, .. }
                    if name == "ping" && args.len() == 2
                )
            });

            let has_run_one_arg = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { method, args, .. }
                    if method == "run" && args.len() == 1
                )
            });

            let has_emit_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 2)
                )
            });

            (has_ping_two_args, has_run_one_arg, has_emit_two_args)
        }
        _ => panic!("Expected program node"),
    };

    assert!(has_ping_two_args);
    assert!(has_run_one_arg);
    assert!(has_emit_two_args);
}

#[test]
fn test_call_argument_lists_accept_mixed_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_mixed_separators.fol")
            .expect("Should read mixed-separator call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse call arguments with mixed separators");

    let (has_ping_three_args, has_run_two_args, has_emit_three_args) = match ast {
        AstNode::Program { declarations } => {
            let has_ping_three_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunctionCall { name, args, .. }
                    if name == "ping" && args.len() == 3
                )
            });

            let has_run_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { method, args, .. }
                    if method == "run" && args.len() == 2
                )
            });

            let has_emit_three_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 3)
                )
            });

            (has_ping_three_args, has_run_two_args, has_emit_three_args)
        }
        _ => panic!("Expected program node"),
    };

    assert!(has_ping_three_args);
    assert!(has_run_two_args);
    assert!(has_emit_three_args);
}

#[test]
fn test_semicolon_call_arguments_parse_in_initializers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_semicolon_initializer.fol")
            .expect("Should read semicolon initializer call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon call arguments in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| matches!(
                node,
                AstNode::VarDecl { name, value: Some(value), .. }
                if name == "value"
                    && matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 2)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_nested_calls_with_trailing_commas_preserve_argument_shapes() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_nested_trailing_comma.fol")
            .expect("Should read nested trailing comma call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested trailing-comma calls");

    let (has_outer_two_args, has_done_one_arg) = match ast {
        AstNode::Program { declarations } => {
            let has_outer_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::FunctionCall { name, args, .. }
                        if name == "outer"
                            && args.len() == 2
                            && matches!(args[0], AstNode::FunctionCall { ref name, args: ref nested_args , ..} if name == "inner" && nested_args.len() == 1)
                            && matches!(args[1], AstNode::MethodCall { ref method, args: ref nested_args, .. } if method == "run" && nested_args.len() == 1)
                    )
                )
            });

            let has_done_one_arg = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "done" && args.len() == 1)
                )
            });

            (has_outer_two_args, has_done_one_arg)
        }
        _ => panic!("Expected program node"),
    };

    assert!(has_outer_two_args);
    assert!(has_done_one_arg);
}

#[test]
fn test_nested_calls_with_semicolons_preserve_argument_shapes() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_nested_semicolon.fol")
            .expect("Should read nested semicolon call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested semicolon calls");

    let (has_outer_two_args, has_done_one_arg) = match ast {
        AstNode::Program { declarations } => {
            let has_outer_two_args = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::FunctionCall { name, args, .. }
                        if name == "outer"
                            && args.len() == 2
                            && matches!(args[0], AstNode::FunctionCall { ref name, args: ref nested_args , ..} if name == "inner" && nested_args.len() == 1)
                            && matches!(args[1], AstNode::MethodCall { ref method, args: ref nested_args, .. } if method == "run" && nested_args.len() == 1)
                    )
                )
            });

            let has_done_one_arg = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "done" && args.len() == 1)
                )
            });

            (has_outer_two_args, has_done_one_arg)
        }
        _ => panic!("Expected program node"),
    };

    assert!(has_outer_two_args);
    assert!(has_done_one_arg);
}
