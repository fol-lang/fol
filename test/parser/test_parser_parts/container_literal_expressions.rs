use super::*;

#[test]
fn test_container_literals_parse_in_assignment_and_return() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_container_literal.fol")
        .expect("Should read container literal test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse container literals");

    match ast {
        AstNode::Program { declarations } => {
            let has_container_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 3
                    )
                )
            });

            let has_container_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 2
                    )
                )
            });

            assert!(has_container_assignment);
            assert!(has_container_return);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_container_literals_parse_in_assignment_and_return() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_container_literal_semicolon.fol")
            .expect("Should read semicolon container literal test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated container literals");

    match ast {
        AstNode::Program { declarations } => {
            let has_container_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 3
                    )
                )
            });

            let has_container_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 2
                    )
                )
            });

            assert!(has_container_assignment);
            assert!(has_container_return);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_container_literals_parse_in_initializers() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_container_literal_semicolon_initializer.fol")
            .expect("Should read semicolon container initializer file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated container literals in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| matches!(
                node,
                AstNode::VarDecl { name, value: Some(value), .. }
                if name == "items"
                    && matches!(value.as_ref(), AstNode::ContainerLiteral { elements, .. } if elements.len() == 3)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_container_literals_parse_in_call_args() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_container_literal_semicolon_call_arg.fol")
            .expect("Should read semicolon container call-arg file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated container literals in call args");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| matches!(
                node,
                AstNode::Return { value: Some(value), .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "emit"
                        && matches!(args.as_slice(), [AstNode::ContainerLiteral { elements, .. }] if elements.len() == 3)
                )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_container_literals_parse_in_assignment_and_return() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_container_literal_trailing_separator.fol")
            .expect("Should read trailing-separator container literal test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing-separator container literals");

    match ast {
        AstNode::Program { declarations } => {
            let has_container_assignment = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Assignment { value, .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 3
                    )
                )
            });

            let has_container_return = only_root_routine_body_nodes(&declarations).into_iter().any(|node| {
                matches!(
                    node,
                    AstNode::Return { value: Some(value), .. }
                    if matches!(
                        value.as_ref(),
                        AstNode::ContainerLiteral { elements, .. } if elements.len() == 2
                    )
                )
            });

            assert!(has_container_assignment);
            assert!(has_container_return);
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_container_literals_parse_in_initializers() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_container_literal_trailing_separator_initializer.fol",
    )
    .expect("Should read trailing-separator container initializer file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing-separator container literals in initializers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| matches!(
                node,
                AstNode::VarDecl { name, value: Some(value), .. }
                if name == "items"
                    && matches!(value.as_ref(), AstNode::ContainerLiteral { elements, .. } if elements.len() == 3)
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_container_literals_parse_in_call_args() {
    let mut file_stream = FileStream::from_file(
        "test/parser/simple_fun_container_literal_trailing_separator_call_arg.fol",
    )
    .expect("Should read trailing-separator container call-arg file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing-separator container literals in call args");

    match ast {
        AstNode::Program { declarations } => {
            assert!(only_root_routine_body_nodes(&declarations).into_iter().any(|node| matches!(
                node,
                AstNode::Return { value: Some(value), .. }
                if matches!(
                    value.as_ref(),
                    AstNode::FunctionCall { name, args, .. }
                    if name == "emit"
                        && matches!(args.as_slice(), [AstNode::ContainerLiteral { elements, .. }] if elements.len() == 3)
                )
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_container_literal_bad_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_container_literal_bad_separator.fol")
            .expect("Should read malformed container literal test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject container literal with missing separator");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ',', ';', or '}' in container expression"),
        "Malformed container literal should report missing separator, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Malformed container literal should report the assignment line"
    );
}
