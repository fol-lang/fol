use super::*;

#[test]
fn test_owned_recursive_type_edge_parses_in_type_position() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_owned_recursive.fol").unwrap();
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser.parse(&mut lexer).expect("opt @Node should parse");

    let AstNode::Program { declarations } = ast else {
        panic!("expected program");
    };
    assert!(declarations.iter().any(|node| matches!(
        node,
        AstNode::TypeDecl {
            type_def: TypeDefinition::Record { fields, .. },
            ..
        } if matches!(
            fields.get("next"),
            Some(FolType::Optional { inner }) if matches!(
                inner.as_ref(),
                FolType::Owned { inner } if matches!(
                    inner.as_ref(),
                    FolType::Named { name, .. } if name == "Node"
                )
            )
        )
    )));
}

#[test]
fn test_canonical_duplicate_record_field_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_duplicate_field_canonical.fol")
            .expect("Should read canonical duplicate record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate record fields");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate record field 'UserName'"),
        "Canonical duplicate record field should report the later spelling, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Canonical duplicate record field parse error should point to the duplicate field line"
    );
}

#[test]
fn test_top_level_type_record_fields_support_lab_prefix() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_lab_fields.fol")
        .expect("Should read lab record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse record definitions with lab-prefixed fields");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    }
                    if matches!(fields.get("name"), Some(FolType::Named { name, .. }) if name == "str")
                        && matches!(fields.get("size"), Some(FolType::Int { size: None, signed: true }))
                        && matches!(fields.get("ready"), Some(FolType::Bool))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_lab_field_missing_name_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_missing_lab_field_name.fol")
            .expect("Should read malformed lab record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject lab record fields without names");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected field name in type record definition"),
        "Malformed lab record field should report the missing name, got: {}",
        first_message
    );
}

#[test]
fn test_top_level_type_record_marker_accepts_empty_brackets() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_marker_empty_options.fol")
            .expect("Should read empty record marker option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typ Name: rec[] = { ... } declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Record { fields, .. },
                                ..
                            }
                            if name == "User"
                                && matches!(fields.get("name"), Some(FolType::Named { name, .. }) if name == "str")
                                && matches!(fields.get("age"), Some(FolType::Int { size: None, signed: true }))
                        )
                    }),
                    "Parser should treat rec[] marker the same as rec marker"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_generic_headers_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_generics.fol")
        .expect("Should read type generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type generic headers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            generics,
                            type_def: TypeDefinition::Record { .. },
                            ..
                        }
                        if name == "Box"
                            && generics.len() == 1
                            && matches!(
                                generics.first(),
                                Some(fol_parser::ast::Generic { name, constraints })
                                    if name == "T"
                                        && matches!(
                                            constraints.as_slice(),
                                            [FolType::Named { name, .. }] if name == "item"
                                        )
                            )
                    )
                }),
                "Type generic header should populate TypeDecl generics for record types"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            generics,
                            type_def: TypeDefinition::Alias { .. },
                            ..
                        }
                        if name == "Pair" && generics.len() == 2
                    )
                }),
                "Type generic header should parse multiple generic constraints"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_generic_headers_accept_unconstrained_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_generics_unconstrained.fol")
            .expect("Should read unconstrained type generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unconstrained type generic headers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            generics,
                            type_def: TypeDefinition::Record { .. },
                            ..
                        }
                        if name == "Rect"
                            && matches!(
                                generics.as_slice(),
                                [fol_parser::ast::Generic { name, constraints }]
                                    if name == "geo" && constraints.is_empty()
                            )
                    )
                }),
                "Type generic header should allow bare generic names"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_generic_headers_reject_duplicate_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_generics_duplicate.fol")
        .expect("Should read duplicate type generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate type generic names");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate generic name 'T'"),
        "Duplicate type generic should report the repeated name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Duplicate type generic parse error should point to the declaration line"
    );
}

#[test]
fn test_type_generic_headers_accept_semicolon_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_generics_semicolon.fol")
        .expect("Should read semicolon-separated type generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type generic headers with ';' separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                generics,
                                type_def: TypeDefinition::Alias { .. },
                                ..
                            }
                            if name == "Pair"
                                && generics.len() == 2
                                && matches!(
                                    generics.as_slice(),
                                    [
                                        fol_parser::ast::Generic { name: first, constraints: first_constraints },
                                        fol_parser::ast::Generic { name: second, constraints: second_constraints }
                                    ]
                                    if first == "T"
                                        && second == "U"
                                        && matches!(first_constraints.as_slice(), [FolType::Named { name, .. }] if name == "left")
                                        && matches!(second_constraints.as_slice(), [FolType::Named { name, .. }] if name == "right")
                                )
                        )
                    }),
                    "Type generic headers should accept ';' separators between generic items"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_declaration_option_brackets_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_options.fol")
        .expect("Should read type option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type option brackets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            options,
                            type_def: TypeDefinition::Alias { .. },
                            ..
                        }
                        if name == "PublicText"
                            && options == &vec![fol_parser::ast::TypeOption::Export]
                    )
                }),
                "typ[+] should parse export type option"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            options,
                            type_def: TypeDefinition::Record { .. },
                            ..
                        }
                        if name == "Accessors"
                            && options
                                == &vec![
                                    fol_parser::ast::TypeOption::Set,
                                    fol_parser::ast::TypeOption::Get
                                ]
                    )
                }),
                "typ[set, get] should parse multiple type options"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_extension_type_option_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_extension_option.fol")
        .expect("Should read extension type option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typ[ext] declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl { name, options, .. }
                    if name == "StrExt"
                        && options.contains(&fol_parser::ast::TypeOption::Extension)
                )
            }));
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_entry_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry.fol")
        .expect("Should read type entry test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse entry type declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Entry { variants, .. },
                                ..
                            }
                            if name == "Color"
                                && matches!(variants.get("BLUE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("RED"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("BLACK"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("WHITE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                        )
                    }),
                    "Parser should build TypeDecl entry variants with shared type hints"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_entry_parsing_accepts_comma_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_commas.fol")
        .expect("Should read comma-separated type entry test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse entry type declarations with comma separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Entry { variants, .. },
                                ..
                            }
                            if name == "ColorCodes"
                                && matches!(variants.get("BLUE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("RED"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("BLACK"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("WHITE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                        )
                    }),
                    "Parser should accept comma-separated entry variants, including trailing comma"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_entry_marker_accepts_empty_brackets() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_marker_empty_options.fol")
            .expect("Should read empty entry marker option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typ Name: ent[] = { ... } declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Entry { variants, .. },
                                ..
                            }
                            if name == "Color"
                                && matches!(variants.get("BLUE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("RED"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("BLACK"), Some(Some(FolType::Named { name, .. })) if name == "str")
                                && matches!(variants.get("WHITE"), Some(Some(FolType::Named { name, .. })) if name == "str")
                        )
                    }),
                    "Parser should treat ent[] marker the same as ent marker"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_unknown_type_option_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_options_unknown.fol")
        .expect("Should read malformed type options test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when type options contain an unknown item");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown type option"),
        "Malformed type option should report unknown option, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Type option parse error should point to the declaration line"
    );
}
