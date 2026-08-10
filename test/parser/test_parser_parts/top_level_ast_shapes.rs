use super::*;

fn parse_program_declarations(path: &str) -> Vec<AstNode> {
    let mut file_stream = FileStream::from_file(path).expect("Should read parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should produce a program AST for the fixture");

    match ast {
        AstNode::Program { declarations } => declarations,
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_fun_stays_a_single_root_declaration() {
    let declarations = parse_program_declarations("test/parser/simple_fun.fol");

    assert_eq!(
        declarations.len(),
        1,
        "Top-level function fixture should no longer leak body statements into Program.declarations"
    );
    assert!(
        matches!(
            &declarations[0],
            AstNode::FunDecl { name, body, .. } if name == "add" && body.len() == 1
        ),
        "The only root node should be the authoritative function declaration"
    );
}

#[test]
fn test_top_level_pro_stays_a_single_root_declaration() {
    let declarations = parse_program_declarations("test/parser/simple_pro.fol");

    assert_eq!(
        declarations.len(),
        1,
        "Top-level procedure fixture should no longer leak body statements into Program.declarations"
    );
    assert!(
        matches!(
            &declarations[0],
            AstNode::ProDecl { name, body, .. } if name == "update" && body.len() == 2
        ),
        "The only root node should be the authoritative procedure declaration"
    );
}

#[test]
fn test_top_level_log_stays_a_single_root_declaration() {
    let declarations = parse_program_declarations("test/parser/simple_log.fol");

    assert_eq!(
        declarations.len(),
        1,
        "Top-level logical fixture should no longer leak body statements into Program.declarations"
    );
    assert!(
        matches!(
            &declarations[0],
            AstNode::LogDecl { name, body, .. } if name == "dating" && body.len() == 1
        ),
        "The only root node should be the authoritative logical declaration"
    );
}

#[test]
fn test_type_member_routines_stay_nested_and_do_not_leak_to_root() {
    let declarations = parse_program_declarations("test/parser/simple_typ_record_methods.fol");

    assert_eq!(
        declarations.len(),
        1,
        "Type member routines should not leak additional top-level nodes into the program root"
    );

    match &declarations[0] {
        AstNode::TypeDecl {
            name,
            type_def:
                TypeDefinition::Record {
                    fields, members, ..
                },
            ..
        } => {
            assert_eq!(name, "Computer", "Fixture should parse the Computer type");
            assert!(
                matches!(fields.get("brand"), Some(FolType::Named { name, .. }) if name == "str"),
                "Record field should remain on the type definition"
            );
            assert_eq!(members.len(), 3, "All three routine members should stay nested");
            assert!(members.iter().any(|member| matches!(
                member,
                AstNode::FunDecl { name, .. } if name == "getBrand"
            )));
            assert!(members.iter().any(|member| matches!(
                member,
                AstNode::ProDecl { name, .. } if name == "reset"
            )));
            assert!(members.iter().any(|member| matches!(
                member,
                AstNode::LogDecl { name, .. } if name == "ready"
            )));
        }
        other => panic!("Expected record type declaration, got {:?}", other),
    }
}

#[test]
fn test_top_level_var_declaration_stays_a_single_root_node() {
    let declarations = parse_program_declarations("test/parser/simple_var.fol");

    assert_eq!(
        declarations.len(),
        1,
        "A simple top-level variable declaration should not synthesize extra root nodes"
    );
    assert!(
        matches!(
            &declarations[0],
            AstNode::VarDecl {
                name,
                type_hint: Some(FolType::Int { .. }),
                value: Some(value),
                ..
            } if name == "x" && matches!(value.as_ref(), AstNode::Literal(Literal::Integer(42)))
        ),
        "Top-level variable fixture should stay as a single VarDecl with its literal initializer"
    );
}

#[test]
fn test_mixed_root_surface_keeps_declarations_statements_and_literals_in_order() {
    let declarations = parse_program_declarations("test/parser/simple_mixed_root_surface.fol");

    assert_eq!(
        declarations.len(),
        5,
        "Mixed file-scope fixture should keep all accepted root nodes in Program.declarations"
    );
    assert!(
        matches!(
            &declarations[0],
            AstNode::VarDecl {
                name,
                type_hint: Some(FolType::Int { .. }),
                value: Some(value),
                ..
            } if name == "answer"
                && matches!(value.as_ref(), AstNode::Literal(Literal::Integer(41)))
        ),
        "First root node should remain the variable declaration"
    );
    assert!(
        matches!(
            &declarations[1],
            AstNode::FunctionCall { name, args, .. } if name == "notify" && args.is_empty()
        ),
        "Second root node should remain the top-level function call"
    );
    assert!(
        matches!(
            &declarations[2],
            AstNode::Assignment { target, value }
                if matches!(target.as_ref(), AstNode::Identifier { name, .. } if name == "answer")
                    && matches!(value.as_ref(), AstNode::Literal(Literal::Integer(42)))
        ),
        "Third root node should remain the top-level assignment"
    );
    assert!(
        matches!(
            &declarations[3],
            AstNode::Literal(Literal::String(value)) if value == "done"
        ),
        "Fourth root node should remain the top-level literal expression"
    );
    assert!(
        matches!(
            &declarations[4],
            AstNode::ProDecl { name, body, .. }
                if name == "main"
                    && matches!(body.as_slice(), [AstNode::Return { value: None, .. }])
        ),
        "Final root node should remain the routine declaration with its body nested"
    );
}
