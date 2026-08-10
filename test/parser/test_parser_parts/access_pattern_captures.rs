use super::*;

#[test]
fn test_pattern_access_capture_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pattern_access_capture.fol")
            .expect("Should read pattern access capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse captured pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. }
                            if matches!(&patterns[0], AstNode::PatternCapture { binding, .. } if binding == "Y"))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pattern_access_wildcard_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pattern_access_wildcard.fol")
            .expect("Should read wildcard pattern access fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse wildcard pattern access expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::PatternAccess { patterns, .. }
                            if matches!(&patterns[0], AstNode::PatternWildcard))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_availability_access_capture_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_availability_capture.fol")
            .expect("Should read availability capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse availability capture expressions");

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
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. }
                                if matches!(&patterns[0], AstNode::PatternCapture { binding, .. } if binding == "Y")))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_availability_access_wildcard_capture_expression_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_availability_wildcard_capture.fol")
            .expect("Should read wildcard availability capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse wildcard availability capture expressions");

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
                            if matches!(target.as_ref(), AstNode::PatternAccess { patterns, .. }
                                if matches!(&patterns[0], AstNode::PatternCapture { pattern, binding }
                                    if binding == "Y" && matches!(pattern.as_ref(), AstNode::PatternWildcard))))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
