use super::*;

#[test]
fn test_when_has_case_missing_close_paren_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_has_missing_close.fol")
            .expect("Should read malformed when-has fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject when-has case missing closing ')'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ')' after has member"),
        "Malformed when-has case should report missing close paren, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Malformed when-has case parse error should point to the case line"
    );
}

#[test]
fn test_function_declaration_missing_bracket_close_in_parameter_type_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_bracket_types_missing_close.fol")
            .expect("Should read malformed bracketed function type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when function parameter type is missing closing ']'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected closing ']' in type reference"),
        "Malformed bracketed function type should report missing close bracket, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Bracketed function type parse error should point to the signature line"
    );
}

#[test]
fn test_procedure_declaration_header_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_pro.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse procedure declaration");

    let procedure_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::ProDecl {
                    name,
                    params,
                    return_type,
                    body,
                    ..
                } = node
                {
                    Some((
                        name.clone(),
                        params.len(),
                        return_type.is_some(),
                        body.len(),
                    ))
                } else {
                    None
                }
            })
            .expect("Program should include procedure declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(procedure_decl.0, "update");
    assert_eq!(procedure_decl.1, 2, "Procedure should have two parameters");
    assert!(procedure_decl.2, "Procedure should have return type");
    assert!(
        procedure_decl.3 > 0,
        "Procedure body should include parsed statements"
    );
}

#[test]
fn test_function_declaration_error_type_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_error_type.fol")
        .expect("Should read function error type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse function error type signature");

    let function_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::FunDecl {
                    name,
                    return_type,
                    error_type,
                    ..
                } = node
                {
                    Some((name.clone(), return_type.clone(), error_type.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include function declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(function_decl.0, "read_data");
    assert!(
        matches!(function_decl.1, Some(FolType::Named { name, .. }) if name == "str"),
        "Function should parse return type in first ':' slot"
    );
    assert!(
        matches!(function_decl.2, Some(FolType::Named { name, .. }) if name == "io_err"),
        "Function should parse error type after '/'"
    );
}

#[test]
fn test_procedure_declaration_error_type_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_pro_error_type.fol")
        .expect("Should read procedure error type test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse procedure error type signature");

    let procedure_decl = match ast {
        AstNode::Program { declarations } => declarations
            .iter()
            .find_map(|node| {
                if let AstNode::ProDecl {
                    name,
                    return_type,
                    error_type,
                    ..
                } = node
                {
                    Some((name.clone(), return_type.clone(), error_type.clone()))
                } else {
                    None
                }
            })
            .expect("Program should include procedure declaration"),
        _ => panic!("Expected program node"),
    };

    assert_eq!(procedure_decl.0, "write_data");
    assert!(
        matches!(
            procedure_decl.1,
            Some(FolType::Int {
                size: None,
                signed: true
            })
        ),
        "Procedure should parse return type in first ':' slot"
    );
    assert!(
        matches!(procedure_decl.2, Some(FolType::Named { name, .. }) if name == "io_err"),
        "Procedure should parse error type after '/'"
    );
}

#[test]
fn test_function_declaration_missing_error_type_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_missing_type.fol")
            .expect("Should read malformed function error type file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when '/' has no error type");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected type reference"),
        "Missing error type after '/' should report type reference error, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Missing error type parse error should point to signature line"
    );
}

#[test]
fn test_function_declaration_legacy_error_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_error_type_legacy_separator.fol")
            .expect("Should read legacy error separator function file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject ':' as the routine error separator");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '/' before routine error type"),
        "Legacy separator should report the new '/' guidance, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Legacy separator parse error should point to the signature line"
    );
}
