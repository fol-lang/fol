use super::*;

#[test]
fn test_macro_definition_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_macro.fol")
        .expect("Should read macro definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse macro definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    params,
                    def_type: FolType::Named { name: def_kind , ..},
                    body,
                    ..
                }
                if name == "$"
                    && def_kind == "mac"
                    && params.len() == 1
                    && params[0].name == "a"
                    && matches!(params[0].param_type, FolType::Any)
                    && body.len() == 1
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_alternative_definition_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_alternative.fol")
        .expect("Should read alternative definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse alternative definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    params,
                    def_type: FolType::Named { name: def_kind , ..},
                    body,
                    ..
                }
                if name == "+var"
                    && params.is_empty()
                    && def_kind == "alt"
                    && body.len() == 1
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_default_definition_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_default.fol")
        .expect("Should read default definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default definitions");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    params,
                    def_type: FolType::Named { name: def_kind , ..},
                    body,
                    ..
                }
                if name == "str"
                    && params.is_empty()
                    && def_kind == "def[]"
                    && body.len() == 1
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_only_macro_definitions_accept_parameter_headers() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_alt_with_params.fol")
        .expect("Should read invalid parameterized alternative definition file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject parameters on non-macro definitions");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Definition parameters are currently supported only for mac definitions"),
        "Non-macro parameterized defs should report the parameter restriction, got: {}",
        parse_error.message
    );
}

#[test]
fn test_macro_definitions_allow_overloads_by_parameter_type() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_macro_overloads.fol")
        .expect("Should read macro overload definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve overloaded macro definitions");

    match ast {
        AstNode::Program { declarations } => {
            let overloads = declarations
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        AstNode::DefDecl {
                            name,
                            def_type: FolType::Named { name: kind , ..},
                            ..
                        } if name == "!" && kind == "mac"
                    )
                })
                .count();
            assert_eq!(
                overloads, 2,
                "Expected both macro overloads to be preserved"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_macro_definitions_accept_grouped_parameters() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_macro_grouped_params.fol")
        .expect("Should read grouped macro parameter definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse grouped macro parameter headers");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    params,
                    def_type: FolType::Named { name: def_kind , ..},
                    ..
                }
                if name == "swap"
                    && def_kind == "mac"
                    && params.len() == 2
                    && params[0].name == "left"
                    && params[1].name == "right"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_macro_definitions_accept_semicolon_parameter_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_def_macro_semicolon_params.fol")
            .expect("Should read semicolon macro parameter definition test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated macro parameters");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl {
                    name,
                    params,
                    def_type: FolType::Named { name: def_kind , ..},
                    ..
                }
                if name == "pair"
                    && def_kind == "mac"
                    && params.len() == 2
                    && params[0].name == "left"
                    && params[1].name == "right"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_meta_definitions_parse_inside_routine_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_nested_meta_defs.fol")
        .expect("Should read nested meta-definition routine fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve meta definitions inside routine bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { name, body, .. }
                if name == "build"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, def_type: FolType::Named { name: kind , ..}, .. } if name == "$" && kind == "mac"))
                    && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, def_type: FolType::Named { name: kind , ..}, .. } if name == "+var" && kind == "alt"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_meta_definitions_parse_inside_definition_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_def_module_meta_defs.fol")
        .expect("Should read nested meta-definition module fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should preserve meta definitions inside definition bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl { name, def_type: FolType::Module { .. }, body, .. }
                if name == "meta"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, def_type: FolType::Named { name: kind , ..}, .. } if name == "$" && kind == "mac"))
                    && body.iter().any(|stmt| matches!(stmt, AstNode::DefDecl { name, def_type: FolType::Named { name: kind , ..}, .. } if name == "str" && kind == "def[]"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
