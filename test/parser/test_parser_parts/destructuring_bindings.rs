use super::*;

#[test]
fn test_destructuring_binding_supports_leading_rest_pattern() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_destructure_leading_rest.fol")
            .expect("Should read leading-rest destructuring fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept leading-rest destructuring bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::DestructureDecl {
                    pattern: BindingPattern::Sequence(parts),
                    ..
                } if matches!(
                    parts.as_slice(),
                    [BindingPattern::Name(start, _), BindingPattern::Rest(rest)]
                        if start == "start" && rest == "_"
                )
            ))
    ));
}

#[test]
fn test_destructuring_binding_supports_trailing_rest_pattern() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_destructure_trailing_rest.fol")
            .expect("Should read trailing-rest destructuring fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept trailing-rest destructuring bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::DestructureDecl {
                    pattern: BindingPattern::Sequence(parts),
                    ..
                } if matches!(
                    parts.as_slice(),
                    [BindingPattern::Rest(rest), BindingPattern::Name(end, _)]
                        if rest == "_" && end == "end"
                )
            ))
    ));
}

#[test]
fn test_destructuring_binding_supports_nested_patterns() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_destructure_nested.fol")
            .expect("Should read nested destructuring fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept nested destructuring bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::DestructureDecl {
                    pattern: BindingPattern::Sequence(parts),
                    ..
                } if matches!(
                    parts.as_slice(),
                    [
                        BindingPattern::Name(start, _),
                        BindingPattern::Rest(rest),
                        BindingPattern::Sequence(nested),
                    ]
                    if start == "start"
                        && rest == "_"
                        && matches!(
                            nested.as_slice(),
                            [BindingPattern::Name(letter, _), BindingPattern::Rest(inner_rest)]
                                if letter == "last_word_first_letter" && inner_rest == "_"
                        )
                )
            ))
    ));
}

#[test]
fn test_grouped_destructuring_bindings_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_grouped_destructure.fol")
            .expect("Should read grouped destructuring fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped destructuring bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::DestructureDecl {
                    pattern: BindingPattern::Sequence(parts),
                    ..
                } if matches!(
                    parts.as_slice(),
                    [BindingPattern::Name(first, _), BindingPattern::Rest(rest)]
                        if first == "first" && rest == "_"
                )
            ))
    ));
}

#[test]
fn test_label_destructuring_bindings_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_lab_destructure.fol")
            .expect("Should read label destructuring fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept label destructuring bindings");

    assert!(matches!(
        ast,
        AstNode::Program { declarations }
            if declarations.iter().any(|node| matches!(
                node,
                AstNode::DestructureDecl {
                    is_label,
                    pattern: BindingPattern::Sequence(parts),
                    ..
                } if *is_label
                    && matches!(
                        parts.as_slice(),
                        [BindingPattern::Name(left, _), BindingPattern::Rest(rest)]
                            if left == "left" && rest == "_"
                    )
            ))
    ));
}
