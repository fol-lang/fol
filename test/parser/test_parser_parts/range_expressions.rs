use super::*;

#[test]
fn test_triple_dot_ranges_parse_as_non_inclusive() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_triple_dot_ranges.fol")
        .expect("Should read triple-dot range fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept triple-dot ranges");

    match ast {
        AstNode::Program { declarations } => {
            let ranges: Vec<&AstNode> = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .filter_map(|node| match node {
                    AstNode::Assignment { value, .. } => Some(value.as_ref()),
                    AstNode::Return {
                        value: Some(value), ..
                    } => Some(value.as_ref()),
                    _ => None,
                })
                .collect();

            assert!(ranges.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Range {
                        start: Some(_),
                        end: None,
                        inclusive: false
                    }
                )
            }));
            assert!(ranges.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Range {
                        start: None,
                        end: Some(_),
                        inclusive: false
                    }
                )
            }));
            assert!(ranges.iter().any(|node| {
                matches!(
                    node,
                    AstNode::Range {
                        start: Some(_),
                        end: Some(_),
                        inclusive: false
                    }
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_semicolon_braced_ranges_parse_as_ranges() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_braced_range_expr_semicolon.fol")
            .expect("Should read semicolon braced-range fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept semicolon braced ranges");

    match ast {
        AstNode::Program { declarations } => {
            let ranges: Vec<&AstNode> = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .filter_map(|node| match node {
                    AstNode::Assignment { value, .. } => Some(value.as_ref()),
                    AstNode::Return {
                        value: Some(value), ..
                    } => Some(value.as_ref()),
                    _ => None,
                })
                .collect();

            assert_eq!(ranges.len(), 2);
            assert!(ranges
                .iter()
                .all(|node| matches!(node, AstNode::Range { .. })));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_trailing_separator_braced_ranges_parse_as_ranges() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_braced_range_expr_trailing_separator.fol")
            .expect("Should read trailing-separator braced-range fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept trailing-separator braced ranges");

    match ast {
        AstNode::Program { declarations } => {
            let ranges: Vec<&AstNode> = only_root_routine_body_nodes(&declarations)
                .into_iter()
                .filter_map(|node| match node {
                    AstNode::Assignment { value, .. } => Some(value.as_ref()),
                    AstNode::Return {
                        value: Some(value), ..
                    } => Some(value.as_ref()),
                    _ => None,
                })
                .collect();

            assert_eq!(ranges.len(), 2);
            assert!(ranges
                .iter()
                .all(|node| matches!(node, AstNode::Range { .. })));
        }
        _ => panic!("Expected program node"),
    }
}
