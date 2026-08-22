use super::*;
use fol_parser::ast::UseOption;

#[test]
fn test_use_declaration_accepts_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_visibility_options.fol")
        .expect("Should read use visibility options fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept use visibility options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "pub_math"
                        && options == &vec![UseOption::Export]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "hid_math"
                        && options == &vec![UseOption::Hidden]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "nor_math"
                        && options == &vec![UseOption::Normal]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_accepts_symbolic_visibility_options() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_use_symbol_visibility_options.fol")
            .expect("Should read symbolic use visibility options fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept symbolic use visibility options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "pub_math"
                        && options == &vec![UseOption::Export]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "hid_math"
                        && options == &vec![UseOption::Hidden]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_accepts_multiple_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_multi_options.fol")
        .expect("Should read multi-option use fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept multiple use options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "math"
                        && options == &vec![UseOption::Export, UseOption::Normal]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_use_declaration_rejects_conflicting_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_conflicting_options.fol")
        .expect("Should read conflicting use-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject conflicting use visibility options");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Conflicting use options 'export' and 'hidden'"),
        "Conflicting use options should report explicit visibility conflict, got: {}",
        parse_error.message
    );
}

#[test]
fn test_use_declaration_rejects_duplicate_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_duplicate_options.fol")
        .expect("Should read duplicate use-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate use visibility options");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate use option 'export'"),
        "Duplicate use options should report the duplicated item, got: {}",
        parse_error.message
    );
}

#[test]
fn test_use_declaration_rejects_duplicate_names() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_duplicate_names.fol")
        .expect("Should read duplicate use-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate names in one use declaration");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .clone()
            .contains("Duplicate use name 'math'"),
        "Duplicate use names should report the repeated name, got: {}",
        parse_error.message
    );
}

#[test]
fn test_use_declaration_rejects_canonical_duplicate_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_use_duplicate_names_canonical.fol")
            .expect("Should read canonical duplicate use-name fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate names in one use declaration");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate use name 'MathName'"),
        "Canonical duplicate use names should report the later spelling, got: {}",
        parse_error.message
    );
}

#[test]
fn test_use_declaration_accepts_semicolon_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_use_options_semicolon.fol")
        .expect("Should read semicolon use-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept semicolon-separated use options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "math"
                        && options == &vec![UseOption::Export, UseOption::Normal]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::UseDecl { name, options, .. }
                    if name == "cache"
                        && options == &vec![UseOption::Hidden]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
