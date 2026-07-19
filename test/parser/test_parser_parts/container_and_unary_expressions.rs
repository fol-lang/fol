use super::*;

#[test]
fn test_braced_range_expressions_parse_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_braced_range_expr.fol")
        .expect("Should read braced range expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse braced range expressions");

    match ast {
        AstNode::Program { declarations } => {
            let has_range_assignment =
                only_root_routine_body_nodes(&declarations)
                    .into_iter()
                    .any(|node| {
                        matches!(
                            node,
                            AstNode::Assignment { value, .. }
                            if matches!(value.as_ref(), AstNode::Range { .. })
                        )
                    });

            let has_range_return =
                only_root_routine_body_nodes(&declarations)
                    .into_iter()
                    .any(|node| {
                        matches!(
                            node,
                            AstNode::Return { value: Some(value) }
                            if matches!(value.as_ref(), AstNode::Range { .. })
                        )
                    });

            assert!(
                has_range_assignment,
                "Assignment should parse braced range expression"
            );
            assert!(
                has_range_return,
                "Return should parse braced range expression"
            );
        }
        _ => panic!("Expected program node"),
    }
}
#[test]
fn test_return_expression_unary_minus_precedence() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_unary_precedence.fol")
        .expect("Should read unary precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unary precedence function");

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
                    AstNode::UnaryOp {
                        op: fol_parser::ast::UnaryOperator::Neg,
                        ..
                    }
                ),
                "Left side should be unary negation subtree"
            );
        }
        _ => panic!("Return value should be binary multiplication expression"),
    }
}

#[test]
fn test_return_expression_unary_plus_precedence() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_unary_plus_precedence.fol")
        .expect("Should read unary plus precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unary plus precedence function");

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
        AstNode::BinaryOp { op, left, right } => {
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Mul));
            assert!(
                matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "a"),
                "Left side should parse to identifier 'a' under unary plus"
            );
            assert!(
                matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "b"),
                "Right side should parse to identifier 'b'"
            );
        }
        _ => panic!("Return value should be binary multiplication expression"),
    }
}

#[test]
fn test_return_expression_chained_unary_plus_precedence() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_unary_plus_chain.fol")
        .expect("Should read chained unary plus precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse chained unary plus precedence function");

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
        AstNode::BinaryOp { op, left, right } => {
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Mul));
            assert!(
                matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "a"),
                "Left side should parse to identifier 'a' under chained unary plus"
            );
            assert!(
                matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "b"),
                "Right side should parse to identifier 'b'"
            );
        }
        _ => panic!("Return value should be binary multiplication expression"),
    }
}

#[test]
fn test_unary_plus_preserves_call_and_method_call_expression_shapes() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_unary_plus_call_exprs.fol")
        .expect("Should read unary-plus call expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unary-plus call expressions");

    let (has_call_assignment, has_method_return) = match ast {
        AstNode::Program { declarations } => {
            let has_call_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::FunctionCall { name, args, .. }
                        if name == "compute"
                            && args.len() == 1
                            && matches!(&args[0], AstNode::Identifier { name, .. } if name == "a")
                    )
                )
            });

            let has_method_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(
                        value.as_ref(),
                        AstNode::MethodCall {
                            object,
                            method,
                            args,
                            ..
                        }
                        if matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "obj")
                            && method == "get"
                            && args.len() == 1
                            && matches!(&args[0], AstNode::Identifier { name, .. } if name == "a")
                    )
                )
            });

            (has_call_assignment, has_method_return)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_call_assignment,
        "Unary plus on compute(a) should preserve function-call assignment shape"
    );
    assert!(
        has_method_return,
        "Unary plus on obj.get(a) should preserve method-call return shape"
    );
}

#[test]
fn test_return_expression_unary_minus_parenthesized_addition() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_unary_paren_precedence.fol")
            .expect("Should read unary parenthesized precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unary parenthesized precedence function");

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
                    AstNode::UnaryOp {
                        op: fol_parser::ast::UnaryOperator::Neg,
                        operand
                    } if matches!(operand.as_ref(), AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Add, .. })
                ),
                "Left side should be negated parenthesized addition"
            );
        }
        _ => panic!("Return value should be binary multiplication expression"),
    }
}

#[test]
fn test_return_expression_subtraction_is_left_associative() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_assoc_sub.fol")
        .expect("Should read subtraction associativity function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse subtraction associativity function");

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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Sub));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Sub,
                        ..
                    }
                ),
                "Left side should contain the first subtraction for left associativity"
            );
        }
        _ => panic!("Return value should be binary subtraction expression"),
    }
}

#[test]
fn test_return_expression_division_is_left_associative() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_assoc_div.fol")
        .expect("Should read division associativity function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse division associativity function");

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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Div));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Div,
                        ..
                    }
                ),
                "Left side should contain the first division for left associativity"
            );
        }
        _ => panic!("Return value should be binary division expression"),
    }
}

#[test]
fn test_return_expression_mixed_precedence_and_associativity() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_mixed_precedence_assoc.fol")
            .expect("Should read mixed precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse mixed precedence function");

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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Sub));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Sub,
                        left: _,
                        right
                    } if matches!(right.as_ref(), AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Mul, .. })
                ),
                "Expected (a - (b * c)) - d tree shape"
            );
        }
        _ => panic!("Return value should be subtraction expression"),
    }
}

#[test]
fn test_return_expression_division_with_grouped_rhs() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_div_paren_rhs.fol")
        .expect("Should read grouped division function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse division with grouped rhs");

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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Div));
            assert!(
                matches!(
                    right.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Add,
                        ..
                    }
                ),
                "Right side should be grouped addition subtree"
            );
        }
        _ => panic!("Return value should be division expression"),
    }
}

#[test]
fn test_assignment_statement_parsing_with_expression_value() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_assignment.fol")
        .expect("Should read assignment function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse assignment statement");

    let assignment = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Assignment { target, value } = node {
                    Some((target.as_ref().clone(), value.as_ref().clone()))
                } else {
                    None
                }
            })
            .expect("Program should contain an assignment statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(assignment.0, AstNode::Identifier { name, .. } if name == "result"),
        "Assignment target should be identifier 'result'"
    );
    assert!(
        matches!(assignment.1, AstNode::BinaryOp { .. }),
        "Assignment value should be parsed as expression tree"
    );
}

#[test]
fn test_field_assignment_target_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_field_assignment.fol")
        .expect("Should read field assignment function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse field assignment target");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::Assignment { target, value }
                            if matches!(
                                target.as_ref(),
                                AstNode::FieldAccess { object, field }
                                if field == "current"
                                    && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "obj")
                            )
                                && matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "value")
                        )
                    }),
                    "Assignment target should parse as field access"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_index_assignment_target_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_index_assignment.fol")
        .expect("Should read index assignment function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse index assignment target");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::Assignment { target, value }
                            if matches!(
                                target.as_ref(),
                                AstNode::IndexAccess { container, index }
                                if matches!(container.as_ref(), AstNode::Identifier { name, .. } if name == "items")
                                    && matches!(index.as_ref(), AstNode::Identifier { name, .. } if name == "idx")
                            )
                                && matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "value")
                        )
                    }),
                    "Assignment target should parse as index access"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_index_assignment_target_missing_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_index_assignment_missing_close.fol")
            .expect("Should read malformed index assignment function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject index assignment target missing closing ']'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' for index assignment target"),
        "Malformed index assignment target should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Malformed index assignment target should report the assignment line"
    );
}
