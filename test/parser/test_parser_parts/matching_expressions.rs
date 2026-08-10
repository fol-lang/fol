use super::*;
use fol_parser::ast::{ContainerType, WhenCase};

#[test]
fn test_if_matching_expression_parses_in_variable_initializer() {
    let mut file_stream = FileStream::from_file("test/parser/simple_if_matching_expr.fol")
        .expect("Should read if-matching expression fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept if-style matching expressions");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::VarDecl {
                    name,
                    value: Some(value),
                    ..
                } if name == "checker"
                    && matches!(
                        value.as_ref(),
                        AstNode::When { cases, default, .. }
                            if cases.len() == 2 && default.is_some()
                    )
            ))
    ));
}

#[test]
fn test_when_matching_expression_parses_in_function_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_when_matching_expr.fol")
        .expect("Should read when-matching expression fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept when-style matching expressions");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl {
                    name,
                    body,
                    ..
                } if name == "someValue"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                            if matches!(value.as_ref(), AstNode::When { cases, default, .. } if cases.len() == 2 && default.is_some())
                    ))
            ))
    ));
}

#[test]
fn test_matching_expression_supports_multi_member_has_cases() {
    let mut file_stream = FileStream::from_file("test/parser/simple_has_matching_expr.fol")
        .expect("Should read has-matching expression fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept multi-member has matching expressions");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::VarDecl {
                    name,
                    value: Some(value),
                    ..
                } if name == "has_it"
                    && matches!(
                        value.as_ref(),
                        AstNode::When { cases, .. }
                            if matches!(
                                cases.as_slice(),
                                [WhenCase::Has { member, .. }, ..]
                                    if matches!(
                                        member,
                                        AstNode::ContainerLiteral {
                                            container_type: ContainerType::Set,
                                            elements,
                                        } if elements.len() == 2
                                    )
                            )
                    )
            ))
    ));
}
