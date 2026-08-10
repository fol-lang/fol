// Comprehensive tests for fol-lexer module

use fol_lexer::{
    lexer::stage3::Elements,
    lexer::{stage0, stage1, stage2},
    token::KEYWORD,
    token::*,
    Location, Source,
};
use fol_stream::FileStream;
use fol_types::SLIDER;

fn tokenize_file(path: &str) -> Vec<(KEYWORD, String)> {
    let mut file_stream =
        FileStream::from_file(path).unwrap_or_else(|_| panic!("Should be able to read {}", path));

    let mut lexer = Elements::init(&mut file_stream);
    let mut tokens = Vec::new();

    // Extract tokens until EOF
    for _ in 0..10_000 {
        match lexer.curr(false) {
            Ok(token) => {
                let keyword = token.key();
                let content = token.con().to_string();

                if keyword == KEYWORD::Void(VOID::EndFile) {
                    tokens.push((keyword, content));
                    break;
                }

                tokens.push((keyword, content));
                if lexer.bump().is_none() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    assert!(
        tokens.len() < 10_000,
        "Tokenization did not terminate for {}",
        path
    );

    tokens
}

fn tokenize_stage1_file(path: &str) -> Vec<(KEYWORD, String)> {
    let mut file_stream =
        FileStream::from_file(path).unwrap_or_else(|_| panic!("Should be able to read {}", path));
    let mut lexer = stage1::Elements::init(&mut file_stream);
    let mut tokens = Vec::new();

    let _ = lexer.bump();

    for _ in 0..10_000 {
        match lexer.curr() {
            Ok(token) => {
                let keyword = token.key().clone();
                let content = token.con().to_string();
                tokens.push((keyword.clone(), content));
                if keyword == KEYWORD::Void(VOID::EndFile) {
                    break;
                }
                if lexer.bump().is_none() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    assert!(
        tokens.len() < 10_000,
        "Stage 1 tokenization did not terminate for {}",
        path
    );

    tokens
}

fn tokenize_stage2_file(path: &str) -> Vec<(KEYWORD, String)> {
    let mut file_stream =
        FileStream::from_file(path).unwrap_or_else(|_| panic!("Should be able to read {}", path));
    let mut lexer = stage2::Elements::init(&mut file_stream);
    let mut tokens = Vec::new();

    let _ = lexer.bump();

    for _ in 0..10_000 {
        match lexer.curr(false) {
            Ok(token) => {
                let keyword = token.key().clone();
                let content = token.con().to_string();
                tokens.push((keyword.clone(), content));
                if keyword == KEYWORD::Void(VOID::EndFile) {
                    break;
                }
                if lexer.bump().is_none() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    assert!(
        tokens.len() < 10_000,
        "Stage 2 tokenization did not terminate for {}",
        path
    );

    tokens
}

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_lexer_{label}"))
}

fn tokenize_folder_contents(files: &[(&str, &str)]) -> Vec<(KEYWORD, String)> {
    use std::fs;

    let temp_root = unique_temp_root("folder");
    fs::create_dir_all(&temp_root).expect("Should create temp lexer fixture dir");
    for (relative, content) in files {
        let path = temp_root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Should create lexer fixture parent directories");
        }
        fs::write(&path, content).expect("Should write lexer fixture file");
    }

    let mut file_stream = FileStream::from_folder(
        temp_root
            .to_str()
            .expect("Lexer fixture folder path should be valid utf-8"),
    )
    .expect("Should create lexer stream from folder fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut tokens = Vec::new();

    for _ in 0..10_000 {
        let token = lexer
            .curr(false)
            .expect("Lexer should expose tokens for temp folder fixture");
        let keyword = token.key();
        let content = token.con().to_string();
        tokens.push((keyword.clone(), content));
        if keyword.is_eof() {
            break;
        }
        if lexer.bump().is_none() {
            break;
        }
    }

    fs::remove_dir_all(&temp_root).ok();
    tokens
}

#[test]
fn unique_temp_root_produces_distinct_paths_for_rapid_calls() {
    let first = unique_temp_root("collision_check");
    let second = unique_temp_root("collision_check");

    assert_ne!(first.path(), second.path());
}

#[cfg(test)]
#[path = "test_lexer_keywords.rs"]
mod lexer_keywords_tests;

#[cfg(test)]
#[path = "test_lexer_literals.rs"]
mod lexer_literals_tests;

#[cfg(test)]
#[path = "test_lexer_comments.rs"]
mod lexer_comments_tests;

#[cfg(test)]
#[path = "test_lexer_performance.rs"]
mod lexer_performance_tests;

#[cfg(test)]
#[path = "test_lexer_errors.rs"]
mod lexer_error_tests;
