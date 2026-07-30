use super::*;
use std::fs;

fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
    crate::fixture::TempFixture::new(&format!("fol_quoted_params_{label}"))
}

#[test]
fn test_routine_declarations_accept_quoted_parameters() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_quoted_params.fol")
        .expect("Should read quoted parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted routine parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { params, .. }
                    if params.len() == 2 && params[0].name == "left" && params[1].name == "right"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_function_type_references_accept_quoted_parameters() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_function_type_quoted_params.fol")
            .expect("Should read quoted function-type parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept quoted parameters in function types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { params, .. }
                    if matches!(
                        params.as_slice(),
                        [Parameter {
                            param_type: FolType::Function { params, .. },
                            ..
                        }] if matches!(params.as_slice(), [FolType::Int { .. }, FolType::Named { name, .. }] if name == "str")
                    )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_single_quoted_parameters_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_single_quoted_params.fol")
        .expect("Should read single-quoted parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, params, .. }
                    if name == "demo"
                        && params.len() == 2
                        && params[0].name == "left"
                        && params[1].name == "right"
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, params, .. }
                    if name == "takes"
                        && matches!(
                            params.as_slice(),
                            [Parameter {
                                param_type: FolType::Function { params, .. },
                                ..
                            }] if matches!(params.as_slice(), [FolType::Int { .. }, FolType::Named { name, .. }] if name == "str")
                        )
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_grouped_quoted_parameters_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_grouped_quoted_params.fol")
        .expect("Should read grouped quoted-parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept grouped quoted parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { params, .. }
                    if params.len() == 2
                        && params[0].name == "left"
                        && params[1].name == "right"
                        && matches!(params[0].param_type, FolType::Int { .. })
                        && matches!(params[1].param_type, FolType::Int { .. })
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_routine_parameters_preserve_inner_opposite_quote_chars() {
    let temp_root = unique_temp_root("inner_quotes");
    fs::create_dir_all(&temp_root).expect("Should create temp parameter fixture dir");
    let fixture = temp_root.join("inner_quotes.fol");
    fs::write(
        &fixture,
        "fun demo(\"le'ft\": int, 'ri\"ght': int): int = {\n    return 0;\n};\n",
    )
    .expect("Should write temp parameter fixture");

    let mut file_stream = FileStream::from_file(
        fixture
            .to_str()
            .expect("Parameter fixture path should be UTF-8"),
    )
    .expect("Should read temp parameter fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve inner opposite-family quotes in parameter names");

    fs::remove_dir_all(&temp_root).ok();

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, params, .. }
                        if name == "demo"
                            && params.len() == 2
                            && params[0].name == "le'ft"
                            && params[1].name == "ri\"ght"
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
