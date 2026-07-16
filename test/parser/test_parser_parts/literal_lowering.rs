use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_literal_lowering_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn parse_first_error_from_source(label: &str, source: &str) -> fol_diagnostics::Diagnostic {
    let temp_root = unique_temp_root(label);
    fs::create_dir_all(&temp_root).expect("Should create temporary literal fixture dir");
    let fixture = temp_root.join("literal_lowering.fol");
    fs::write(&fixture, source).expect("Should write temporary literal fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Temporary literal fixture path should be UTF-8"),
    )
    .expect("Should open temporary literal fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject out-of-range decimal literals");

    fs::remove_dir_all(&temp_root).ok();

    errors
        .into_iter()
        .next()
        .expect("First parser error should exist")
}

#[test]
fn test_top_level_string_and_character_literals_lower_cleanly() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_strings.fol")
        .expect("Should read literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level string-like literals");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::String("hello".to_string())),
                    AstNode::Literal(Literal::Character('c')),
                    AstNode::Literal(Literal::String("xy".to_string())),
                ],
                "Quoted literals should lower to clean AST values without wrapper quotes"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_cooked_family_fixture_lowers_by_inner_width() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_cooked_family.fol")
        .expect("Should read cooked family literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower cooked-family literals through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Character('a')),
                    AstNode::Literal(Literal::String("beta".to_string())),
                ],
                "Cooked double-quoted fixture literals should lower by decoded inner width"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_raw_family_fixture_lowers_by_inner_width() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_raw_family.fol")
        .expect("Should read raw family literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower raw-family literals through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Character('z')),
                    AstNode::Literal(Literal::String("omega".to_string())),
                ],
                "Raw single-quoted fixture literals should lower by undecoded inner width"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_integer_literals_lower_to_exact_values() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_numbers.fol")
        .expect("Should read numeric literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level numeric literals");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Integer(42)),
                    AstNode::Literal(Literal::Integer(1_000)),
                    AstNode::Literal(Literal::Integer(26)),
                    AstNode::Literal(Literal::Integer(15)),
                    AstNode::Literal(Literal::Integer(10)),
                ],
                "Numeric literals should preserve their integer value across supported families"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_parse_literal_supports_float_payloads() {
    let parser = AstParser::new();
    let literal = parser
        .parse_literal("3.5")
        .expect("Direct parser literal lowering should support floats");

    assert_eq!(
        literal,
        AstNode::Literal(Literal::Float(3.5)),
        "Float payloads should lower to Literal::Float"
    );
}

#[test]
fn test_parse_literal_rejects_out_of_range_decimal_instead_of_lowering_to_identifier() {
    let parser = AstParser::new();
    let error = parser
        .parse_literal("9223372036854775808")
        .expect_err("Out-of-range decimal literal should fail instead of becoming an identifier");

    let error_msg = error.to_string();
    assert!(
        error_msg.contains("out of range for current parser literal lowering"),
        "Decimal overflow should report an explicit parse failure, got: {}",
        error_msg
    );
}

#[test]
fn test_top_level_out_of_range_decimal_reports_parse_error() {
    let error = parse_first_error_from_source("decimal_overflow", "9223372036854775808\n");

    assert!(
        error
            .message
            .contains("out of range for current parser literal lowering"),
        "Out-of-range decimal tokens should report a parse error, got: {}",
        error.message
    );
    assert_eq!(
        error.primary_location().unwrap().line,
        1,
        "Decimal overflow should report its own line"
    );
    assert_eq!(
        error.primary_location().unwrap().column,
        1,
        "Decimal overflow should point at the literal token itself"
    );
}

#[test]
fn test_parse_literal_rejects_out_of_range_prefixed_integers() {
    let parser = AstParser::new();

    for (literal, family) in [
        ("0x8000000000000000", "Hexadecimal"),
        ("0o1000000000000000000000", "Octal"),
        (
            "0b1000000000000000000000000000000000000000000000000000000000000000",
            "Binary",
        ),
    ] {
        let error = parser.parse_literal(literal).expect_err(
            "Out-of-range prefixed literal should fail instead of becoming an identifier",
        );

        let error_msg = error.to_string();
        assert!(
            error_msg.contains(&format!("{family} literal")),
            "Prefixed overflow should use explicit {family} wording, got: {}",
            error_msg
        );
    }
}

#[test]
fn test_top_level_out_of_range_prefixed_literals_report_parse_errors() {
    for (label, source, family) in [
        ("hex_overflow", "0x8000000000000000\n", "Hexadecimal"),
        ("octal_overflow", "0o1000000000000000000000\n", "Octal"),
        (
            "binary_overflow",
            "0b1000000000000000000000000000000000000000000000000000000000000000\n",
            "Binary",
        ),
    ] {
        let error = parse_first_error_from_source(label, source);

        assert!(
            error.message.contains(&format!("{family} literal")),
            "Out-of-range {family} tokens should report a parse error, got: {}",
            error.message
        );
        assert_eq!(
            error.primary_location().unwrap().line,
            1,
            "{family} overflow should report its own line"
        );
        assert_eq!(
            error.primary_location().unwrap().column,
            1,
            "{family} overflow should point at the literal token itself"
        );
    }
}

#[test]
fn test_double_quotes_now_lower_by_inner_width() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("\"c\"")
            .expect("Double-quoted single-character text should parse"),
        AstNode::Literal(Literal::Character('c')),
        "Double quotes should lower one-character payloads to Literal::Character under the chosen width policy"
    );
    assert_eq!(
        parser
            .parse_literal("\"xy\"")
            .expect("Double-quoted multi-character text should parse"),
        AstNode::Literal(Literal::String("xy".to_string())),
        "Double quotes should still lower wider payloads to Literal::String"
    );
}

#[test]
fn test_single_quotes_lower_by_inner_width() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("'c'")
            .expect("Single-quoted single-character text should parse"),
        AstNode::Literal(Literal::Character('c')),
        "Single quotes should lower one-character payloads to Literal::Character"
    );
    assert_eq!(
        parser
            .parse_literal("'xy'")
            .expect("Single-quoted multi-character text should parse"),
        AstNode::Literal(Literal::String("xy".to_string())),
        "Single quotes should fall back to Literal::String when the inner payload is wider than one character"
    );
}

#[test]
fn test_cooked_double_quotes_decode_basic_escapes_before_width_lowering() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("\"\\n\"")
            .expect("Cooked newline escape should parse"),
        AstNode::Literal(Literal::Character('\n')),
        "Cooked escapes should decode before one-element literals are lowered to Literal::Character"
    );
    assert_eq!(
        parser
            .parse_literal("\"say \\\"hi\\\"\"")
            .expect("Cooked quote escape should parse"),
        AstNode::Literal(Literal::String("say \"hi\"".to_string())),
        "Cooked double quotes should decode escaped delimiters in string payloads"
    );
    assert_eq!(
        parser
            .parse_literal("\"\\\\path\"")
            .expect("Cooked backslash escape should parse"),
        AstNode::Literal(Literal::String("\\path".to_string())),
        "Cooked double quotes should decode escaped backslashes"
    );
}

#[test]
fn test_top_level_cooked_quote_and_backslash_escapes_lower_through_fixture() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_literal_cooked_escape_quotes.fol")
            .expect("Should read cooked quote/backslash literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower cooked quote and backslash escapes through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::String("say \"hi\"".to_string())),
                    AstNode::Literal(Literal::String("\\path".to_string())),
                ],
                "Cooked fixture literals should decode escaped quotes and escaped backslashes before AST lowering"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_raw_single_quotes_keep_escape_spelling_verbatim() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("'\\n'")
            .expect("Raw quoted backslash-n should parse"),
        AstNode::Literal(Literal::String("\\n".to_string())),
        "Raw single quotes should not decode cooked escape spellings"
    );
}

#[test]
fn test_cooked_double_quotes_decode_numeric_and_unicode_escapes() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("\"\\65\"")
            .expect("Cooked decimal escape should parse"),
        AstNode::Literal(Literal::Character('A')),
        "Cooked decimal escapes should decode using all directly following digits"
    );
    assert_eq!(
        parser
            .parse_literal("\"\\x41\"")
            .expect("Cooked hex escape should parse"),
        AstNode::Literal(Literal::Character('A')),
        "Cooked hex escapes should decode exactly two hex digits"
    );
    assert_eq!(
        parser
            .parse_literal("\"\\u0041\"")
            .expect("Cooked four-digit unicode escape should parse"),
        AstNode::Literal(Literal::Character('A')),
        "Cooked unicode escapes should decode exactly four hex digits"
    );
    assert_eq!(
        parser
            .parse_literal("\"\\u{41}\"")
            .expect("Cooked braced unicode escape should parse"),
        AstNode::Literal(Literal::Character('A')),
        "Cooked braced unicode escapes should decode all enclosed hex digits"
    );
}

#[test]
fn test_top_level_cooked_unicode_escape_fixture_lowers_through_pipeline() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_literal_cooked_unicode_escapes.fol")
            .expect("Should read cooked unicode escape literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower cooked unicode escape literals through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Character('A')),
                    AstNode::Literal(Literal::Character('A')),
                    AstNode::Literal(Literal::Character('A')),
                    AstNode::Literal(Literal::Character('A')),
                ],
                "Cooked numeric and unicode escape spellings should decode identically when they spell the same scalar value"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_cooked_double_quotes_trim_backslash_line_continuations() {
    let parser = AstParser::new();

    assert_eq!(
        parser
            .parse_literal("\"foo\\\n              bar\"")
            .expect("Cooked LF continuation should parse"),
        AstNode::Literal(Literal::String("foobar".to_string())),
        "Cooked backslash-LF continuations should drop the line break and next-line indentation"
    );
    assert_eq!(
        parser
            .parse_literal("\"foo\\\r\n\tbar\"")
            .expect("Cooked CRLF continuation should parse"),
        AstNode::Literal(Literal::String("foobar".to_string())),
        "Cooked backslash-CRLF continuations should also drop the platform line break and indentation"
    );
}

#[test]
fn test_top_level_multiline_cooked_literal_lowers_through_fixture() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_multiline_cooked.fol")
        .expect("Should read multiline cooked literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower multiline cooked literals through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![AstNode::Literal(Literal::String("foobar".to_string()))],
                "Cooked backslash-newline continuation should trim the line break and indentation in file fixtures too"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_cooked_and_raw_escape_modes_lower_through_full_pipeline() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_escape_modes.fol")
        .expect("Should read mixed escape-mode literal fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower cooked and raw escape modes through the full pipeline");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Character('\n')),
                    AstNode::Literal(Literal::String("\\n".to_string())),
                    AstNode::Literal(Literal::Character('A')),
                    AstNode::Literal(Literal::String("\\x41".to_string())),
                    AstNode::Literal(Literal::String("foobar".to_string())),
                ],
                "Cooked literals should decode escapes and continuations, while raw literals should preserve their source spelling"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_cooked_and_raw_quote_families_normalize_to_one_ast_literal_shape() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_literal_normalized_quote_families.fol")
            .expect("Should read normalized quote-family fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should normalize quote families after literal lowering");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Character('c')),
                    AstNode::Literal(Literal::Character('c')),
                    AstNode::Literal(Literal::String("xy".to_string())),
                    AstNode::Literal(Literal::String("xy".to_string())),
                ],
                "Cooked and raw quote families should lower to the same AST literal variants once their payload content is equivalent"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_boolean_and_nil_literals_lower_cleanly() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_logic.fol")
        .expect("Should read logical literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level boolean and nil literals");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Boolean(true)),
                    AstNode::Literal(Literal::Boolean(false)),
                    AstNode::Literal(Literal::Nil),
                ],
                "Top-level logical literals should lower to concrete AST literals"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
#[allow(clippy::approx_constant)] // The fixture intentionally contains the literal `3.14`.
fn test_top_level_float_literal_lowers_cleanly() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_float.fol")
        .expect("Should read float literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level float literal");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![AstNode::Literal(Literal::Float(3.14))],
                "Top-level float literal should lower to Literal::Float"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_trailing_dot_float_literal_lowers_cleanly() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_literal_trailing_dot_float.fol")
            .expect("Should read trailing-dot float literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level trailing-dot float literal");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![AstNode::Literal(Literal::Float(1.0))],
                "Top-level trailing-dot float literal should lower to Literal::Float"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_underscored_float_literal_lowers_cleanly() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_underscored_float.fol")
        .expect("Should read underscored float literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower top-level underscored float literal");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![AstNode::Literal(Literal::Float(123.45))],
                "Top-level underscored float literal should normalize underscores and lower to Literal::Float"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_prefixed_integer_literals_lower_cleanly() {
    let mut file_stream = FileStream::from_file("test/parser/simple_literal_prefixed_numbers.fol")
        .expect("Should read prefixed literal lowering fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower prefixed integer literals");

    match ast {
        AstNode::Program { declarations } => {
            assert_eq!(
                declarations,
                vec![
                    AstNode::Literal(Literal::Integer(0xCAFE)),
                    AstNode::Literal(Literal::Integer(0o77)),
                    AstNode::Literal(Literal::Integer(0b1010_0001)),
                    AstNode::Literal(Literal::Integer(0x1A)),
                    AstNode::Literal(Literal::Integer(0o17)),
                    AstNode::Literal(Literal::Integer(0b1010)),
                ],
                "Prefixed integer literals should lower through the full lexer/parser pipeline"
            );
        }
        _ => panic!("Expected program node"),
    }
}
