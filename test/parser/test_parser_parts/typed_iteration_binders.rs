use super::*;

#[test]
fn test_top_level_each_iteration_supports_typed_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_typed_binder.fol")
        .expect("Should read top-level typed each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level each with typed binder");

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
                type_hint: Some(FolType::Named { name, .. }),
                condition: Some(_),
                ..
            } if var == "line" && name == "str"
        ),
        "Top-level each should preserve typed iteration binder metadata"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Top-level typed each body should contain yield statement"
    );
}

#[test]
fn test_top_level_each_typed_binder_requires_matching_iteration_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_each_typed_binder_mismatch.fol")
            .expect("Should read malformed typed each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject mismatched typed iteration binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains(
            "Typed iteration binder 'line' must match the iteration variable before 'in'"
        ),
        "Mismatched typed binder should report the binder-name mismatch, got: {}",
        first_message
    );
}

#[test]
fn test_top_level_loop_iteration_supports_typed_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_loop_typed_binder.fol")
        .expect("Should read top-level typed loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level loop with typed binder");

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
                type_hint: Some(FolType::Named { name, .. }),
                condition: Some(_),
                ..
            } if var == "line" && name == "str"
        ),
        "Top-level loop should preserve typed iteration binder metadata"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Top-level typed loop body should contain yield statement"
    );
}

#[test]
fn test_top_level_for_iteration_supports_typed_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_for_typed_binder.fol")
        .expect("Should read top-level typed for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level for with typed binder");

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
                type_hint: Some(FolType::Named { name, .. }),
                condition: Some(_),
                ..
            } if var == "item" && name == "str"
        ),
        "Top-level for should preserve typed iteration binder metadata"
    );
    assert!(
        loop_stmt
            .1
            .iter()
            .any(|node| matches!(node, AstNode::Yield { .. })),
        "Top-level typed for body should contain yield statement"
    );
}

#[test]
fn test_top_level_each_iteration_supports_typed_silent_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_typed_silent_binder.fol")
        .expect("Should read top-level typed silent each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level each with typed silent binder");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "_" && name == "str"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_loop_iteration_supports_typed_silent_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_loop_typed_silent_binder.fol")
        .expect("Should read top-level typed silent loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level loop with typed silent binder");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "_" && name == "str"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_for_iteration_supports_typed_silent_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_for_typed_silent_binder.fol")
        .expect("Should read top-level typed silent for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level for with typed silent binder");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "_" && name == "str"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_loop_typed_binder_requires_matching_iteration_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_loop_typed_binder_mismatch.fol")
            .expect("Should read malformed typed loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject mismatched typed loop binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains(
            "Typed iteration binder 'line' must match the iteration variable before 'in'"
        ),
        "Mismatched typed loop binder should report the binder-name mismatch, got: {}",
        first_message
    );
}

#[test]
fn test_top_level_each_iteration_supports_keyword_typed_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_keyword_typed_binder.fol")
        .expect("Should read top-level keyword typed each-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level each with keyword typed binder");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "get" && name == "str"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_loop_iteration_supports_keyword_typed_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_loop_keyword_typed_binder.fol")
        .expect("Should read top-level keyword typed loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level loop with keyword typed binder");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "std" && name == "str"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_for_keyword_typed_binder_requires_matching_iteration_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_for_keyword_typed_binder_mismatch.fol")
            .expect("Should read malformed keyword typed for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject mismatched keyword typed binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message
            .contains("Typed iteration binder 'get' must match the iteration variable before 'in'"),
        "Mismatched keyword typed binder should report the binder-name mismatch, got: {}",
        first_message
    );
}
