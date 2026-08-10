use super::*;

#[test]
fn test_anonymous_routine_capture_lists_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_anonymous_routine_captures.fol")
        .expect("Should read anonymous routine captures fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse capture lists on anonymous routines");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "make"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousFun { captures, .. }
                                if captures == &vec!["outer".to_string(), "count".to_string()]
                            )
                        ))
                )
            }));

            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { name, body, .. }
                    if name == "build"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousPro { captures, .. }
                                if captures == &vec!["outer".to_string()]
                            )
                        ))
                )
            }));

            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::LogDecl { name, body, .. }
                    if name == "check_it"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousLog { captures, return_type: Some(FolType::Bool), .. }
                                if captures == &vec!["ready".to_string()]
                            )
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shorthand_anonymous_function_capture_lists_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_shorthand_anonymous_capture_expr.fol")
            .expect("Should read shorthand anonymous capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse capture lists on shorthand anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "outer"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::AnonymousFun { captures, params, .. }
                                if captures == &vec!["left".to_string(), "right".to_string()]
                                    && params.is_empty()
                            )
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
