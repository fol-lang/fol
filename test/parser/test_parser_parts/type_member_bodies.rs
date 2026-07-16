use super::*;
use fol_parser::ast::FunOption;

#[test]
fn test_record_type_accepts_routine_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_methods.fol")
        .expect("Should read record routine-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept routine members in record type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Computer"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, .. } if name == "getBrand"))
                    && members.iter().any(|member| matches!(member, AstNode::ProDecl { name, .. } if name == "reset"))
                    && members.iter().any(|member| matches!(member, AstNode::LogDecl { name, .. } if name == "ready"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_routine_members_retain_receiver_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_method_receivers.fol")
            .expect("Should read type-member receiver fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain receiver types on type-body routine members");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Computer"
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::FunDecl {
                            name,
                            receiver_type: Some(receiver),
                            ..
                        } if name == "parse_msg"
                            && fol_type_has_qualified_segments(receiver, &["pkg", "Parser"])
                    ))
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::ProDecl {
                            name,
                            receiver_type: Some(FolType::Vector { element_type }),
                            ..
                        }
                            if name == "store"
                                && fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Item"])
                    ))
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::LogDecl {
                            name,
                            receiver_type: Some(receiver),
                            ..
                        } if name == "ready" && fol_type_has_qualified_segments(receiver, &["pkg", "Status"])
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_accepts_routine_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_methods.fol")
        .expect("Should read entry routine-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept routine members in entry type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { members, .. },
                    ..
                }
                if name == "Status"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, .. } if name == "label"))
                    && members.iter().any(|member| matches!(member, AstNode::ProDecl { name, .. } if name == "reset"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_accepts_alias_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_alias_member.fol")
        .expect("Should read record alias-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept alias members in record type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Distance"
                    && members.iter().any(|member| matches!(member, AstNode::AliasDecl { name, target: FolType::Named { name: target , ..}, .. } if name == "Unit" && target == "str"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_accepts_nested_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_type_member.fol")
        .expect("Should read record type-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept nested type members in record type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Distance"
                    && members.iter().any(|member| matches!(member, AstNode::TypeDecl { name, type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} }, .. } if name == "Unit" && target == "str"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_accepts_alias_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_alias_member.fol")
        .expect("Should read entry alias-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept alias members in entry type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { members, .. },
                    ..
                }
                if name == "Status"
                    && members.iter().any(|member| matches!(member, AstNode::AliasDecl { name, target: FolType::Named { name: target , ..}, .. } if name == "Label" && target == "str"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_accepts_nested_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_type_member.fol")
        .expect("Should read entry type-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept nested type members in entry type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { members, .. },
                    ..
                }
                if name == "Status"
                    && members.iter().any(|member| matches!(member, AstNode::TypeDecl { name, type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} }, .. } if name == "Label" && target == "str"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_accepts_prefixed_export_methods() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_export_method.fol")
            .expect("Should read prefixed record method fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept prefixed export methods in record type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Computer"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, options, .. } if name == "getType" && options.contains(&FunOption::Export)))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_accepts_prefixed_export_methods() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_export_method.fol")
            .expect("Should read prefixed entry method fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept prefixed export methods in entry type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { members, .. },
                    ..
                }
                if name == "Status"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, options, .. } if name == "label" && options.contains(&FunOption::Export)))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_routine_members_retain_captures() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_method_captures.fol")
            .expect("Should read type-member capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain captures on type-body routine members");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Counter"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, captures, .. } if name == "next" && captures == &vec!["step".to_string()]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_routine_members_retain_inquiries() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_method_inquiries.fol")
            .expect("Should read type-member inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain inquiries on type-body routine members");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Counter"
                    && members.iter().any(|member| matches!(member, AstNode::FunDecl { name, inquiries, .. } if name == "next" && !inquiries.is_empty()))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_bodies_accept_grouped_type_members() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_grouped_type_members.fol")
            .expect("Should read grouped type-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should flatten grouped type members inside record bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Outer"
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl { name, type_def: TypeDefinition::Record { .. }, .. }
                        if name == "Inner"
                    ))
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} },
                            ..
                        } if name == "Label" && target == "str"
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_type_bodies_accept_empty_object_members() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_object_member_empty.fol")
            .expect("Should read empty object-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain empty object members inside record bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { members, .. },
                    ..
                }
                if name == "Outer"
                    && members.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Record { fields, members, .. },
                            ..
                        } if name == "Inner" && fields.is_empty() && members.is_empty()
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
