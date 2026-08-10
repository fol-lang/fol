use super::*;

#[test]
fn test_quoted_member_access_and_calls_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_quoted_members.fol")
        .expect("Should read quoted member access test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted member access and method calls");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Assignment { value, .. }
                        if matches!(value.as_ref(), AstNode::MethodCall { method, .. } if method == "$")
                    )) && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::MethodCall { method, .. } if method == "kind"
                    )) && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(value.as_ref(), AstNode::FieldAccess { field, .. } if field == "name")
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
