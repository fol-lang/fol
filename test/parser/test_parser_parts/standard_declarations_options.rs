use super::*;
use fol_parser::ast::{DeclOption, StandardKind, VarOption};

#[test]
fn test_standard_declaration_parsing_inside_function_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_std_protocol.fol")
        .expect("Should read nested standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse standards inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "build"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::StdDecl { name, kind: StandardKind::Protocol, .. } if name == "geometry"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_standard_declaration_accepts_empty_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_empty_options.fol")
        .expect("Should read std[] test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse std declarations with empty options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(node, AstNode::StdDecl { name, .. } if name == "geometry")
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_accepts_kind_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_protocol_kind_options.fol")
        .expect("Should read pro[] standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse pro[] standard declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl {
                        name,
                        kind: StandardKind::Protocol,
                        kind_options,
                        ..
                    }
                    if name == "geometry" && kind_options == &vec![DeclOption::Export]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_empty_kind_brackets() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_blueprint_kind_options.fol")
            .expect("Should read blu[] standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse blu[] standard declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl {
                        name,
                        kind: StandardKind::Blueprint,
                        kind_options,
                        ..
                    }
                    if name == "geometry" && kind_options == &vec![DeclOption::Hidden]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_const_fields() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_blueprint_const_field.fol")
        .expect("Should read const blueprint standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse const fields in blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "theme" && options.contains(&VarOption::Immutable)))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_blueprint_standard_accepts_field_alternatives() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_blueprint_field_alternatives.fol")
            .expect("Should read blueprint field alternative fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding alternatives in blueprint standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "theme" && options.contains(&VarOption::Export)))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "status" && options.contains(&VarOption::Hidden)))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_empty_kind_brackets() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_kind_options.fol")
        .expect("Should read ext[] standard test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse ext[] standard declarations");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl {
                        name,
                        kind: StandardKind::Extended,
                        kind_options,
                        ..
                    }
                    if name == "geometry" && kind_options == &vec![DeclOption::Normal]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_const_fields() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_extended_const_field.fol")
        .expect("Should read const extended standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse const members in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "theme" && options.contains(&VarOption::Immutable)))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, .. } if name == "area"))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standard_accepts_field_alternatives() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_extended_field_alternatives.fol")
            .expect("Should read extended field alternative fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse binding alternatives in extended standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
                    if name == "geometry"
                        && body.iter().any(|stmt| matches!(stmt, AstNode::FunDecl { name, .. } if name == "area"))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "theme" && options.contains(&VarOption::Export)))
                        && body.iter().any(|stmt| matches!(stmt, AstNode::VarDecl { name, options, .. } if name == "status" && options.contains(&VarOption::Hidden)))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_protocol_standard_rejects_duplicate_signatures() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_protocol_duplicate_signature.fol")
            .expect("Should read duplicate protocol standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate protocol signatures");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate standard member 'area#0'"),
        "Expected duplicate standard signature error, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Duplicate protocol signature should point to the duplicate routine name"
    );
}

#[test]
fn test_protocol_standard_rejects_canonical_duplicate_signatures() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_protocol_duplicate_signature_canonical.fol")
            .expect("Should read canonical duplicate protocol standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate protocol signatures");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate standard member 'AreaValue#0'"),
        "Expected canonical duplicate protocol signature error, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Canonical duplicate protocol signature should point to the duplicate routine name"
    );
}

#[test]
fn test_blueprint_standard_rejects_duplicate_fields() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_blueprint_duplicate_field.fol")
            .expect("Should read duplicate blueprint standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate blueprint fields");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate standard member 'color'"),
        "Expected duplicate blueprint member error, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Duplicate blueprint field should point to the duplicate field name"
    );
}

#[test]
fn test_blueprint_standard_rejects_canonical_duplicate_fields() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_blueprint_duplicate_field_canonical.fol")
            .expect("Should read canonical duplicate blueprint standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject canonical duplicate blueprint fields");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate standard member 'ColorName'"),
        "Expected canonical duplicate blueprint member error, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Canonical duplicate blueprint field should point to the duplicate field name"
    );
}

#[test]
fn test_extended_standard_rejects_duplicate_members() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_std_extended_duplicate_member.fol")
            .expect("Should read duplicate extended standard fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject duplicate extended members");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Duplicate standard member 'area#0'"),
        "Expected duplicate extended member error, got: {}",
        parse_error.message
    );
    assert_eq!(
        parse_error.primary_location().unwrap().column,
        9,
        "Duplicate extended member should point to the duplicate member name"
    );
}

#[test]
fn test_standard_rejects_unknown_declaration_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_unknown_options.fol")
        .expect("Should read malformed std[] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unknown std declaration options");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error.message.contains("Unknown standard option"),
        "Expected std option error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_standard_rejects_unknown_kind_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_unknown_kind_options.fol")
        .expect("Should read malformed standard kind fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject unknown standard kind options");

    let parse_error = errors
        .first()
        .expect("First parser error should be ParseError");

    assert!(
        parse_error
            .message
            .contains("Unknown protocol standard kind option"),
        "Expected standard kind option error, got: {}",
        parse_error.message
    );
}

#[test]
fn test_extended_standards_accept_grouped_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_grouped_types.fol")
        .expect("Should read grouped standard-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should flatten grouped type members inside standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, body, .. }
                if name == "Shapes"
                    && body.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl { name, type_def: TypeDefinition::Record { .. }, .. }
                        if name == "Inner"
                    ))
                    && body.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Alias { target: FolType::Named { name: target , ..} },
                            ..
                        } if name == "Label" && target == "str"
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_extended_standards_accept_empty_object_type_members() {
    let mut file_stream = FileStream::from_file("test/parser/simple_std_object_member_empty.fol")
        .expect("Should read empty object standard-member fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should retain empty object type members inside standards");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, body, .. }
                if name == "Shapes"
                    && body.iter().any(|member| matches!(
                        member,
                        AstNode::TypeDecl {
                            name,
                            type_def: TypeDefinition::Record { fields, members, .. },
                            ..
                        } if name == "Inner" && fields.is_empty() && members.is_empty()
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
