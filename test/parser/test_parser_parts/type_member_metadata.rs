use super::*;
use fol_parser::ast::{EntryVariantMeta, RecordFieldMeta, VarOption};

#[test]
fn test_record_type_retains_field_defaults_and_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_metadata.fol")
        .expect("Should read record metadata fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain record field metadata");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { field_meta, .. },
                    ..
                }
                if name == "Widget"
                    && matches!(field_meta.get("size"), Some(RecordFieldMeta { default: Some(_), options }) if options.contains(&VarOption::Mutable))
                    && matches!(field_meta.get("name"), Some(RecordFieldMeta { default: None, options }) if options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_type_retains_variant_defaults_and_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_metadata.fol")
        .expect("Should read entry metadata fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain entry variant metadata");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { variant_meta, .. },
                    ..
                }
                if name == "Maybe"
                    && matches!(variant_meta.get("None"), Some(EntryVariantMeta { default: None, options }) if options.contains(&VarOption::Immutable))
                    && matches!(variant_meta.get("Some"), Some(EntryVariantMeta { default: Some(_), options }) if options.contains(&VarOption::Mutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_const_field_metadata_is_retained() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_const_fields.fol")
        .expect("Should read const record field fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain const record field metadata");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { field_meta, .. },
                    ..
                }
                if name == "Config"
                    && matches!(field_meta.get("host"), Some(RecordFieldMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
                    && matches!(field_meta.get("port"), Some(RecordFieldMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_const_variant_metadata_is_retained() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_const_variants.fol")
        .expect("Should read const entry variant fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain const entry variant metadata");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { variant_meta, .. },
                    ..
                }
                if name == "Result"
                    && matches!(variant_meta.get("Ok"), Some(EntryVariantMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
                    && matches!(variant_meta.get("Err"), Some(EntryVariantMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_field_binding_options_are_retained() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_field_options.fol")
        .expect("Should read record field option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain record field binding options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { field_meta, .. },
                    ..
                }
                if name == "Config"
                    && matches!(field_meta.get("host"), Some(RecordFieldMeta { options, .. }) if options.contains(&VarOption::Export) && options.contains(&VarOption::Static))
                    && matches!(field_meta.get("port"), Some(RecordFieldMeta { options, .. }) if options.contains(&VarOption::Hidden) && options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_variant_binding_options_are_retained() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_entry_variant_options.fol")
        .expect("Should read entry variant option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain entry variant binding options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { variant_meta, .. },
                    ..
                }
                if name == "Result"
                    && matches!(variant_meta.get("Ok"), Some(EntryVariantMeta { options, .. }) if options.contains(&VarOption::Export) && options.contains(&VarOption::Static))
                    && matches!(variant_meta.get("Err"), Some(EntryVariantMeta { options, .. }) if options.contains(&VarOption::Hidden) && options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_grouped_fields_are_retained() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_record_grouped_fields.fol")
        .expect("Should read grouped record field fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse grouped record fields");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record {
                        fields,
                        field_meta,
                        ..
                    },
                    ..
                }
                if name == "Pair"
                    && matches!(fields.get("left"), Some(FolType::Int { .. }))
                    && matches!(fields.get("right"), Some(FolType::Int { .. }))
                    && matches!(fields.get("label"), Some(FolType::Named { name, .. }) if name == "str")
                    && matches!(fields.get("title"), Some(FolType::Named { name, .. }) if name == "str")
                    && matches!(field_meta.get("label"), Some(RecordFieldMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
                    && matches!(field_meta.get("title"), Some(RecordFieldMeta { default: Some(_), options }) if options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_record_field_binding_alternatives_are_retained() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_field_alternatives.fol")
            .expect("Should read record field alternative fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding alternatives in record fields");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Record { field_meta, .. },
                    ..
                }
                if name == "Config"
                    && matches!(field_meta.get("host"), Some(RecordFieldMeta { options, .. }) if options.contains(&VarOption::Export) && options.contains(&VarOption::Mutable))
                    && matches!(field_meta.get("port"), Some(RecordFieldMeta { options, default: Some(_) }) if options.contains(&VarOption::Hidden) && options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_entry_variant_binding_alternatives_are_retained() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_entry_variant_alternatives.fol")
            .expect("Should read entry variant alternative fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding alternatives in entry variants");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Entry { variant_meta, .. },
                    ..
                }
                if name == "Result"
                    && matches!(variant_meta.get("Ok"), Some(EntryVariantMeta { options, .. }) if options.contains(&VarOption::Export) && options.contains(&VarOption::Mutable))
                    && matches!(variant_meta.get("Err"), Some(EntryVariantMeta { options, default: Some(_) }) if options.contains(&VarOption::Hidden) && options.contains(&VarOption::Immutable))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_record_duplicate_field_error_anchors_to_duplicate_name() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_typ_record_grouped_duplicate_field.fol")
            .expect("Should read grouped duplicate record field fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate grouped record fields");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate record field 'left'"),
        "Expected duplicate grouped record field error, got: {}",
        parse_error.message
    );
    assert_eq!(parse_error.primary_location().unwrap().line, 2);
    assert_eq!(parse_error.primary_location().unwrap().column, 11);
}
