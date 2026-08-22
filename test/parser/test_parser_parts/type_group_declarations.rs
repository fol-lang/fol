use super::*;

#[test]
fn test_grouped_type_declarations_expand_into_multiple_nodes() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_group_basic.fol")
        .expect("Should read grouped type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should expand grouped type declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    options,
                    type_def: TypeDefinition::Record { .. },
                    ..
                } if name == "User" && options.contains(&fol_parser::ast::TypeOption::Export)
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} },
                    ..
                } if name == "Label" && target == "str"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_type_declarations_accept_mixed_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_group_mixed_separators.fol")
            .expect("Should read grouped type mixed-separator fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped type separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl { name, type_def: TypeDefinition::Entry { .. }, .. }
                if name == "Status"
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias { target: FolType::Int { .. } },
                    ..
                } if name == "Count"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_multi_name_type_alias_declarations_expand_into_multiple_nodes() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_multi_aliases.fol")
        .expect("Should read multi-name type alias fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should expand multi-name type aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    options,
                    type_def: TypeDefinition::Alias { target: FolType::Int { .. } },
                    ..
                } if name == "IntAlias"
                    && options.contains(&fol_parser::ast::TypeOption::Extension)
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    options,
                    type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} },
                    ..
                } if name == "TextAlias"
                    && target == "str"
                    && options.contains(&fol_parser::ast::TypeOption::Extension)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_multi_name_type_declarations_share_object_definitions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_multi_object_shared.fol")
        .expect("Should read shared object-definition fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should clone a shared object definition across names");

    match ast {
        AstNode::Program { declarations } => {
            let matched = declarations
                .iter()
                .filter(|node| matches!(
                    node,
                    AstNode::TypeDecl { name, type_def: TypeDefinition::Record { fields, .. }, .. }
                    if (name == "User" || name == "Admin")
                        && matches!(fields.get("name"), Some(FolType::Named { name, .. }) if name == "str")
                ))
                .count();
            assert_eq!(
                matched, 2,
                "Expected shared object definition for both names"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_multi_name_type_declarations_reject_generic_headers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_multi_names_with_generics.fol")
            .expect("Should read invalid multi-name generic type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject generics on multi-name type declarations");

    assert!(errors.iter().any(|error| {
        error.message.contains(
            "Type generics and explicit contracts are currently supported only on single-name type declarations",
        )
    }));
}

#[test]
fn test_multi_name_type_declarations_reject_explicit_contract_headers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_multi_names_with_contracts.fol")
            .expect("Should read invalid multi-name contract type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject explicit contracts on multi-name type declarations");

    assert!(errors.iter().any(|error| {
        error.message.contains(
            "Type generics and explicit contracts are currently supported only on single-name type declarations",
        )
    }));
}

#[test]
fn test_multi_name_type_declarations_reject_mismatched_definition_counts() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_multi_names_mismatched_defs.fol")
            .expect("Should read mismatched multi-name type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject mismatched multi-name type definitions");

    assert!(errors.iter().any(|error| {
        error.message.contains(
            "Type definition count must match declared names or provide a single shared definition",
        )
    }));
}

#[test]
fn test_grouped_type_declarations_accept_empty_object_markers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_group_object_empty_body.fol")
            .expect("Should read grouped object marker fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped empty object markers");

    match ast {
        AstNode::Program { declarations } => {
            let matched = declarations
                .iter()
                .filter(|node| matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, members, .. },
                        ..
                    } if (name == "User" || name == "Admin") && fields.is_empty() && members.is_empty()
                ))
                .count();
            assert_eq!(
                matched, 2,
                "Expected both grouped object markers to lower to empty records"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_multi_name_type_declarations_accept_shared_empty_object_markers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_multi_object_empty_body.fol")
            .expect("Should read multi-name empty object fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should share empty object markers across names");

    match ast {
        AstNode::Program { declarations } => {
            let matched = declarations
                .iter()
                .filter(|node| matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Record { fields, members, .. },
                        ..
                    } if (name == "User" || name == "Admin") && fields.is_empty() && members.is_empty()
                ))
                .count();
            assert_eq!(
                matched, 2,
                "Expected both names to receive the shared empty object definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}
