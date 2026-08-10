use super::*;

#[test]
fn test_multi_pattern_access_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pattern_access_expr.fol")
        .expect("Should read pattern access fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse multi-pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_multi_pattern_access_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pattern_access_expr_semicolon.fol")
            .expect("Should read semicolon pattern access fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_pattern_access_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pattern_access_expr_trailing_separator.fol")
            .expect("Should read trailing-separator pattern access fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing-separator pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.len() == 2)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_empty_pattern_access_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pattern_access_expr_empty.fol")
            .expect("Should read empty pattern access fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse empty pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. } if patterns.is_empty())
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
