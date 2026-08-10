use super::*;

#[test]
fn test_simple_rolling_expression_parses_in_return_position() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_expr.fol")
            .expect("Should read rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, condition: None, .. }
                if bindings.len() == 1
                    && bindings[0].name == "x"
                    && matches!(bindings[0].iterable, AstNode::Identifier { ref name, .. } if name == "items")
        ),
        "Return value should parse as rolling expression"
    );
}

#[test]
fn test_parenthesized_multi_binding_rolling_expression_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_multi_binding.fol")
            .expect("Should read multi-binding rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept parenthesized multi-binding rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 2
                    && bindings[0].name == "x"
                    && bindings[1].name == "y"
        ),
        "Return value should keep both rolling binders"
    );
}

#[test]
fn test_parenthesized_semicolon_multi_binding_rolling_expression_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_multi_binding_semicolon.fol")
            .expect("Should read semicolon multi-binding rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept parenthesized semicolon-separated rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 2
                    && bindings[0].name == "x"
                    && bindings[1].name == "y"
        ),
        "Semicolon-separated rolling syntax should keep both binders"
    );
}

#[test]
fn test_bare_multi_binding_rolling_expression_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_bare_multi_binding.fol")
            .expect("Should read bare multi-binding rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept bare multi-binding rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 2
                    && bindings[0].name == "x"
                    && bindings[1].name == "y"
        ),
        "Bare rolling syntax should keep both binders"
    );
}

#[test]
fn test_bare_semicolon_multi_binding_rolling_expression_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_bare_multi_binding_semicolon.fol")
            .expect("Should read bare semicolon multi-binding rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept bare semicolon-separated rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 2
                    && bindings[0].name == "x"
                    && bindings[1].name == "y"
        ),
        "Bare semicolon rolling syntax should keep both binders"
    );
}

#[test]
fn test_rolling_expression_supports_optional_filter() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_filtered.fol")
            .expect("Should read filtered rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept filtered rolling expressions");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling {
                condition: Some(_),
                ..
            }
        ),
        "Filtered rolling expression should keep the trailing condition"
    );
}

#[test]
fn test_rolling_expression_supports_when_filter() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_when_filtered.fol")
            .expect("Should read when-filtered rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept rolling expressions filtered with 'when'");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling {
                condition: Some(_),
                ..
            }
        ),
        "When-filtered rolling expression should keep the trailing condition"
    );
}

#[test]
fn test_rolling_expression_requires_in_keyword() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_missing_in.fol")
            .expect("Should read malformed rolling expression fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject rolling expressions without 'in'");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Expected 'in' in rolling binding"),
        "Malformed rolling expression should report missing 'in', got: {}",
        parse_error.message
    );
}

#[test]
fn test_rolling_expression_rejects_duplicate_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_duplicate_binders.fol")
            .expect("Should read duplicate rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate rolling binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate rolling binding 'x'"),
        "Duplicate rolling binders should report the repeated name, got: {}",
        parse_error.message
    );
    assert!(
        parse_error.primary_location().unwrap().line > 0 && parse_error.primary_location().unwrap().column > 0,
        "Duplicate rolling binders should carry a concrete source location"
    );
}

#[test]
fn test_rolling_expression_rejects_canonical_duplicate_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_duplicate_binders_canonical.fol")
            .expect("Should read canonical duplicate rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate rolling binders");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate rolling binding 'XValue'"),
        "Canonical duplicate rolling binders should report the later spelling, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Canonical duplicate rolling binder parse error should point to the duplicate binding"
    );
}

#[test]
fn test_rolling_expression_supports_silent_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_silent_binder.fol")
            .expect("Should read silent rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept silent rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1 && bindings[0].name == "_"
        ),
        "Silent rolling binder should keep the discard name"
    );
}

#[test]
fn test_rolling_expression_supports_typed_silent_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_typed_silent_binder.fol")
            .expect("Should read typed silent rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept typed silent rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1
                    && bindings[0].name == "_"
                    && matches!(bindings[0].type_hint, Some(FolType::Int { .. }))
        ),
        "Typed silent rolling binder should preserve the type hint"
    );
}

#[test]
fn test_rolling_expression_supports_quoted_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_quoted_binder.fol")
            .expect("Should read quoted rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1 && bindings[0].name == "item"
        ),
        "Quoted rolling binder should normalize to its inner name"
    );
}

#[test]
fn test_rolling_expression_supports_keyword_named_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_keyword_binder.fol")
            .expect("Should read keyword-named rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword-named rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1 && bindings[0].name == "self"
        ),
        "Keyword-named rolling binder should preserve the logical name"
    );
}

#[test]
fn test_rolling_expression_supports_var_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_var_binder.fol")
            .expect("Should read var rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept 'var' rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1 && bindings[0].name == "item"
        ),
        "Var rolling binder should preserve the declared name"
    );
}

#[test]
fn test_rolling_expression_supports_typed_var_binders() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_typed_var_binder.fol")
            .expect("Should read typed var rolling binder fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept typed 'var' rolling binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 1
                    && bindings[0].name == "item"
                    && matches!(bindings[0].type_hint, Some(FolType::Int { .. }))
        ),
        "Typed var rolling binder should preserve the type hint"
    );
}

#[test]
fn test_rolling_expression_supports_var_multi_bindings() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_rolling_var_multi_binding.fol")
            .expect("Should read var multi-binding rolling fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept multi-binding rolling expressions with 'var' binders");

    let return_value = match ast {
        AstNode::Program { declarations } => only_root_routine_body_nodes(&declarations)
            .into_iter()
            .find_map(|node| match node {
                AstNode::Return { value: Some(value), .. } => Some(value.as_ref().clone()),
                _ => None,
            })
            .expect("Program should contain return value"),
        _ => panic!("Expected program node"),
    };

    assert!(
        matches!(
            return_value,
            AstNode::Rolling { bindings, .. }
                if bindings.len() == 2
                    && bindings[0].name == "x"
                    && bindings[1].name == "y"
                    && matches!(bindings[0].type_hint, Some(FolType::Int { .. }))
        ),
        "Var rolling multi-binding syntax should preserve both binders"
    );
}
