use super::*;
use fol_parser::ast::WhenCase;

#[test]
fn test_when_flow_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_body.fol")
        .expect("Should read when flow body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if matches!(body.as_slice(), [AstNode::Literal(Literal::Integer(0))])
                                && matches!(second_body.as_slice(), [AstNode::Identifier { name, .. }] if name == "value")
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_body.fol")
        .expect("Should read if flow body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::Identifier { name, .. }] if name == "value")
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Literal(Literal::Integer(0))]))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_dollar_default_flow_body_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_dollar_default.fol")
        .expect("Should read when dollar-default fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse dollar when defaults");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { default, .. }
                        if matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Identifier { name, .. }] if name == "value"))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_assignment_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_assignment.fol")
        .expect("Should read if flow assignment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else assignment flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::Assignment { .. }])
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Assignment { .. }]))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_assignment_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_assignment.fol")
        .expect("Should read when flow assignment fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when assignment flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if matches!(body.as_slice(), [AstNode::Assignment { .. }])
                                && matches!(second_body.as_slice(), [AstNode::Assignment { .. }])
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_declaration_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_decls.fol")
        .expect("Should read if flow declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else declaration flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }] if body.len() == 2
                                && matches!(body[0], AstNode::VarDecl { .. })
                                && matches!(body[1], AstNode::VarDecl { .. })
                        )
                        && matches!(default, Some(default_body)
                            if default_body.len() == 2
                                && matches!(default_body[0], AstNode::UseDecl { .. })
                                && matches!(default_body[1], AstNode::UseDecl { .. }))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_grouped_type_bodies_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_if_flow_grouped_types.fol")
            .expect("Should read if grouped-type flow fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse grouped type flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if body.len() == 2
                                && matches!(body[0], AstNode::TypeDecl { ref name, type_def: TypeDefinition::Record { .. }, .. } if name == "Hit")
                                && matches!(body[1], AstNode::TypeDecl { ref name, type_def: TypeDefinition::Alias { .. }, .. } if name == "Miss")
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Return { .. }]))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_empty_object_type_bodies_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_if_flow_object_empty.fol")
            .expect("Should read if empty-object flow fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse empty object flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(),
                                [AstNode::TypeDecl {
                                    name,
                                    type_def: TypeDefinition::Record { fields, members, .. },
                                    ..
                                }] if name == "User" && fields.is_empty() && members.is_empty()
                            )
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Return { .. }]))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_declaration_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_decls.fol")
        .expect("Should read when flow declaration fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when declaration flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if body.len() == 2
                                && matches!(body[0], AstNode::VarDecl { .. })
                                && matches!(body[1], AstNode::VarDecl { .. })
                                && matches!(second_body.as_slice(), [AstNode::VarDecl { .. }])
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_builtin_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_builtins.fol")
        .expect("Should read if flow builtin fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else builtin flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::FunctionCall { name, .. }] if name == "report")
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::FunctionCall { name, .. }] if name == "assert"))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_builtin_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_builtins.fol")
        .expect("Should read when flow builtin fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when builtin flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if matches!(body.as_slice(), [AstNode::FunctionCall { name, .. }] if name == "check")
                                && matches!(second_body.as_slice(), [AstNode::FunctionCall { name, .. }] if name == "panic")
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_block_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_block.fol")
        .expect("Should read if flow block fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else block flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::Block { statements, .. }] if statements.len() == 2)
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Block { statements, .. }] if statements.len() == 2))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_block_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_block.fol")
        .expect("Should read when flow block fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when block flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if matches!(body.as_slice(), [AstNode::Block { statements, .. }] if statements.len() == 2)
                                && matches!(second_body.as_slice(), [AstNode::Block { statements, .. }] if statements.len() == 2)
                        )
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_control_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_control.fol")
        .expect("Should read if flow control fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse if/else control flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Loop { body, .. }
                    if body.iter().any(|loop_stmt| matches!(
                        loop_stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::Break])
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::Return { .. }]))
                    ))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_control_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_control.fol")
        .expect("Should read when flow control fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse when control flow bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::Loop { body, .. }
                    if body.iter().any(|loop_stmt| matches!(
                        loop_stmt,
                        AstNode::When { cases, .. }
                        if matches!(cases.as_slice(),
                            [
                                WhenCase::Is { body, .. },
                                WhenCase::Has { body: second_body, .. }
                            ]
                            if matches!(body.as_slice(), [AstNode::Break])
                                && matches!(second_body.as_slice(), [AstNode::Yield { .. }])
                        )
                    ))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_nested_branch_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_nested_branch.fol")
        .expect("Should read if flow nested-branch fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested branch flow bodies under if");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, default, .. }
                    if matches!(cases.as_slice(),
                        [WhenCase::Case { body, .. }]
                        if matches!(body.as_slice(), [AstNode::When { .. }])
                    )
                    && matches!(default, Some(default_body)
                        if matches!(default_body.as_slice(), [AstNode::When { .. }]))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_nested_branch_bodies_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_flow_nested_branch.fol")
            .expect("Should read when flow nested-branch fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse nested branch flow bodies under when");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, .. }
                    if matches!(cases.as_slice(),
                        [
                            WhenCase::Is { body, .. },
                            WhenCase::Has { body: second_body, .. }
                        ]
                        if matches!(body.as_slice(), [AstNode::When { .. }])
                            && matches!(second_body.as_slice(), [AstNode::When { .. }])
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_loop_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_loops.fol")
        .expect("Should read if flow loop fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop-family flow bodies under if");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, default, .. }
                    if matches!(cases.as_slice(),
                        [WhenCase::Case { body, .. }]
                        if matches!(body.as_slice(), [AstNode::Loop { .. }])
                    )
                    && matches!(default, Some(default_body)
                        if matches!(default_body.as_slice(), [AstNode::Loop { .. }]))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_loop_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_loops.fol")
        .expect("Should read when flow loop fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse loop-family flow bodies under when");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, .. }
                    if matches!(cases.as_slice(),
                        [
                            WhenCase::Is { body, .. },
                            WhenCase::Has { body: second_body, .. }
                        ]
                        if matches!(body.as_slice(), [AstNode::Loop { .. }])
                            && matches!(second_body.as_slice(), [AstNode::Loop { .. }])
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_routine_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_routines.fol")
        .expect("Should read if flow routine fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine flow bodies under if");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, default, .. }
                    if matches!(cases.as_slice(),
                        [WhenCase::Case { body, .. }]
                        if matches!(body.as_slice(), [AstNode::FunDecl { .. }])
                    )
                    && matches!(default, Some(default_body)
                        if matches!(default_body.as_slice(), [AstNode::ProDecl { .. }]))
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_routine_bodies_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_when_flow_routines.fol")
        .expect("Should read when flow routine fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse routine flow bodies under when");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, .. }
                    if matches!(cases.as_slice(),
                        [
                            WhenCase::Is { body, .. },
                            WhenCase::Has { body: second_body, .. }
                        ]
                        if matches!(body.as_slice(), [AstNode::LogDecl { .. }])
                            && matches!(second_body.as_slice(), [AstNode::FunDecl { .. }])
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_type_module_bodies_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_if_flow_type_module_decls.fol")
            .expect("Should read if flow type/module fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type/module flow bodies under if");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().filter(|stmt| matches!(stmt, AstNode::When { .. })).count() == 2
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::AliasDecl { .. }])
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::DefDecl { .. }]))
                    ))
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, default, .. }
                        if matches!(cases.as_slice(),
                            [WhenCase::Case { body, .. }]
                            if matches!(body.as_slice(), [AstNode::SegDecl { .. }])
                        )
                        && matches!(default, Some(default_body)
                            if matches!(default_body.as_slice(), [AstNode::AliasDecl { .. }]))
                    ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_when_flow_type_module_bodies_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_flow_type_module_decls.fol")
            .expect("Should read when flow type/module fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse type/module flow bodies under when");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, .. }
                    if matches!(cases.as_slice(),
                        [
                            WhenCase::Is { body, .. },
                            WhenCase::Has { body: second_body, .. }
                        ]
                        if matches!(body.as_slice(), [AstNode::TypeDecl { .. }])
                            && matches!(second_body.as_slice(), [AstNode::StdDecl { .. }])
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_if_flow_missing_body_reports_branch_message() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_flow_missing_body.fol")
        .expect("Should read missing if flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject missing if branch body");

    let parse_error = errors
        .first()
        .expect("First parser error should exist");

    assert!(
        parse_error.message.contains("Expected '{' or '=>' to start branch body"),
        "Missing if body should use generic branch-body wording, got: {}",
        parse_error.message
    );
}

#[test]
fn test_when_flow_missing_body_reports_branch_message() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_when_flow_missing_body.fol")
            .expect("Should read missing when flow-body fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject missing when branch body");

    let parse_error = errors
        .first()
        .expect("First parser error should exist");

    assert!(
        parse_error.message.contains("Expected '{' or '=>' to start branch body"),
        "Missing when body should use generic branch-body wording, got: {}",
        parse_error.message
    );
}

#[test]
fn test_if_flow_supports_availability_invoke_bodies() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_if_flow_availability_invoke.fol")
            .expect("Should read if flow availability invoke fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse availability invoke flow bodies under if");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::FunDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::When { cases, .. }
                    if matches!(cases.as_slice(),
                        [WhenCase::Case { body, .. }]
                        if matches!(body.as_slice(), [AstNode::Invoke { callee, args }]
                            if args.len() == 1 && matches!(callee.as_ref(), AstNode::AvailabilityAccess { .. }))
                    )
                ))
            )));
        }
        _ => panic!("Expected program node"),
    }
}
