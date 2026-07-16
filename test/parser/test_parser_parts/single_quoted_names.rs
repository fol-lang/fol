use super::*;

#[test]
fn test_single_quoted_names_parse_across_declaration_surfaces() {
    let mut file_stream = FileStream::from_file("test/parser/simple_single_quoted_names.fol")
        .expect("Should read single-quoted declaration-surface fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted names across declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::SegDecl { name, .. } if name == "core")
            }));
            assert!(declarations.iter().any(|node| {
                use_decl_matches_path(node, "warn", "std/warn")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, .. } if name == "state")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { body, .. }
                    if body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, .. } if name == "helper"))
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "demo"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, .. } if name == "count"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
