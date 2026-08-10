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

// A call in statement position keeps its whole postfix chain, the same as in
// expression position: `a.b(x).c(y);` and `f().g();` are one statement each.
#[test]
fn test_chained_call_statement_keeps_full_postfix_chain() {
    let mut file_stream = FileStream::from_file("test/parser/simple_chained_call_statement.fol")
        .expect("Should read chained call statement fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept chained calls in statement position");

    let AstNode::Program { declarations } = ast else {
        panic!("Expected program node");
    };
    let body = declarations
        .iter()
        .find_map(|node| match node {
            AstNode::FunDecl { name, body, .. } if name == "main" => Some(body),
            _ => None,
        })
        .expect("Fixture should declare main");

    assert!(
        body.iter().any(|node| matches!(
            node,
            AstNode::MethodCall { object, method, .. }
                if method == "second"
                    && matches!(
                        object.as_ref(),
                        AstNode::MethodCall { method, .. } if method == "first"
                    )
        )),
        "chained method-call statement should parse as one nested method call, got: {body:#?}"
    );

    assert!(
        body.iter().any(|node| matches!(
            node,
            AstNode::MethodCall { object, method, .. }
                if method == "read"
                    && matches!(
                        object.as_ref(),
                        AstNode::FunctionCall { name, .. } if name == "make"
                    )
        )),
        "call-result method statement should parse as a method call on a call, got: {body:#?}"
    );
}
