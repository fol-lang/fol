use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_type_quoted_names_{label}"))
}

#[test]
fn test_type_declaration_accepts_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_typ_quoted_name.fol")
        .expect("Should read quoted type-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted type names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias { target },
                        ..
                    } if name == "Result"
                        && fol_type_has_qualified_segments(target, &["pkg", "Value"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_type_declaration_preserves_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp type fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "typ \"re'sult\": pkg::Value;\ntyp 'quo\"te': pkg::Other;\n",
    )
    .expect("Should write temp type fixture");

    let mut file_stream =
        FileStream::from_file(fixture.to_str().expect("Type fixture path should be UTF-8"))
            .expect("Should read temp type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in type names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias { target },
                        ..
                    } if name == "re'sult"
                        && fol_type_has_qualified_segments(target, &["pkg", "Value"])
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::TypeDecl {
                        name,
                        type_def: TypeDefinition::Alias { target },
                        ..
                    } if name == "quo\"te"
                        && fol_type_has_qualified_segments(target, &["pkg", "Other"])
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
