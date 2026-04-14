use super::*;

#[test]
fn test_quoted_type_references_parse_in_declaration_targets() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_quoted_declaration_targets.fol")
            .expect("Should read quoted declaration-target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type references in declaration targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target: FolType::Named { name: target , ..},
                    } if name == "Alias" && target == "Target"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_references_parse_in_declaration_targets() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_single_quoted_declaration_targets.fol",
    )
    .expect("Should read single-quoted declaration-target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type refs in declaration targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target: FolType::Named { name: target , ..},
                    } if name == "Alias" && target == "Target"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
