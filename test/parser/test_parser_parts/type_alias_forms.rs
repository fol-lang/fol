use super::*;
use fol_parser::PointerQualifier;

#[test]
fn test_type_alias_parsing_supports_special_boxed_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_special_types.fol")
        .expect("Should read special type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse special boxed typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias {
                                target: FolType::Optional { inner }
                            },
                            ..
                        }
                        if name == "MaybeText"
                            && matches!(inner.as_ref(), FolType::Named { name, .. } if name == "str")
                    )
                }),
                "Type alias should lower opt[...] to FolType::Optional"
            );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Multiple { types }
                                },
                                ..
                            }
                            if name == "Pair"
                                && types.len() == 2
                                && matches!(types.first(), Some(FolType::Int { size: None, signed: true }))
                                && matches!(types.get(1), Some(FolType::Named { name, .. }) if name == "str")
                        )
                    }),
                    "Type alias should lower mul[...] to FolType::Multiple"
                );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias {
                                target: FolType::Pointer { target, .. }
                            },
                            ..
                        }
                        if name == "CounterPtr"
                            && matches!(target.as_ref(), FolType::Int { size: None, signed: true })
                    )
                }),
                "Type alias should lower ptr[...] to FolType::Pointer"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias {
                                target: FolType::Error { inner: Some(inner) }
                            },
                            ..
                        }
                        if name == "Failure"
                            && matches!(inner.as_ref(), FolType::Named { name, .. } if name == "str")
                    )
                }),
                "Type alias should lower err[T] to FolType::Error with payload"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias {
                                target: FolType::Error { inner: None }
                            },
                            ..
                        }
                        if name == "BareFailure"
                    )
                }),
                "Type alias should lower err[] to FolType::Error without payload"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_pointer_qualifiers_and_optional_pointer_chain_parse() {
    let mut stream =
        FileStream::from_file("test/parser/simple_typ_pointer_qualifiers.fol").unwrap();
    let mut lexer = Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let AstNode::Program { declarations } = parser.parse(&mut lexer).unwrap() else {
        panic!("expected program");
    };

    let qualifier_for = |wanted: &str| {
        declarations.iter().find_map(|node| match node {
            AstNode::TypeDecl {
                name,
                type_def:
                    TypeDefinition::Alias {
                        target: FolType::Pointer { qualifier, .. },
                    },
                ..
            } if name == wanted => Some(*qualifier),
            _ => None,
        })
    };
    assert_eq!(qualifier_for("Unique"), Some(PointerQualifier::Unique));
    assert_eq!(qualifier_for("Shared"), Some(PointerQualifier::Shared));
    assert_eq!(qualifier_for("Raw"), Some(PointerQualifier::Raw));
    assert!(declarations.iter().any(|node| matches!(
        node,
        AstNode::TypeDecl {
            name,
            type_def: TypeDefinition::Alias {
                target: FolType::Optional { inner }
            },
            ..
        } if name == "MaybeShared" && matches!(
            inner.as_ref(),
            FolType::Pointer {
                qualifier: PointerQualifier::Shared,
                ..
            }
        )
    )));
}

#[test]
fn test_special_boxed_type_bad_arity_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_special_types_bad_arity.fol")
            .expect("Should read malformed special type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when opt[...] has the wrong number of arguments");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected exactly one type argument for opt[...]"),
        "Malformed opt type should report bad arity, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Special type bad-arity parse error should point to the declaration line"
    );
}

#[test]
fn test_type_alias_parsing_supports_container_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_container_types.fol")
        .expect("Should read container type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse container typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Vector { element_type }
                                },
                                ..
                            }
                            if name == "Names"
                                && matches!(element_type.as_ref(), FolType::Named { name, .. } if name == "str")
                        )
                    }),
                    "Type alias should lower vec[...] to FolType::Vector"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Sequence { element_type }
                                },
                                ..
                            }
                            if name == "Queue"
                                && fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Task"])
                        )
                    }),
                    "Type alias should lower seq[...] to FolType::Sequence"
                );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias {
                                target: FolType::Set { types }
                            },
                            ..
                        }
                        if name == "Palette"
                            && types.len() == 2
                    )
                }),
                "Type alias should lower set[...] to FolType::Set"
            );
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
                            if name == "Lookup"
                                && matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                                && matches!(
                                    value_type.as_ref(),
                                    FolType::Vector { element_type }
                                    if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                                )
                        )
                    }),
                    "Type alias should lower map[...] to FolType::Map"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_container_type_bad_arity_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_container_types_bad_arity.fol")
            .expect("Should read malformed container type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when map[...] has the wrong number of arguments");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected exactly two type arguments for map[...]"),
        "Malformed map type should report bad arity, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Container type bad-arity parse error should point to the declaration line"
    );
}

#[test]
fn test_type_alias_parsing_supports_module_and_block_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_module_block_types.fol")
        .expect("Should read module/block type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse module and block typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Module { name: module_name } },
                                ..
                            }
                            if name == "StdModule" && module_name == "std"
                        )
                    }),
                    "Type alias should lower mod[std] to FolType::Module"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Module { name: module_name } },
                                ..
                            }
                            if name == "InitModule" && module_name.is_empty()
                        )
                    }),
                    "Type alias should lower mod[] to an empty FolType::Module name"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Block { name: block_name } },
                                ..
                            }
                            if name == "JumpBlock" && block_name == "label"
                        )
                    }),
                    "Type alias should lower blk[label] to FolType::Block"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Block { name: block_name } },
                                ..
                            }
                            if name == "EmptyBlock" && block_name.is_empty()
                        )
                    }),
                    "Type alias should lower blk[] to an empty FolType::Block name"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_parsing_supports_bare_module_and_block_types() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_bare_module_block_types.fol")
            .expect("Should read bare module/block type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bare module and block typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Module { name: module_name } },
                                ..
                            }
                            if name == "BareModule" && module_name.is_empty()
                        )
                    }),
                    "Type alias should lower bare mod to FolType::Module"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias { target: FolType::Block { name: block_name } },
                                ..
                            }
                            if name == "BareBlock" && block_name.is_empty()
                        )
                    }),
                    "Type alias should lower bare blk to FolType::Block"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_parsing_supports_any_and_none_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_any_none.fol")
        .expect("Should read any/none type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse any/non typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias { target: FolType::Any },
                            ..
                        }
                        if name == "Anything"
                    )
                }),
                "Type alias should lower any to FolType::Any"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias { target: FolType::None },
                            ..
                        }
                        if name == "Nothing"
                    )
                }),
                "Type alias should lower non to FolType::None"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_parsing_lowers_scalar_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_scalar_types.fol")
        .expect("Should read scalar type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower scalar type aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def:
                                TypeDefinition::Alias {
                                    target: FolType::Int {
                                        size: Some(IntSize::I32),
                                        signed: false,
                                    }
                                },
                            ..
                        } if name == "UnsignedWord"
                    )
                }),
                "int[u32] alias should lower to unsigned I32"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def:
                                TypeDefinition::Alias {
                                    target: FolType::Int {
                                        size: Some(IntSize::I64),
                                        signed: true,
                                    }
                                },
                            ..
                        } if name == "SignedWord"
                    )
                }),
                "int[64] alias should lower to signed I64"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def:
                                TypeDefinition::Alias {
                                    target: FolType::Float {
                                        size: Some(FloatSize::F32)
                                    }
                                },
                            ..
                        } if name == "FloatSmall" || name == "BareF32"
                    )
                }),
                "flt[32] and bare f32 aliases should lower to F32"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def:
                                TypeDefinition::Alias {
                                    target: FolType::Char {
                                        encoding: CharEncoding::Utf16
                                    }
                                },
                            ..
                        } if name == "WideRune"
                    )
                }),
                "chr[utf16] alias should lower to Utf16 chars"
            );
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::TypeDecl {
                            name,
                            type_def:
                                TypeDefinition::Alias {
                                    target: FolType::Int {
                                        size: Some(IntSize::I64),
                                        signed: false,
                                    }
                                },
                            ..
                        } if name == "BareU64"
                    )
                }),
                "bare u64 alias should lower to unsigned I64"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_routine_signatures_support_scalar_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_scalar_types.fol")
        .expect("Should read scalar-typed routine test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower scalar types in routine signatures");

    match ast {
        AstNode::Program { declarations } => {
            let function = declarations.iter().find_map(|node| match node {
                AstNode::FunDecl {
                    name,
                    params,
                    return_type,
                    error_type,
                    ..
                } if name == "convert" => Some((params, return_type, error_type)),
                _ => None,
            });

            let (params, return_type, error_type) =
                function.expect("Program should contain convert function");

            assert!(matches!(
                params.first(),
                Some(Parameter {
                    param_type: FolType::Int {
                        size: Some(IntSize::I16),
                        signed: false,
                    },
                    ..
                })
            ));
            assert!(matches!(
                params.get(1),
                Some(Parameter {
                    param_type: FolType::Float {
                        size: Some(FloatSize::F32),
                    },
                    ..
                })
            ));
            assert!(matches!(
                params.get(2),
                Some(Parameter {
                    param_type: FolType::Char {
                        encoding: CharEncoding::Utf32,
                    },
                    ..
                })
            ));
            assert!(matches!(
                return_type,
                Some(FolType::Int {
                    size: Some(IntSize::I64),
                    signed: true,
                })
            ));
            assert!(matches!(error_type, Some(FolType::Bool)));
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_type_alias_parsing_rejects_unknown_scalar_type_option() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_scalar_type_bad_option.fol")
            .expect("Should read malformed scalar type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unknown scalar type options");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown integer type option 'wat'"),
        "Malformed scalar type alias should report unknown option, got: {}",
        first_message
    );
}

#[test]
fn test_use_declarations_support_module_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_mod_type_lowering.fol")
        .expect("Should read module-typed use declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declarations with module path types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::UseDecl {
                            name,
                            path_type: FolType::Module { name: module_name },
                            ..
                        }
                        if name == "file"
                            && module_name == "std"
                            && use_decl_path_text(node).as_deref() == Some("std::fs::File")
                    )
                }),
                "Use declarations should lower mod[...] path types to FolType::Module"
            );
        }
        _ => panic!("Should return Program node"),
    }
}
