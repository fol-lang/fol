use super::*;

#[test]
fn test_keyword_named_routines_and_calls_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_keyword_named_routines.fol")
        .expect("Should read keyword-named routine test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword-named routines and calls");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::FunDecl { name, .. } if name == "get")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::FunDecl { name, .. } if name == "log")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "demo"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Assignment { value, .. }
                            if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "get" && args.len() == 1)
                        ))
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::MethodCall { method, args, .. } if method == "log" && args.is_empty())
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
