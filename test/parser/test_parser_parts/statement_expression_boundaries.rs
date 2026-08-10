use super::*;

fn parse_program_declarations(path: &str) -> Vec<AstNode> {
    let mut file_stream = FileStream::from_file(path).expect("Should read parser boundary fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    match parser
        .parse(&mut lexer)
        .expect("Parser should accept boundary fixture")
    {
        AstNode::Program { declarations } => declarations,
        other => panic!("Expected program node, got {other:?}"),
    }
}

#[test]
fn test_top_level_grouped_callee_lowers_as_invoke_not_function_call() {
    let declarations = parse_program_declarations("test/parser/simple_invoke_top_level.fol");

    assert_eq!(
        declarations.len(),
        1,
        "Top-level grouped callee should lower to a single root node"
    );

    assert!(matches!(
        declarations.first(),
        Some(AstNode::Invoke { callee, args })
            if args.len() == 1
                && matches!(callee.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "factory" && args.is_empty())
                && matches!(&args[0], AstNode::Identifier { name, .. } if name == "value")
    ));
}

#[test]
fn test_routine_body_keeps_call_invoke_and_assignment_boundaries() {
    let declarations =
        parse_program_declarations("test/parser/simple_fun_call_invoke_assignment_boundaries.fol");

    let body = declarations
        .iter()
        .find_map(|node| {
            if let AstNode::FunDecl { name, body, .. } = node {
                (name == "shape").then(|| body.clone())
            } else {
                None
            }
        })
        .expect("Program should contain the shape routine declaration");

    assert!(matches!(
        body.as_slice(),
        [
            AstNode::FunctionCall { name, args, .. },
            AstNode::Invoke { callee, args: invoke_args },
            AstNode::Assignment { target, value }
        ]
            if name == "run"
                && args.len() == 1
                && matches!(&args[0], AstNode::Identifier { name, .. } if name == "value")
                && invoke_args.len() == 1
                && matches!(callee.as_ref(), AstNode::FunctionCall { name, args, .. } if name == "factory" && args.is_empty())
                && matches!(&invoke_args[0], AstNode::Identifier { name, .. } if name == "value")
                && matches!(target.as_ref(), AstNode::Identifier { name, .. } if name == "target")
                && matches!(value.as_ref(), AstNode::Identifier { name, .. } if name == "value")
    ));
}

#[test]
fn test_top_level_when_stays_a_root_statement_with_nested_bodies() {
    let declarations =
        parse_program_declarations("test/parser/simple_when_top_level_statement.fol");

    assert!(matches!(
        declarations.as_slice(),
        [AstNode::When { cases, default, .. }]
            if matches!(
                cases.as_slice(),
                [fol_parser::ast::WhenCase::Case { body, .. }]
                    if matches!(body.as_slice(), [AstNode::FunctionCall { name, args, .. }] if name == "run" && args.is_empty())
            )
                && matches!(
                    default,
                    Some(default_body)
                        if matches!(default_body.as_slice(), [AstNode::FunctionCall { name, args, .. }] if name == "stop" && args.is_empty())
                )
    ));
}

#[test]
fn test_when_matching_forms_stay_nested_in_expression_positions() {
    let initializer_declarations =
        parse_program_declarations("test/parser/simple_if_matching_expr.fol");

    assert!(matches!(
        initializer_declarations.as_slice(),
        [AstNode::VarDecl { name, value: Some(value), .. }]
            if name == "checker" && matches!(value.as_ref(), AstNode::When { .. })
    ));
    assert!(
        !initializer_declarations
            .iter()
            .any(|node| matches!(node, AstNode::When { .. })),
        "Matching expressions in initializers should stay under their declaration value"
    );

    let routine_declarations =
        parse_program_declarations("test/parser/simple_when_matching_expr.fol");
    let body = routine_declarations
        .iter()
        .find_map(|node| {
            if let AstNode::FunDecl { name, body, .. } = node {
                (name == "someValue").then(|| body.clone())
            } else {
                None
            }
        })
        .expect("Program should contain the someValue routine declaration");

    assert!(matches!(
        body.as_slice(),
        [AstNode::Return { value: Some(value), .. }]
            if matches!(value.as_ref(), AstNode::When { .. })
    ));
}
