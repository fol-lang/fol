use super::*;
use fol_parser::ast::LoopCondition;

#[test]
fn test_function_while_loop_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_while.fol")
        .expect("Should read function while-loop fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse while loops in function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, body, .. }
                    if name == "spin"
                        && body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Loop {
                                condition,
                                body,
                                ..
                            }
                            if matches!(condition.as_ref(), LoopCondition::Condition(expr) if matches!(expr.as_ref(), AstNode::Identifier { name, .. } if name == "flag"))
                                && body.iter().any(|node| matches!(node, AstNode::Break))
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_top_level_while_loop_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_while_top_level.fol")
        .expect("Should read top-level while-loop fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse top-level while loops");

    let loop_stmt = match ast {
        AstNode::Program { declarations } => declarations
            .into_iter()
            .find(|node| matches!(node, AstNode::Loop { .. }))
            .expect("Expected top-level while loop"),
        _ => panic!("Expected program node"),
    };

    match loop_stmt {
        AstNode::Loop {
            syntax_id,
            condition,
            body,
        } => {
            assert!(syntax_id.is_some(), "loop keyword should retain a scope anchor");
            assert!(matches!(
                condition.as_ref(),
                LoopCondition::Condition(expr)
                if matches!(expr.as_ref(), AstNode::Identifier { name, .. } if name == "ready")
            ));
            assert!(body.iter().any(|node| matches!(node, AstNode::Yield { .. })));
            assert!(body.iter().any(|node| matches!(node, AstNode::Break)));
        }
        _ => panic!("Expected loop node"),
    }
}
