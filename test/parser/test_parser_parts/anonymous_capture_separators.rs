use super::*;

#[test]
fn test_anonymous_routines_accept_semicolon_capture_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_anonymous_capture_semicolon.fol")
            .expect("Should read semicolon anonymous capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated captures on anonymous routines");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
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
            )));
            assert!(declarations.iter().any(|node| matches!(
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
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::LogDecl { name, body, .. }
                if name == "check_it"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousLog { captures, .. }
                            if captures == &vec!["ready".to_string()]
                        )
                    ))
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, body, .. }
                if name == "outer"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::AnonymousFun { captures, .. }
                            if captures == &vec!["left".to_string(), "right".to_string()]
                        )
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
