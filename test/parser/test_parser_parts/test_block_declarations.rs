use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_test_block_{label}"))
}

#[test]
fn test_test_type_reference_lowering() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_test_block_type.fol")
        .expect("Should read tst type lowering test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower tst[...] type references");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Test { name: Some(label), access }
                        },
                        ..
                    }
                    if name == "TestBlock" && label == "unit" && access == &vec!["shko".to_string()]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_test_block_definition_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_test_block.fol")
        .expect("Should read named tst def test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse tst block definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: Some(label), access },
                        body,
                        ..
                    }
                    if name == "test1"
                        && label == "sometest"
                        && access == &vec!["shko".to_string()]
                        && body.is_empty()
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_access_only_test_block_definition_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_def_access_only_test_block.fol")
            .expect("Should read access-only tst def test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse access-only tst block definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: None, access },
                        ..
                    }
                    if name == "some unit testing" && access == &vec!["shko".to_string()]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_test_block_labels_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp tst fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "typ Alpha: tst[\"unit 'alpha'\", shko];\ndef beta: tst['case \"beta\"', shko] = {};\n",
    )
    .expect("Should write temp tst fixture");

    let mut file_stream =
        FileStream::from_file(fixture.to_str().expect("tst fixture path should be UTF-8"))
            .expect("Should read temp tst fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in tst labels");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Test { name: Some(label), access }
                        },
                        ..
                    } if name == "Alpha"
                        && label == "unit 'alpha'"
                        && access == &vec!["shko".to_string()]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: Some(label), access },
                        ..
                    } if name == "beta"
                        && label == "case \"beta\""
                        && access == &vec!["shko".to_string()]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_test_block_rejects_quoted_access_argument() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_bad_test_block_access.fol")
        .expect("Should read malformed tst def test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject quoted tst access arguments");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Quoted tst[...] arguments are only allowed for the optional test label"),
        "Expected quoted tst access diagnostic, got: {}",
        parse_error.message
    );
}

#[test]
fn test_test_block_types_accept_semicolon_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_test_type_semicolon.fol")
        .expect("Should read semicolon tst[...] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated tst[...] arguments");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Test { name: Some(label), access }
                        },
                        ..
                    }
                    if name == "TestBlock" && label == "unit" && access == &vec!["shko".to_string()]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: Some(label), access },
                        ..
                    }
                    if name == "test1"
                        && label == "sometest"
                        && access == &vec!["shko".to_string()]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: None, access },
                        ..
                    }
                    if name == "some unit testing" && access == &vec!["shko".to_string()]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_test_block_types_accept_trailing_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_test_type_trailing_separator.fol")
            .expect("Should read trailing tst[...] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse trailing tst[...] separators");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias {
                            target: FolType::Test { name: Some(label), access }
                        },
                        ..
                    }
                    if name == "TestBlock" && label == "unit" && access == &vec!["shko".to_string()]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: Some(label), access },
                        ..
                    }
                    if name == "test1"
                        && label == "sometest"
                        && access == &vec!["shko".to_string()]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Test { name: None, access },
                        ..
                    }
                    if name == "some unit testing" && access == &vec!["shko".to_string()]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
