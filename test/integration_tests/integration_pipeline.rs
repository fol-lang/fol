use super::*;

#[test]
fn test_internal_runtime_tier_modules_compile_through_root_integration_graph() {
    assert_eq!(fol_runtime::core::tier_name(), "core");
    assert_eq!(fol_runtime::memo::tier_name(), "memo");
    assert_eq!(fol_runtime::std::tier_name(), "std");

    assert_eq!(fol_runtime::memo::base_core_tier(), fol_runtime::core::TIER);
    assert_eq!(fol_runtime::std::base_core_tier(), fol_runtime::core::TIER);
    assert_eq!(fol_runtime::std::base_memo_tier(), fol_runtime::memo::TIER);
}

#[test]
fn test_stream_to_lexer_integration() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_stream::FileStream;

    // Test that stream output works with lexer input
    let mut file_stream =
        FileStream::from_file("test/lexer/mixed.fol").expect("Should read test file");

    let lexer = Elements::init(&mut file_stream);

    // Should be able to get at least one token
    match lexer.curr(false) {
        Ok(token) => {
            println!("Integration test: First token = '{}'", token.con());
            // Check that we get a valid token (even if empty content like spaces)
            // or EOF - this verifies the integration is working
            assert!(
                !token.key().is_illegal(),
                "Integration token should be valid"
            );
        }
        Err(e) => panic!("Stream to lexer integration failed: {:?}", e),
    }
}

#[test]
fn test_stream_to_lexer_order_stays_stable_across_multiple_files() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_lexer::token::KEYWORD;
    use fol_stream::FileStream;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!(
        "fol_stream_lexer_order_{}_{}",
        std::process::id(),
        stamp
    ));

    fs::create_dir_all(temp_root.join("alpha_10")).expect("Should create alpha fixture dir");
    fs::create_dir_all(temp_root.join("beta_20")).expect("Should create beta fixture dir");
    fs::write(temp_root.join("00_root.fol"), "root_token").expect("Should write root fixture");
    fs::write(temp_root.join("alpha_10/entry.fol"), "alpha_token")
        .expect("Should write alpha fixture");
    fs::write(temp_root.join("beta_20/entry.fol"), "beta_token")
        .expect("Should write beta fixture");

    let mut file_stream = FileStream::from_folder(
        temp_root
            .to_str()
            .expect("Order fixture path should be valid utf-8"),
    )
    .expect("Should create file stream from ordered folder fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut identifiers = Vec::new();

    for _ in 0..10_000 {
        let token = lexer
            .curr(false)
            .expect("Lexer should expose tokens while walking the ordered fixture");
        if token.key().is_eof() {
            break;
        }
        if matches!(token.key(), KEYWORD::Identifier) {
            identifiers.push(token.con().to_string());
        }
        if lexer.bump().is_none() {
            break;
        }
    }

    assert_eq!(
        identifiers,
        vec![
            "root_token".to_string(),
            "alpha_token".to_string(),
            "beta_token".to_string(),
        ],
        "Flattened lexer output should preserve the stream's deterministic file order"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_lexer_to_parser_literal_continuity_for_supported_forms() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_parser::ast::{AstNode, AstParser, Literal};
    use fol_stream::FileStream;
    use std::fs;

    let temp_root = unique_temp_root("literal_continuity");
    fs::create_dir_all(&temp_root).expect("Should create temp literal fixture dir");
    let fixture = temp_root.join("literal_continuity.fol");
    fs::write(
        &fixture,
        "\"hello\"\n'c'\n'true'\n42\n3.5\n0x1A\n0o17\n0b1010\ntrue\nfalse\nnil\n",
    )
    .expect("Should write literal continuity fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Literal continuity fixture path should be utf-8"),
    )
    .expect("Should open literal continuity fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let package = parser
        .parse_script_package(&mut lexer)
        .expect("Supported literal forms should survive stream and lexer into exact AST literals");

    let declarations: Vec<AstNode> = package
        .source_units
        .into_iter()
        .flat_map(|unit| unit.items.into_iter().map(|item| item.node))
        .collect();

    assert_eq!(
            declarations,
            vec![
                AstNode::Literal(Literal::String("hello".to_string())),
                AstNode::Literal(Literal::Character('c')),
                AstNode::Literal(Literal::String("true".to_string())),
                AstNode::Literal(Literal::Integer(42)),
                AstNode::Literal(Literal::Float(3.5)),
                AstNode::Literal(Literal::Integer(0x1A)),
                AstNode::Literal(Literal::Integer(0o17)),
                AstNode::Literal(Literal::Integer(0b1010)),
                AstNode::Literal(Literal::Boolean(true)),
                AstNode::Literal(Literal::Boolean(false)),
                AstNode::Literal(Literal::Nil),
            ],
            "Cross-phase literal continuity should preserve exact AST literal values for supported forms"
        );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_lexer_to_parser_integration() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_parser::ast::AstParser;
    use fol_stream::FileStream;

    // Test that lexer output works with parser input
    let mut file_stream =
        FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    // Should be able to parse without crashing
    match parser.parse_script_package(&mut lexer) {
        Ok(_ast) => {
            println!("Lexer to parser integration successful");
        }
        Err(errors) => {
            println!("Parser errors (may be expected): {:?}", errors);
            // For minimal parser, we mainly test that it doesn't crash
        }
    }
}

#[test]
fn test_full_pipeline_integration() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_parser::ast::AstParser;
    use fol_resolver::resolve_package;
    use fol_stream::FileStream;

    let mut file_stream =
        FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Full pipeline happy-path fixture should parse cleanly");
    let resolved =
        resolve_package(parsed).expect("Full pipeline happy-path fixture should resolve");

    assert_eq!(resolved.package_name(), "parser");
    assert_eq!(resolved.source_units.len(), 1);
    assert!(
        !resolved.symbols_in_scope(resolved.program_scope).is_empty(),
        "Resolver-backed full pipeline runs should produce top-level symbols"
    );
}

#[test]
fn test_full_pipeline_cross_file_import_resolution() {
    use fol_lexer::lexer::stage3::Elements;
    use fol_parser::ast::AstParser;
    use fol_resolver::resolve_package;
    use fol_stream::FileStream;
    use std::fs;

    let temp_root = unique_temp_root("pipeline_cross_file_import");
    fs::create_dir_all(temp_root.join("net/http"))
        .expect("Should create a temporary integration fixture directory");
    fs::write(
        temp_root.join("net/http/route.fol"),
        "var handler: int = 1;\n",
    )
    .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use http: loc = {\"net::http\"};\nfun[] main(): int = {\n    return http;\n};\n",
    )
    .expect("Should write the importing source unit fixture");

    let mut file_stream = FileStream::from_folder(
        temp_root
            .to_str()
            .expect("Integration fixture path should be valid UTF-8"),
    )
    .expect("Should read integration folder fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Cross-file import fixture should parse cleanly");
    let resolved =
        resolve_package(parsed).expect("Cross-file import fixture should resolve cleanly");
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "http")
        .expect("Resolved program should keep the import record");

    assert!(
            matches!(
                import
                    .target_scope
                    .and_then(|scope_id| resolved.scope(scope_id))
                    .map(|scope| &scope.kind),
                Some(fol_resolver::ScopeKind::NamespaceRoot { namespace }) if namespace.ends_with("http")
            ),
            "Cross-file full pipeline runs should resolve intra-package loc imports to the existing namespace scope"
        );

    fs::remove_dir_all(&temp_root).ok();
}
