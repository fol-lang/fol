use super::*;

fn parse_report_fixture(path: &str) -> AstNode {
    let mut file_stream = FileStream::from_file(path).expect("Should read report syntax fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    parser
        .parse(&mut lexer)
        .expect("Report syntax should remain parser-acceptable without semantic validation")
}

fn body_contains_report_call(body: &[AstNode]) -> bool {
    body.iter().any(|node| match node {
        AstNode::FunctionCall { name, .. } => name == "report",
        AstNode::Return { value, .. } => value
            .as_deref()
            .is_some_and(|inner| matches!(inner, AstNode::FunctionCall { name, .. } if name == "report")),
        _ => false,
    })
}

#[test]
fn test_function_custom_error_report_identifier_stays_syntax_only() {
    let ast = parse_report_fixture("test/parser/simple_fun_error_type_report_identifier_ok.fol");

    match ast {
        AstNode::Program { declarations } => match declarations.first() {
            Some(AstNode::FunDecl { body, .. }) => assert!(
                body_contains_report_call(body),
                "Function fixture should still lower a report call inside the routine body"
            ),
            other => panic!("Expected first declaration to be FunDecl, got {:?}", other),
        },
        other => panic!("Expected Program AST, got {:?}", other),
    }
}

#[test]
fn test_procedure_custom_error_report_expression_stays_syntax_only() {
    let ast = parse_report_fixture("test/parser/simple_pro_error_type_report_expression_ok.fol");

    match ast {
        AstNode::Program { declarations } => match declarations.first() {
            Some(AstNode::ProDecl { body, .. }) => assert!(
                body_contains_report_call(body),
                "Procedure fixture should still lower a report call inside the routine body"
            ),
            other => panic!("Expected first declaration to be ProDecl, got {:?}", other),
        },
        other => panic!("Expected Program AST, got {:?}", other),
    }
}

#[test]
fn test_quoted_report_call_target_parses_without_forward_resolution() {
    parse_report_fixture("test/parser/simple_fun_error_type_report_forward_quoted_call_result_ok.fol");
}

#[test]
fn test_quoted_report_method_target_parses_without_forward_resolution() {
    parse_report_fixture(
        "test/parser/simple_pro_error_type_report_forward_quoted_method_call_result_ok.fol",
    );
}
