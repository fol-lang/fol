use super::*;

fn contains_node(node: &AstNode, predicate: fn(&AstNode) -> bool) -> bool {
    predicate(node)
        || node
            .children()
            .into_iter()
            .any(|child| contains_node(child, predicate))
}

#[test]
fn test_book_eventual_examples_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_book_eventual_examples.fol")
        .expect("Should read book eventual examples fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept book eventual examples");

    match ast {
        AstNode::Program { declarations } => {
            let mut saw_async = false;
            let mut saw_await = false;
            for node in declarations {
                match node {
                    AstNode::ProDecl { body, .. } | AstNode::FunDecl { body, .. } => {
                        for stmt in body {
                            saw_async |=
                                contains_node(&stmt, |node| matches!(node, AstNode::AsyncStage));
                            saw_await |=
                                contains_node(&stmt, |node| matches!(node, AstNode::AwaitStage));
                        }
                    }
                    _ => {}
                }
            }

            assert!(saw_async, "Expected async stage from book eventual example");
            assert!(saw_await, "Expected await stage from book eventual example");
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_book_coroutine_examples_parse() {
    let mut file_stream = FileStream::from_file("test/parser/simple_book_coroutine_examples.fol")
        .expect("Should read book coroutine examples fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept book coroutine examples");

    match ast {
        AstNode::Program { declarations } => {
            let mut saw_spawn = false;
            let mut saw_select = false;
            let mut saw_mutex = false;

            for node in declarations {
                match node {
                    AstNode::FunDecl { params, body, .. }
                    | AstNode::ProDecl { params, body, .. } => {
                        saw_mutex |= params.iter().any(|param| param.is_mutex);
                        for stmt in body {
                            saw_spawn |=
                                contains_node(&stmt, |node| matches!(node, AstNode::Spawn { .. }));
                            saw_select |=
                                contains_node(&stmt, |node| matches!(node, AstNode::Select { .. }));
                        }
                    }
                    _ => {}
                }
            }

            assert!(
                saw_spawn,
                "Expected coroutine spawn from book coroutine example"
            );
            assert!(
                saw_select,
                "Expected select statement from book coroutine example"
            );
            assert!(
                saw_mutex,
                "Expected mutex parameter from book coroutine example"
            );
        }
        _ => panic!("Expected program node"),
    }
}
