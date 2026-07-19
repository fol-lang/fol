use super::*;

#[test]
fn test_multi_else_if_chain_lowers_to_recursive_when_defaults() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_else_if_multi.fol")
        .expect("Should read multi else-if test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multi else-if chain");

    let top_when = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When { cases, default, .. } = node {
                    Some((cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level lowered when node"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(top_when.0.len(), 1, "Top if should have one case");

    let first_default = top_when
        .1
        .expect("First else-if step should produce default branch");
    let nested_when_1 = first_default
        .iter()
        .find_map(|node| {
            if let AstNode::When { cases, default, .. } = node {
                Some((cases.clone(), default.clone()))
            } else {
                None
            }
        })
        .expect("First default should contain nested when");
    assert_eq!(
        nested_when_1.0.len(),
        1,
        "First nested else-if should have one case"
    );

    let second_default = nested_when_1
        .1
        .expect("Second else-if step should produce default branch");
    let nested_when_2 = second_default
        .iter()
        .find_map(|node| {
            if let AstNode::When { cases, default, .. } = node {
                Some((cases.clone(), default.clone()))
            } else {
                None
            }
        })
        .expect("Second default should contain nested when");
    assert_eq!(
        nested_when_2.0.len(),
        1,
        "Second nested else-if should have one case"
    );

    let final_default = nested_when_2
        .1
        .expect("Final else branch should exist at deepest nested default");
    assert!(
        final_default
            .iter()
            .any(|node| matches!(node, AstNode::Return { .. })),
        "Final else branch should contain return statement"
    );
}

#[test]
fn test_loop_statement_parsing_with_condition_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop.fol")
        .expect("Should read loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
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
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(loop_stmt.0, fol_parser::ast::LoopCondition::Condition(_)),
        "Loop should parse condition expression"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Assignment { .. })),
        "Loop body should contain assignment statement"
    );
}

#[test]
fn test_loop_statement_parsing_with_break_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_break.fol")
        .expect("Should read loop break test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop with break statement");

    let loop_body = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Loop { body, .. } = node {
                    Some(body.clone())
                } else {
                    None
                }
            })
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        loop_body.iter().any(|node| matches!(node, AstNode::Break)),
        "Loop body should contain break statement"
    );
}

#[test]
fn test_loop_break_without_semicolon_is_accepted() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_break_no_semi.fol")
        .expect("Should read loop break without semicolon test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse break without semicolon");

    let loop_body = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Loop { body, .. } = node {
                    Some(body.clone())
                } else {
                    None
                }
            })
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        loop_body.iter().any(|node| matches!(node, AstNode::Break)),
        "Loop body should contain break statement"
    );
}

#[test]
fn test_loop_statement_parsing_with_yield_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_yield.fol")
        .expect("Should read loop yield test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop with yield statement");

    let loop_body = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Loop { body, .. } = node {
                    Some(body.clone())
                } else {
                    None
                }
            })
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        loop_body
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Loop body should contain yield statement"
    );
}

#[test]
fn test_loop_yield_without_semicolon_is_accepted() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_yield_no_semi.fol")
        .expect("Should read loop yield without semicolon test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse yield without semicolon");

    let loop_body = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Loop { body, .. } = node {
                    Some(body.clone())
                } else {
                    None
                }
            })
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        loop_body
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Loop body should contain yield statement"
    );
}

#[test]
fn test_loop_iteration_condition_parsing_with_in() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_in.fol")
        .expect("Should read loop iteration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop iteration condition");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
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
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration { var, .. } if var == "i"
        ),
        "Loop should parse iteration form with variable i"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Iteration loop body should contain yield statement"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "Iteration loop body should contain break statement"
    );
}

#[test]
fn test_loop_iteration_condition_with_when_guard() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_in_when.fol")
        .expect("Should read guarded iteration loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse iteration loop with when guard");

    let loop_condition = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Loop { condition, .. } = node {
                    Some(condition.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should include a loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_condition,
            fol_parser::ast::LoopCondition::Iteration {
                var,
                condition: Some(_),
                ..
            } if var == "i"
        ),
        "Iteration loop should include variable and parsed when-guard condition"
    );
}

#[test]
fn test_loop_iteration_condition_supports_silent_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_loop_silent_in.fol")
        .expect("Should read silent-binder loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse iteration loop with '_' binder");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
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
            .expect("Program should include a loop statement"),
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
        "loop(_ in iterable when guard) should preserve '_' binder"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "silent-binder loop body should contain yield statement"
    );
}

#[test]
fn test_for_statement_parsing_with_condition_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_for.fol")
        .expect("Should read for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse for statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
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
            .expect("Program should include a lowered loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(loop_stmt.0, fol_parser::ast::LoopCondition::Condition(_)),
        "for(condition) should lower to LoopCondition::Condition"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Assignment { .. })),
        "for body should contain assignment statement"
    );
}

#[test]
fn test_each_statement_parsing_with_iteration_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_each.fol")
        .expect("Should read each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse each statement");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
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
            .expect("Program should include a lowered loop statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            loop_stmt.0,
            fol_parser::ast::LoopCondition::Iteration { var, condition: None, .. } if var == "item"
        ),
        "each(x in iterable) should lower to iteration loop form"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "each body should contain yield statement"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Break)),
        "each body should contain break statement"
    );
}

#[test]
fn test_builtin_diagnostic_statements_parse_as_function_calls() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_builtin_diag.fol")
        .expect("Should read builtin diagnostic test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin diagnostic statements");

    let call_names = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .filter_map(|node| {
                if let AstNode::FunctionCall { name, .. } = node {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => panic!("Expected program node"),
    };

    assert!(
        call_names.contains(&"check".to_string()),
        "Should parse check statement as function call"
    );
    assert!(
        call_names.contains(&"report".to_string()),
        "Should parse report statement as function call"
    );
    assert!(
        call_names.contains(&"assert".to_string()),
        "Should parse assert statement as function call"
    );
    assert!(
        call_names.contains(&"panic".to_string()),
        "Should parse panic statement as function call"
    );
}

#[test]
fn test_builtin_diagnostic_statements_without_args_parse_as_empty_calls() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_builtin_diag_no_args.fol")
        .expect("Should read builtin diagnostic no-args test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin diagnostic statements without args");

    let calls = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .filter_map(|node| {
                if let AstNode::FunctionCall { name, args, .. } = node {
                    Some((name.clone(), args.len()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => panic!("Expected program node"),
    };

    assert!(
        calls
            .iter()
            .any(|(name, argc)| name == "check" && *argc == 0),
        "check without args should parse as zero-arg call"
    );
    assert!(
        calls
            .iter()
            .any(|(name, argc)| name == "report" && *argc == 0),
        "report without args should parse as zero-arg call"
    );
    assert!(
        calls
            .iter()
            .any(|(name, argc)| name == "assert" && *argc == 0),
        "assert without args should parse as zero-arg call"
    );
    assert!(
        calls
            .iter()
            .any(|(name, argc)| name == "panic" && *argc == 0),
        "panic without args should parse as zero-arg call"
    );
}

#[test]
fn test_function_body_identifier_calls_parse_as_functioncall_nodes() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_stmt.fol")
        .expect("Should read function call statement test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse identifier call statements");

    let call_names = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .filter_map(|node| {
                if let AstNode::FunctionCall { name, .. } = node {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => panic!("Expected program node"),
    };

    assert!(
        call_names.contains(&"process".to_string()),
        "Should parse process(a, b) as function call"
    );
    assert!(
        call_names.contains(&"ping".to_string()),
        "Should parse ping() as function call"
    );
}

#[test]
fn test_top_level_identifier_call_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_call_top_level.fol")
        .expect("Should read top-level call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level identifier call");

    let call = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::FunctionCall { name, args, .. } = node {
                    Some((name.clone(), args.len()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level function call"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(call.0, "run");
    assert_eq!(call.1, 2, "Top-level call should include two arguments");
}

#[test]
fn test_top_level_keyword_named_call_and_assignment_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_top_level_keyword_call_and_assignment.fol")
            .expect("Should read top-level keyword call/assignment test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level keyword-named calls and assignments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(program_root_nodes(&declarations).into_iter().any(|node| {
                matches!(node, AstNode::FunctionCall { name, args, .. } if name == "get" && args.is_empty())
            }));
            assert!(program_root_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { target, value }
                    if matches!(target.as_ref(), AstNode::Identifier { name, .. } if name == "std")
                        && matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "ready")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_multiline_identifier_call_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_call_top_level_multiline.fol")
        .expect("Should read top-level multiline call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level multiline identifier call");

    let call = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::FunctionCall { name, args, .. } = node {
                    Some((name.clone(), args.len()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level multiline function call"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(call.0, "run");
    assert_eq!(
        call.1, 3,
        "Top-level multiline call should include three arguments"
    );
}

#[test]
fn test_top_level_call_with_unary_plus_arguments_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_call_top_level_unary_plus_args.fol")
            .expect("Should read top-level unary-plus call test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level unary-plus call arguments");

    let call = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::FunctionCall { name, args, .. } = node {
                    Some((name.clone(), args.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include top-level function call"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(call.0, "run");
    assert_eq!(
        call.1.len(),
        2,
        "Top-level unary-plus call should have two args"
    );
    assert!(
        matches!(&call.1[0], AstNode::Identifier { name, .. } if name == "a"),
        "Unary plus on first arg should fold to identifier 'a'"
    );
    assert!(
        matches!(
            &call.1[1],
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Add,
                ..
            }
        ),
        "Unary plus on parenthesized second arg should preserve inner addition expression"
    );
}

#[test]
fn test_call_and_method_call_with_unary_plus_arguments_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_unary_plus_args.fol")
        .expect("Should read unary-plus call args fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unary-plus arguments in call and method-call contexts");

    let (has_run_assignment, has_update_method_call, has_emit_return) = match ast {
        AstNode::Program { declarations } => {
            let has_run_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "run"
                                && args.len() == 2
                                && matches!(&args[0], AstNode::Identifier { name, .. } if name == "a")
                                && matches!(&args[1], AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Add, .. })
                        )
                    )
                });

            let has_update_method_call = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::MethodCall { method, args, .. }
                        if method == "update"
                            && args.len() == 2
                            && matches!(&args[0], AstNode::Identifier { name, .. } if name == "a")
                            && matches!(&args[1], AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Add, .. })
                    )
                });

            let has_emit_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "emit"
                                && args.len() == 2
                                && matches!(&args[0], AstNode::Identifier { name, .. } if name == "a")
                                && matches!(&args[1], AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Add, .. })
                        )
                    )
                });

            (has_run_assignment, has_update_method_call, has_emit_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_run_assignment,
        "Function call assignment should parse unary-plus args with expected shapes"
    );
    assert!(
        has_update_method_call,
        "Method call should parse unary-plus args with expected shapes"
    );
    assert!(
        has_emit_return,
        "Return call should parse unary-plus args with expected shapes"
    );
}
