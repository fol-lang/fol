use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_named_generics_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_routine_generic_headers_accept_keyword_and_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_named_generics.fol")
        .expect("Should read named routine generics fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword and quoted routine generic names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { generics, .. }
                    if generics.len() == 2 && generics[0].name == "get" && generics[1].name == "item"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_type_generic_headers_accept_keyword_and_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_named_generics.fol")
        .expect("Should read named type generics fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept keyword and quoted type generic names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl { generics, .. }
                    if generics.len() == 2 && generics[0].name == "get" && generics[1].name == "item"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_generic_headers_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_single_quoted_generics.fol")
        .expect("Should read single-quoted generic-header fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted generic names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::FunDecl { generics, .. } if generics.len() == 2 && generics[0].name == "get" && generics[1].name == "item")
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::TypeDecl { generics, .. } if generics.len() == 2 && generics[0].name == "get" && generics[1].name == "item")
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_named_generic_constraints_accept_quoted_type_references() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_named_generic_constraints.fol")
            .expect("Should read named generic constraints fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type references in generic constraints");

    match ast {
        AstNode::Program { declarations } => {
            let has_expected_constraints = |generics: &Vec<fol_parser::ast::Generic>| {
                generics.len() == 2
                    && matches!(generics[0].constraints.as_slice(), [FolType::Named { name, .. }] if name == "Bound")
                    && matches!(generics[1].constraints.as_slice(), [typ] if fol_type_has_qualified_segments(typ, &["pkg", "Shape"]))
            };

            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::FunDecl { generics, .. } if has_expected_constraints(generics))
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::TypeDecl { generics, .. } if has_expected_constraints(generics))
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_generic_headers_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp generic fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "fun demo(\"g'et\", 'it\"em')(value: int): int = {\n    return value;\n};\ntyp Box(\"g'et\", 'it\"em'): rec = {\n    value: int;\n};\n",
    )
    .expect("Should write temp generic fixture");

    let mut file_stream =
        FileStream::from_file(fixture.to_str().expect("Generic fixture path should be UTF-8"))
            .expect("Should read temp generic fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in generic names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            let has_expected_generics = |generics: &Vec<fol_parser::ast::Generic>| {
                generics.len() == 2
                    && generics[0].name == "g'et"
                    && generics[1].name == "it\"em"
            };

            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::FunDecl { name, generics, .. } if name == "demo" && has_expected_generics(generics))
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::TypeDecl { name, generics, .. } if name == "Box" && has_expected_generics(generics))
            }));
        }
        _ => panic!("Expected program node"),
    }
}
