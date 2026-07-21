use super::*;

#[test]
fn test_routine_generic_headers_accept_unconstrained_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_generics_unconstrained.fol")
            .expect("Should read unconstrained routine generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse unconstrained routine generic headers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, generics, params, .. }
                        if name == "identity"
                            && matches!(
                                generics.as_slice(),
                                [fol_parser::ast::Generic { name, constraints }]
                                    if name == "T" && constraints.is_empty()
                            )
                            && params.len() == 1
                    )
                }),
                "Function generic header should allow bare generic names"
            );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl { name, generics, params, .. }
                        if name == "passthrough"
                            && generics.len() == 2
                            && generics.iter().all(|generic| generic.constraints.is_empty())
                            && params.len() == 2
                    )
                }),
                "Procedure generic header should allow multiple bare generic names"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_routine_generic_headers_reject_duplicate_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_generics_duplicate.fol")
            .expect("Should read duplicate routine generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate routine generic names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate generic name 'T'"),
        "Duplicate routine generic should report the repeated name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Duplicate routine generic parse error should point to the declaration line"
    );
}

#[test]
fn test_routine_generic_headers_reject_canonical_duplicate_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_generics_duplicate_canonical.fol")
            .expect("Should read canonical duplicate routine generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate routine generic names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate generic name 'tvalue'"),
        "Canonical duplicate routine generic should report the later spelling, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Canonical duplicate routine generic parse error should point to the declaration line"
    );
}

#[test]
fn test_routine_generic_headers_reject_default_values() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_generics_default_forbidden.fol")
            .expect("Should read routine generic default-value test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject default values in routine generic headers");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Default values are not allowed in routine generic headers"),
        "Routine generic defaults should report the dedicated diagnostic, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Routine generic default parse error should point to the declaration line"
    );
}

#[test]
fn test_routine_parameters_support_default_values() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_default_params.fol")
        .expect("Should read routine default parameters test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine parameter default values");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, params, .. }
                        if name == "add_bias"
                            && matches!(params.as_slice(),
                                [
                                    Parameter { name: first_name, default: None, .. },
                                    Parameter {
                                        name: second_name,
                                        param_type: FolType::Int { size: None, signed: true },
                                        default: Some(AstNode::BinaryOp { .. }),
                                        ..
                                    }
                                ] if first_name == "a"
                                    && second_name == "bias"
                            )
                    )
                }),
                "Function parameters should preserve default expressions in the AST"
            );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl { name, params, .. }
                        if name == "log_value"
                            && matches!(params.as_slice(),
                                [
                                    Parameter {
                                        name: label_name,
                                        default: Some(AstNode::Literal(Literal::String(_))),
                                        ..
                                    },
                                    Parameter {
                                        name: amount_name,
                                        default: Some(AstNode::Literal(Literal::Integer(1))),
                                        ..
                                    }
                                ] if label_name == "label"
                                    && amount_name == "amount"
                            )
                    )
                }),
                "Procedure parameters should support literal default expressions"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_routine_parameters_support_grouped_names_and_semicolon_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_grouped_params.fol")
        .expect("Should read grouped routine parameters test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse grouped routine parameter declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::FunDecl { name, params, .. }
                            if name == "combine"
                                && matches!(
                                    params.as_slice(),
                                    [
                                        Parameter { name: first, param_type: FolType::Int { size: None, signed: true }, default: None, .. },
                                        Parameter { name: second, param_type: FolType::Int { size: None, signed: true }, default: None, .. },
                                        Parameter { name: label, param_type: FolType::Named { name: label_ty , ..}, default: Some(AstNode::Literal(Literal::Character('x'))), .. }
                                    ] if first == "a"
                                        && second == "b"
                                        && label == "label"
                                        && label_ty == "str"
                                )
                        )
                    }),
                    "Routine headers should expand grouped parameter names and accept ';' separators"
                );
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::ProDecl { name, params, .. }
                            if name == "apply"
                                && matches!(
                                    params.as_slice(),
                                    [
                                        Parameter {
                                            name: op_name,
                                            param_type: FolType::Function { params: fn_params, return_type, env_lifetime: _,
},
                                            ..
                                        },
                                        Parameter { name: left, param_type: FolType::Int { size: None, signed: true }, .. },
                                        Parameter { name: right, param_type: FolType::Int { size: None, signed: true }, .. }
                                    ] if op_name == "op"
                                        && matches!(
                                            fn_params.as_slice(),
                                            [
                                                FolType::Int { size: None, signed: true },
                                                FolType::Int { size: None, signed: true }
                                            ]
                                        )
                                        && matches!(return_type.as_ref(), FolType::Int { size: None, signed: true })
                                        && left == "left"
                                        && right == "right"
                                )
                        )
                    }),
                    "Grouped parameter parsing should also work inside braced function types"
                );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_parameters_missing_colon_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_grouped_params_missing_colon.fol")
            .expect("Should read malformed grouped routine parameters test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject grouped parameters without a shared type");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ':' after parameter name"),
        "Malformed grouped parameters should report the missing type separator, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Grouped parameter parse error should point to the signature line"
    );
}

#[test]
fn test_duplicate_routine_parameter_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_duplicate_param.fol")
        .expect("Should read duplicate parameter routine test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate routine parameter names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate parameter name 'a'"),
        "Duplicate routine parameter should report the repeated name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Duplicate routine parameter parse error should point to the signature line"
    );
}

#[test]
fn test_duplicate_routine_parameter_reports_canonical_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_duplicate_param_canonical.fol")
            .expect("Should read canonical duplicate parameter routine test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate routine parameter names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate parameter name 'AA'"),
        "Canonical duplicate routine parameter should report the later spelling, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Canonical duplicate routine parameter parse error should point to the signature line"
    );
}

#[test]
fn test_duplicate_function_type_parameter_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_function_type_duplicate_param.fol")
            .expect("Should read duplicate function-type parameter test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate function-type parameter names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate parameter name 'x'"),
        "Duplicate function-type parameter should report the repeated name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Duplicate function-type parameter parse error should point to the signature line"
    );
}

#[test]
fn test_parameter_default_missing_value_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_default_param_missing_value.fol")
            .expect("Should read malformed routine default parameter test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject parameter defaults without a value expression");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected default value expression after '=' in parameter"),
        "Malformed parameter default should report missing value, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Parameter default parse error should point to the signature line"
    );
}

#[test]
fn test_unknown_routine_option_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_options_unknown.fol")
        .expect("Should read malformed routine options test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when routine options contain an unknown item");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown routine option"),
        "Malformed routine option should report unknown option, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Routine option parse error should point to the signature line"
    );
}

#[test]
fn test_routine_generic_header_missing_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_routine_generics_missing_separator.fol")
            .expect("Should read malformed routine generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when generic header items are missing a separator");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ',', ';', or ')' after generic parameter"),
        "Malformed generic header should report missing separator, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Generic header parse error should point to the signature line"
    );
}

#[test]
fn test_function_declaration_supports_qualified_type_references() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_qualified_types.fol")
        .expect("Should read qualified function type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified type references in function declarations");

    let function_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl {
                    name,
                    params,
                    return_type,
                    error_type,
                    ..
                } = node
                {
                    Some((
                        name.clone(),
                        params.clone(),
                        return_type.clone(),
                        error_type.clone(),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include function declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(function_decl.0, "convert");
    assert!(
        matches!(
            function_decl.1.as_slice(),
            [Parameter {
                param_type,
                ..
            }] if fol_type_has_qualified_segments(param_type, &["pkg", "Input"])
        ),
        "Parameter type should preserve qualified path"
    );
    assert!(
        matches!(
            function_decl.2,
            Some(ref typ) if fol_type_has_qualified_segments(typ, &["pkg", "Output"])
        ),
        "Return type should preserve qualified path"
    );
    assert!(
        matches!(
            function_decl.3,
            Some(ref typ) if fol_type_named_text_is(typ, "errs::Failure")
        ),
        "Error type should preserve qualified path"
    );
}

#[test]
fn test_function_declaration_supports_bracketed_type_references() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_bracket_types.fol")
        .expect("Should read bracketed function type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bracketed type references in function declarations");

    let function_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl {
                    name,
                    params,
                    return_type,
                    error_type,
                    ..
                } = node
                {
                    Some((
                        name.clone(),
                        params.clone(),
                        return_type.clone(),
                        error_type.clone(),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include function declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(function_decl.0, "collect");
    assert!(
        matches!(
            function_decl.1.as_slice(),
            [Parameter {
                param_type: FolType::Sequence { element_type },
                ..
            }] if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Input"])
        ),
        "Parameter type should preserve bracketed syntax"
    );
    assert!(
        matches!(
            function_decl.2,
            Some(FolType::Map { key_type, value_type })
                if matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                    && matches!(
                        value_type.as_ref(),
                        FolType::Vector { element_type }
                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                    )
        ),
        "Return type should preserve nested bracketed syntax"
    );
    assert!(
        matches!(
            function_decl.3,
            Some(ref typ) if fol_type_named_text_is(typ, "errs::Failure")
        ),
        "Error type should remain intact alongside bracketed types"
    );
}

#[test]
fn test_routine_declarations_support_any_and_none_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_any_none_types.fol")
        .expect("Should read any/none routine type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse any/non in routine signatures");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                    program_root_nodes(&declarations).into_iter().any(|node| {
                        matches!(
                            node,
                            AstNode::FunDecl { name, params, return_type, .. }
                            if name == "classify"
                                && matches!(
                                    params.as_slice(),
                                    [Parameter { param_type: FolType::Any, .. }]
                                )
                                && matches!(return_type, Some(FolType::Int { size: None, signed: true }))
                        )
                    }),
                    "Routine parameters should lower any to FolType::Any"
                );
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl { name, return_type, .. }
                        if name == "finish" && matches!(return_type, Some(FolType::None))
                    )
                }),
                "Routine return types should lower non to FolType::None"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_statement_supports_of_cases_with_complex_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_of_types.fol")
        .expect("Should read when-of type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when-of cases with qualified and bracketed types");

    match ast {
        AstNode::Program { declarations } => {
            let when_stmt = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::When { cases, default, .. } = node {
                        Some((cases.clone(), default.clone()))
                    } else {
                        None
                    }
                });

            let (cases, default) = when_stmt.expect("Program should include when statement");
            assert!(
                cases.iter().any(|case| {
                    matches!(
                        case,
                        fol_parser::ast::WhenCase::Of {
                            type_match,
                            ..
                        } if fol_type_has_qualified_segments(type_match, &["pkg", "Input"])
                    )
                }),
                "When statement should preserve qualified type in of-case"
            );
            assert!(
                    cases.iter().any(|case| {
                        matches!(
                            case,
                            fol_parser::ast::WhenCase::Of {
                                type_match: FolType::Map { key_type, value_type },
                                ..
                            }
                                if matches!(key_type.as_ref(), FolType::Named { name, .. } if name == "str")
                                    && matches!(
                                        value_type.as_ref(),
                                        FolType::Vector { element_type }
                                        if fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Output"])
                                    )
                        )
                    }),
                    "When statement should preserve nested bracketed type in of-case"
                );
            assert!(
                default.is_some(),
                "When statement should still parse default body"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_statement_supports_is_in_and_has_cases() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_special_cases.fol")
        .expect("Should read when-special-cases test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when is/in/has cases");

    match ast {
        AstNode::Program { declarations } => {
            let when_stmt = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::When { cases, default, .. } = node {
                        Some((cases.clone(), default.clone()))
                    } else {
                        None
                    }
                });

            let (cases, default) = when_stmt.expect("Program should include when statement");
            assert!(
                cases.iter().any(|case| {
                    matches!(
                        case,
                        fol_parser::ast::WhenCase::Is {
                            value: AstNode::Identifier { name, .. },
                            ..
                        } if name == "needle"
                    )
                }),
                "When statement should preserve is(...) case expression"
            );
            assert!(
                cases.iter().any(|case| {
                    matches!(
                        case,
                        fol_parser::ast::WhenCase::In {
                            range: AstNode::Identifier { name, .. },
                            ..
                        } if name == "limit"
                    )
                }),
                "When statement should preserve in(...) case expression"
            );
            assert!(
                cases.iter().any(|case| {
                    matches!(
                        case,
                        fol_parser::ast::WhenCase::Has {
                            member: AstNode::BinaryOp { .. },
                            ..
                        }
                    )
                }),
                "When statement should preserve has(...) case expression"
            );
            assert!(
                default.is_some(),
                "When statement should still parse default body"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_statement_supports_on_cases() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_on_case.fol")
        .expect("Should read when-on-case test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when on(...) cases");

    match ast {
        AstNode::Program { declarations } => {
            let when_stmt = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::When { cases, default, .. } = node {
                        Some((cases.clone(), default.clone()))
                    } else {
                        None
                    }
                });

            let (cases, default) = when_stmt.expect("Program should include when statement");
            assert!(
                cases.iter().any(|case| {
                    matches!(
                        case,
                        fol_parser::ast::WhenCase::On {
                            channel: AstNode::Identifier { name, .. },
                            ..
                        } if name == "stream"
                    )
                }),
                "When statement should preserve on(...) case expression"
            );
            assert!(
                default.is_some(),
                "When statement should still parse default body"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_statement_supports_star_default_case() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_star_default.fol")
        .expect("Should read when-star-default test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when * default case syntax");

    match ast {
        AstNode::Program { declarations } => {
            let when_stmt = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .find_map(|node| {
                    if let AstNode::When { default, .. } = node {
                        default.clone()
                    } else {
                        None
                    }
                });

            let default_body = when_stmt.expect("When statement should include default body");
            assert!(
                default_body
                    .iter()
                    .any(|node| matches!(node, AstNode::Return { .. })),
                "Star default case should parse its block body"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_of_case_missing_bracket_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_of_type_missing_close.fol")
            .expect("Should read malformed when-of type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject when-of type missing closing ']'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed when-of type should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Malformed when-of type parse error should point to the case line"
    );
}

#[test]
fn test_when_star_default_missing_body_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_star_default_missing_body.fol")
            .expect("Should read malformed when-star-default fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject when '*' default without a block body");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '{' after when default '*'"),
        "Malformed when '*' default should report missing body, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        4,
        "Malformed when '*' default parse error should point to the default line"
    );
}

#[test]
fn test_when_on_case_missing_close_paren_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_on_missing_close.fol")
        .expect("Should read malformed when-on fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject when-on case missing closing ')'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ')' after on channel"),
        "Malformed when-on case should report missing close paren, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Malformed when-on case parse error should point to the case line"
    );
}
