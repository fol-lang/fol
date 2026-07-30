use super::*;
use fol_parser::ast::DeclOption;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_segment_decl_{label}"))
}

#[test]
fn test_top_level_segment_declaration_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_module.fol")
        .expect("Should read segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level segment declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { name, seg_type, body, .. }
                    if name == "core"
                        && matches!(seg_type, FolType::Module { .. })
                        && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, .. } if name == "helper"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_parsing_inside_function_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_seg_module.fol")
        .expect("Should read function-body segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse segment declarations inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(stmt, AstNode::SegDecl { name, .. } if name == "local"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_rejects_non_module_types() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_invalid_type.fol")
        .expect("Should read malformed segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject segments with non-module types");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Segment declarations require module types"),
        "Malformed segment declaration should report invalid type, got: {}",
        first_message
    );
}

#[test]
fn test_segment_declaration_accepts_keyword_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_keyword_name.fol")
        .expect("Should read keyword-name segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin-token segment names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { name, seg_type, .. }
                    if name == "get" && matches!(seg_type, FolType::Module { .. })
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_accepts_quoted_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_quoted_name.fol")
        .expect("Should read quoted-name segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted segment names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { name, seg_type, body, .. }
                    if name == "core"
                        && matches!(seg_type, FolType::Module { .. })
                        && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, .. } if name == "helper"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_preserves_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp segment fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "seg \"co're\": mod = { def helper: blk = {}; };\nseg 'quo\"te': mod = { def value: blk = {}; };\n",
    )
    .expect("Should write temp segment fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Segment fixture path should be UTF-8"),
    )
    .expect("Should read temp segment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in segment names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { name, seg_type, body, .. }
                        if name == "co're"
                            && matches!(seg_type, FolType::Module { .. })
                            && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, .. } if name == "helper"))
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::SegDecl { name, seg_type, body, .. }
                        if name == "quo\"te"
                            && matches!(seg_type, FolType::Module { .. })
                            && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, .. } if name == "value"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_accepts_empty_option_brackets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_empty_options.fol")
        .expect("Should read empty-options segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept empty segment option brackets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::SegDecl { name, seg_type, body, .. }
                if name == "core" && matches!(seg_type, FolType::Module { .. }) && body.is_empty()
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_accepts_empty_marker_form() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_marker.fol")
        .expect("Should read marker-form segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept marker-form segment declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::SegDecl { name, seg_type, body, .. }
                if name == "core" && matches!(seg_type, FolType::Module { .. }) && body.is_empty()
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_accepts_empty_option_marker_form() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_empty_options_marker.fol")
        .expect("Should read empty-option marker segment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept seg[] marker declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::SegDecl { name, seg_type, body, .. }
                if name == "core" && matches!(seg_type, FolType::Module { .. }) && body.is_empty()
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_segment_declaration_rejects_non_empty_option_brackets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_unknown_option.fol")
        .expect("Should read invalid segment option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject non-empty segment options");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    let message = parse_error.message.clone();
    assert!(
        message.contains("Unknown segment option"),
        "Non-empty segment option brackets should be rejected, got: {}",
        message
    );
}

#[test]
fn test_segment_declaration_visibility_options_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_seg_visibility_options.fol")
        .expect("Should read visibility-option segment declaration test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse segment visibility options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::SegDecl { name, options, .. }
                if name == "core" && options == &vec![DeclOption::Export]
            )));
        }
        _ => panic!("Expected program node"),
    }
}
