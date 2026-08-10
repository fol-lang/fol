use super::*;

#[test]
fn test_nil_literals_parse_in_bindings_and_returns() {
    let mut file_stream = FileStream::from_file("test/parser/simple_nil_literals.fol")
        .expect("Should read nil literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nil literals");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::VarDecl {
                    name,
                    value: Some(value),
                    ..
                }
                if name == "empty" && matches!(value.as_ref(), AstNode::Literal(Literal::Nil))
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, body, .. }
                if name == "clear"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return {
                            value: Some(value),
                            ..
                        }
                        if matches!(value.as_ref(), AstNode::Literal(Literal::Nil))
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
