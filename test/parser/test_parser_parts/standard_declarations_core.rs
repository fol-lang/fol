use super::*;
use fol_parser::ast::{DeclOption, StandardKind, VarOption};

#[test]
fn test_protocol_standard_declaration_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol.fol")
        .expect("Should read protocol standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                    if name == "geometry"
                        && body.len() == 2
                        && matches!(&body[0], AstNode::FunDecl { name, body, .. } if name == "area" && body.is_empty())
                        && matches!(&body[1], AstNode::FunDecl { name, body, .. } if name == "perim" && body.is_empty())
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_default_function_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_default_fun.fol")
        .expect("Should read protocol default-function standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default function implementations in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                    if name == "geometry"
                        && body.len() == 2
                        && matches!(&body[0], AstNode::FunDecl { name, body, .. } if name == "area" && !body.is_empty())
                        && matches!(&body[1], AstNode::FunDecl { name, body, .. } if name == "perim" && body.is_empty())
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_alias_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_alias.fol")
        .expect("Should read protocol alias-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse alias members in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::AliasDecl { name, .. } if name == "Area"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, return_type: Some(FolType::Named { name: ret , ..}), .. } if name == "area" && ret == "Area"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_type.fol")
        .expect("Should read protocol type-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type members in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::TypeDecl { name, .. } if name == "Area"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, return_type: Some(FolType::Named { name: ret , ..}), .. } if name == "area" && ret == "Area"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_constant_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_const.fol")
        .expect("Should read protocol constant-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse constant members in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "epsilon" && options.contains(&VarOption::Immutable)))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, .. } if name == "area"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_standard_routine_members_reject_capture_lists() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_captures.fol")
        .expect("Should read standard routine capture fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject capture lists on standard routine members");
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("Standard routine requirements cannot declare captures")),
        "capturing required routines should fail at parse time, got: {errors:?}"
    );
}

#[test]
fn test_protocol_standard_accepts_default_procedure_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_default_pro.fol")
        .expect("Should read protocol default-procedure standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default procedure implementations in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                if name == "geometry"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::ProDecl { name, body, .. } if name == "reset" && !body.is_empty()))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_default_logical_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_default_log.fol")
        .expect("Should read protocol default-logical standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default logical implementations in protocol standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
                if name == "geometry"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::LogDecl { name, body, .. } if name == "valid" && !body.is_empty()))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_declaration_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_blueprint.fol")
        .expect("Should read blueprint standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.len() == 2
                        && matches!(&body[0], AstNode::VarDecl { name, .. } if name == "color")
                        && matches!(&body[1], AstNode::VarDecl { name, .. } if name == "size")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_routine_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_blueprint_methods.fol")
        .expect("Should read blueprint routine-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine members in blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, .. } if name == "color"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, body, .. } if name == "describe" && !body.is_empty()))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::ProDecl { name, body, .. } if name == "reset" && body.is_empty()))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_alias_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_blueprint_alias.fol")
        .expect("Should read blueprint alias-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse alias members in blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::AliasDecl { name, .. } if name == "Color"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: typ , ..}), .. } if name == "color" && typ == "Color"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_blueprint_type.fol")
        .expect("Should read blueprint type-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type members in blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::TypeDecl { name, .. } if name == "Color"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: typ , ..}), .. } if name == "color" && typ == "Color"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_declaration_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended.fol")
        .expect("Should read extended standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                    if name == "geometry"
                        && body.len() == 2
                        && matches!(&body[0], AstNode::FunDecl { name, .. } if name == "area")
                        && matches!(&body[1], AstNode::VarDecl { name, .. } if name == "color")
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_alias_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_alias.fol")
        .expect("Should read extended alias-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse alias members in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::AliasDecl { name, .. } if name == "Color"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: typ , ..}), .. } if name == "color" && typ == "Color"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_type.fol")
        .expect("Should read extended type-member standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type members in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::TypeDecl { name, .. } if name == "Color"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, type_hint: Some(FolType::Named { name: typ , ..}), .. } if name == "color" && typ == "Color"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_default_function_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_default_fun.fol")
        .expect("Should read extended default-function standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default function implementations in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                if name == "geometry"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, body, .. } if name == "area" && !body.is_empty()))
                    && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, .. } if name == "color"))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_default_procedure_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_default_pro.fol")
        .expect("Should read extended default-procedure standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default procedure implementations in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                if name == "geometry"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::ProDecl { name, body, .. } if name == "reset" && !body.is_empty()))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_default_logical_implementations() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_default_log.fol")
        .expect("Should read extended default-logical standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse default logical implementations in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                if name == "geometry"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::LogDecl { name, body, .. } if name == "valid" && !body.is_empty()))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_standard_declaration_visibility_options_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_visibility_options.fol")
        .expect("Should read visibility-option standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse standard visibility options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, options, .. }
                if name == "geometry" && options == &vec![DeclOption::Export]
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_standard_declaration_rejects_conflicting_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_conflicting_options.fol")
        .expect("Should read conflicting standard option test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject conflicting standard options");

    let parse_error = errors
        .first()
        
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Conflicting standard visibility options"),
        "Conflicting std options should report a targeted error, got: {}",
        parse_error.message
    );
}
