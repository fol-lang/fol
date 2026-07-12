use super::*;

#[test]
fn test_shared_binding_value_expands_to_multiple_declarations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_shared_value_multi.fol")
        .expect("Should read shared-value multi-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept shared-value multi-bindings");

    match ast {
        AstNode::Program { declarations } => {
            let vars: Vec<_> = declarations
                .iter()
                .filter_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        type_hint,
                        value,
                        options, .. } = node
                    {
                        Some((name, type_hint, value, options))
                    } else {
                        None
                    }
                })
                .collect();

            assert_eq!(
                vars.len(),
                2,
                "Multi-binding should expand into two VarDecl nodes"
            );
            assert!(vars.iter().any(|(name, _, _, _)| *name == "left"));
            assert!(vars.iter().any(|(name, _, _, _)| *name == "right"));
            assert!(vars.iter().all(|(_, type_hint, _, _)| {
                matches!(
                    type_hint,
                    Some(FolType::Int {
                        size: None,
                        signed: true
                    })
                )
            }));
            assert!(vars.iter().all(|(_, _, value, _)| value.is_some()));
            assert!(vars.iter().all(|(_, _, _, options)| {
                options.contains(&fol_parser::ast::VarOption::Mutable)
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_parallel_binding_values_expand_in_order() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_parallel_multi.fol")
        .expect("Should read parallel-value multi-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept parallel-value multi-bindings");

    match ast {
        AstNode::Program { declarations } => {
            let vars: Vec<_> = declarations
                .iter()
                .filter_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        value: Some(value),
                        ..
                    } = node
                    {
                        Some((name.clone(), value.as_ref().clone()))
                    } else {
                        None
                    }
                })
                .collect();

            assert_eq!(
                vars.len(),
                2,
                "Parallel multi-binding should expand into two nodes"
            );
            assert!(matches!(
                vars.as_slice(),
                [(left, AstNode::Literal(Literal::Integer(1))), (right, AstNode::Literal(Literal::Integer(2)))]
                if left == "left" && right == "right"
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_bindings_expand_to_multiple_declarations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_grouped_multi.fol")
        .expect("Should read grouped multi-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped bindings");

    match ast {
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

            assert_eq!(names, vec!["first", "second", "third"]);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_binding_alternatives_preserve_shared_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_plus_var_grouped.fol")
        .expect("Should read grouped alternative binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped binding alternatives");

    match ast {
        AstNode::Program { declarations } => {
            let exported_count = declarations
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        AstNode::VarDecl { options, .. }
                        if options.contains(&fol_parser::ast::VarOption::Export)
                    )
                })
                .count();
            assert_eq!(
                exported_count, 2,
                "Shared grouped options should apply to each item"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_binding_entry_supports_shared_and_parallel_values() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_grouped_entry_multi_values.fol")
            .expect("Should read grouped entry multi-value fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped entries with shared and parallel values");

    match ast {
        AstNode::Program { declarations } => {
            let vars: Vec<_> = declarations
                .iter()
                .filter_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        value: Some(value),
                        ..
                    } = node
                    {
                        Some((name.clone(), value.as_ref().clone()))
                    } else {
                        None
                    }
                })
                .collect();

            assert!(matches!(
                vars.as_slice(),
                [
                    (a, AstNode::Literal(Literal::Integer(1))),
                    (b, AstNode::Literal(Literal::Integer(2))),
                    (c, AstNode::Literal(Literal::Integer(3))),
                    (d, AstNode::Literal(Literal::Integer(3))),
                ] if a == "left"
                    && b == "right"
                    && c == "shared_a"
                    && d == "shared_b"
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_binding_entry_supports_parallel_values_in_function_body() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_var_grouped_entry_multi_values.fol")
            .expect("Should read grouped function-body multi-value fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped multi-values in function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().filter(|stmt| matches!(stmt, AstNode::VarDecl { .. })).count() == 4
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_keyword_named_binding_segments_expand_at_top_level() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_keyword_segment_multi.fol")
        .expect("Should read keyword-named top-level binding segment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword-named binding segments at top level");

    match ast {
        AstNode::Program { declarations } => {
            let names: Vec<_> = declarations
                .iter()
                .filter_map(|node| match node {
                    AstNode::VarDecl { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(names, vec!["left", "get"]);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_keyword_named_binding_segments_expand_in_function_bodies() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_var_keyword_segment_multi.fol")
            .expect("Should read keyword-named function-body binding segment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword-named binding segments in function bodies");

    match ast {
        AstNode::Program { declarations } => {
            let body = declarations
                .iter()
                .find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => Some(body),
                    _ => None,
                })
                .expect("Program should include function body");
            let names: Vec<_> = body
                .iter()
                .filter_map(|node| match node {
                    AstNode::VarDecl { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(names, vec!["left", "get"]);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_mixed_binding_entries_expand_in_order() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_mixed_multi.fol")
        .expect("Should read mixed multi-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept mixed binding entries");

    match ast {
        AstNode::Program { declarations } => {
            let vars: Vec<_> = declarations
                .iter()
                .filter_map(|node| {
                    if let AstNode::VarDecl {
                        name,
                        type_hint,
                        value,
                        ..
                    } = node
                    {
                        Some((
                            name.as_str(),
                            type_hint.clone(),
                            value.as_ref().map(|v| v.as_ref().clone()),
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            assert_eq!(vars.len(), 3);
            assert!(matches!(
                vars[0],
                (
                    "one",
                    Some(FolType::Int {
                        size: None,
                        signed: true
                    }),
                    Some(AstNode::Literal(Literal::Integer(24)))
                )
            ));
            assert!(matches!(
                vars[1],
                ("two", None, Some(AstNode::Literal(Literal::Integer(13))))
            ));
            assert!(matches!(vars[2].0, "three"));
            assert!(matches!(
                vars[2].1.as_ref(),
                Some(FolType::Named { name, .. }) if name == "str"
            ));
            assert!(matches!(
                vars[2].2.as_ref(),
                Some(AstNode::Literal(Literal::String(_)))
            ));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_mixed_binding_entries_work_in_function_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_var_mixed_multi.fol")
        .expect("Should read function mixed multi-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept mixed binding entries in function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().filter(|stmt| matches!(stmt, AstNode::VarDecl { .. })).count() == 3
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
