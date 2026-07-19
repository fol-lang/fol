use super::*;

#[test]
fn test_quoted_type_references_parse_inside_function_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_quoted_function_type_refs.fol")
        .expect("Should read quoted function-type reference fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type refs inside function types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Function { params, return_type, env_lifetime: _,
}
                        },
                        ..
                    } if name == "Handler"
                        && matches!(params.as_slice(), [FolType::Named { name, .. }] if name == "Input")
                        && fol_type_has_qualified_segments(return_type.as_ref(), &["pkg", "Output"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_references_parse_inside_function_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_function_type_refs.fol")
            .expect("Should read single-quoted function-type reference fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type refs inside function types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Function { params, return_type, env_lifetime: _,
}
                        },
                        ..
                    } if name == "Handler"
                        && matches!(params.as_slice(), [FolType::Named { name, .. }] if name == "Input")
                        && fol_type_has_qualified_segments(return_type.as_ref(), &["pkg", "Output"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
