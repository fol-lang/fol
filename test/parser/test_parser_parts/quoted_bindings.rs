use super::*;

#[test]
fn test_top_level_var_declaration_accepts_quoted_name() {
    let mut file_stream = FileStream::from_file("test/parser/simple_var_quoted_name.fol")
        .expect("Should read quoted-name var declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted top-level binding names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, .. }
                    if name == "state"
                        && matches!(type_hint, Some(FolType::Named { name, .. }) if name == "str")
                        && matches!(value.as_deref(), Some(AstNode::Identifier { name, .. }) if name == "ready")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_function_body_let_declaration_accepts_quoted_name() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_let_quoted_name.fol")
        .expect("Should read quoted-name let declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted function-body binding names");

    match ast {
        AstNode::Program { declarations } => {
            let body = declarations
                .iter()
                .find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => Some(body),
                    _ => None,
                })
                .expect("Program should include function body");
            assert!(body.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, type_hint, value, options, .. }
                    if name == "count"
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                        && matches!(type_hint, Some(FolType::Int { .. }))
                        && matches!(value.as_deref(), Some(AstNode::Literal(Literal::Integer(1))))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_quoted_binding_names_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_grouped_quoted_names.fol")
            .expect("Should read grouped quoted-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped quoted binding names");

    match ast {
        AstNode::Program { declarations } => {
            let names: Vec<_> = declarations
                .iter()
                .filter_map(|node| match node {
                    AstNode::VarDecl { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(names, vec!["left", "right"]);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segmented_quoted_binding_names_parse() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var_segment_quoted_names.fol")
            .expect("Should read segmented quoted-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept segmented quoted binding names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, value, .. }
                    if name == "left"
                        && matches!(value.as_deref(), Some(AstNode::Identifier { name, .. }) if name == "one")
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, value, .. }
                    if name == "right"
                        && matches!(value.as_deref(), Some(AstNode::Identifier { name, .. }) if name == "two")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segmented_quoted_binding_names_parse_in_function_bodies() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_segment_quoted_names.fol")
            .expect("Should read function-body segmented quoted-binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept segmented quoted binding names in function bodies");

    match ast {
        AstNode::Program { declarations } => {
            let body = declarations
                .iter()
                .find_map(|node| match node {
                    AstNode::FunDecl { body, .. } => Some(body),
                    _ => None,
                })
                .expect("Program should include function body");
            assert!(body.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, .. } if name == "left")
            }));
            assert!(body.iter().any(|node| {
                matches!(node, AstNode::VarDecl { name, .. } if name == "right")
            }));
        }
        _ => panic!("Expected program node"),
    }
}
