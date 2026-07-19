use super::*;

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_receiver_diag_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_procedure_method_receiver_syntax_rejects_missing_method_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_missing_name.fol")
            .expect("Should read procedure missing receiver method-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject procedure receiver syntax missing method name");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected procedure name after 'pro'"),
        "Missing procedure method name should report expected-name parse error, got: {}",
        first_message
    );
}

#[test]
fn test_function_method_receiver_syntax_rejects_missing_method_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_receiver_missing_name.fol")
            .expect("Should read function missing receiver method-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject function receiver syntax missing method name");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected function name after 'fun'"),
        "Missing function method name should report expected-name parse error, got: {}",
        first_message
    );
}

#[test]
fn test_procedure_method_receiver_syntax_rejects_missing_receiver_close_paren() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_missing_close_paren.fol")
            .expect("Should read procedure missing receiver close paren fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject procedure receiver syntax missing ')' token");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
            first_message.contains("Expected ')' after method receiver type"),
            "Missing procedure receiver close paren should report explicit receiver syntax error, got: {}",
            first_message
        );
}

#[test]
fn test_procedure_method_receiver_syntax_rejects_missing_receiver_type() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_missing_type.fol")
            .expect("Should read procedure missing receiver type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject procedure receiver syntax missing receiver type");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected type reference"),
        "Missing procedure receiver type should report type-reference parsing error, got: {}",
        first_message
    );
}

#[test]
fn test_function_method_receiver_supports_qualified_type_references() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_receiver_qualified.fol")
            .expect("Should read qualified function receiver fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept qualified method receiver type");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl {
                            name,
                            receiver_type: Some(receiver),
                            ..
                        } if name == "parse_msg"
                            && fol_type_has_qualified_segments(receiver, &["pkg", "Parser"])
                    )
                }),
                "Qualified receiver function should retain its receiver type in the AST"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_logical_method_receiver_supports_qualified_type_references() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_log_method_receiver_qualified.fol")
            .expect("Should read qualified logical receiver fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept logical method receiver type");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::LogDecl {
                            name,
                            receiver_type: Some(receiver),
                            ..
                        } if name == "ready"
                            && fol_type_has_qualified_segments(receiver, &["pkg", "Parser"])
                    )
                }),
                "Qualified receiver logical method should retain its receiver type in the AST"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_procedure_method_receiver_supports_bracketed_type_references() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_bracketed.fol")
            .expect("Should read bracketed procedure receiver fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept bracketed method receiver type");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                program_root_nodes(&declarations).into_iter().any(|node| {
                    matches!(
                        node,
                        AstNode::ProDecl {
                            name,
                            receiver_type: Some(FolType::Vector { element_type }),
                            ..
                        }
                            if name == "store"
                                && fol_type_has_qualified_segments(element_type.as_ref(), &["pkg", "Item"])
                    )
                }),
                "Bracketed receiver procedure should retain its receiver type in the AST"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_function_method_receiver_missing_bracket_close_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_receiver_bracket_missing_close.fol")
            .expect("Should read malformed bracketed function receiver fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject receiver type missing closing ']'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed bracketed receiver type should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Malformed receiver type parse error should point to the signature line"
    );
}

#[test]
fn test_invalid_method_receiver_type_reports_receiver_token_span() {
    let temp_root = unique_temp_root("invalid_receiver_span");
    std::fs::create_dir_all(&temp_root).expect("Should create temp receiver diagnostic dir");
    let fixture = temp_root.join("invalid_receiver_span.fol");
    std::fs::write(&fixture, "fun (any) parse_msg(): int = { return 1; };\n")
        .expect("Should write invalid receiver diagnostic fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Receiver diagnostic fixture path should be UTF-8"),
    )
    .expect("Should read invalid receiver diagnostic fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject `any` as a method receiver type");

    std::fs::remove_dir_all(&temp_root).ok();

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Method receiver type cannot be any, non, or none"),
        "Invalid method receiver type should report the dedicated receiver diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Invalid receiver diagnostic should stay on the method signature line"
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        6,
        "Invalid receiver diagnostic should anchor to the receiver token"
    );
    assert_eq!(
        parse_error.primary_location().unwrap().length.unwrap_or(0),
        3,
        "Invalid receiver diagnostic span should cover the rejected receiver token"
    );
}

#[test]
fn test_none_like_method_receiver_type_reports_dedicated_receiver_diagnostic() {
    let temp_root = unique_temp_root("invalid_none_receiver_span");
    std::fs::create_dir_all(&temp_root).expect("Should create temp receiver diagnostic dir");
    let fixture = temp_root.join("invalid_none_receiver_span.fol");
    std::fs::write(&fixture, "fun (non) parse_msg(): int = { return 1; };\n")
        .expect("Should write invalid none-like receiver diagnostic fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Receiver diagnostic fixture path should be UTF-8"),
    )
    .expect("Should read invalid none-like receiver diagnostic fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject `non` as a method receiver type");

    std::fs::remove_dir_all(&temp_root).ok();

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Method receiver type cannot be any, non, or none"),
        "None-like method receiver types should share the dedicated receiver diagnostic, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        6,
        "Invalid none-like receiver diagnostic should anchor to the receiver token"
    );
    assert_eq!(
        parse_error.primary_location().unwrap().length.unwrap_or(0),
        3,
        "Invalid none-like receiver diagnostic span should cover the rejected receiver token"
    );
}

#[test]
fn test_function_method_receiver_syntax_accepts_builtin_receiver_type() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_receiver_builtin_type.fol")
            .expect("Should read function builtin receiver type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept function receiver syntax with builtin receiver type");

    match ast {
        AstNode::Program { declarations } => assert!(
            program_root_nodes(&declarations)
                .into_iter()
                .any(|node| matches!(
                    node,
                    AstNode::FunDecl {
                        name,
                        receiver_type: Some(FolType::Int { .. }),
                        ..
                    } if name == "parse_msg"
                )),
            "Builtin scalar receiver method should retain its receiver type in the AST"
        ),
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_procedure_method_receiver_syntax_accepts_builtin_receiver_type() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_builtin_type.fol")
            .expect("Should read procedure builtin receiver type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept procedure receiver syntax with builtin receiver type");

    match ast {
        AstNode::Program { declarations } => assert!(
            program_root_nodes(&declarations)
                .into_iter()
                .any(|node| matches!(
                    node,
                    AstNode::ProDecl {
                        name,
                        receiver_type: Some(FolType::Int { .. }),
                        ..
                    } if name == "parse_msg"
                )),
            "Builtin scalar receiver method should retain its receiver type in the AST"
        ),
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_function_method_receiver_syntax_rejects_builtin_keyword_receiver_type() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_method_receiver_builtin_keyword.fol")
            .expect("Should read function builtin-keyword receiver type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should reject function receiver syntax with builtin keyword receiver type",
    );

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected type reference"),
        "Builtin keyword receiver type should report a type-reference diagnostic, got: {}",
        first_message
    );
}

#[test]
fn test_procedure_method_receiver_syntax_rejects_builtin_keyword_receiver_type() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_method_receiver_builtin_keyword.fol")
            .expect("Should read procedure builtin-keyword receiver type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parse_script_as_program(&mut parser, &mut lexer).expect_err(
        "Parser should reject procedure receiver syntax with builtin keyword receiver type",
    );

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
            first_message.contains("Expected type reference"),
            "Procedure builtin keyword receiver type should report a type-reference diagnostic, got: {}",
            first_message
        );
}

#[test]
fn test_function_custom_error_type_accepts_compatible_report_local_var() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_report_local_var_ok.fol")
            .expect("Should read compatible custom-error report local var file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parser
        .parse(&mut lexer)
        .expect("Parser should accept report local var compatible with custom error type");
}

#[test]
fn test_function_custom_error_type_allows_incompatible_report_local_var_as_syntax() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_report_local_var_mismatch.fol")
            .expect("Should read incompatible custom-error report local var file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should keep incompatible report local vars as syntax-only forms after semantic validation removal",
    );
}

#[test]
fn test_function_custom_error_type_accepts_compatible_report_inferred_local_var() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_report_inferred_local_ok.fol")
            .expect("Should read compatible inferred local var report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parser
        .parse(&mut lexer)
        .expect("Parser should accept report inferred local var compatible with custom error type");
}

#[test]
fn test_function_custom_error_type_allows_incompatible_report_inferred_local_var_as_syntax() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_inferred_local_mismatch.fol",
    )
    .expect("Should read incompatible inferred local var report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should keep incompatible inferred report locals as syntax-only forms after semantic validation removal",
    );
}

#[test]
fn test_function_custom_error_type_accepts_nested_inferred_local_report() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_nested_inferred_local_ok.fol",
    )
    .expect("Should read nested inferred-local compatible report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should accept nested inferred-local report compatible with custom error type",
    );
}

#[test]
fn test_function_custom_error_type_allows_nested_inferred_local_report_mismatch_as_syntax() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_nested_inferred_local_mismatch.fol",
    )
    .expect("Should read nested inferred-local mismatch report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should keep nested inferred-local report mismatches as syntax-only forms after semantic validation removal",
    );
}

#[test]
fn test_function_custom_error_type_accepts_loop_inferred_local_report() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_loop_inferred_local_ok.fol",
    )
    .expect("Should read loop inferred-local compatible report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should accept loop inferred-local report compatible with custom error type",
    );
}

#[test]
fn test_function_custom_error_type_allows_loop_inferred_local_report_mismatch_as_syntax() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_loop_inferred_local_mismatch.fol",
    )
    .expect("Should read loop inferred-local mismatch report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should keep loop inferred-local report mismatches as syntax-only forms after semantic validation removal",
    );
}

#[test]
fn test_function_custom_error_type_accepts_nested_shadowed_report_identifier() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_report_nested_shadow_ok.fol")
            .expect("Should read nested shadow compatible report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should accept nested shadowed identifier compatible with custom error type",
    );
}

#[test]
fn test_function_custom_error_type_allows_nested_shadowed_report_identifier_mismatch_as_syntax() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_error_type_report_nested_shadow_mismatch.fol",
    )
    .expect("Should read nested shadow mismatch report file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parse_script_as_program(&mut parser, &mut lexer).expect(
        "Parser should keep nested shadowed report mismatches as syntax-only forms after semantic validation removal",
    );
}

#[test]
fn test_when_statement_parsing_with_case_and_default() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when.fol")
        .expect("Should read when test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when statement");

    let when_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include a when statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(when_stmt.0, AstNode::Identifier { name, .. } if name == "a"),
        "When expression should parse identifier a"
    );
    assert_eq!(when_stmt.1.len(), 1, "When should include one case");
    assert!(when_stmt.2.is_some(), "When should include default body");
}

#[test]
fn test_when_statement_parsing_with_multiple_cases() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_multi_case.fol")
        .expect("Should read multi-case when test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when statement with multiple cases");

    let when_stmt = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include a when statement"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(when_stmt.0, AstNode::Identifier { name, .. } if name == "a"),
        "When expression should parse identifier a"
    );
    assert_eq!(
        when_stmt.1.len(),
        2,
        "When should include two case branches"
    );
    assert!(when_stmt.2.is_some(), "When should include default body");
}

#[test]
fn test_when_case_body_supports_nested_if_and_loop() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_nested_control.fol")
        .expect("Should read nested-control when test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested control flow inside when case body");

    let when_cases = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When { cases, .. } = node {
                    Some(cases.clone())
                } else {
                    None
                }
            })
            .expect("Program should include a when statement"),
        _ => panic!("Expected program node"),
    };

    let first_case_body = when_cases
        .iter()
        .find_map(|case| {
            if let fol_parser::ast::WhenCase::Case { body, .. } = case {
                Some(body.clone())
            } else {
                None
            }
        })
        .expect("When should include at least one case body");

    assert!(
        first_case_body
            .iter()
            .any(|node| matches!(node, AstNode::When { .. })),
        "Case body should include lowered if statement"
    );

    let lowered_if = first_case_body
        .iter()
        .find_map(|node| {
            if let AstNode::When { default, .. } = node {
                Some(default.clone())
            } else {
                None
            }
        })
        .expect("Case body should include lowered if node");

    let default_body = lowered_if.expect("Lowered if should include else/default body");
    assert!(
        default_body
            .iter()
            .any(|node| matches!(node, AstNode::Loop { .. })),
        "Case body should include loop statement from else branch"
    );
}

#[test]
fn test_if_statement_lowers_to_when_shape() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_if.fol").expect("Should read if test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if statement");

    let lowered_if = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include lowered if/when node"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            lowered_if.0,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Eq,
                ..
            }
        ),
        "If condition should parse equality expression"
    );
    assert_eq!(lowered_if.1.len(), 1, "Lowered if should include one case");
    assert!(
        lowered_if.2.is_some(),
        "Lowered if should include default branch body"
    );
}

#[test]
fn test_if_chain_lowers_to_nested_when_default() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_chain.fol")
        .expect("Should read if-chain test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse chained if statements");

    let lowered_if = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include lowered if/when node"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            lowered_if.0,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Eq,
                ..
            }
        ),
        "Outer if condition should parse equality expression"
    );
    let default = lowered_if
        .2
        .expect("Outer if should include default chain/default block");
    assert!(
        default
            .iter()
            .any(|node| matches!(node, AstNode::When { .. })),
        "Outer if default should contain nested lowered if"
    );
}

#[test]
fn test_if_statement_without_else_has_no_default_branch() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_no_else.fol")
        .expect("Should read if-no-else test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if statement without else");

    let lowered_if = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include lowered if/when node"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            lowered_if.0,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Lt,
                ..
            }
        ),
        "If condition should parse less-than expression"
    );
    assert_eq!(lowered_if.1.len(), 1, "If should include one case");
    // The single case matches on the literal `true` (subject == true), not on
    // a re-evaluation of the condition, and a missing else desugars to an
    // empty default branch so statement-position `if` needs no `else`.
    assert!(
        matches!(
            &lowered_if.1[0],
            fol_parser::ast::WhenCase::Case {
                condition: AstNode::Literal(fol_parser::ast::Literal::Boolean(true)),
                ..
            }
        ),
        "If case should match on the literal true"
    );
    assert!(
        lowered_if
            .2
            .as_ref()
            .is_some_and(|default| default.is_empty()),
        "If without else should desugar to an empty default branch"
    );
}

#[test]
fn test_else_if_keyword_chain_lowers_to_nested_when_default() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_else_if.fol")
        .expect("Should read else-if test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse else-if keyword chain");

    let lowered_if = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include lowered if/when node"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            lowered_if.0,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Eq,
                ..
            }
        ),
        "Outer if condition should parse equality expression"
    );
    let default = lowered_if
        .2
        .expect("Else-if chain should include default branch body");
    assert!(
        default
            .iter()
            .any(|node| matches!(node, AstNode::When { .. })),
        "Else-if should lower to nested when in default branch"
    );
}

#[test]
fn test_else_keyword_block_maps_to_direct_default_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_else_only.fol")
        .expect("Should read else-only test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if-else keyword block");

    let lowered_if = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| {
                if let AstNode::When {
                    expr,
                    cases,
                    default,
                } = node
                {
                    Some((expr.as_ref().clone(), cases.clone(), default.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include lowered if/when node"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            lowered_if.0,
            AstNode::BinaryOp {
                op: fol_parser::ast::BinaryOperator::Lt,
                ..
            }
        ),
        "If condition should parse less-than expression"
    );
    let default = lowered_if
        .2
        .expect("Else block should produce default body");
    assert!(
        default
            .iter()
            .all(|node| !matches!(node, AstNode::When { .. })),
        "Else-only block should not introduce nested when nodes"
    );
    assert!(
        default
            .iter()
            .any(|node| matches!(node, AstNode::Return { .. })),
        "Else-only default body should include return statement"
    );
}
