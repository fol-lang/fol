use super::*;

#[test]
fn test_prefix_availability_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_prefix_availability_expr.fol")
            .expect("Should read availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse prefix availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AvailabilityAccess { target }
                            if matches!(target.as_ref(), AstNode::IndexAccess { .. }))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_multi_pattern_availability_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_multi_pattern_availability_expr.fol")
            .expect("Should read multi-pattern availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multi-pattern availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AvailabilityAccess { target }
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_multi_pattern_availability_expression_parsing() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_multi_pattern_availability_expr_semicolon.fol",
    )
    .expect("Should read semicolon multi-pattern availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AvailabilityAccess { target }
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_availability_expression_parsing() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_multi_pattern_availability_expr_trailing_separator.fol",
    )
    .expect("Should read trailing-separator availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing-separator availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AvailabilityAccess { target }
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_empty_prefix_availability_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_prefix_availability_expr_empty.fol")
            .expect("Should read empty availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse empty prefix availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::AvailabilityAccess { target }
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.is_empty()))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_suffix_availability_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_suffix_availability_expr.fol")
            .expect("Should read suffix availability fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse suffix availability expressions");

    match ast {
        AstNode::Program { declarations } => {
            let mut saw_pattern = false;
            let mut saw_slice = false;

            for node in declarations {
                if let AstNode::FunDecl { body, .. } = node {
                    for stmt in body {
                        if let AstNode::Return {
                            value: Some(value), ..
                        } = stmt
                        {
                            if let AstNode::AvailabilityAccess { target } = value.as_ref() {
                                if matches!(target.as_ref(), AstNode::PatternAccess { .. }) {
                                    saw_pattern = true;
                                }
                                if matches!(target.as_ref(), AstNode::SliceAccess { .. }) {
                                    saw_slice = true;
                                }
                            }
                        }
                    }
                }
            }

            assert!(
                saw_pattern,
                "Expected suffix availability on pattern access"
            );
            assert!(saw_slice, "Expected suffix availability on slice access");
        }
        _ => panic!("Expected program node"),
    }
}
