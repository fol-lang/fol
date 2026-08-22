use super::*;

#[test]
fn test_record_type_definition_accepts_quoted_fields() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_quoted_fields.fol")
        .expect("Should read quoted record-field fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted record field names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    } if name == "Data" && fields.contains_key("id") && fields.contains_key("label")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_definition_accepts_quoted_variants() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_quoted_variants.fol")
        .expect("Should read quoted entry-variant fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted entry variant names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    } if name == "Result" && variants.contains_key("ok") && variants.contains_key("err")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_members_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_type_members.fol")
            .expect("Should read single-quoted type-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type members");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    } if name == "Data" && fields.contains_key("id") && fields.contains_key("label")
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    } if name == "Result" && variants.contains_key("ok") && variants.contains_key("err")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_type_references_parse_in_type_member_hints() {
    let mut file_stream = FileStream::from_file("test/parser/simple_quoted_type_member_hints.fol")
        .expect("Should read quoted type-member hint fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type refs in type member hints");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    } if name == "Data"
                        && matches!(fields.get("id"), Some(FolType::Named { name, .. }) if name == "Item")
                        && matches!(fields.get("label"), Some(typ) if fol_type_has_qualified_segments(typ, &["pkg", "Label"]))
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    } if name == "Result"
                        && matches!(variants.get("ok"), Some(Some(FolType::Named { name, .. })) if name == "Success")
                        && matches!(variants.get("err"), Some(Some(typ)) if fol_type_named_text_is(typ, "errs::Failure"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_references_parse_in_type_member_hints() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_type_member_hints.fol")
            .expect("Should read single-quoted type-member hint fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type refs in type member hints");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    } if name == "Data"
                        && matches!(fields.get("id"), Some(FolType::Named { name, .. }) if name == "Item")
                        && matches!(fields.get("label"), Some(typ) if fol_type_has_qualified_segments(typ, &["pkg", "Label"]))
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    } if name == "Result"
                        && matches!(variants.get("ok"), Some(Some(FolType::Named { name, .. })) if name == "Success")
                        && matches!(variants.get("err"), Some(Some(typ)) if fol_type_named_text_is(typ, "errs::Failure"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
