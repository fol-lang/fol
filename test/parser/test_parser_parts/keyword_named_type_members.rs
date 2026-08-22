use super::*;

#[test]
fn test_top_level_type_record_fields_support_keyword_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_keyword_fields.fol")
        .expect("Should read keyword-named record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse record definitions with keyword field names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        type_def: TypeDefinition::Record { fields, .. },
                        ..
                    }
                    if matches!(fields.get("get"), Some(FolType::Int { size: None, signed: true }))
                        && matches!(fields.get("log"), Some(FolType::Named { name, .. }) if name == "str")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_duplicate_keyword_named_record_field_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_duplicate_keyword_field.fol")
            .expect("Should read duplicate keyword record field test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate keyword-named record fields");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.contains("Duplicate record field 'get'"),
        "Duplicate keyword field should report the duplicate name, got: {}",
        parse_error.message
    );
}

#[test]
fn test_type_entry_definition_supports_keyword_variant_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_keyword_variants.fol")
            .expect("Should read keyword-named entry variant test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse entry definitions with keyword variant names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    }
                    if matches!(variants.get("get"), Some(Some(FolType::Int { size: None, signed: true })))
                        && matches!(variants.get("log"), Some(Some(FolType::Named { name, .. })) if name == "str")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_duplicate_keyword_named_entry_variant_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_duplicate_keyword_variant.fol")
            .expect("Should read duplicate keyword entry variant test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate keyword-named entry variants");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate entry variant 'get'"),
        "Duplicate keyword variant should report the duplicate name, got: {}",
        parse_error.message
    );
}
