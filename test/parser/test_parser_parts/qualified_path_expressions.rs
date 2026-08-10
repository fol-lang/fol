use super::*;

#[test]
fn test_qualified_path_identifier_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_qualified_path_expr.fol")
        .expect("Should read qualified path expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified path expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::QualifiedIdentifier { path }
                            if path.segments == vec![
                                "io".to_string(),
                                "console".to_string(),
                                "writer".to_string(),
                            ]
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_path_method_chain_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_method_chain.fol")
            .expect("Should read qualified path method-chain fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified path postfix chains");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::MethodCall { object, method, .. }
                            if method == "echo"
                                && matches!(
                                    object.as_ref(),
                                    AstNode::QualifiedIdentifier { path }
                                    if path.segments == vec![
                                        "io".to_string(),
                                        "console".to_string(),
                                        "writer".to_string(),
                                    ]
                                )
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_path_call_statement_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_call_stmt.fol")
            .expect("Should read qualified path call statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified path call statements");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::QualifiedFunctionCall { path, .. }
                        if path.segments == vec![
                            "io".to_string(),
                            "console".to_string(),
                            "write_out".to_string(),
                        ]
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_path_method_call_statement_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_method_call_stmt.fol")
            .expect("Should read qualified path method-call statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified path method call statements");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::MethodCall { object, method, .. }
                        if method == "echo"
                            && matches!(
                                object.as_ref(),
                                AstNode::QualifiedIdentifier { path }
                                if path.segments == vec![
                                    "io".to_string(),
                                    "console".to_string(),
                                    "writer".to_string(),
                                ]
                            )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_path_assignment_target_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_assignment_target.fol")
            .expect("Should read qualified path assignment target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified path assignment targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Assignment { target, .. }
                        if matches!(
                            target.as_ref(),
                            AstNode::QualifiedIdentifier { path }
                            if path.segments == vec![
                                "io".to_string(),
                                "console".to_string(),
                                "writer".to_string(),
                            ]
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
