use super::*;

#[test]
fn test_basic_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    match parse_script_as_program(&mut parser, &mut lexer) {
        Ok(ast) => {
            match &ast {
                AstNode::Program { declarations } => {
                    assert!(
                        !declarations.is_empty(),
                        "Parser should collect at least identifiers/literals"
                    );
                    assert!(
                        declarations.iter().any(|node| {
                            matches!(
                                node,
                                AstNode::VarDecl {
                                    name,
                                    type_hint: Some(_),
                                    value: Some(_),
                                    ..
                                } if name == "x"
                            )
                        }),
                        "Parser should build a var declaration node for simple_var.fol"
                    );
                }
                _ => panic!("Should return Program node"),
            }
            println!("Successfully parsed AST: {:?}", ast);
        }
        Err(errors) => {
            panic!("Parser should not fail: {:?}", errors);
        }
    }
}

#[test]
fn test_top_level_let_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_let.fol").expect("Should read let test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level let declarations");

    match ast {
        AstNode::Program { declarations } => {
            let has_inferred_let = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, options }
                    if name == "message"
                        && type_hint.is_none()
                        && value.is_some()
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_typed_let = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl {
                        name,
                        type_hint: Some(FolType::Int { size: None, signed: true }),
                        value: Some(_),
                        options
                    }
                    if name == "count"
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            assert!(
                has_inferred_let,
                "Parser should lower let message = ... into immutable VarDecl"
            );
            assert!(has_typed_let, "Parser should parse typed let declaration");
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_con_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_con.fol").expect("Should read con test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level con declarations");

    match ast {
        AstNode::Program { declarations } => {
            let has_inferred_con = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, options }
                    if name == "message"
                        && type_hint.is_none()
                        && value.is_some()
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_typed_con = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl {
                        name,
                        type_hint: Some(FolType::Int { size: None, signed: true }),
                        value: Some(_),
                        options
                    }
                    if name == "count"
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            assert!(
                has_inferred_con,
                "Parser should lower con message = ... into immutable VarDecl"
            );
            assert!(has_typed_con, "Parser should parse typed con declaration");
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_alias_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_ali.fol").expect("Should read alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level alias declarations");

    match ast {
        AstNode::Program { declarations } => {
            let has_text_alias = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl { name, target: FolType::Named { name: target_name , ..}, .. }
                    if name == "Text" && target_name == "str"
                )
            });

            let has_count_alias = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target: FolType::Int { size: None, signed: true }, .. }
                    if name == "Count"
                )
            });

            assert!(
                has_text_alias,
                "Parser should build alias declaration for Text: str"
            );
            assert!(
                has_count_alias,
                "Parser should build alias declaration for Count: int"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_alias_parsing_supports_qualified_target_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_ali_qualified.fol")
        .expect("Should read qualified alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse alias declarations with qualified target types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::AliasDecl {
                            name,
                            target, .. } if name == "ResultAlias"
                            && fol_type_has_qualified_segments(target, &["pkg", "result", "Value"])
                    )
                }),
                "Alias target type should preserve qualified path segments"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_alias_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_alias.fol")
        .expect("Should read type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse minimal typ alias declarations");

    match ast {
        AstNode::Program { declarations } => {
            let has_text_type_alias = declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias { target: FolType::Named { name: target_name , ..} },
                            ..
                        }
                        if name == "Text" && target_name == "str"
                    )
                });

            let has_counter_type_alias = declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Int { size: None, signed: true }
                        },
                        ..
                    }
                    if name == "Counter"
                )
            });

            assert!(
                has_text_type_alias,
                "Parser should build TypeDecl alias for Text: str"
            );
            assert!(
                has_counter_type_alias,
                "Parser should build TypeDecl alias for Counter: int"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_parsing_supports_bracketed_target_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_bracket_alias.fol")
        .expect("Should read bracketed type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typ aliases with bracketed target types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Map { key_type, value_type }
                                },
                                ..
                            }
                            if name == "OutputMap"
                                && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                                && matches!(
                                    value_type.as_ref(),
                                    FolType::Vector { element_type }
                                    if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                                )
                        )
                    }),
                    "Type alias target should preserve nested bracketed syntax"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_missing_bracket_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_bracket_alias_missing_close.fol")
            .expect("Should read malformed bracketed type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when typ alias target is missing closing ']'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed bracketed typ alias should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Bracketed typ alias parse error should point to the declaration line"
    );
}

#[test]
fn test_top_level_type_record_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record.fol")
        .expect("Should read type record test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse minimal typ record declarations");

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
                            if name == "Person"
                                && matches!(fields.get("name"), Some(FolType::Named { name, .. }) if name == "str")
                                && matches!(fields.get("age"), Some(FolType::Int { size: None, signed: true }))
                        )
                    }),
                    "Parser should build TypeDecl record with named fields"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_record_marker_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_marker.fol")
        .expect("Should read type record marker test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse typ Name: rec = { ... } declarations");

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
                    "Parser should build TypeDecl record from explicit rec marker syntax"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_top_level_type_record_fields_support_var_prefix_and_defaults() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_var_fields.fol")
        .expect("Should read record var-field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse record fields declared with 'var' and defaults");

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
                                && matches!(fields.get("username"), Some(FolType::Named { name, .. }) if name == "str")
                                && matches!(fields.get("email"), Some(FolType::Named { name, .. }) if name == "str")
                                && matches!(fields.get("sign_in_count"), Some(FolType::Int { size: None, signed: true }))
                                && matches!(fields.get("active"), Some(FolType::Bool))
                        )
                    }),
                    "Record fields should accept 'var' prefixes and ignore default initializers"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_record_var_field_missing_colon_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_var_field_missing_colon.fol")
            .expect("Should read malformed record var-field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject record fields missing ':' after the name");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ':' after record field name"),
        "Malformed record field should report missing ':', got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Record field parse error should point to the bad field line"
    );
}

#[test]
fn test_duplicate_record_field_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_duplicate_field.fol")
            .expect("Should read duplicate record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate record fields");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate record field 'user'"),
        "Duplicate record field should report the field name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Duplicate record field parse error should point to the duplicate field line"
    );
}
