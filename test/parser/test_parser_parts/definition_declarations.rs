use super::*;
use fol_parser::ast::DeclOption;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_definition_decl_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_top_level_def_module_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_module.fol")
        .expect("Should read module definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse module definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Module { name: module_name },
                            body,
                            ..
                        }
                        if name == "argo"
                            && module_name == "init"
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "name"
                            ))
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::FunDecl { name, .. } if name == "add"
                            ))
                    )
                }),
                "Program should include parsed module definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_def_bare_module_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_bare_module.fol")
        .expect("Should read bare-module definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bare-module definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Module { name: module_name },
                            body,
                            ..
                        }
                        if name == "argo"
                            && module_name.is_empty()
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "name"
                            ))
                    )
                }),
                "Program should include parsed bare-module definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_def_block_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_block.fol")
        .expect("Should read block definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "marker"
                            && block_name.is_empty()
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "count"
                            ))
                    )
                }),
                "Program should include parsed block definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_bare_block_marker_without_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_bare_block_marker.fol")
        .expect("Should read bare block-marker definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse bare block marker definitions without bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed bare empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_def_module_parsing_with_trailing_semicolon() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_module_with_semi.fol")
        .expect("Should read semicolon-terminated module definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-terminated module definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Module { name: module_name },
                            body,
                            ..
                        }
                        if name == "argo"
                            && module_name == "init"
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "name"
                            ))
                    )
                }),
                "Program should include parsed semicolon-terminated module definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_with_quoted_name_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_quoted_name.fol")
        .expect("Should read quoted-name definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted definition names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "startup block"
                            && block_name.is_empty()
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "ready"
                            ))
                    )
                }),
                "Program should include parsed quoted-name block definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_with_keyword_name_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_keyword_name.fol")
        .expect("Should read keyword-name definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse builtin-token definition names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Block { name: block_name },
                        body,
                        ..
                    }
                    if name == "log" && block_name.is_empty() && body.is_empty()
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_with_single_quoted_name_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_single_quoted_name.fol")
        .expect("Should read single-quoted definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse single-quoted definition names");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "startup block"
                            && block_name.is_empty()
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::VarDecl { name, .. } if name == "ready"
                            ))
                    )
                }),
                "Program should include parsed single-quoted block definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_names_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp definition fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "def \"start 'up'\": blk = {};\ndef 'stop \"now\"': blk = {};\n",
    )
    .expect("Should write temp definition fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Definition fixture path should be UTF-8"),
    )
    .expect("Should read temp definition fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in def names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Block { name: block_name },
                        ..
                    } if name == "start 'up'" && block_name.is_empty()
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::DefDecl {
                        name,
                        def_type: FolType::Block { name: block_name },
                        ..
                    } if name == "stop \"now\"" && block_name.is_empty()
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_with_empty_option_brackets_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_empty_options.fol")
        .expect("Should read empty-options definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept empty def option brackets");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    def_type: FolType::Block { name: block_name },
                    body,
                    ..
                }
                if name == "mark" && block_name.is_empty() && body.is_empty()
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_rejects_non_empty_option_brackets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_unknown_option.fol")
        .expect("Should read invalid def option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject non-empty def options");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let message = parse_error.message.clone();
    assert!(
        message.contains("Unknown definition option"),
        "Non-empty def option brackets should be rejected, got: {}",
        message
    );
}

#[test]
fn test_def_visibility_options_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_visibility_options.fol")
        .expect("Should read visibility-option definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse definition visibility options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl { name, options, .. }
                if name == "mark" && options == &vec![DeclOption::Export]
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_block_marker_without_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_block_marker.fol")
        .expect("Should read block-marker definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block marker definitions without bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_quoted_block_marker_without_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_quoted_block_marker.fol")
        .expect("Should read quoted block-marker definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted block marker definitions without bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "jump mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed quoted empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_block_marker_without_body_parsing_with_trailing_semicolon() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_def_block_marker_with_semi.fol")
            .expect("Should read block-marker-with-semicolon test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse block marker definitions with trailing semicolons");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed semicolon-terminated empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_single_quoted_block_marker_without_body_parsing_with_trailing_semicolon() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_def_single_quoted_block_marker.fol")
            .expect("Should read single-quoted block-marker test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse single-quoted block marker definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "jump mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed single-quoted empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_quoted_block_marker_without_body_parsing_with_trailing_semicolon() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_def_quoted_block_marker_with_semi.fol")
            .expect("Should read quoted block-marker-with-semicolon test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse quoted block marker definitions with trailing semicolons");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Block { name: block_name },
                            body,
                            ..
                        }
                        if name == "jump mark" && block_name.is_empty() && body.is_empty()
                    )
                }),
                "Program should include parsed semicolon-terminated quoted empty-body block marker definition"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_bodies_support_nested_definitions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_nested_defs.fol")
        .expect("Should read nested definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested definitions inside def bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Module { name: module_name },
                            body,
                            ..
                        }
                        if name == "outer"
                            && module_name == "root"
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::DefDecl {
                                    name,
                                    def_type: FolType::Block { name: block_name },
                                    body,
                                    ..
                                }
                                if name == "inner"
                                    && block_name.is_empty()
                                    && body.iter().any(|nested| matches!(
                                        nested,
                                        AstNode::VarDecl { name, .. } if name == "count"
                                    ))
                            ))
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::DefDecl {
                                    name,
                                    def_type: FolType::Block { name: block_name },
                                    body,
                                    ..
                                }
                                if name == "jump mark" && block_name.is_empty() && body.is_empty()
                            ))
                    )
                }),
                "Module def bodies should preserve nested block definitions"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_regular_blocks_support_nested_definitions() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_nested_def_block.fol")
        .expect("Should read nested def-in-block test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested definitions inside regular blocks");

    match ast {
        AstNode::Program { declarations } => {
            assert!(
                declarations.iter().any(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { name, body, .. }
                        if name == "build"
                            && body.iter().any(|stmt| matches!(
                                stmt,
                                AstNode::Block { statements, .. }
                                if statements.iter().any(|nested| matches!(
                                    nested,
                                    AstNode::DefDecl {
                                        name,
                                        def_type: FolType::Block { name: block_name },
                                        body,
                                        ..
                                    }
                                    if name == "helper" && block_name.is_empty() && body.is_empty()
                                ))
                            ))
                    )
                }),
                "Regular nested blocks should preserve nested def declarations"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_def_invalid_type_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_invalid_type.fol")
        .expect("Should read malformed def fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject definitions with non block/module types");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains(
            "Definition declarations currently support only mod[...], blk[...], tst[...], pkg, loc, mac, alt, or def[] types, found"
        ) && first_message.contains("Int"),
        "Invalid definition type should report the supported def target kinds, got: {}",
        first_message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().line,
        1,
        "Invalid def type parse error should stay on the declaration line"
    );
    assert!(
        parse_error.primary_location().unwrap().column >= 13,
        "Invalid def type parse error should anchor at the type site, got column {}",
        parse_error.primary_location().unwrap().column
    );
}

#[test]
fn test_def_module_without_body_reports_parse_error() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_module_missing_body.fol")
        .expect("Should read module-missing-body def fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject module definitions without bodies");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    let first_message = parse_error.message.clone();
    assert!(
        first_message.contains("Expected '=' before definition body"),
        "Module definitions without bodies should keep requiring '=', got: {}",
        first_message
    );
}
