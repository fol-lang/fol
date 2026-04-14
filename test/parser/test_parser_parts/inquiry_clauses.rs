use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_inquiry_clause_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_function_inquiry_clause_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_clause.fol")
        .expect("Should read function inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse inquiry clauses attached to functions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, inquiries, .. }
                    if name == "sum"
                        && inquiries.len() == 1
                        && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 3 && matches!(&body[0], AstNode::BinaryOp { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_procedure_inquiry_clause_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pro_inquiry_clause.fol")
        .expect("Should read procedure inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse inquiry clauses attached to procedures");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { name, inquiries, .. }
                    if name == "store"
                        && inquiries.len() == 1
                        && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && body.len() == 2 && matches!(&body[0], AstNode::BinaryOp { .. }))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_duplicate_function_inquiry_clause_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_duplicate_inquiry_clause.fol")
            .expect("Should read duplicate function inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate inquiry clauses on functions");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.clone().contains("Duplicate inquiry clause for 'self'"),
        "Expected duplicate inquiry error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_duplicate_procedure_inquiry_clause_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro_duplicate_inquiry_clause.fol")
            .expect("Should read duplicate procedure inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate inquiry clauses on procedures");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.clone().contains("Duplicate inquiry clause for 'self'"),
        "Expected duplicate inquiry error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_duplicate_trailing_function_inquiry_clause_rejected_canonically() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_duplicate_trailing_inquiry_clause_canonical.fol",
    )
    .expect("Should read duplicate trailing function inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate trailing inquiry clauses");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate inquiry clause for 'CacheName'"),
        "Expected canonical duplicate inquiry error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_this_inquiry_clause_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_this.fol")
        .expect("Should read this-target inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse 'where(this)' inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "show"
                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "this") && body.len() == 1)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_distinct_inquiry_targets_can_coexist() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_multi_target.fol")
        .expect("Should read multi-target inquiry clause test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should allow distinct inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if inquiries.len() == 2
                    && matches!(&inquiries[0], AstNode::Inquiry { target, .. } if inquiry_target_is(target, "self"))
                    && matches!(&inquiries[1], AstNode::Inquiry { target, .. } if inquiry_target_is(target, "this"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_comma_separated_targets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_target_list.fol")
        .expect("Should read comma-separated inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse comma-separated inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, inquiries, .. }
                    if name == "show"
                        && inquiries.len() == 2
                        && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "self")))
                        && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "this")))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_semicolon_separated_targets() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_inquiry_target_semicolons.fol")
            .expect("Should read semicolon-separated inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, inquiries, .. }
                    if name == "show"
                        && inquiries.len() == 2
                        && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "self")))
                        && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "this")))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_body.fol")
        .expect("Should read inquiry flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse flow-bodied inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "show"
                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "self") && matches!(body.as_slice(), [AstNode::Identifier { name, .. }] if name == "self"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_duplicate_inquiry_target_in_single_clause_rejected() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_duplicate_inquiry_target_list.fol")
            .expect("Should read duplicate inquiry target-list fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate targets in one inquiry clause");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.clone().contains("Duplicate inquiry clause for 'self'"),
        "Expected duplicate inquiry target error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_named_inquiry_target_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_named_target.fol")
        .expect("Should read named inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse named inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && matches!(&inquiries[0], AstNode::Inquiry { target, body } if inquiry_target_is(target, "cache") && body.len() == 1)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_inquiry_targets_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_quoted_target.fol")
        .expect("Should read quoted inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "cache")))
                    && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "sink")))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_inquiry_targets_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp inquiry fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "fun[] inspect(value: int): int = {\n    return value;\n    where(\"cache 'hot'\"; 'sink \"cold\"') {\n        value;\n    };\n};\n",
    )
    .expect("Should write temp inquiry fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Inquiry fixture path should be UTF-8"),
    )
    .expect("Should read temp inquiry fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in inquiry targets");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                    if name == "inspect"
                        && inquiries.iter().any(|node| matches!(
                            node,
                            AstNode::Inquiry { target, .. }
                                if inquiry_target_is(target, "cache 'hot'")
                        ))
                        && inquiries.iter().any(|node| matches!(
                            node,
                            AstNode::Inquiry { target, .. }
                                if inquiry_target_is(target, "sink \"cold\"")
                        ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_inquiry_targets_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_inquiry_qualified_target.fol")
            .expect("Should read qualified inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse qualified inquiry targets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "pkg::cache")))
                    && inquiries.iter().any(|node| matches!(node, AstNode::Inquiry { target, .. } if inquiry_target_is(target, "sys::sink")))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_statement_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_statement_body.fol")
        .expect("Should read inquiry statement-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse statement forms inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && matches!(
                        &inquiries[0],
                        AstNode::Inquiry { body, .. }
                        if body.iter().any(|node| matches!(node, AstNode::Assignment { .. }))
                            && body.iter().any(|node| matches!(node, AstNode::Block { .. }))
                    )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_declaration_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_decl_body.fol")
        .expect("Should read inquiry declaration-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse declaration forms inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && matches!(
                        &inquiries[0],
                        AstNode::Inquiry { body, .. }
                        if body.iter().any(|node| matches!(node, AstNode::VarDecl { .. }))
                            && body.iter().any(|node| matches!(node, AstNode::LabDecl { .. }))
                    )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_control_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_control_body.fol")
        .expect("Should read inquiry control-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse control forms inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, inquiries, .. }
                if name == "inspect"
                    && matches!(
                        &inquiries[0],
                        AstNode::Inquiry { body, .. }
                        if body.iter().any(|node| matches!(node, AstNode::When { .. }))
                            && body.iter().any(|node| matches!(
                                node,
                                AstNode::Loop { body, .. }
                                if body.iter().any(|stmt| matches!(stmt, AstNode::Break))
                            ))
                            && body.iter().any(|node| matches!(node, AstNode::Yield { .. }))
                            && body.iter().any(|node| matches!(node, AstNode::Return { .. }))
                    )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_builtin_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_builtin_body.fol")
        .expect("Should read inquiry builtin-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin diagnostic forms inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(
                    &inquiries[0],
                    AstNode::Inquiry { body, .. }
                    if body.iter().filter(|node| matches!(node, AstNode::FunctionCall { .. })).count() == 2
                )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_use_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_use_body.fol")
        .expect("Should read inquiry use-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse use declarations inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if body.iter().any(|node| matches!(node, AstNode::UseDecl { .. })))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_type_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_type_body.fol")
        .expect("Should read inquiry type-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type/module declarations inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. }
                    if body.iter().any(|node| matches!(node, AstNode::AliasDecl { .. }))
                        && body.iter().any(|node| matches!(node, AstNode::TypeDecl { .. }))
                        && body.iter().any(|node| matches!(node, AstNode::DefDecl { .. })))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_module_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_module_body.fol")
        .expect("Should read inquiry module-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse seg/std declarations inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. }
                    if body.iter().any(|node| matches!(node, AstNode::SegDecl { .. }))
                        && body.iter().any(|node| matches!(node, AstNode::StdDecl { .. })))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_routine_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_routine_body.fol")
        .expect("Should read inquiry routine-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine declarations inside inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. }
                    if body.iter().filter(|node| matches!(node, AstNode::FunDecl { .. })).count() >= 1
                        && body.iter().any(|node| matches!(node, AstNode::LogDecl { .. }))
                        && body.iter().any(|node| matches!(node, AstNode::ProDecl { .. })))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_assignment_body() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_inquiry_flow_assignment.fol")
            .expect("Should read inquiry flow-assignment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse assignment flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::Assignment { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_declaration_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_decl.fol")
        .expect("Should read inquiry flow-declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse declaration flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::VarDecl { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_builtin_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_builtin.fol")
        .expect("Should read inquiry flow-builtin fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. }
                    if matches!(body.as_slice(), [AstNode::FunctionCall { name, .. }] if name == "report"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_type_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_type.fol")
        .expect("Should read inquiry flow-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type/module declaration flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::AliasDecl { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_module_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_module.fol")
        .expect("Should read inquiry flow-module fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse module declaration flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::SegDecl { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_routine_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_routine.fol")
        .expect("Should read inquiry flow-routine fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine declaration flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::FunDecl { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_control_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_control.fol")
        .expect("Should read inquiry flow-control fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse control flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::Return { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_block_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_block.fol")
        .expect("Should read inquiry flow-block fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::Block { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_if_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_if.fol")
        .expect("Should read inquiry flow-if fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::When { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_when_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_when.fol")
        .expect("Should read inquiry flow-when fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::When { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_clause_accepts_flow_loop_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_inquiry_flow_loop.fol")
        .expect("Should read inquiry flow-loop fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop flow bodies in inquiry clauses");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. } if matches!(body.as_slice(), [AstNode::Loop { .. }]))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_inquiry_body_supports_availability_invoke_statements() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_inquiry_availability_invoke_body.fol")
            .expect("Should read inquiry availability invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept availability invoke statements in inquiry bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                if matches!(&inquiries[0], AstNode::Inquiry { body, .. }
                    if matches!(body.as_slice(), [AstNode::Invoke { callee, args }]
                        if args.len() == 1 && matches!(callee.as_ref(), AstNode::AvailabilityAccess { .. })))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
