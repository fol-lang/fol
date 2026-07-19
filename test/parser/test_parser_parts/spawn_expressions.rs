use super::*;

#[test]
fn test_spawn_routine_call_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_spawn_call.fol")
        .expect("Should read spawn call fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse spawn call expressions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. } | AstNode::ProDecl { body, .. }
                        if body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value) }
                                if matches!(value.as_ref(), AstNode::Spawn { task, .. }
                                    if matches!(task.as_ref(), AstNode::FunctionCall { name, .. } if name == "doItFast"))
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_spawn_anonymous_function_expression_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_spawn_anonymous_fun.fol")
        .expect("Should read spawn anonymous function fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse spawned anonymous functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. } | AstNode::ProDecl { body, .. }
                        if body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Return { value: Some(value) }
                                if matches!(value.as_ref(), AstNode::Spawn { task, .. }
                                    if matches!(task.as_ref(), AstNode::AnonymousFun { .. }))
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
