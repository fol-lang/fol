use super::*;

#[test]
fn test_bounded_slice_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_slice_expr.fol")
        .expect("Should read slice expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bounded slice expressions");

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
                            AstNode::SliceAccess { start, end, reverse, .. }
                            if !reverse && start.is_some() && end.is_some()
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_open_slice_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_open_slice_expr.fol")
        .expect("Should read open slice expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse open slice expressions");

    match ast {
        AstNode::Program { declarations } => {
            let mut found_open_start = false;
            let mut found_open_end = false;
            let mut found_full = false;

            for node in declarations {
                if let AstNode::FunDecl { body, .. } = node {
                    for stmt in body {
                        if let AstNode::Return { value: Some(value), .. } = stmt {
                            if let AstNode::SliceAccess {
                                start, end, reverse, ..
                            } = value.as_ref()
                            {
                                if !reverse && start.is_none() && end.is_some() {
                                    found_open_start = true;
                                }
                                if !reverse && start.is_some() && end.is_none() {
                                    found_open_end = true;
                                }
                                if !reverse && start.is_none() && end.is_none() {
                                    found_full = true;
                                }
                            }
                        }
                    }
                }
            }

            assert!(found_open_start, "Expected [:end] slice");
            assert!(found_open_end, "Expected [start:] slice");
            assert!(found_full, "Expected [:] slice");
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_reverse_slice_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_reverse_slice_expr.fol")
        .expect("Should read reverse slice expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse reverse slice expressions");

    match ast {
        AstNode::Program { declarations } => {
            let mut found_full = false;
            let mut found_bounded = false;
            let mut found_open_end = false;

            for node in declarations {
                if let AstNode::FunDecl { body, .. } = node {
                    for stmt in body {
                        if let AstNode::Return { value: Some(value), .. } = stmt {
                            if let AstNode::SliceAccess {
                                start, end, reverse, ..
                            } = value.as_ref()
                            {
                                if *reverse && start.is_none() && end.is_none() {
                                    found_full = true;
                                }
                                if *reverse && start.is_some() && end.is_some() {
                                    found_bounded = true;
                                }
                                if *reverse && start.is_some() && end.is_none() {
                                    found_open_end = true;
                                }
                            }
                        }
                    }
                }
            }

            assert!(found_full, "Expected [::] slice");
            assert!(found_bounded, "Expected [start::end] slice");
            assert!(found_open_end, "Expected [start::] slice");
        }
        _ => panic!("Expected program node"),
    }
}
