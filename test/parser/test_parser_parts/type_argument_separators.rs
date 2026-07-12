use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_aliases_and_bindings_accept_semicolon_type_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_type_args_semicolon_alias_binding.fol")
            .expect("Should read semicolon type-argument alias/binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated type arguments in aliases and bindings");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Sequence { element_type }
                    },
                    ..
                }
                if name == "Numbers"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::VarDecl {
                    name,
                    type_hint: Some(FolType::Sequence { element_type }),
                    ..
                }
                if name == "items"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::VarDecl {
                        name,
                        type_hint: Some(FolType::Map { key_type, value_type }),
                        ..
                    }
                    if name == "cache"
                        && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                        && matches!(
                            value_type.as_ref(),
                            FolType::Vector { element_type }
                            if matches!(element_type.as_ref(), FolType::Int { .. })
                        )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_routines_and_use_declarations_accept_semicolon_type_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_type_args_semicolon_routine_use.fol")
            .expect("Should read semicolon type-argument routine/use fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated type arguments in routines and uses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::UseDecl {
                    name,
                    path_type: FolType::Module { name: module_name },
                    ..
                }
                if name == "loader" && module_name == "std"
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl {
                    name,
                    params,
                    return_type: Some(FolType::Map { key_type, value_type }),
                    ..
                }
                if name == "read"
                    && params.len() == 1
                    && matches!(
                        params[0].param_type,
                        FolType::Sequence { ref element_type }
                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Input"])
                    )
                    && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                    && matches!(
                        value_type.as_ref(),
                        FolType::Vector { element_type }
                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                    )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_type_bodies_accept_semicolon_type_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_type_args_semicolon_type_bodies.fol")
            .expect("Should read semicolon type-argument type-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated type arguments in type bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { fields, .. },
                    ..
                }
                if name == "Config"
                    && matches!(
                        fields.get("values"),
                        Some(FolType::Sequence { element_type })
                        if matches!(element_type.as_ref(), FolType::Int { .. })
                    )
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { variants, .. },
                    ..
                }
                if name == "Event"
                    && matches!(
                        variants.get("Data"),
                        Some(Some(FolType::Map { key_type, value_type }))
                        if matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                            && matches!(
                                value_type.as_ref(),
                                FolType::Vector { element_type }
                                if matches!(element_type.as_ref(), FolType::Int { .. })
                            )
                    )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_special_type_forms_accept_semicolon_type_arguments() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_type_args_semicolon_specials.fol")
            .expect("Should read semicolon type-argument special-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated type arguments in special forms");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Optional { inner }
                    },
                    ..
                }
                if name == "MaybePath"
                    && matches!(inner.as_ref(), FolType::Location { name } if name == "std")
            )));

            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Multiple { types }
                    },
                    ..
                }
                if name == "Paths"
                    && matches!(types.as_slice(),
                        [FolType::Location { name: left }, FolType::Package { name: right }]
                        if left == "std" && right == "web")
            )));

        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_shared_type_argument_lists_accept_trailing_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_type_args_trailing_separator.fol")
            .expect("Should read trailing shared type-argument fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing shared type-argument separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Sequence { element_type }
                    },
                    ..
                }
                if name == "Numbers"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Map { key_type, value_type }
                    },
                    ..
                }
                if name == "Lookup"
                    && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                    && matches!(
                        value_type.as_ref(),
                        FolType::Vector { element_type }
                        if matches!(element_type.as_ref(), FolType::Int { .. })
                    )
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl {
                    name,
                    params,
                    return_type: Some(FolType::Map { key_type, value_type }),
                    body,
                    ..
                }
                if name == "build"
                    && params.len() == 1
                    && matches!(
                        params[0].param_type,
                        FolType::Sequence { ref element_type }
                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Input"])
                    )
                    && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                    && matches!(
                        value_type.as_ref(),
                        FolType::Vector { element_type }
                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                    )
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl {
                            name,
                            type_hint: Some(FolType::Map { key_type, value_type }),
                            ..
                        }
                        if name == "cache"
                            && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                            && matches!(
                                value_type.as_ref(),
                                FolType::Vector { element_type }
                                if matches!(element_type.as_ref(), FolType::Int { .. })
                            )
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_generic_type_instantiations_parse_in_alias_and_qualified_signature_positions() {
    let temp_root = unique_temp_root("generic_type_instantiation_parser");
    std::fs::create_dir_all(&temp_root).expect("Should create temp parser fixture dir");
    let fixture = temp_root.join("generic_type_instantiation.fol");
    std::fs::write(
        &fixture,
        "ali LocalBox: Box[int];\nfun[] main(value: shared::Box[int]): Box[int] = {\n    var copy: shared::Box[int] = value;\n    return copy;\n};\n",
    )
    .expect("Should write generic type instantiation parser fixture");

    let mut file_stream =
        FileStream::from_file(fixture.to_str().expect("Generic parser fixture path should be UTF-8"))
            .expect("Should read generic type instantiation parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept instantiated generic types in alias and signature positions");

    std::fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::AliasDecl {
                    name,
                    target: FolType::Named { name: target, .. },
                    ..
                }
                if name == "LocalBox" && target == "Box[int]"
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl {
                    name,
                    params,
                    return_type: Some(FolType::Named { name: return_name, .. }),
                    body,
                    ..
                }
                if name == "main"
                    && matches!(
                        params.as_slice(),
                        [fol_parser::ast::Parameter {
                            param_type: FolType::Named { name: param_name, .. },
                            ..
                        }] if param_name == "shared::Box[int]"
                    )
                    && return_name == "Box[int]"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl {
                            name,
                            type_hint: Some(FolType::Named { name: hint, .. }),
                            ..
                        }
                        if name == "copy" && hint == "shared::Box[int]"
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_array_types_accept_semicolon_separator() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_array_types_semicolon.fol")
            .expect("Should read semicolon array-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated array type arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Array { element_type, size: Some(8) }
                    },
                    ..
                }
                if name == "Buffer"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_matrix_types_accept_semicolon_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_matrix_types_semicolon.fol")
            .expect("Should read semicolon matrix-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated matrix type arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Matrix { element_type, dimensions }
                    },
                    ..
                }
                if name == "Grid"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
                    && dimensions == &vec![3, 4]
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_numeric_container_types_accept_trailing_separators() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_typ_numeric_types_trailing_separator.fol",
    )
    .expect("Should read trailing numeric-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing numeric container separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Array { element_type, size: Some(8) }
                    },
                    ..
                }
                if name == "Buffer"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Matrix { element_type, dimensions }
                    },
                    ..
                }
                if name == "Grid"
                    && matches!(element_type.as_ref(), FolType::Int { .. })
                    && dimensions == &vec![3, 4]
            )));
        }
        _ => panic!("Expected program node"),
    }
}
