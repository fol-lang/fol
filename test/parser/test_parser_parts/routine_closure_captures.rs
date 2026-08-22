use super::*;

#[test]
fn test_named_function_closure_captures() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_named_closure_capture.fol")
        .expect("Should read named closure capture test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse named function capture lists");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "add"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::FunDecl { name, captures, .. }
                            if name == "added" && captures == &vec!["n".to_string()]
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_logical_and_procedure_closure_captures() {
    let mut file_stream = FileStream::from_file("test/parser/simple_named_log_pro_capture.fol")
        .expect("Should read logical/procedure capture test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical and procedure capture lists");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::LogDecl { name, captures, .. }
                if name == "matches" && captures == &vec!["rule".to_string(), "value".to_string()]
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::ProDecl { name, captures, .. }
                if name == "emit" && captures == &vec!["sink".to_string()]
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_duplicate_routine_closure_captures_are_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_duplicate_closure_capture.fol")
            .expect("Should read duplicate closure capture test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate capture names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let message = parse_error.message.clone();
    assert!(
        message.contains("Duplicate capture name 'n'"),
        "Duplicate capture names should be rejected, got: {}",
        message
    );
}

#[test]
fn test_canonical_duplicate_routine_closure_captures_are_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_duplicate_closure_capture_canonical.fol")
            .expect("Should read canonical duplicate closure capture test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate capture names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let message = parse_error.message.clone();
    assert!(
        message.contains("Duplicate capture name 'NValue'"),
        "Canonical duplicate capture names should report the later spelling, got: {}",
        message
    );
}

#[test]
fn test_named_routine_closure_captures_accept_semicolon_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_named_capture_semicolon.fol")
        .expect("Should read semicolon routine capture test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated routine capture lists");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "add"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::FunDecl { name, captures, .. }
                            if name == "added" && captures == &vec!["n".to_string()]
                        ))
                )
            }));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::LogDecl { name, captures, .. }
                if name == "matches" && captures == &vec!["rule".to_string(), "value".to_string()]
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::ProDecl { name, captures, .. }
                if name == "emit" && captures == &vec!["sink".to_string()]
            )));
        }
        _ => panic!("Expected program node"),
    }
}
