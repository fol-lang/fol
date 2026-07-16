use super::*;

fn unwrap_comment_wrappers(node: &AstNode) -> &AstNode {
    match node {
        AstNode::Commented { node, .. } => unwrap_comment_wrappers(node.as_ref()),
        node => node,
    }
}

#[test]
fn test_call_expressions_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_expr.fol")
        .expect("Should read call expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse call expressions in statements");

    let (has_call_assignment, has_call_return) = match ast {
        AstNode::Program { declarations } => {
            let has_call_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, .. } if name == "compute")
                    )
                });

            let has_call_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, .. } if name == "emit")
                    )
                });

            (has_call_assignment, has_call_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_call_assignment,
        "Assignment value should parse as function call expression"
    );
    assert!(
        has_call_return,
        "Return value should parse as function call expression"
    );
}

#[test]
fn test_zero_argument_calls_in_statement_and_return_positions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_no_args.fol")
        .expect("Should read zero-argument call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse zero-argument calls");

    let (has_ping_stmt, has_pong_stmt, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_ping_stmt = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunctionCall { name, args, .. } if name == "ping" && args.is_empty()
                )
            });

            let has_pong_stmt = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunctionCall { name, args, .. } if name == "pong" && args.is_empty()
                )
            });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.is_empty())
                    )
                });

            (has_ping_stmt, has_pong_stmt, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_ping_stmt,
        "Should parse ping() as zero-arg statement call"
    );
    assert!(
        has_pong_stmt,
        "Should parse pong() without semicolon as zero-arg statement call"
    );
    assert!(
        has_emit_return,
        "Should parse return emit() as zero-arg return call"
    );
}

#[test]
fn test_method_calls_in_statement_and_return_positions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_method_call.fol")
        .expect("Should read method call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse method calls");

    let (has_update_stmt, has_get_return) = match ast {
        AstNode::Program { declarations } => {
            let has_update_stmt =
                only_root_routine_body_nodes(&declarations)
                    .into_iter()
                    .any(|node| {
                        matches!(
                            node,
                            AstNode::MethodCall { method, .. } if method == "update"
                        )
                    });

            let has_get_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::MethodCall { method, .. } if method == "get")
                    )
                });

            (has_update_stmt, has_get_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_update_stmt,
        "Should parse object.update(value) method call"
    );
    assert!(
        has_get_return,
        "Should parse return object.get() method call"
    );
}

#[test]
fn test_zero_argument_method_calls_with_optional_semicolons() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_method_call_no_args.fol")
        .expect("Should read zero-arg method call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse zero-argument method calls");

    let (has_start_stmt, has_stop_stmt, has_done_return) = match ast {
        AstNode::Program { declarations } => {
            let has_start_stmt = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { method, args, .. } if method == "start" && args.is_empty()
                )
            });

            let has_stop_stmt = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { method, args, .. } if method == "stop" && args.is_empty()
                )
            });

            let has_done_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::MethodCall { method, args, .. } if method == "done" && args.is_empty())
                    )
                });

            (has_start_stmt, has_stop_stmt, has_done_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_start_stmt,
        "Should parse obj.start() as zero-arg statement method call"
    );
    assert!(
        has_stop_stmt,
        "Should parse obj.stop() without semicolon as zero-arg statement method call"
    );
    assert!(
        has_done_return,
        "Should parse return obj.done() as zero-arg return method call"
    );
}

#[test]
fn test_field_access_expressions_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_field_access.fol")
        .expect("Should read field access test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse field access expressions");

    let (has_field_assignment, has_nested_field_return) = match ast {
        AstNode::Program { declarations } => {
            let has_field_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FieldAccess { object, field }
                            if field == "inner"
                                && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "obj")
                        )
                    )
                });

            let has_nested_field_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::FieldAccess { object, field }
                            if field == "size"
                                && matches!(
                                    object.as_ref(),
                                    AstNode::FieldAccess { object, field }
                                    if field == "inner"
                                        && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "obj")
                                )
                        )
                    )
                });

            (has_field_assignment, has_nested_field_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_field_assignment,
        "Assignment value should parse as field access"
    );
    assert!(
        has_nested_field_return,
        "Return value should parse as chained field access"
    );
}

#[test]
fn test_index_access_expressions_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_index_access.fol")
        .expect("Should read index access test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse index access expressions");

    let (has_index_assignment, has_index_return) = match ast {
        AstNode::Program { declarations } => {
            let has_index_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::IndexAccess { container, index }
                            if matches!(container.as_ref(), AstNode::Identifier { name, .. } if name == "items")
                                && matches!(index.as_ref(), AstNode::Identifier { name, .. } if name == "idx")
                        )
                    )
                });

            let has_index_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::IndexAccess { container, index }
                            if matches!(container.as_ref(), AstNode::Identifier { name, .. } if name == "items")
                                && matches!(index.as_ref(), AstNode::BinaryOp { .. })
                        )
                    )
                });

            (has_index_assignment, has_index_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_index_assignment,
        "Assignment value should parse as index access"
    );
    assert!(
        has_index_return,
        "Return value should parse indexed expression with nested arithmetic"
    );
}

#[test]
fn test_chained_postfix_expressions_mix_fields_indexes_and_methods() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_postfix_chain.fol")
        .expect("Should read postfix chain test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse chained postfix expressions");

    let (has_chained_assignment, has_chained_return) = match ast {
        AstNode::Program { declarations } => {
            let has_chained_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::MethodCall {
                                object,
                                method,
                                args,
                                ..
                            }
                            if method == "format"
                                && args.is_empty()
                                && matches!(
                                    object.as_ref(),
                                    AstNode::IndexAccess { container, index }
                                    if matches!(
                                        container.as_ref(),
                                        AstNode::FieldAccess { object, field }
                                        if field == "items"
                                            && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "obj")
                                    )
                                        && matches!(index.as_ref(), AstNode::Identifier { name, .. } if name == "idx")
                                )
                        )
                    )
                });

            let has_chained_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::IndexAccess { container, index }
                            if matches!(
                                container.as_ref(),
                                AstNode::FieldAccess { object, field }
                                if field == "bytes"
                                    && matches!(
                                        object.as_ref(),
                                        AstNode::MethodCall { method, .. }
                                        if method == "format"
                                    )
                            )
                                && matches!(index.as_ref(), AstNode::Identifier { name, .. } if name == "idx")
                        )
                    )
                });

            (has_chained_assignment, has_chained_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_chained_assignment,
        "Assignment should parse chained field/index/method expression"
    );
    assert!(
        has_chained_return,
        "Return should parse chained method result field/index expression"
    );
}

#[test]
fn test_nested_function_and_method_calls_in_expression_positions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_nested_calls.fol")
        .expect("Should read nested calls test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested function/method calls");

    let (has_wrapped_method_assignment, has_nested_return_emit) = match ast {
        AstNode::Program { declarations } => {
            let has_wrapped_method_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "wrap"
                                && args.len() == 1
                                && matches!(args[0], AstNode::MethodCall { ref method, .. } if method == "get")
                        )
                    )
                });

            let has_nested_return_emit = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "emit"
                                && args.len() == 2
                                && matches!(args[0], AstNode::FunctionCall { ref name, .. } if name == "process")
                                && matches!(args[1], AstNode::MethodCall { ref method, .. } if method == "done")
                        )
                    )
                });

            (has_wrapped_method_assignment, has_nested_return_emit)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_wrapped_method_assignment,
        "Assignment should parse wrap(obj.get()) nesting"
    );
    assert!(
        has_nested_return_emit,
        "Return should parse emit(process(a), obj.done()) nesting"
    );
}
#[test]
fn test_multiline_call_arguments_parse_with_expected_shapes() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_multiline.fol")
        .expect("Should read multiline call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multiline call arguments");

    let (has_compose_assignment, has_update_call, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_compose_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "compose"
                                && args.len() == 3
                                && matches!(args[1], AstNode::FunctionCall { ref name, args: ref inner_args , ..} if name == "wrap" && inner_args.len() == 1)
                        )
                    )
                });

            let has_update_call =
                only_root_routine_body_nodes(&declarations)
                    .into_iter()
                    .any(|node| {
                        matches!(
                            node,
                            AstNode::MethodCall { method, args, .. }
                            if method == "update" && args.len() == 2
                        )
                    });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 1)
                    )
                });

            (has_compose_assignment, has_update_call, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_compose_assignment,
        "Multiline compose(...) assignment should parse with nested wrap(...) argument"
    );
    assert!(
        has_update_call,
        "Multiline obj.update(...) call should parse with two arguments"
    );
    assert!(
        has_emit_return,
        "Multiline return emit(...) call should parse with one argument"
    );
}

#[test]
fn test_multiline_call_arguments_with_comments_parse_with_expected_shapes() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_comments_multiline.fol")
            .expect("Should read multiline call-with-comments test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multiline call arguments with backtick comments");

    let (has_combine_assignment, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_combine_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            unwrap_comment_wrappers(value.as_ref()),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "combine"
                                && args.len() == 3
                                && matches!(unwrap_comment_wrappers(&args[1]), AstNode::FunctionCall { name, args: inner_args , ..} if name == "wrap" && inner_args.len() == 1)
                                && matches!(unwrap_comment_wrappers(&args[2]), AstNode::Literal(Literal::Integer(42)))
                        )
                    )
                });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 1)
                    )
                });

            (has_combine_assignment, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
            has_combine_assignment,
            "combine(...) should parse with three arguments including nested wrap(...) and integer literal around backtick comments"
        );
    assert!(
        has_emit_return,
        "return emit(...) should parse with one argument after commented multiline call"
    );
}

#[test]
fn test_multiline_call_arguments_with_slash_comments_still_parse_as_compatibility_surface() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_comments_multiline_compat.fol")
            .expect("Should read multiline slash-comment compatibility fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should keep accepting slash comments while compatibility support remains");

    let (has_combine_assignment, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_combine_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            unwrap_comment_wrappers(value.as_ref()),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "combine"
                                && args.len() == 3
                                && matches!(unwrap_comment_wrappers(&args[1]), AstNode::FunctionCall { name, args: inner_args , ..} if name == "wrap" && inner_args.len() == 1)
                                && matches!(unwrap_comment_wrappers(&args[2]), AstNode::Literal(Literal::Integer(42)))
                        )
                    )
                });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 1)
                    )
                });

            (has_combine_assignment, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_combine_assignment,
        "Slash-comment compatibility should not disturb multiline combine(...) argument parsing"
    );
    assert!(
        has_emit_return,
        "Slash-comment compatibility should keep the trailing return emit(...) shape intact"
    );
}

#[test]
fn test_multiline_call_arguments_with_doc_comments_remain_ast_visible() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_doc_comments_multiline.fol")
            .expect("Should read multiline doc-comment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve doc comments without disturbing multiline call shapes");

    let (has_combine_assignment, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_combine_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            unwrap_comment_wrappers(value.as_ref()),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "combine"
                                && args.len() == 3
                                && matches!(unwrap_comment_wrappers(&args[1]), AstNode::FunctionCall { name, args: inner_args , ..} if name == "wrap" && inner_args.len() == 1)
                                && matches!(unwrap_comment_wrappers(&args[2]), AstNode::Literal(Literal::Integer(42)))
                        )
                    )
                });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "emit" && args.len() == 1)
                    )
                });

            (has_combine_assignment, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_combine_assignment,
        "Doc comments should not disturb multiline combine(...) argument parsing while remaining AST-visible"
    );
    assert!(
        has_emit_return,
        "Doc comments should keep the trailing return emit(...) shape intact"
    );
}

#[test]
fn test_top_level_loop_iteration_shape_matches_function_loop_shape() {
    let mut file_stream = FileStream::from_file("test/parser/simple_loop_top_level.fol")
        .expect("Should read top-level loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level loop statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::Loop {
                    condition, body, ..
                } = node
                {
                    Some((condition.as_ref().clone(), body.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration {
                var,
                condition: Some(_),
                ..
            } if var == "i"
        ),
        "Top-level loop should parse as guarded iteration"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "Top-level loop body should contain break statement"
    );
}

#[test]
fn test_top_level_for_iteration_shape_matches_loop_shape() {
    let mut file_stream = FileStream::from_file("test/parser/simple_for_top_level.fol")
        .expect("Should read top-level for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level for statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::Loop {
                    condition, body, ..
                } = node
                {
                    Some((condition.as_ref().clone(), body.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level lowered loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration {
                var,
                condition: Some(_),
                ..
            } if var == "item"
        ),
        "Top-level for should parse as guarded iteration"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Top-level for body should contain yield statement"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "Top-level for body should contain break statement"
    );
}

#[test]
fn test_top_level_each_iteration_shape_matches_loop_shape() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_top_level.fol")
        .expect("Should read top-level each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level each statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::Loop {
                    condition, body, ..
                } = node
                {
                    Some((condition.as_ref().clone(), body.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level lowered loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration {
                var,
                condition: Some(_),
                ..
            } if var == "entry"
        ),
        "Top-level each should parse as guarded iteration"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Top-level each body should contain yield statement"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "Top-level each body should contain break statement"
    );
}

#[test]
fn test_top_level_each_iteration_supports_silent_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_top_level_silent.fol")
        .expect("Should read top-level silent each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level each with '_' binder");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::Loop {
                    condition, body, ..
                } = node
                {
                    Some((condition.as_ref().clone(), body.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level lowered loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration {
                var,
                condition: Some(_),
                ..
            } if var == "_"
        ),
        "Top-level each should preserve '_' binder in iteration form"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "Top-level silent each body should contain break statement"
    );
}
