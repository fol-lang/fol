use super::*;

#[test]
fn test_use_declarations_support_bare_module_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_bare_mod_type.fol")
        .expect("Should read bare module-typed use declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declarations with bare module path types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::UseDecl {
                            name,
                            path_type: FolType::Module { name: module_name },
                            ..
                        }
                        if name == "fmt"
                            && module_name.is_empty()
                            && use_decl_path_text(node).as_deref() == Some("core::fmt")
                    )
                }),
                "Use declarations should lower bare mod path types to FolType::Module"
            );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_module_type_bad_arity_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_module_type_bad_arity.fol")
        .expect("Should read malformed module type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when mod[...] has too many arguments");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected zero or one type argument for mod[...]"),
        "Malformed module type should report bad arity, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Module type bad-arity parse error should point to the declaration line"
    );
}

#[test]
fn test_type_alias_parsing_supports_array_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_array_types.fol")
        .expect("Should read array type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse array typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Array { element_type, size: Some(16) }
                                },
                                ..
                            }
                            if name == "Bytes"
                                && matches!(element_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                    }),
                    "Type alias should lower arr[T, N] to FolType::Array"
                );
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Array { element_type, size: Some(4) }
                                },
                                ..
                            }
                            if name == "MatrixRow"
                                && matches!(
                                    element_type.as_ref(),
                                    FolType::Vector { element_type }
                                    if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Value"])
                                )
                        )
                    }),
                    "Array element type should support nested structured container types"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_array_type_bad_size_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_array_types_bad_size.fol")
        .expect("Should read malformed array type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when arr[...] size is not a decimal literal");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected decimal array size in arr[...]"),
        "Malformed array type should report bad size, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Array type bad-size parse error should point to the declaration line"
    );
}

#[test]
fn test_type_alias_parsing_supports_matrix_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_matrix_types.fol")
        .expect("Should read matrix type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse matrix typ aliases");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Matrix { element_type, dimensions }
                                },
                                ..
                            }
                            if name == "Grid"
                                && dimensions.as_slice() == [3, 4]
                                && matches!(element_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                    }),
                    "Type alias should lower mat[T, dims...] to FolType::Matrix"
                );
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Matrix { element_type, dimensions }
                                },
                                ..
                            }
                            if name == "Tensor"
                                && dimensions.as_slice() == [2, 2, 2]
                                && matches!(
                                    element_type.as_ref(),
                                    FolType::Vector { element_type }
                                    if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Value"])
                                )
                        )
                    }),
                    "Matrix element type should support nested structured container types"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_matrix_type_bad_dimension_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_matrix_types_bad_dim.fol")
        .expect("Should read malformed matrix type alias test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when mat[...] dimension is not a decimal literal");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected decimal matrix dimension in mat[...]"),
        "Malformed matrix type should report bad dimension, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Matrix type bad-dimension parse error should point to the declaration line"
    );
}

#[test]
fn test_type_references_support_braced_function_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_function_type_refs.fol")
        .expect("Should read function type reference test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse braced function type references");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::TypeDecl {
                                name,
                                type_def: TypeDefinition::Alias {
                                    target: FolType::Function { params, return_type }
                                },
                                ..
                            }
                            if name == "Adder"
                                && matches!(
                                    params.as_slice(),
                                    [
                                        FolType::Int { size: None, signed: true },
                                        FolType::Int { size: None, signed: true }
                                    ]
                                )
                                && matches!(return_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                    }),
                    "Type alias should lower braced function types into FolType::Function"
                );
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::FunDecl { name, params, .. }
                            if name == "apply"
                                && matches!(
                                    params.as_slice(),
                                    [
                                        Parameter {
                                            name: param_name,
                                            param_type: FolType::Function { params, return_type },
                                            ..
                                        },
                                        ..
                                    ] if param_name == "op"
                                        && matches!(params.as_slice(), [FolType::Int { size: None, signed: true }])
                                        && matches!(return_type.as_ref(), FolType::Int { size: None, signed: true })
                                )
                        )
                    }),
                    "Routine parameters should accept braced function type references"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_function_type_missing_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_function_type_missing_close.fol")
            .expect("Should read malformed function type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject function types missing the closing '}'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '}' to close function type"),
        "Malformed function type should report missing close brace, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Function type parse error should point to the declaration line"
    );
}

#[test]
fn test_function_types_are_supported_in_use_and_binding_declarations() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_function_type_bindings_and_use.fol")
            .expect("Should read function type binding/use test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept function types in use and binding declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::UseDecl {
                                name,
                                path_type: FolType::Function { params, return_type },
                                ..
                            }
                            if name == "callback"
                                && matches!(params.as_slice(), [FolType::Named { name, .. }] if name == "str")
                                && matches!(return_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                    }),
                    "Use declarations should preserve function path types"
                );
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::VarDecl {
                                name,
                                type_hint: Some(FolType::Function { params, return_type }),
                                value: None,
                                ..
                            }
                            if name == "transform"
                                && matches!(params.as_slice(), [FolType::Int { size: None, signed: true }])
                                && matches!(return_type.as_ref(), FolType::Int { size: None, signed: true })
                        )
                    }),
                    "Top-level bindings should accept function type hints"
                );
            assert!(
                    declarations.iter().any(|node| {
                        matches!(
                            node,
                            AstNode::FunDecl { name, body, .. }
                            if name == "local_binding"
                                && body.iter().any(|statement| matches!(
                                    statement,
                                    AstNode::VarDecl {
                                        name,
                                        type_hint: Some(FolType::Function { params, return_type }),
                                        value: Some(_),
                                        ..
                                    }
                                    if name == "formatter"
                                        && matches!(params.as_slice(), [FolType::Named { name, .. }] if name == "str")
                                        && matches!(return_type.as_ref(), FolType::Named { name, .. } if name == "str")
                                ))
                        )
                    }),
                    "Local let bindings should accept function type hints"
                );
        }
        _ => panic!("Should return Program node"),
    }
}

#[test]
fn test_function_type_defaults_are_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_function_type_default_param_forbidden.fol")
            .expect("Should read malformed function type default test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject defaults inside function types");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Default values are not allowed in function types"),
        "Malformed function type default should report the dedicated diagnostic, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Function type default parse error should point to the declaration line"
    );
}

#[test]
fn test_function_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    match parse_script_as_program(&mut parser, &mut lexer) {
        Ok(ast) => {
            match &ast {
                AstNode::Program { declarations } => {
                    assert!(
                        !declarations.is_empty(),
                        "Function source should produce parser nodes"
                    );
                    assert!(
                        only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                            matches!(
                                node,
                                AstNode::Return {
                                    value: Some(value)
                                } if matches!(value.as_ref(), AstNode::BinaryOp { .. })
                            )
                        }),
                        "Function source should include a return node with binary expression"
                    );
                }
                _ => panic!("Should return Program node"),
            }
            println!("Successfully parsed function AST: {:?}", ast);
        }
        Err(errors) => {
            println!("Parser errors (expected for now): {:?}", errors);
            // For now, we expect the minimal parser to work
        }
    }
}

#[test]
fn test_function_body_let_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_let.fol")
        .expect("Should read function let test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse let declarations inside function bodies");

    let (has_inferred_local, has_typed_local, has_return_identifier) = match ast {
        AstNode::Program { declarations } => {
            let has_inferred_local = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, options, .. }
                    if name == "base"
                        && type_hint.is_none()
                        && value.is_some()
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_typed_local = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl {
                        name,
                        type_hint: Some(FolType::Int { size: None, signed: true }),
                        value: Some(_),
                        options,
                        ..
                    }
                    if name == "next"
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_return_identifier = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "next")
                )
            });

            (has_inferred_local, has_typed_local, has_return_identifier)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_inferred_local,
        "Function body should include immutable let local without explicit type"
    );
    assert!(
        has_typed_local,
        "Function body should include immutable typed let local"
    );
    assert!(
        has_return_identifier,
        "Return should still parse after let locals"
    );
}

#[test]
fn test_function_body_con_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_con.fol")
        .expect("Should read function con test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse con declarations inside function bodies");

    let (has_inferred_local, has_typed_local, has_return_identifier) = match ast {
        AstNode::Program { declarations } => {
            let has_inferred_local = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, options, .. }
                    if name == "base"
                        && type_hint.is_none()
                        && value.is_some()
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_typed_local = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl {
                        name,
                        type_hint: Some(FolType::Int { size: None, signed: true }),
                        value: Some(_),
                        options,
                        ..
                    }
                    if name == "next"
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            });

            let has_return_identifier = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value) }
                    if matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "next")
                )
            });

            (has_inferred_local, has_typed_local, has_return_identifier)
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_inferred_local,
        "Function body should include immutable con local without explicit type"
    );
    assert!(
        has_typed_local,
        "Function body should include immutable typed con local"
    );
    assert!(
        has_return_identifier,
        "Return should still parse after con locals"
    );
}

#[test]
fn test_nested_block_statements_parse_inside_function_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_nested_block_stmt.fol")
        .expect("Should read nested block fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested block statements in function bodies");

    let nested_block = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl { name, body, .. } = node {
                    if name == "blocky" {
                        return body.iter().find_map(|statement| {
                            if let AstNode::Block { statements, .. } = statement {
                                Some(statements.clone())
                            } else {
                                None
                            }
                        });
                    }
                }
                None
            })
            .expect("Function body should contain a nested block statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        nested_block.iter().any(|statement| {
            matches!(
                statement,
                AstNode::VarDecl { name, options, .. }
                if name == "inner"
                    && options.contains(&fol_parser::ast::VarOption::Immutable)
            )
        }),
        "Nested block should preserve inner let declarations"
    );
    assert!(
        nested_block.iter().any(|statement| {
            matches!(
                statement,
                AstNode::Return { value: Some(value) }
                if matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "inner")
            )
        }),
        "Nested block should preserve return statements"
    );
}

#[test]
fn test_nested_block_missing_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_nested_block_missing_close.fol")
            .expect("Should read malformed nested block fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject nested blocks missing closing '}'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '}' to close block"),
        "Malformed nested block should report missing close brace, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Malformed nested block parse error should point to the nested block line"
    );
}

#[test]
fn test_function_declaration_header_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse function declaration");

    let function_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl {
                    name,
                    params,
                    return_type,
                    body,
                    ..
                } = node
                {
                    Some((
                        name.clone(),
                        params.len(),
                        return_type.is_some(),
                        body.len(),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include function declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(function_decl.0, "add");
    assert_eq!(function_decl.1, 2, "Function should have two parameters");
    assert!(function_decl.2, "Function should have return type");
    assert!(
        function_decl.3 > 0,
        "Function body should include parsed statements"
    );
}

#[test]
fn test_logical_declaration_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_log.fol").expect("Should read log test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse logical declaration");

    let logical_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::LogDecl {
                    name,
                    params,
                    return_type,
                    body,
                    ..
                } = node
                {
                    if name == "dating" {
                        return Some((
                            params.len(),
                            return_type.clone(),
                            body.iter().any(|statement| {
                                matches!(
                                    statement,
                                    AstNode::Return {
                                        value: Some(value)
                                    } if matches!(
                                        value.as_ref(),
                                        AstNode::BinaryOp {
                                            op: fol_parser::ast::BinaryOperator::Eq,
                                            ..
                                        }
                                    )
                                )
                            }),
                        ));
                    }
                }
                None
            })
            .expect("Program should include a dedicated logical declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(logical_decl.0, 2, "Logical should have two parameters");
    assert!(
        matches!(logical_decl.1, Some(FolType::Bool)),
        "Logical return type should lower to bol/bool"
    );
    assert!(
        logical_decl.2,
        "Logical body should include equality return expression"
    );
}

#[test]
fn test_routine_option_brackets_parse_for_functions_and_procedures() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_options.fol")
        .expect("Should read routine options test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine option brackets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, options, .. }
                        if name == "pure_add" && options.is_empty()
                    )
                }),
                "fun[] should parse with an explicit empty options list"
            );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl { name, options, .. }
                        if name == "publish"
                            && options
                                == &vec![
                                    fol_parser::ast::FunOption::Export,
                                    fol_parser::ast::FunOption::Iterator
                                ]
                    )
                }),
                "pro[+, itr] should parse export and iterator routine options"
            );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, options, .. }
                        if name == "helper"
                            && options == &vec![fol_parser::ast::FunOption::Hidden]
                    )
                }),
                "fun[hid] should parse the hidden routine option"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_routine_generic_headers_parse_for_functions_and_procedures() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_generics.fol")
        .expect("Should read routine generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine generic headers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, generics, params, .. }
                        if name == "lift"
                            && generics.len() == 1
                            && matches!(
                                generics.first(),
                                Some(fol_parser::ast::Generic { name, constraints })
                                    if name == "T"
                                        && matches!(
                                            constraints.as_slice(),
                                            [FolType::Named { name, .. }] if name == "foo"
                                        )
                            )
                            && matches!(
                                params.as_slice(),
                                [Parameter {
                                    name,
                                    param_type: FolType::Named { name: type_name , ..},
                                    ..
                                }] if name == "value" && type_name == "T"
                            )
                    )
                }),
                "Function generic header should populate generics separately from params"
            );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl { name, generics, params, .. }
                        if name == "call_bar"
                            && generics.len() == 2
                            && params.len() == 2
                    )
                }),
                "Procedure generic header should parse multiple generics and params"
            );
        }
        _ => panic!("Expected program node"),
    }
}
