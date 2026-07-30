use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_use_quoted_names_{label}"))
}

#[test]
fn test_use_declaration_accepts_quoted_name() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_quoted_name.fol")
        .expect("Should read quoted-name use declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted use names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, path_type, .. }
                    if name == "warn"
                        && use_decl_path_text(node).as_deref() == Some("std/warn")
                        && matches!(path_type, FolType::Module { .. })
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_preserves_inner_opposite_quote_chars_in_names() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp use-name fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "use \"wa'rn\": mod = {\"std/warn\"};\nuse 'tr\"ace': mod = {'std/trace'};\n",
    )
    .expect("Should write temp use-name fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Use-name fixture path should be UTF-8"),
    )
    .expect("Should read temp use-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in use names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, .. }
                        if name == "wa'rn" && use_decl_path_text(node).as_deref() == Some("std/warn")
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, .. }
                        if name == "tr\"ace" && use_decl_path_text(node).as_deref() == Some("std/trace")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_accepts_multiple_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_multi_quoted_names.fol")
        .expect("Should read multi quoted-name use declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept multiple quoted use names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations
                .iter()
                .any(|node| use_decl_matches_path(node, "warn", "std/warn")));
            assert!(declarations.iter().any(|node| use_decl_matches_path(
                node,
                "trace",
                "std/trace"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_normalizes_single_quoted_paths() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_single_quoted_path.fol")
        .expect("Should read single-quoted use path fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should normalize single-quoted use paths");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations
                .iter()
                .any(|node| use_decl_matches_path(node, "warn", "std/warn")));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_accepts_multiple_single_quoted_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_use_multi_single_quoted_names.fol")
            .expect("Should read multi single-quoted use declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept multiple single-quoted use names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations
                .iter()
                .any(|node| use_decl_matches_path(node, "warn", "std/warn")));
            assert!(declarations.iter().any(|node| use_decl_matches_path(
                node,
                "trace",
                "std/trace"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_type_references_parse_in_use_declarations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_quoted_use_type_refs.fol")
        .expect("Should read quoted use-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type references in use declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::UseDecl { name, path_type, .. } if name == "warn" && fol_type_named_text_is(path_type, "Module"))
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::UseDecl { name, path_type, .. } if name == "trace" && fol_type_named_text_is(path_type, "pkg::Module"))
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_type_references_parse_in_use_declarations() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_use_type_refs.fol")
            .expect("Should read single-quoted use-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted type refs in use declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::UseDecl { name, path_type, .. } if name == "warn" && fol_type_named_text_is(path_type, "Module"))
            }));
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::UseDecl { name, path_type, .. } if name == "trace" && fol_type_named_text_is(path_type, "pkg::Module"))
            }));
        }
        _ => panic!("Expected program node"),
    }
}
