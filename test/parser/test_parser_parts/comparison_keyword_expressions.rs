use super::*;

#[test]
fn test_function_return_parses_in_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_in_expr.fol")
        .expect("Should read in-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept 'in' expressions");

    let return_expr = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl { body, .. } = node {
                    body.iter().find_map(|stmt| {
                        if let AstNode::Return {
                            value: Some(value), ..
                        } = stmt
                        {
                            Some(value.as_ref().clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .expect("Function body should include return expression"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_expr,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::In,
                ref left,
                ref right,
            } if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "item")
                && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "items")
        ),
        "Return expression should lower into BinaryOperator::In",
    );
    assert_eq!(
        return_expr.syntactic_type_hint(),
        Some(FolType::Bool),
        "'in' expressions should infer a bool type hint",
    );
}

#[test]
fn test_function_return_parses_has_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_has_expr.fol")
        .expect("Should read has-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept 'has' expressions");

    let return_expr = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl { body, .. } = node {
                    body.iter().find_map(|stmt| {
                        if let AstNode::Return {
                            value: Some(value), ..
                        } = stmt
                        {
                            Some(value.as_ref().clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .expect("Function body should include return expression"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_expr,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Has,
                ref left,
                ref right,
            } if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "user")
                && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "name")
        ),
        "Return expression should lower into BinaryOperator::Has",
    );
    assert_eq!(
        return_expr.syntactic_type_hint(),
        Some(FolType::Bool),
        "'has' expressions should infer a bool type hint",
    );
}

#[test]
fn test_function_return_parses_is_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_is_expr.fol")
        .expect("Should read is-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept 'is' expressions");

    let return_expr = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl { body, .. } = node {
                    body.iter().find_map(|stmt| {
                        if let AstNode::Return {
                            value: Some(value), ..
                        } = stmt
                        {
                            Some(value.as_ref().clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .expect("Function body should include return expression"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_expr,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Is,
                ref left,
                ref right,
            } if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "value")
                && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "expected")
        ),
        "Return expression should lower into BinaryOperator::Is",
    );
    assert_eq!(
        return_expr.syntactic_type_hint(),
        Some(FolType::Bool),
        "'is' expressions should infer a bool type hint",
    );
}

#[test]
fn test_function_return_parses_as_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_as_expr.fol")
        .expect("Should read as-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept 'as' expressions");

    let return_expr = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl { body, .. } = node {
                    body.iter().find_map(|stmt| {
                        if let AstNode::Return {
                            value: Some(value), ..
                        } = stmt
                        {
                            Some(value.as_ref().clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .expect("Function body should include return expression"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_expr,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::As,
                ref left,
                ref right,
            } if matches!(left.as_ref(), AstNode::Identifier { name, .. } if name == "value")
                && matches!(right.as_ref(), AstNode::Identifier { name, .. } if name == "text")
        ),
        "Return expression should lower into BinaryOperator::As",
    );
}
