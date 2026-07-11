use super::*;

#[test]
fn test_keyword_named_alias_and_type_declarations_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_keyword_named_types.fol")
        .expect("Should read keyword-named type declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept builtin-token alias and type names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl { name, target, .. }
                    if name == "log"
                        && matches!(target, FolType::Int { size: None, signed: true })
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    }
                    if name == "get"
                        && matches!(fields.get("value"), Some(FolType::Int { size: None, signed: true }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
