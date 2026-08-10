use super::*;

#[test]
fn test_self_and_this_inquiry_targets_lower_structurally() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_inquiry_target_self_this.fol")
            .expect("Should read self/this inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should keep self/this inquiry targets structurally");

    match ast {
        AstNode::Program { declarations } => {
            let Some(AstNode::FunDecl { inquiries, .. }) =
                declarations.iter().find(|node| matches!(node, AstNode::FunDecl { .. }))
            else {
                panic!("Expected a function declaration");
            };

            assert!(matches!(
                inquiries.as_slice(),
                [
                    AstNode::Inquiry {
                        target: InquiryTarget::SelfValue,
                        ..
                    },
                    AstNode::Inquiry {
                        target: InquiryTarget::ThisValue,
                        ..
                    }
                ]
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_and_quoted_inquiry_targets_lower_structurally() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_inquiry_target_named_quoted.fol")
            .expect("Should read named/quoted inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should keep named and quoted inquiry targets structurally");

    match ast {
        AstNode::Program { declarations } => {
            let Some(AstNode::FunDecl { inquiries, .. }) =
                declarations.iter().find(|node| matches!(node, AstNode::FunDecl { .. }))
            else {
                panic!("Expected a function declaration");
            };

            assert!(matches!(
                inquiries.as_slice(),
                [
                    AstNode::Inquiry {
                        target: InquiryTarget::Named(first),
                        ..
                    },
                    AstNode::Inquiry {
                        target: InquiryTarget::Quoted(second),
                        ..
                    }
                ] if first == "cache" && second == "sink"
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_qualified_inquiry_targets_lower_structurally() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_inquiry_target_qualified.fol")
            .expect("Should read qualified inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should keep qualified inquiry targets structurally");

    match ast {
        AstNode::Program { declarations } => {
            let Some(AstNode::FunDecl { inquiries, .. }) =
                declarations.iter().find(|node| matches!(node, AstNode::FunDecl { .. }))
            else {
                panic!("Expected a function declaration");
            };

            assert!(matches!(
                inquiries.as_slice(),
                [
                    AstNode::Inquiry {
                        target: InquiryTarget::Qualified(first),
                        ..
                    },
                    AstNode::Inquiry {
                        target: InquiryTarget::Qualified(second),
                        ..
                    }
                ]
                if first.segments == vec!["pkg".to_string(), "cache".to_string()]
                    && second.segments == vec!["sys".to_string(), "sink".to_string()]
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_and_quoted_duplicate_inquiry_targets_are_rejected() {
    let message = {
        let mut file_stream =
            FileStream::from_file("test/parser/simple_inquiry_target_duplicate_named_quoted.fol")
                .expect("Should read duplicate named/quoted inquiry target fixture");

        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse(&mut lexer)
            .expect_err("Parser should reject duplicate named/quoted inquiry targets");

        errors
            .first()
            
            .expect("First parser error should be ParseError")
            .message.clone()
    };

    assert!(
        message.contains("Duplicate inquiry clause for 'cache'"),
        "Expected duplicate inquiry diagnostic, got: {message}",
    );
}

#[test]
fn test_named_and_quoted_canonical_duplicate_inquiry_targets_are_rejected() {
    let message = {
        let mut file_stream = FileStream::from_file(
            "test/parser/simple_inquiry_target_duplicate_named_quoted_canonical.fol",
        )
        .expect("Should read canonical duplicate named/quoted inquiry target fixture");

        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse(&mut lexer)
            .expect_err("Parser should reject canonical duplicate named/quoted inquiry targets");

        errors
            .first()
            
            .expect("First parser error should be ParseError")
            .message.clone()
    };

    assert!(
        message.contains("Duplicate inquiry clause for 'CacheName'"),
        "Expected canonical duplicate inquiry diagnostic, got: {message}",
    );
}

#[test]
fn test_qualified_duplicate_inquiry_targets_are_rejected() {
    let message = {
        let mut file_stream =
            FileStream::from_file("test/parser/simple_inquiry_target_duplicate_qualified.fol")
                .expect("Should read duplicate qualified inquiry target fixture");

        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse(&mut lexer)
            .expect_err("Parser should reject duplicate qualified inquiry targets");

        errors
            .first()
            
            .expect("First parser error should be ParseError")
            .message.clone()
    };

    assert!(
        message.contains("Duplicate inquiry clause for 'pkg::cache'"),
        "Expected duplicate inquiry diagnostic, got: {message}",
    );
}

#[test]
fn test_qualified_canonical_duplicate_inquiry_targets_are_rejected() {
    let message = {
        let mut file_stream = FileStream::from_file(
            "test/parser/simple_inquiry_target_duplicate_qualified_canonical.fol",
        )
        .expect("Should read canonical duplicate qualified inquiry target fixture");

        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse(&mut lexer)
            .expect_err("Parser should reject canonical duplicate qualified inquiry targets");

        errors
            .first()
            
            .expect("First parser error should be ParseError")
            .message.clone()
    };

    assert!(
        message.contains("Duplicate inquiry clause for 'Pkg::CacheName'"),
        "Expected canonical duplicate inquiry diagnostic, got: {message}",
    );
}

#[test]
fn test_semicolon_separated_inquiry_targets_keep_structural_variants() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_inquiry_target_semicolons.fol")
            .expect("Should read semicolon inquiry target fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should keep semicolon-separated inquiry targets structurally");

    match ast {
        AstNode::Program { declarations } => {
            let Some(AstNode::FunDecl { inquiries, .. }) =
                declarations.iter().find(|node| matches!(node, AstNode::FunDecl { .. }))
            else {
                panic!("Expected a function declaration");
            };

            assert!(matches!(
                inquiries.as_slice(),
                [
                    AstNode::Inquiry {
                        target: InquiryTarget::SelfValue,
                        ..
                    },
                    AstNode::Inquiry {
                        target: InquiryTarget::ThisValue,
                        ..
                    }
                ]
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_routine_inquiries_keep_named_quoted_and_qualified_targets() {
    for (path, expected) in [
        (
            "test/parser/simple_fun_inquiry_named_target.fol",
            InquiryTarget::Named("cache".to_string()),
        ),
        (
            "test/parser/simple_fun_inquiry_quoted_target.fol",
            InquiryTarget::Quoted("cache".to_string()),
        ),
        (
            "test/parser/simple_fun_inquiry_qualified_target.fol",
            InquiryTarget::Qualified(QualifiedPath::new(vec![
                "pkg".to_string(),
                "cache".to_string(),
            ])),
        ),
    ] {
        let mut file_stream = FileStream::from_file(path).expect("Should read inquiry fixture");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let ast = parser
            .parse(&mut lexer)
            .expect("Parser should preserve inquiry target structure");

        match ast {
            AstNode::Program { declarations } => {
                let Some(AstNode::FunDecl { inquiries, .. }) =
                    declarations.iter().find(|node| matches!(node, AstNode::FunDecl { .. }))
                else {
                    panic!("Expected a function declaration");
                };

                let target = match &inquiries[0] {
                    AstNode::Inquiry { target, .. } => target,
                    other => panic!("Expected inquiry node, got {other:?}"),
                };
                let matches_expected = match (&expected, target) {
                    (InquiryTarget::Named(expected_name), InquiryTarget::Named(name))
                    | (InquiryTarget::Quoted(expected_name), InquiryTarget::Quoted(name)) => {
                        name == expected_name
                    }
                    (
                        InquiryTarget::Qualified(expected_path),
                        InquiryTarget::Qualified(path),
                    ) => path.segments == expected_path.segments,
                    _ => false,
                };

                assert!(
                    matches_expected,
                    "Expected first inquiry target {:?} for fixture {}",
                    expected,
                    path
                );
            }
            _ => panic!("Expected program node"),
        }
    }
}

#[test]
fn test_flow_bodied_routines_keep_structural_inquiry_targets() {
    for (path, expected) in [
        (
            "test/parser/simple_fun_flow_body_inquiry.fol",
            InquiryTarget::SelfValue,
        ),
        (
            "test/parser/simple_pro_flow_body_inquiry.fol",
            InquiryTarget::ThisValue,
        ),
        (
            "test/parser/simple_log_flow_body_inquiry.fol",
            InquiryTarget::SelfValue,
        ),
    ] {
        let mut file_stream = FileStream::from_file(path).expect("Should read flow inquiry fixture");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let ast = parser
            .parse(&mut lexer)
            .expect("Parser should preserve flow-body inquiry target structure");

        match ast {
            AstNode::Program { declarations } => {
                let inquiry_target = declarations.iter().find_map(|node| match node {
                    AstNode::FunDecl { inquiries, .. }
                    | AstNode::LogDecl { inquiries, .. }
                    | AstNode::ProDecl { inquiries, .. } => inquiries.first(),
                    _ => None,
                });

                let Some(AstNode::Inquiry { target, .. }) = inquiry_target else {
                    panic!("Expected an inquiry clause");
                };

                assert_eq!(target, &expected, "Unexpected inquiry target for fixture {path}");
            }
            _ => panic!("Expected program node"),
        }
    }
}

#[test]
fn test_alternative_headers_keep_structural_inquiry_targets() {
    for (path, expected) in [
        (
            "test/parser/simple_fun_alt_header_flow_inquiry.fol",
            InquiryTarget::SelfValue,
        ),
        (
            "test/parser/simple_pro_alt_header_flow_inquiry.fol",
            InquiryTarget::ThisValue,
        ),
        (
            "test/parser/simple_log_alt_header_flow_inquiry.fol",
            InquiryTarget::SelfValue,
        ),
    ] {
        let mut file_stream = FileStream::from_file(path).expect("Should read alt-header inquiry fixture");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let ast = parser
            .parse(&mut lexer)
            .expect("Parser should preserve alt-header inquiry target structure");

        match ast {
            AstNode::Program { declarations } => {
                let inquiry_target = declarations.iter().find_map(|node| match node {
                    AstNode::FunDecl { inquiries, .. }
                    | AstNode::LogDecl { inquiries, .. }
                    | AstNode::ProDecl { inquiries, .. } => inquiries.first(),
                    _ => None,
                });

                let Some(AstNode::Inquiry { target, .. }) = inquiry_target else {
                    panic!("Expected an inquiry clause");
                };

                assert_eq!(target, &expected, "Unexpected inquiry target for fixture {path}");
            }
            _ => panic!("Expected program node"),
        }
    }
}

#[test]
fn test_anonymous_and_shorthand_routines_keep_structural_inquiry_targets() {
    for path in [
        "test/parser/simple_fun_anonymous_flow_inquiry_expr.fol",
        "test/parser/simple_fun_shorthand_anonymous_flow_inquiry_expr.fol",
    ] {
        let mut file_stream = FileStream::from_file(path).expect("Should read anonymous inquiry fixture");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let ast = parser
            .parse(&mut lexer)
            .expect("Parser should preserve anonymous inquiry target structure");

        match ast {
            AstNode::Program { declarations } => {
                let inquiry_target = declarations.iter().find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                        AstNode::VarDecl { value: Some(value), .. } => match value.as_ref() {
                            AstNode::AnonymousFun { inquiries, .. }
                            | AstNode::AnonymousLog { inquiries, .. }
                            | AstNode::AnonymousPro { inquiries, .. } => inquiries.first(),
                            _ => None,
                        },
                        _ => None,
                    }),
                    _ => None,
                });

                let Some(AstNode::Inquiry {
                    target: InquiryTarget::SelfValue,
                    ..
                }) = inquiry_target
                else {
                    panic!("Expected anonymous inquiry clause");
                };
            }
            _ => panic!("Expected program node"),
        }
    }
}

#[test]
fn test_pipe_lambdas_keep_structural_inquiry_targets() {
    for path in [
        "test/parser/simple_pipe_lambda_expr_inquiry.fol",
        "test/parser/simple_fun_pipe_lambda_flow_inquiry_expr.fol",
    ] {
        let mut file_stream = FileStream::from_file(path).expect("Should read pipe inquiry fixture");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let ast = parser
            .parse(&mut lexer)
            .expect("Parser should preserve pipe-lambda inquiry target structure");

        match ast {
            AstNode::Program { declarations } => {
                let inquiry_target = declarations.iter().find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => body.iter().find_map(|stmt| match stmt {
                        AstNode::VarDecl { value: Some(value), .. } => match value.as_ref() {
                            AstNode::AnonymousFun { inquiries, .. } => inquiries.first(),
                            _ => None,
                        },
                        AstNode::Return { value: Some(value), .. } => match value.as_ref() {
                            AstNode::AnonymousFun { inquiries, .. } => inquiries.first(),
                            _ => None,
                        },
                        _ => None,
                    }),
                    _ => None,
                });

                let Some(AstNode::Inquiry {
                    target: InquiryTarget::SelfValue,
                    ..
                }) = inquiry_target
                else {
                    panic!("Expected pipe-lambda inquiry clause");
                };
            }
            _ => panic!("Expected program node"),
        }
    }
}
