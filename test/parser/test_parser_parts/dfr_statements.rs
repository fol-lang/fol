use super::*;

#[test]
fn test_dfr_statement_parses_inside_routine_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_dfr_statement.fol")
        .expect("Should read dfr parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Parser should accept dfr inside a routine body");
    let body = parsed.source_units[0].items[0]
        .node
        .routine_body()
        .expect("Expected parsed root item to be a routine declaration");

    assert!(
        matches!(body, [AstNode::Dfr { body, .. }, AstNode::Return { .. }] if body.len() == 1),
        "Expected dfr followed by return in routine body, got: {body:?}"
    );
}

#[test]
fn test_dfr_statement_is_rejected_at_file_root() {
    let mut file_stream = FileStream::from_file("test/parser/simple_dfr_file_root.fol")
        .expect("Should read file-root dfr fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let errors = parser
        .parse_package(&mut lexer)
        .expect_err("Parser should reject dfr at file root");
    let first = errors
        .first()
        .expect("Expected parser to emit at least one file-root dfr diagnostic");

    assert!(
        first.message.contains("Control-flow statements are not allowed at file root")
            || first.message.contains("'dfr' is only allowed inside routines"),
        "Expected file-root control-flow diagnostic, got: {}",
        first.message
    );
}

#[test]
fn test_dfr_statement_parses_nested_scopes_and_when_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_dfr_nested_scopes.fol")
        .expect("Should read nested dfr parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Parser should accept nested dfr scopes inside a routine body");
    let body = parsed.source_units[0].items[0]
        .node
        .routine_body()
        .expect("Expected parsed root item to be a routine declaration");

    assert!(
        matches!(&body[0], AstNode::Dfr { body, .. } if body.len() == 1),
        "Expected first body statement to be a dfr block, got: {body:?}"
    );
    assert!(
        matches!(&body[1], AstNode::Block { statements, .. } if matches!(statements.as_slice(), [AstNode::Dfr { .. }, AstNode::When { .. }])),
        "Expected nested block to keep its dfr and when statements, got: {body:?}"
    );
    assert!(
        matches!(&body[2], AstNode::Return { .. }),
        "Expected trailing return after nested dfr block, got: {body:?}"
    );
}

#[test]
fn test_dfr_statement_keeps_nested_return_in_ast_for_later_validation() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_dfr_reject_return_nested.fol")
            .expect("Should read nested-return dfr parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Parser should accept nested return syntax inside dfr blocks");
    let body = parsed.source_units[0].items[0]
        .node
        .routine_body()
        .expect("Expected parsed root item to be a routine declaration");

    assert!(
        matches!(
            &body[0],
            AstNode::Dfr { body, .. }
                if matches!(body.as_slice(), [AstNode::Block { statements, .. }] if matches!(statements.as_slice(), [AstNode::Return { .. }]))
        ),
        "Expected nested return to remain inside the parsed dfr block, got: {body:?}"
    );
}
