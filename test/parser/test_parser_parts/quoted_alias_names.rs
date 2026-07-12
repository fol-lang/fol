use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_alias_quoted_names_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_alias_declaration_accepts_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_ali_quoted_name.fol")
        .expect("Should read quoted alias-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted alias names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target, .. } if name == "Result"
                        && fol_type_has_qualified_segments(target, &["pkg", "Value"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_alias_declaration_preserves_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp alias fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "ali \"re'sult\": pkg::Value;\nali 'quo\"te': pkg::Other;\n",
    )
    .expect("Should write temp alias fixture");

    let mut file_stream =
        FileStream::from_file(fixture.to_str().expect("Alias fixture path should be UTF-8"))
            .expect("Should read temp alias fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in alias names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target, .. } if name == "re'sult"
                        && fol_type_has_qualified_segments(target, &["pkg", "Value"])
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::AliasDecl {
                        name,
                        target, .. } if name == "quo\"te"
                        && fol_type_has_qualified_segments(target, &["pkg", "Other"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
