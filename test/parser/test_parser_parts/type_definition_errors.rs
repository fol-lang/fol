use super::*;

#[test]
fn test_type_entry_missing_variant_name_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_missing_variant_name.fol")
            .expect("Should read malformed type entry test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when an entry variant name is missing");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected entry variant name"),
        "Malformed type entry should report missing variant name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        2,
        "Type entry parse error should point to the malformed variant line"
    );
}

#[test]
fn test_duplicate_entry_variant_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_duplicate_variant.fol")
            .expect("Should read duplicate type entry test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate entry variants");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate entry variant 'BLUE'"),
        "Duplicate entry variant should report the variant name, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Type entry duplicate parse error should point to the duplicate variant line"
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Type entry duplicate parse error should point to the duplicate variant name"
    );
}

#[test]
fn test_canonical_duplicate_entry_variant_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_duplicate_variant_canonical.fol")
            .expect("Should read canonical duplicate type entry test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate entry variants");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Duplicate entry variant 'BlueValue'"),
        "Canonical duplicate entry variant should report the later spelling, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Canonical duplicate entry parse error should point to the duplicate variant line"
    );
}

#[test]
fn test_type_entry_marker_unknown_option_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_marker_unknown_option.fol")
            .expect("Should read malformed entry marker option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when ent marker uses an unknown option");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown entry type marker option"),
        "Malformed ent marker option should report unknown option, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Entry marker option parse error should point to the declaration line"
    );
}

#[test]
fn test_type_entry_definition_supports_lab_variants() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_lab_variants.fol")
        .expect("Should read lab entry variant test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse entry definitions with lab variants");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        type_def: TypeDefinition::Entry { variants, .. },
                        ..
                    }
                    if matches!(variants.get("None"), Some(Some(FolType::None)))
                        && matches!(variants.get("Some"), Some(Some(FolType::Named { name, .. })) if name == "str")
                        && matches!(variants.get("Many"), Some(Some(FolType::Int { size: None, signed: true })))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_type_entry_definition_reports_missing_lab_variant_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_missing_variant_label.fol")
            .expect("Should read malformed lab entry variant test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject lab variants without names");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected entry variant name"),
        "Malformed lab variant should report the missing name, got: {}",
        first_message
    );
}

#[test]
fn test_type_record_marker_missing_assign_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_marker_missing_assign.fol")
            .expect("Should read malformed type record marker test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when rec marker is not followed by '='");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '=' after record type marker"),
        "Malformed rec marker should report missing '=', got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Record marker parse error should point to the declaration line"
    );
}

#[test]
fn test_type_record_marker_unknown_option_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_marker_unknown_option.fol")
            .expect("Should read malformed record marker option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when rec marker uses an unknown option");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Unknown record type marker option"),
        "Malformed rec marker option should report unknown option, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Record marker option parse error should point to the declaration line"
    );
}

#[test]
fn test_type_generic_header_missing_separator_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_generics_missing_separator.fol")
            .expect("Should read malformed type generics test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when type generic items are missing a separator");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected ',', ';', or ')' after generic parameter"),
        "Malformed type generic header should report missing separator, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Type generic header parse error should point to the declaration line"
    );
}

#[test]
fn test_type_record_missing_close_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_missing_close.fol")
        .expect("Should read malformed type record test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should fail when type record is missing closing '}'");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '}' to close type record definition"),
        "Malformed type record should report missing close brace, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        3,
        "Type record parse error should point to the end of the declaration"
    );
}

#[test]
fn test_duplicate_record_method_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_duplicate_method.fol")
            .expect("Should read duplicate record method test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate record methods");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .clone()
            .contains("Duplicate type member 'getBrand#0'"),
        "Duplicate record method should report the member key, got: {}",
        parse_error.message
    );
}

#[test]
fn test_record_field_and_alias_name_conflict_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_field_alias_conflict.fol")
            .expect("Should read record field/alias conflict test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject record field and alias name conflicts");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .clone()
            .contains("Duplicate type member 'host'"),
        "Record field/alias conflict should report duplicate type member, got: {}",
        parse_error.message
    );
}

#[test]
fn test_record_field_and_alias_canonical_name_conflict_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_field_alias_conflict_canonical.fol")
            .expect("Should read canonical record field/alias conflict test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical record field and alias name conflicts");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate type member 'HostName'"),
        "Canonical field/alias conflict should report the later spelling, got: {}",
        parse_error.message
    );
}

#[test]
fn test_entry_variant_and_type_name_conflict_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_variant_type_conflict.fol")
            .expect("Should read entry variant/type conflict test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject entry variant and nested type name conflicts");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .clone()
            .contains("Duplicate type member 'Ok'"),
        "Entry variant/type conflict should report duplicate type member, got: {}",
        parse_error.message
    );
}

#[test]
fn test_entry_variant_and_type_canonical_name_conflict_reports_parse_error() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_variant_type_conflict_canonical.fol")
            .expect("Should read canonical entry variant/type conflict test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical entry variant and nested type name conflicts");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate type member 'OkValue'"),
        "Canonical entry/type conflict should report the later spelling, got: {}",
        parse_error.message
    );
}
