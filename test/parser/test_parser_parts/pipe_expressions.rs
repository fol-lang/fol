use super::*;
use fol_parser::ast::BinaryOperator;
use fol_parser::VarOption;

fn contains_pipe_panic_stage(node: &AstNode) -> bool {
    match node {
        AstNode::BinaryOp { op, left, right } => {
            (matches!(op, BinaryOperator::Pipe | BinaryOperator::PipeOr)
                && matches!(right.as_ref(), AstNode::FunctionCall { name, .. } if name == "panic"))
                || contains_pipe_panic_stage(left)
                || contains_pipe_panic_stage(right)
        }
        _ => false,
    }
}

fn contains_pipe_stage(node: &AstNode, predicate: fn(&AstNode) -> bool) -> bool {
    predicate(node) || node.children().into_iter().any(|child| contains_pipe_stage(child, predicate))
}

#[test]
fn test_single_pipe_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_expr.fol")
        .expect("Should read pipe expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse single pipe expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp { op: BinaryOperator::Pipe, .. }
    ));
}

#[test]
fn test_pipe_expression_supports_if_call_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_if_stage.fol")
        .expect("Should read pipe-if stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if(...) stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::FunctionCall { name, .. } if name == "if")
    ));
}

#[test]
fn test_pipe_expression_supports_if_statement_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_if_flow_stmt.fol")
        .expect("Should read pipe-if statement-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if statement stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::When { .. })
    ));
}

#[test]
fn test_pipe_expression_supports_when_statement_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_when_stmt.fol")
        .expect("Should read pipe-when statement-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when statement stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::When { cases, default, .. } if !cases.is_empty() && default.is_some())
    ));
}

#[test]
fn test_pipe_expression_supports_loop_statement_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_while_stmt.fol")
        .expect("Should read pipe-loop statement-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop statement stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::Loop { .. })
    ));
}

#[test]
fn test_pipe_expression_supports_async_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_async_stage.fol")
        .expect("Should read pipe async-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse async stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } | AstNode::ProDecl { body, .. } => {
                    body.iter().find_map(|stmt| match stmt {
                        AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::AsyncStage)
    ));
}

#[test]
fn test_pipe_expression_supports_await_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_await_stage.fol")
        .expect("Should read pipe await-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse await stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } | AstNode::ProDecl { body, .. } => {
                    body.iter().find_map(|stmt| match stmt {
                        AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                        AstNode::Assignment { value, .. } => Some(value.as_ref().clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("Expected pipe expression"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::AwaitStage)
    ));
}

#[test]
fn test_pipe_expression_supports_bare_builtin_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_builtin_stage.fol")
        .expect("Should read pipe builtin stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bare builtin stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_panic_stage(&return_value),
        "Expected chained pipe tree to contain a panic stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_return_stage() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_return_stage.fol")
        .expect("Should read pipe return stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse return stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::PipeOr,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::Return { value: Some(_) })
    ));
}

#[test]
fn test_pipe_expression_supports_binding_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_binding_stage.fol")
        .expect("Should read pipe binding-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::VarDecl { .. })
    ));
}

#[test]
fn test_pipe_expression_supports_binding_alternative_stages() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pipe_binding_alternative_stage.fol")
            .expect("Should read pipe binding-alternative stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding alternative stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp {
            op: BinaryOperator::Pipe,
            right,
            ..
        } if matches!(right.as_ref(), AstNode::VarDecl { options, .. } if options.iter().any(|opt| matches!(opt, VarOption::Export)))
    ));
}

#[test]
fn test_pipe_expression_supports_declaration_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_decl_stage.fol")
        .expect("Should read pipe declaration-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse declaration stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::UseDecl { .. })),
        "Expected pipe tree to contain a use-declaration stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_routine_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_routine_stage.fol")
        .expect("Should read pipe routine-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::FunDecl { .. })),
        "Expected pipe tree to contain a routine stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_control_transfer_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_control_stage.fol")
        .expect("Should read pipe control-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse control-transfer stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::Yield { .. }))
            && contains_pipe_stage(
                &return_value,
                |node| matches!(
                    node,
                    AstNode::Loop { body, .. }
                    if body.iter().any(|stmt| matches!(stmt, AstNode::Break))
                )
            ),
        "Expected pipe tree to contain yield and loop-wrapped break stages, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_assignment_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_assignment_stage.fol")
        .expect("Should read pipe assignment-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse assignment stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::Assignment { .. })),
        "Expected pipe tree to contain an assignment stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_call_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_call_stage.fol")
        .expect("Should read pipe call-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse call stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::FunctionCall { name, .. } if name == "report")),
        "Expected pipe tree to contain a call stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_invoke_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_invoke_stage.fol")
        .expect("Should read pipe invoke-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse invoke stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::Invoke { .. })),
        "Expected pipe tree to contain an invoke stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_supports_block_stages() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_block_stage.fol")
        .expect("Should read pipe block-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(&return_value, |node| matches!(node, AstNode::Block { .. })),
        "Expected pipe tree to contain a block stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_rejects_missing_rhs() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_missing_rhs.fol")
        .expect("Should read malformed pipe fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject missing pipe RHS");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.clone().contains("Expected expression after '|'"),
        "Expected missing pipe RHS error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_pipe_or_expression_rejects_missing_rhs() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_or_missing_rhs.fol")
        .expect("Should read malformed pipe-or fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject missing pipe-or RHS");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.clone().contains("Expected expression after '||'"),
        "Expected missing pipe-or RHS error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_double_pipe_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_or_expr.fol")
        .expect("Should read pipe-or expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse double-pipe expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(matches!(
        return_value,
        AstNode::BinaryOp { op: BinaryOperator::PipeOr, .. }
    ));
}

#[test]
fn test_chained_pipe_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_chain.fol")
        .expect("Should read chained pipe fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse chained pipe expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_panic_stage(&return_value),
        "Expected chained pipe tree to contain a panic stage, got: {return_value:#?}"
    );
}

#[test]
fn test_pipe_expression_parsing_in_binding_initializer() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pipe_binding_initializer.fol")
            .expect("Should read pipe binding initializer fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse pipe expressions in binding initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl {
                            name,
                            value: Some(value),
                            ..
                        }
                        if name == "result"
                            && matches!(value.as_ref(), AstNode::BinaryOp { op: BinaryOperator::Pipe, .. })
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_expression_parsing_in_call_arguments() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_call_arg.fol")
        .expect("Should read pipe call argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse pipe expressions in call arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return {
                            value: Some(value)
                        }
                        if matches!(
                            value.as_ref(),
                            AstNode::FunctionCall { name, args, .. }
                            if name == "emit"
                                && matches!(args.as_slice(), [AstNode::BinaryOp { op: BinaryOperator::Pipe, .. }])
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pipe_expression_supports_availability_invoke_stages() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pipe_availability_invoke_stage.fol")
            .expect("Should read availability invoke pipe-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse availability invoke stages on pipes");

    let return_value = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| match node {
                AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                    AstNode::Return { value: Some(value) } => Some(value.as_ref().clone()),
                    _ => None,
                }),
                _ => None,
            })
            .expect("Expected return statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        contains_pipe_stage(
            &return_value,
            |node| matches!(node, AstNode::Invoke { callee, .. }
                if matches!(callee.as_ref(), AstNode::AvailabilityAccess { .. }))
        ),
        "Expected pipe tree to contain an availability invoke stage, got: {return_value:#?}"
    );
}
