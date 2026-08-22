use super::*;

#[test]
fn test_scalar_type_options_accept_semicolon_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_scalar_types_semicolon.fol")
            .expect("Should read semicolon scalar type-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated scalar type options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Int {
                            size: Some(IntSize::I8),
                            signed: false
                        }
                    },
                    ..
                }
                if name == "SmallUnsigned"
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Float {
                            size: Some(FloatSize::F32)
                        }
                    },
                    ..
                }
                if name == "Float32"
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Char {
                            encoding: CharEncoding::Utf8
                        }
                    },
                    ..
                }
                if name == "UtfChar"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_scalar_type_options_accept_trailing_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_scalar_types_trailing_separator.fol")
            .expect("Should read trailing scalar type-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing scalar type-option separators");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(declarations.len(), 3);
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Int {
                            size: Some(IntSize::I8),
                            signed: false
                        }
                    },
                    ..
                }
                if name == "SmallUnsigned"
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Float {
                            size: Some(FloatSize::F32)
                        }
                    },
                    ..
                }
                if name == "Float32"
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Char {
                            encoding: CharEncoding::Utf8
                        }
                    },
                    ..
                }
                if name == "UtfChar"
            )));
        }
        _ => panic!("Expected program node"),
    }
}
