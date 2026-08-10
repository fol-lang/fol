use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_quoted_routine_{label}"))
}

#[test]
fn test_quoted_routine_names_parse_in_function_declarations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_quoted_routine_names.fol")
        .expect("Should read quoted routine name test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted routine names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations
                .iter()
                .any(|node| { matches!(node, AstNode::FunDecl { name, .. } if name == "$") }));
            assert!(declarations
                .iter()
                .any(|node| { matches!(node, AstNode::FunDecl { name, .. } if name == "%") }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_quoted_routine_names_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp quoted-routine fixture dir");
    let fixture = temp_root.join("quoted_routine_inner_quotes.fol");
    fs::write(
        &fixture,
        "fun \"'$'\"(): int = { return 1; };\nfun '\"%\"'(): int = { return 2; };\n",
    )
    .expect("Should write temp quoted-routine fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Quoted-routine fixture path should be UTF-8"),
    )
    .expect("Should read temp quoted-routine fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quote chars in routine names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations
                .iter()
                .any(|node| { matches!(node, AstNode::FunDecl { name, .. } if name == "'$'") }));
            assert!(declarations
                .iter()
                .any(|node| { matches!(node, AstNode::FunDecl { name, .. } if name == "\"%\"") }));
        }
        _ => panic!("Expected program node"),
    }
}
