use super::*;

#[test]
fn test_each_iteration_accepts_quoted_binder() {
    let mut file_stream = FileStream::from_file("test/parser/simple_each_quoted_binder.fol")
        .expect("Should read quoted each-binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted iteration binders");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop {
                        condition, body, ..
                    }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration { var, condition: Some(_), .. }
                        if var == "line"
                    ) && body.iter().any(|stmt| matches!(stmt, AstNode::Yield { value } if matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "line")))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_for_iteration_accepts_typed_quoted_binder() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_for_quoted_typed_binder.fol")
            .expect("Should read quoted typed for-binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept typed quoted binders");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop { condition, .. }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "item" && name == "str"
                    )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_each_iteration_accepts_single_quoted_binder() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_each_single_quoted_binder.fol")
            .expect("Should read single-quoted each-binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted iteration binders");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop { condition, .. }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration { var, condition: Some(_), .. }
                        if var == "line"
                    )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_loop_iteration_accepts_typed_single_quoted_binder() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_loop_single_quoted_typed_binder.fol")
            .expect("Should read single-quoted typed loop-binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept typed single-quoted binders");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Loop { condition, .. }
                    if matches!(
                        condition.as_ref(),
                        fol_parser::ast::LoopCondition::Iteration {
                            var,
                            type_hint: Some(FolType::Named { name, .. }),
                            condition: Some(_),
                            ..
                        } if var == "item" && name == "str"
                    )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
