use super::*;

fn collect_loop_bodies(ast: AstNode) -> Vec<Vec<AstNode>> {
    match ast {
        AstNode::Program { declarations } => declarations
            .into_iter()
            .flat_map(|node| match node {
                AstNode::FunDecl { body, .. } => body
                    .into_iter()
                    .filter_map(|stmt| match stmt {
                        AstNode::Loop { body, .. } => Some(body),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect(),
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_for_typed_binder_requires_matching_iteration_name() {
    let mut file_stream = FileStream::from_file("test/parser/simple_for_typed_binder_mismatch.fol")
        .expect("Should read malformed typed for-loop test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject mismatched typed for-loop binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains(
            "Typed iteration binder 'item' must match the iteration variable before 'in'"
        ),
        "Mismatched typed for binder should report the binder-name mismatch, got: {}",
        first_message
    );
}

#[test]
fn test_while_and_loop_support_flow_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_while_flow_body.fol")
        .expect("Should read while/loop flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow bodies for while and loop");

    let bodies = collect_loop_bodies(ast);
    assert_eq!(bodies.len(), 2);
    assert!(matches!(bodies[0].as_slice(), [AstNode::Literal(Literal::Integer(1))]));
    assert!(matches!(bodies[1].as_slice(), [AstNode::Literal(Literal::Integer(2))]));
}

#[test]
fn test_for_and_each_support_flow_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_for_each_flow_body.fol")
        .expect("Should read for/each flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow bodies for for and each");

    let bodies = collect_loop_bodies(ast);
    assert_eq!(bodies.len(), 2);
    assert!(matches!(bodies[0].as_slice(), [AstNode::Identifier { name, .. }] if name == "item"));
    assert!(matches!(bodies[1].as_slice(), [AstNode::Identifier { name, .. }] if name == "current"));
}

#[test]
fn test_use_declaration_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_use.fol").expect("Should read use test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declaration");

    let use_decl = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::UseDecl { name, path_type, .. } = node {
                    Some((
                        name.clone(),
                        path_type.clone(),
                        use_decl_path_text(node)
                            .expect("Use declarations should reconstruct a path from segments"),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include use declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(use_decl.0, "math");
    assert!(
        matches!(use_decl.1, FolType::Location { name } if name.is_empty()),
        "Use declaration should parse loc type"
    );
    assert_eq!(use_decl.2, "core::math");
}

#[test]
fn test_use_declaration_supports_multiple_names_and_paths() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_multi.fol")
        .expect("Should read multi-use test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declarations with multiple names and paths");

    match ast {
        AstNode::Program { declarations } => {
            let imports: Vec<_> = program_root_nodes(&declarations)
                .into_iter()
                .filter_map(|node| {
                    if let AstNode::UseDecl { name, path_type, .. } = node {
                        Some((
                            name.clone(),
                            path_type.clone(),
                            use_decl_path_text(node).expect(
                                "Use declarations should reconstruct a path from segments",
                            ),
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            assert!(matches!(
                imports.as_slice(),
                [
                    (log, FolType::Package { name: type_name_a }, path_a),
                    (sync, FolType::Package { name: type_name_b }, path_b),
                    (color, FolType::Package { name: type_name_c }, path_c),
                ] if log == "log"
                    && sync == "sync"
                    && color == "color"
                    && type_name_a.is_empty()
                    && type_name_b.is_empty()
                    && type_name_c.is_empty()
                    && path_a == "fmt/log"
                    && path_b == "os/sync"
                    && path_c == "fmt/color"
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_supports_multiple_names_in_function_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_use_multi.fol")
        .expect("Should read function-body multi-use test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multi-use declarations inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().filter(|stmt| matches!(stmt, AstNode::UseDecl { .. })).count() == 2
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_accepts_empty_option_brackets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_empty_options.fol")
        .expect("Should read empty use options test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use[] declarations");

    let use_decl = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::UseDecl { name, options, path_type, .. } = node {
                    Some((
                        name.clone(),
                        options.clone(),
                        path_type.clone(),
                        use_decl_path_text(node)
                            .expect("Use declarations should reconstruct a path from segments"),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include use declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(use_decl.0, "math");
    assert!(
        use_decl.1.is_empty(),
        "use[] should parse as explicit empty options"
    );
    assert!(
        matches!(use_decl.2, FolType::Location { name } if name.is_empty()),
        "Use declaration should still parse loc type"
    );
    assert_eq!(use_decl.3, "core::math");
}

#[test]
fn test_use_declaration_allows_omitted_colon_before_path_type() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_no_colon.fol")
        .expect("Should read colonless use declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declaration without colon after name");

    let use_decl = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::UseDecl { name, path_type, .. } = node {
                    Some((
                        name.clone(),
                        path_type.clone(),
                        use_decl_path_text(node)
                            .expect("Use declarations should reconstruct a path from segments"),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include use declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(use_decl.0, "warn");
    assert!(
        matches!(use_decl.1, FolType::Package { name } if name.is_empty()),
        "Colonless use declaration should still parse source-kind type"
    );
    assert_eq!(use_decl.2, "fmt/log.warn");
}

#[test]
fn test_use_declaration_unwraps_quoted_paths() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_quoted_path.fol")
        .expect("Should read quoted use path test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declaration with quoted path");

    let use_decl = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::UseDecl { name, path_type, .. } = node {
                    Some((
                        name.clone(),
                        path_type.clone(),
                        use_decl_path_text(node)
                            .expect("Use declarations should reconstruct a path from segments"),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include use declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(use_decl.0, "fmt");
    assert!(
        matches!(use_decl.1, FolType::Package { name } if name.is_empty()),
        "Quoted-path use declaration should still parse source-kind type"
    );
    assert_eq!(use_decl.2, "fmt/log");
}

#[test]
fn test_use_declaration_supports_qualified_and_bracketed_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_qualified_type.fol")
        .expect("Should read qualified use type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declaration with qualified bracketed source-kind type");

    let use_decl = match ast {
        AstNode::Program { declarations } => program_root_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::UseDecl { name, path_type, .. } = node {
                    Some((
                        name.clone(),
                        path_type.clone(),
                        use_decl_path_text(node)
                            .expect("Use declarations should reconstruct a path from segments"),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include use declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(use_decl.0, "results");
    assert!(
        matches!(
            use_decl.1,
            FolType::Map { key_type, value_type }
                if matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                    && fol_type_has_qualified_segments(value_type.as_ref(), &["pkg", "Value"])
        ),
        "Use declaration should preserve qualified bracketed source-kind type"
    );
    assert_eq!(use_decl.2, "core::results");
}

#[test]
fn test_unknown_use_option_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_unknown_option.fol")
        .expect("Should read malformed use option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when use options contain an unknown item");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown use option"),
        "Malformed use option should report unknown option, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Use option parse error should point to the declaration line"
    );
}

#[test]
fn test_use_declaration_missing_bracket_close_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_missing_bracket_close.fol")
        .expect("Should read malformed use declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when use source-kind type is missing closing ']'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed use source-kind type should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Malformed use declaration should report the declaration line"
    );
}

#[test]
fn test_var_parsing_without_type_hint() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_infer.fol")
        .expect("Should read infer var test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse var declaration without type hint");

    match ast {
        AstNode::Program { declarations } => {
            let var_decl = program_root_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        type_hint,
                        value,
                        ..
                    } = node
                    {
                        Some((name, type_hint, value))
                    } else {
                        None
                    }
                })
                .expect("Program should contain a variable declaration");

            assert_eq!(var_decl.0, "message");
            assert!(var_decl.1.is_none(), "Type hint should be omitted");
            assert!(var_decl.2.is_some(), "Value should be parsed");
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_var_parsing_supports_qualified_and_bracketed_type_hints() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_qualified_type.fol")
        .expect("Should read qualified var type hint test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse var declaration with qualified bracketed type hint");

    match ast {
        AstNode::Program { declarations } => {
            let var_decl = program_root_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        type_hint,
                        value,
                        ..
                    } = node
                    {
                        Some((name, type_hint, value))
                    } else {
                        None
                    }
                })
                .expect("Program should contain a variable declaration");

            assert_eq!(var_decl.0, "cache");
            assert!(
                matches!(
                    var_decl.1,
                    Some(FolType::Map { key_type, value_type })
                        if matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                            && fol_type_has_qualified_segments(value_type.as_ref(), &["pkg", "Value"])
                ),
                "Var type hint should preserve qualified bracketed syntax"
            );
            assert!(var_decl.2.is_some(), "Value should still be parsed");
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_var_type_hint_missing_bracket_close_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_type_missing_close.fol")
        .expect("Should read malformed var type hint test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when var type hint is missing closing ']'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed var type hint should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Malformed var type hint should report the declaration line"
    );
}

#[test]
fn test_boolean_keyword_literals_parse_in_var_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_bool_literals.fol")
        .expect("Should read boolean literal function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse boolean keyword literals");

    let (has_true_var, has_false_return) = match ast {
        AstNode::Program { declarations } => {
            let has_true_var = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, value: Some(value), .. }
                    if name == "flag"
                        && matches!(value.as_ref(), AstNode::Literal(Literal::Boolean(true)))
                )
            });

            let has_false_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::Literal(Literal::Boolean(false)))
                )
            });

            (has_true_var, has_false_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_true_var,
        "Function body should include var assignment with true literal"
    );
    assert!(
        has_false_return,
        "Function body should include return with false literal"
    );
}

#[test]
fn test_return_expression_precedence_mul_before_add() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_precedence.fol")
        .expect("Should read precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse precedence function");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value) } = node {
                    Some(value.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should contain a return value"),
        _ => panic!("Expected program node"),
    };

    match &return_value {
        AstNode::BinaryOp { op, left: _, right } => {
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Add));
            assert!(
                matches!(
                    right.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Mul,
                        ..
                    }
                ),
                "Right side should be multiplication subtree"
            );
        }
        _ => panic!("Return value should be binary add expression"),
    }
}

#[test]
fn test_return_expression_parentheses_override_precedence() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_paren_precedence.fol")
        .expect("Should read parenthesized precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse parenthesized precedence function");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value) } = node {
                    Some(value.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should contain a return value"),
        _ => panic!("Expected program node"),
    };

    match &return_value {
        AstNode::BinaryOp { op, left, right: _ } => {
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Mul));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Add,
                        ..
                    }
                ),
                "Left side should be parenthesized addition subtree"
            );
        }
        _ => panic!("Return value should be binary multiplication expression"),
    }
}

#[test]
fn test_range_expressions_parse_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_range_expr.fol")
        .expect("Should read range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse range expressions");

    match ast {
        AstNode::Program { declarations } => {
            let has_range_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: Some(start),
                            end: Some(end),
                            inclusive: true
                        }
                        if matches!(start.as_ref(), AstNode::Identifier { name, .. } if name == "a")
                            && matches!(end.as_ref(), AstNode::Identifier { name, .. } if name == "b")
                    )
                )
            });

            let has_range_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: Some(start),
                            end: Some(end),
                            inclusive: true
                        }
                        if matches!(start.as_ref(), AstNode::BinaryOp { .. })
                            && matches!(end.as_ref(), AstNode::BinaryOp { .. })
                    )
                )
            });

            assert!(
                has_range_assignment,
                "Assignment should parse closed range expression"
            );
            assert!(
                has_range_return,
                "Return should parse range expression with arithmetic bounds"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_range_expression_missing_rhs_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_range_missing_rhs.fol")
        .expect("Should read malformed range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject range expression missing right-hand side");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected expression after '..'"),
        "Malformed range expression should report missing rhs, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Malformed range expression should report the return line"
    );
}

#[test]
fn test_open_start_range_expressions_parse_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_open_start_range_expr.fol")
        .expect("Should read open-start range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse open-start range expressions");

    match ast {
        AstNode::Program { declarations } => {
            let has_open_start_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: None,
                            end: Some(end),
                            inclusive: true
                        }
                        if matches!(end.as_ref(), AstNode::Identifier { name, .. } if name == "b")
                    )
                )
            });

            let has_open_start_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: None,
                            end: Some(end),
                            inclusive: true
                        }
                        if matches!(end.as_ref(), AstNode::BinaryOp { .. })
                    )
                )
            });

            assert!(
                has_open_start_assignment,
                "Assignment should parse open-start range expression"
            );
            assert!(
                has_open_start_return,
                "Return should parse open-start range expression with arithmetic rhs"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_open_start_range_expression_missing_rhs_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_open_start_range_missing_rhs.fol")
            .expect("Should read malformed open-start range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject open-start range expression missing rhs");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected expression after '..'"),
        "Malformed open-start range expression should report missing rhs, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Malformed open-start range expression should report the return line"
    );
}

#[test]
fn test_open_end_range_expressions_parse_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_open_end_range_expr.fol")
        .expect("Should read open-end range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse open-end range expressions");

    match ast {
        AstNode::Program { declarations } => {
            let has_open_end_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: Some(start),
                            end: None,
                            inclusive: true
                        }
                        if matches!(start.as_ref(), AstNode::Identifier { name, .. } if name == "a")
                    )
                )
            });

            let has_open_end_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::Range {
                            start: Some(start),
                            end: None,
                            inclusive: true
                        }
                        if matches!(start.as_ref(), AstNode::BinaryOp { .. })
                    )
                )
            });

            assert!(
                has_open_end_assignment,
                "Assignment should parse open-end range expression"
            );
            assert!(
                has_open_end_return,
                "Return should parse open-end range expression with arithmetic lhs"
            );
        }
        _ => panic!("Expected program node"),
    }
}
