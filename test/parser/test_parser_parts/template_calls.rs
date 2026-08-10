use super::*;

#[test]
fn test_template_call_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_template_call.fol")
        .expect("Should read template call fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse postfix template calls");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                        if body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value), .. }
                                if matches!(value.as_ref(), AstNode::TemplateCall { template, .. } if template == "$")
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_template_call_argument_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_template_call_arg.fol")
        .expect("Should read template call argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse postfix template calls in arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                        if body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::FunctionCall { args, .. }
                                if matches!(args.as_slice(), [AstNode::TemplateCall { template, .. }] if template == "$")
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
