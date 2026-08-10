use super::*;

#[test]
fn test_chained_assignment_target_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_chained_assignment_target.fol")
            .expect("Should read chained assignment target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse chained assignment target");

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
                                && matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "value")
                        )
                    }),
                    "Assignment target should parse as chained field/index access"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_field_assignment_target_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_quoted_assignment_target.fol")
            .expect("Should read quoted assignment-target test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted field assignment targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { target, .. }
                    if matches!(
                        target.as_ref(),
                        AstNode::FieldAccess { object, field }
                        if field == "$"
                            && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "box")
                    )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_self_assignment_targets_and_this_method_calls_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_self_this_targets.fol")
        .expect("Should read self/this target test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse self assignment targets and this method calls");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { target, .. }
                    if matches!(
                        target.as_ref(),
                        AstNode::FieldAccess { object, field }
                        if field == "value"
                            && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "self")
                    )
                )
            }));
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::MethodCall { object, method, .. }
                    if method == "log"
                        && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "this")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_field_assignment_target_missing_name_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_field_assignment_missing_name.fol")
            .expect("Should read malformed field assignment target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject field assignment target missing field name");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected field name after '.' in assignment target"),
        "Malformed field assignment target should report missing field name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Malformed field assignment target should report the assignment line"
    );
}

#[test]
fn test_method_call_assignment_target_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_call_assignment_target.fol")
            .expect("Should read method-call assignment target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject method call assignment target");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Method call cannot be used as an assignment target"),
        "Method-call assignment target should report explicit target diagnostic, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Method-call assignment target should report the assignment line"
    );
}

#[test]
fn test_function_call_assignment_target_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_assignment_target.fol")
            .expect("Should read function-call assignment target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject function call assignment target");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Function call cannot be used as an assignment target"),
        "Function-call assignment target should report explicit target diagnostic, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Function-call assignment target should report the assignment line"
    );
}

#[test]
fn test_compound_assignment_statements_are_lowered_to_binary_ops() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_compound_assignment.fol")
        .expect("Should read compound assignment function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse compound assignment statements");

    let assignment_ops = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .filter_map(|node| {
                if let AstNode::Assignment { value, .. } = node {
                    if let AstNode::BinaryOp { op, .. } = value.as_ref() {
                        Some(op.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => panic!("Expected program node"),
    };

    assert!(
        assignment_ops.len() >= 4,
        "Expected compound assignments to produce binary expression values"
    );
    assert!(
        matches!(assignment_ops[0], fol_parser::ast::BinaryOperator::Add),
        "'+=' should lower to Add"
    );
    assert!(
        matches!(assignment_ops[1], fol_parser::ast::BinaryOperator::Sub),
        "'-=' should lower to Sub"
    );
    assert!(
        matches!(assignment_ops[2], fol_parser::ast::BinaryOperator::Mul),
        "'*=' should lower to Mul"
    );
    assert!(
        matches!(assignment_ops[3], fol_parser::ast::BinaryOperator::Div),
        "'/=' should lower to Div"
    );
}

#[test]
fn test_field_compound_assignment_target_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_field_compound_assignment.fol")
            .expect("Should read field compound assignment test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse field compound assignment target");

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
                                && matches!(
                                    value.as_ref(),
                                    AstNode::BinaryOp {
                                        op: fol_parser::ast::BinaryOperator::Add,
                                        ..
                                    }
                                )
                        )
                    }),
                    "Field compound assignment should preserve field target and lower to Add"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_index_compound_assignment_target_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_index_compound_assignment.fol")
            .expect("Should read index compound assignment test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse index compound assignment target");

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
                                && matches!(
                                    value.as_ref(),
                                    AstNode::BinaryOp {
                                        op: fol_parser::ast::BinaryOperator::Mod,
                                        ..
                                    }
                                )
                        )
                    }),
                    "Index compound assignment should preserve index target and lower to Mod"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_mod_assignment_and_comparison_expressions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_mod_and_compare.fol")
        .expect("Should read mod and comparison function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse modulo and comparison expressions");

    let (has_mod_assignment, return_ops, return_values) = match ast {
        AstNode::Program { declarations } => {
            let has_mod_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::Assignment { value, .. }
                        if matches!(value.as_ref(), AstNode::BinaryOp { op: fol_parser::ast::BinaryOperator::Mod, .. })
                    )
                });

            let return_ops = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .filter_map(|node| {
                    if let AstNode::Return { value: Some(value), .. } = node {
                        if let AstNode::BinaryOp { op, .. } = value.as_ref() {
                            Some(op.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let return_values = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .filter_map(|node| {
                    if let AstNode::Return { value, .. } = node {
                        Some(format!("{:?}", value))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            (has_mod_assignment, return_ops, return_values)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_mod_assignment,
        "Expected assignment lowered/parsed with modulo binary operator"
    );
    assert!(
            return_ops
                .iter()
                .any(|op| matches!(op, fol_parser::ast::BinaryOperator::Eq)),
            "Expected return expression parsed with equality operator, got ops {:?} and return values {:?}",
            return_ops,
            return_values
        );
}

#[test]
fn test_pow_expression_parsing_is_right_associative() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pow_expr.fol")
        .expect("Should read power expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse exponent expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::Return {
                                value: Some(value),
                                ..
                            }
                            if matches!(
                                value.as_ref(),
                                AstNode::BinaryOp {
                                    op: fol_parser::ast::BinaryOperator::Pow,
                                    left,
                                    right,
                                }
                                if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "a")
                                    && matches!(
                                        right.as_ref(),
                                        AstNode::BinaryOp {
                                            op: fol_parser::ast::BinaryOperator::Pow,
                                            left,
                                            right,
                                        }
                                        if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "b")
                                            && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "c")
                                    )
                            )
                        )
                    }),
                    "Exponent expressions should parse as right-associative binary trees"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pow_compound_assignment_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pow_compound_assignment.fol")
            .expect("Should read power compound assignment test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse exponent compound assignment");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::Assignment { target, value }
                            if matches!(target.as_ref(), AstNode::Identifier { name, .. } if name == "power")
                                && matches!(
                                    value.as_ref(),
                                    AstNode::BinaryOp {
                                        op: fol_parser::ast::BinaryOperator::Pow,
                                        left,
                                        right,
                                    }
                                    if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "power")
                                        && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "base")
                                )
                        )
                    }),
                    "Exponent compound assignments should lower to Pow binary expressions"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_logical_and_has_lower_precedence_than_comparison() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_logical.fol")
        .expect("Should read logical expression function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::And));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Eq,
                        ..
                    }
                ),
                "Left side should be comparison subtree"
            );
            assert!(
                matches!(
                    right.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Eq,
                        ..
                    }
                ),
                "Right side should be comparison subtree"
            );
        }
        _ => panic!("Return value should be logical and expression"),
    }
}

#[test]
fn test_logical_or_has_lower_precedence_than_and() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_logical_or_precedence.fol")
        .expect("Should read logical or precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical or precedence expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Or));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Eq,
                        ..
                    }
                ),
                "Left side should be equality comparison"
            );
            assert!(
                matches!(
                    right.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::And,
                        ..
                    }
                ),
                "Right side should be grouped logical and subtree"
            );
        }
        _ => panic!("Return value should be logical or expression"),
    }
}

#[test]
fn test_logical_not_parses_as_unary_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_logical_not.fol")
        .expect("Should read logical not function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical not expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
                    Some(value.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should contain a return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::UnaryOp {
                op: fol_parser::ast::UnaryOperator::Not,
                ..
            }
        ),
        "Return value should be unary logical-not expression"
    );
}

#[test]
fn test_logical_xor_precedence_between_or_and_and() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_logical_xor_precedence.fol")
            .expect("Should read logical xor precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical xor precedence expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::Or));
            assert!(
                matches!(
                    right.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Xor,
                        ..
                    }
                ),
                "Right side should be logical xor subtree"
            );
            if let AstNode::BinaryOp {
                right: xor_right, ..
            } = right.as_ref()
            {
                assert!(
                    matches!(
                        xor_right.as_ref(),
                        AstNode::BinaryOp {
                            op: fol_parser::ast::BinaryOperator::And,
                            ..
                        }
                    ),
                    "Xor right side should keep tighter logical and subtree"
                );
            }
        }
        _ => panic!("Return value should be logical or expression"),
    }
}

#[test]
fn test_logical_nand_lowers_to_not_of_and() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_logical_nand_nor.fol")
        .expect("Should read logical nand/nor function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical nand/nor expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
                    Some(value.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should contain a return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            &return_value,
            AstNode::UnaryOp {
                op: fol_parser::ast::UnaryOperator::Not,
                operand
            } if matches!(
                operand.as_ref(),
                AstNode::BinaryOp {
                    op: fol_parser::ast::BinaryOperator::And,
                    ..
                }
            )
        ),
        "Nand should lower to not(and(...)), got {:?}",
        return_value
    );
}

#[test]
fn test_logical_nor_lowers_to_not_of_or() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_logical_nor.fol")
        .expect("Should read logical nor function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical nor expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
                    Some(value.as_ref().clone())
                } else {
                    None
                }
            })
            .expect("Program should contain a return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            &return_value,
            AstNode::UnaryOp {
                op: fol_parser::ast::UnaryOperator::Not,
                operand
            } if matches!(
                operand.as_ref(),
                AstNode::BinaryOp {
                    op: fol_parser::ast::BinaryOperator::Or,
                    ..
                }
            )
        ),
        "Nor should lower to not(or(...)), got {:?}",
        return_value
    );
}

#[test]
fn test_logical_not_precedence_over_comparison_and_and() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_logical_not_precedence.fol")
            .expect("Should read logical not precedence function test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical not precedence expression");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::Return { value: Some(value), .. } = node {
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
            assert!(matches!(op, fol_parser::ast::BinaryOperator::And));
            assert!(
                matches!(
                    left.as_ref(),
                    AstNode::BinaryOp {
                        op: fol_parser::ast::BinaryOperator::Eq,
                        left,
                        ..
                    } if matches!(left.as_ref(), AstNode::UnaryOp { op: fol_parser::ast::UnaryOperator::Not, .. })
                ),
                "Expected left comparison to contain unary not on its lhs"
            );
        }
        _ => panic!("Return value should be logical and expression"),
    }
}

#[test]
fn test_literal_parsing() {
    let parser = AstParser::new();

    // Test integer literal
    match parser.parse_literal("42") {
        Ok(ast) => {
            assert!(
                matches!(ast, AstNode::Literal(_)),
                "Should parse integer literal"
            );
        }
        Err(e) => panic!("Should parse integer literal: {:?}", e),
    }

    // Test string literal
    match parser.parse_literal("\"hello\"") {
        Ok(ast) => {
            assert!(
                matches!(ast, AstNode::Literal(_)),
                "Should parse string literal"
            );
        }
        Err(e) => panic!("Should parse string literal: {:?}", e),
    }

    // Test identifier
    match parser.parse_literal("variable_name") {
        Ok(ast) => {
            assert!(
                matches!(ast, AstNode::Identifier { .. }),
                "Should parse identifier"
            );
        }
        Err(e) => panic!("Should parse identifier: {:?}", e),
    }
}

#[test]
fn test_parse_error_has_location_for_illegal_token() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    lexer
        .set_key(KEYWORD::Illegal)
        .expect("Should be able to force an illegal token for parser test");

    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when current token is illegal");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(parse_error.primary_location().unwrap().line > 0, "Line should be non-zero");
    assert!(parse_error.primary_location().unwrap().column > 0, "Column should be non-zero");
    assert!(
        parse_error.primary_location().unwrap().length.unwrap_or(0) > 0,
        "Token length should be non-zero for diagnostics"
    );
}

#[test]
fn test_unary_plus_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_unary_plus_missing_operand.fol")
            .expect("Should read unary-plus missing operand test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when unary plus is missing its operand");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
        first_message.contains("Expected expression after unary '+'"),
        "Unary plus without operand should report explicit unary-plus operand error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Unary plus missing-operand parse error should point to return line"
    );
}

#[test]
fn test_call_argument_unary_plus_missing_operand_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_call_unary_plus_missing_operand.fol")
            .expect("Should read unary-plus missing operand call-arg test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when call arg unary plus is missing an operand");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();

    assert!(
            first_message.contains("Expected expression after unary '+'"),
            "Unary plus without operand in call arg should report explicit unary-plus operand error, got: {}",
            first_message
        );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Call-arg unary plus missing-operand parse error should point to call line"
    );
}
