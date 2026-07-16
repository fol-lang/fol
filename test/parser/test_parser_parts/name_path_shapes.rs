use super::*;

#[test]
fn test_parser_name_surfaces_normalize_to_plain_strings() {
    let mut keyword_stream = FileStream::from_file("test/parser/simple_binding_keyword_names.fol")
        .expect("Should read keyword-named binding fixture");
    let mut keyword_lexer = Elements::init(&mut keyword_stream);
    let mut parser = AstParser::new();
    let keyword_ast = parser
        .parse(&mut keyword_lexer)
        .expect("Parser should normalize keyword-like binding names");

    match keyword_ast {
        AstNode::Program { declarations } => {
            let names: Vec<_> = declarations
                .iter()
                .filter_map(|node| {
                    if let AstNode::VarDecl { name, .. } = node {
                        Some(name.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                names,
                vec!["get", "log", "seg"],
                "Keyword-like names should land in the AST as plain unquoted strings"
            );
        }
        _ => panic!("Expected program node"),
    }

    let mut quoted_stream = FileStream::from_file("test/parser/simple_single_quoted_names.fol")
        .expect("Should read single-quoted declaration-surface fixture");
    let mut quoted_lexer = Elements::init(&mut quoted_stream);
    let mut parser = AstParser::new();
    let quoted_ast = parser
        .parse(&mut quoted_lexer)
        .expect("Parser should normalize quoted names");

    match quoted_ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::SegDecl { name, .. } if name == "core")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::UseDecl { name, .. } if name == "warn")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, .. } if name == "state")
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_parser_name_and_path_ast_shapes_stay_distinct_by_surface() {
    let mut value_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_expr.fol")
            .expect("Should read qualified value-path expression fixture");
    let mut value_lexer = Elements::init(&mut value_stream);
    let mut parser = AstParser::new();
    let value_ast = parser
        .parse(&mut value_lexer)
        .expect("Parser should keep qualified value paths in their expression shape");

    match value_ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value) }
                        if matches!(
                            value.as_ref(),
                            AstNode::QualifiedIdentifier { path }
                            if path.segments == vec![
                                "io".to_string(),
                                "console".to_string(),
                                "writer".to_string(),
                            ]
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }

    let mut call_stream =
        FileStream::from_file("test/parser/simple_fun_qualified_path_call_stmt.fol")
            .expect("Should read qualified call-path fixture");
    let mut call_lexer = Elements::init(&mut call_stream);
    let mut parser = AstParser::new();
    let call_ast = parser
        .parse(&mut call_lexer)
        .expect("Parser should keep qualified call paths in their call shape");

    match call_ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::QualifiedFunctionCall { path, .. }
                        if path.segments == vec![
                            "io".to_string(),
                            "console".to_string(),
                            "write_out".to_string(),
                        ]
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }

    let mut type_stream = FileStream::from_file("test/parser/simple_ali_qualified.fol")
        .expect("Should read qualified type-path alias fixture");
    let mut type_lexer = Elements::init(&mut type_stream);
    let mut parser = AstParser::new();
    let type_ast = parser
        .parse(&mut type_lexer)
        .expect("Parser should keep qualified type paths in named type nodes");

    match type_ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target, .. } if name == "ResultAlias"
                        && fol_type_has_qualified_segments(target, &["pkg", "result", "Value"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }

    let mut use_stream = FileStream::from_file("test/parser/simple_use_quoted_name.fol")
        .expect("Should read use-path fixture");
    let mut use_lexer = Elements::init(&mut use_stream);
    let mut parser = AstParser::new();
    let use_ast = parser
        .parse(&mut use_lexer)
        .expect("Parser should preserve structured import path segments on use declarations");

    match use_ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, path_type, .. }
                    if name == "warn"
                        && use_decl_path_text(node).as_deref() == Some("std/warn")
                        && matches!(path_type, FolType::Module { .. })
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
