use super::*;

#[test]
fn test_self_and_this_parse_as_identifier_expressions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_self_this_expr.fol")
        .expect("Should read self/this expression test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept self/this in expression positions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Assignment { value, .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FieldAccess { object, field }
                            if field == "value"
                                && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "self")
                        )
                    )) && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                        if matches!(
                            value.as_ref(),
                            AstNode::FieldAccess { object, field }
                            if field == "count"
                                && matches!(object.as_ref(), AstNode::Identifier { name, .. } if name == "this")
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
