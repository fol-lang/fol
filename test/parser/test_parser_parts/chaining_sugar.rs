use super::*;

// The canonical shell unwrap is inner-place `value[]` (a `PatternAccess` with
// no patterns); the removed postfix `value!` form is rejected (V3_MEM §2.1).
#[test]
fn test_optional_unwrap_postfix_parses() {
    let mut file_stream = FileStream::from_file("test/parser/simple_optional_unwrap_expr.fol")
        .expect("Should read optional unwrap fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept inner-place unwrap expressions");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::FunctionCall { name, args, .. }
                    if name == "echo"
                        && matches!(
                            args.as_slice(),
                            [AstNode::PatternAccess { container, patterns }]
                                if patterns.is_empty()
                                    && matches!(container.as_ref(), AstNode::Identifier { name, .. } if name == "printString")
                        )
            ))
    ));
}

#[test]
fn test_optional_unwrap_type_inference_uses_inner_type() {
    let mut file_stream = FileStream::from_file("test/parser/simple_optional_unwrap_binding.fol")
        .expect("Should read optional unwrap binding fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept postfix optional unwrap in bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::VarDecl {
                    name,
                    value: Some(value),
                    ..
                } if name == "message"
                    && matches!(
                        value.as_ref(),
                        AstNode::PatternAccess { container, patterns }
                            if patterns.is_empty()
                                && matches!(container.as_ref(), AstNode::Identifier { name, .. } if name == "printString")
                    )
            ))
    ));
}
