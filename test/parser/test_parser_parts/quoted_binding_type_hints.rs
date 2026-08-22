use super::*;

#[test]
fn test_quoted_type_references_parse_in_binding_hints() {
    let mut file_stream = FileStream::from_file("test/parser/simple_quoted_binding_type_hints.fol")
        .expect("Should read quoted binding-type hint fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type references in binding hints");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: hint , ..}), .. } if name == "value" && hint == "Item")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, type_hint: Some(hint), .. }
                        if name == "count" && fol_type_named_text_is(hint, "pkg::Count")
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_references_parse_in_binding_hints() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_binding_type_hints.fol")
            .expect("Should read single-quoted binding-type hint fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type refs in binding hints");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: hint , ..}), .. } if name == "value" && hint == "Item")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, type_hint: Some(hint), .. }
                        if name == "count" && fol_type_named_text_is(hint, "pkg::Count")
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
