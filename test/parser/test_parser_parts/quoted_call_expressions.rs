use super::*;

#[test]
fn test_function_body_accepts_quoted_call_expression_values() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_quoted_call_expression.fol")
            .expect("Should read quoted call-expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted root calls in expression position");

    match ast {
        AstNode::Program { declarations } => {
            let body = declarations
                .iter()
                .find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => Some(body),
                    _ => None,
                })
                .expect("Program should include function body");
            assert!(body.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(value.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "make_err" && args.len() == 1)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
