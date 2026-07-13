use super::*;

#[test]
fn test_keywords() {
    let tokens = tokenize_file("test/lexer/keywords.fol");

    // Filter out spaces and EOF
    let keywords: Vec<_> = tokens
        .iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert!(keywords.len() >= 10, "Should have multiple keywords");

    // Check for specific keywords
    let keyword_strings: Vec<String> = keywords
        .iter()
        .map(|(_, content)| content.clone())
        .collect();

    assert!(
        keyword_strings.contains(&"use".to_string()),
        "Should contain 'use' keyword"
    );
    assert!(
        keyword_strings.contains(&"var".to_string()),
        "Should contain 'var' keyword"
    );
    assert!(
        keyword_strings.contains(&"fun".to_string()),
        "Should contain 'fun' keyword"
    );
    assert!(
        keyword_strings.contains(&"pro".to_string()),
        "Should contain 'pro' keyword"
    );
    assert!(
        keyword_strings.contains(&"typ".to_string()),
        "Should contain 'typ' keyword"
    );
    assert!(
        keyword_strings.contains(&"std".to_string()),
        "Should contain 'std' keyword"
    );
    assert!(
        keyword_strings.contains(&"log".to_string()),
        "Should contain 'log' keyword"
    );
    assert!(
        keyword_strings.contains(&"cast".to_string()),
        "Should contain 'cast' keyword"
    );
    assert!(
        keyword_strings.contains(&"on".to_string()),
        "Should contain 'on' keyword"
    );
    assert!(
        keyword_strings.contains(&"while".to_string()),
        "Should contain 'while' keyword"
    );
    assert!(
        keyword_strings.contains(&"async".to_string()),
        "Should contain 'async' keyword"
    );
    assert!(
        keyword_strings.contains(&"await".to_string()),
        "Should contain 'await' keyword"
    );
    assert!(
        keyword_strings.contains(&"select".to_string()),
        "Should contain 'select' keyword"
    );
}

#[test]
fn deleted_let_spelling_lexes_as_an_identifier() {
    let tokens = tokenize_folder_contents(&[("binding.fol", "let")]);
    let significant = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect::<Vec<_>>();

    assert_eq!(
        significant,
        vec![(KEYWORD::Identifier, "let".to_string())],
        "deleted binding spelling should not remain reserved"
    );
    assert!(!fol_lexer::token::buildin::OTHER_KEYWORDS.contains(&"let"));
}

#[test]
fn test_keyword_recognition_is_exact_case_only() {
    let tokens = tokenize_file("test/lexer/keyword_case_edges.fol");
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "Fun".to_string()),
            (KEYWORD::Keyword(BUILDIN::Fun), "fun".to_string()),
            (KEYWORD::Identifier, "WHILE".to_string()),
            (KEYWORD::Keyword(BUILDIN::While), "while".to_string()),
            (KEYWORD::Identifier, "LOG".to_string()),
            (KEYWORD::Keyword(BUILDIN::Log), "log".to_string()),
            (KEYWORD::Identifier, "Select".to_string()),
            (KEYWORD::Keyword(BUILDIN::Select), "select".to_string()),
        ],
        "Keyword recognition should remain exact-case only until the lexer contract changes intentionally"
    );
}

#[test]
fn test_stage3_starts_on_first_real_token() {
    let mut file_stream =
        FileStream::from_file("test/lexer/mixed.fol").expect("Should read mixed.fol");
    let lexer = Elements::init(&mut file_stream);
    let token = lexer
        .curr(false)
        .expect("Stage 3 lexer should expose the first token immediately");

    assert_eq!(token.loc().row(), 1, "First token should not be synthetic");
    assert_eq!(token.loc().col(), 1, "First token should start at column 1");
    assert_eq!(token.con(), "var", "First token should be the first real token");
}

#[test]
fn test_cross_file_boundaries_surface_as_explicit_void_tokens() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Adjacent files should be separated by an explicit boundary token instead of a synthetic newline character"
    );
}

#[test]
fn test_cross_file_boundaries_survive_trailing_newlines() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha\n"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| {
            !key.is_space()
                && !matches!(key, KEYWORD::Void(VOID::EndLine))
                && !key.is_eof()
        })
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "A trailing newline in the previous file must not erase the explicit source boundary token"
    );
}

#[test]
fn test_cross_file_boundaries_survive_trailing_semicolons() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha;"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Symbol(SYMBOL::Semi), ";".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "A trailing semicolon must not absorb the explicit source boundary token"
    );
}

#[test]
fn test_unterminated_quotes_stop_at_file_boundaries() {
    let tokens =
        tokenize_folder_contents(&[("a.fol", "\"unterminated"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Illegal, "\"unterminated".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Quoted spans must stop at source boundaries instead of consuming the next file"
    );
}

#[test]
fn test_unterminated_backtick_comments_stop_at_file_boundaries() {
    let tokens = tokenize_folder_contents(&[("a.fol", "`unterminated"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Illegal, "`unterminated".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Backtick comments must stop at source boundaries instead of consuming the next file"
    );
}

#[test]
fn test_unterminated_slash_block_comments_stop_at_file_boundaries() {
    let tokens = tokenize_folder_contents(&[("a.fol", "/*unterminated"), ("b.fol", "beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Illegal, "/*unterminated".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Slash block comments must stop at source boundaries instead of consuming the next file"
    );
}

#[test]
fn test_identifier_number_boundaries_do_not_merge_across_files() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha"), ("b.fol", "42")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Literal(LITERAL::Decimal), "42".to_string()),
        ],
        "Cross-file boundaries must keep identifiers and following numeric literals separate"
    );
}

#[test]
fn test_identifier_string_boundaries_do_not_merge_across_files() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha"), ("b.fol", "\"beta\"")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Literal(LITERAL::CookedQuoted), "\"beta\"".to_string()),
        ],
        "Cross-file boundaries must keep identifiers and following quoted literals separate"
    );
}

#[test]
fn test_identifier_comment_boundaries_do_not_merge_across_files() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha"), ("b.fol", "`hidden`beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Comment(COMMENT::Backtick), "`hidden`".to_string()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Cross-file boundaries must keep identifiers, preserved comments, and following identifiers separate"
    );
}

#[test]
fn test_identifier_operator_boundaries_do_not_merge_across_files() {
    let tokens = tokenize_folder_contents(&[("a.fol", "alpha"), ("b.fol", "+beta")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Identifier, "alpha".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Symbol(SYMBOL::Plus), "+".to_string()),
            (KEYWORD::Identifier, "beta".to_string()),
        ],
        "Cross-file boundaries must keep identifiers and following operators separate"
    );
}

#[test]
fn test_invalid_prefixed_numeric_literals_stop_at_file_boundaries() {
    let tokens = tokenize_folder_contents(&[("a.fol", "0x"), ("b.fol", "CAFE")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Illegal, "0x".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Identifier, "CAFE".to_string()),
        ],
        "Malformed prefixed numerics must stop at a file boundary instead of consuming the next file's content"
    );
}

#[test]
fn test_invalid_decimal_literals_stop_at_file_boundaries() {
    let tokens = tokenize_folder_contents(&[("a.fol", "1_"), ("b.fol", "2")]);
    let significant: Vec<(KEYWORD, String)> = tokens
        .into_iter()
        .filter(|(key, _)| !key.is_space() && !key.is_eof())
        .collect();

    assert_eq!(
        significant,
        vec![
            (KEYWORD::Illegal, "1_".to_string()),
            (KEYWORD::Void(VOID::Boundary), String::new()),
            (KEYWORD::Literal(LITERAL::Decimal), "2".to_string()),
        ],
        "Malformed decimal literals must stop at a file boundary instead of consuming the next file's content"
    );
}

#[test]
fn test_cross_file_boundaries_keep_second_file_locations_real() {
    use std::fs;

    let temp_root = unique_temp_root("boundary_locations");
    fs::create_dir_all(&temp_root).expect("Should create temp lexer fixture dir");
    fs::write(temp_root.join("a.fol"), "alpha").expect("Should write first fixture file");
    fs::write(temp_root.join("b.fol"), "beta").expect("Should write second fixture file");

    let mut file_stream = FileStream::from_folder(
        temp_root
            .to_str()
            .expect("Lexer fixture folder path should be valid utf-8"),
    )
    .expect("Should create lexer stream from folder fixture");
    let mut lexer = Elements::init(&mut file_stream);

    let first = lexer
        .curr(false)
        .expect("Lexer should expose the first file token");
    assert_eq!(first.key(), KEYWORD::Identifier);
    assert_eq!(first.con(), "alpha");
    assert_eq!(first.loc().row(), 1);
    assert_eq!(first.loc().col(), 1);
    assert!(
        first
            .loc()
            .source()
            .expect("First token should carry a source path")
            .path(false)
            .ends_with("a.fol"),
        "First token should stay anchored to the first file"
    );

    lexer.bump();
    let boundary = lexer
        .curr(false)
        .expect("Lexer should expose the explicit file boundary token");
    assert_eq!(boundary.key(), KEYWORD::Void(VOID::Boundary));
    assert_eq!(boundary.loc().row(), 1);
    assert_eq!(boundary.loc().col(), 0);
    assert!(
        boundary
            .loc()
            .source()
            .expect("Boundary token should still identify the incoming file")
            .path(false)
            .ends_with("b.fol"),
        "Boundary token should point at the incoming file without pretending to be a real source character"
    );

    lexer.bump();
    let second = lexer
        .curr(false)
        .expect("Lexer should expose the second file token after the boundary");
    assert_eq!(second.key(), KEYWORD::Identifier);
    assert_eq!(second.con(), "beta");
    assert_eq!(second.loc().row(), 1);
    assert_eq!(second.loc().col(), 1);
    assert!(
        second
            .loc()
            .source()
            .expect("Second token should carry a source path")
            .path(false)
            .ends_with("b.fol"),
        "Second-file tokens must keep their real source path after the explicit boundary token"
    );

    fs::remove_dir_all(&temp_root).ok();
}
