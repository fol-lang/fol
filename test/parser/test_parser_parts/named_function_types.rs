use super::*;

#[test]
fn test_function_type_references_accept_named_headers() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_named_function_types.fol")
        .expect("Should read named function-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept plain, keyword, and quoted function-type names");

    match ast {
        AstNode::Program { declarations } => {
            let count = declarations
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { params, .. }
                        if matches!(
                            params.as_slice(),
                            [Parameter {
                                param_type: FolType::Function { params, .. },
                                ..
                            }] if matches!(params.as_slice(), [FolType::Int { .. }])
                        )
                    )
                })
                .count();
            assert_eq!(
                count, 3,
                "All named function-type declarations should parse"
            );
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_function_type_references_accept_single_quoted_names() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_single_quoted_named_function_types.fol")
            .expect("Should read single-quoted named function-type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept single-quoted function-type names");

    match ast {
        AstNode::Program { declarations } => {
            let count = declarations
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        AstNode::FunDecl { params, .. }
                        if matches!(
                            params.as_slice(),
                            [Parameter {
                                param_type: FolType::Function { params, .. },
                                ..
                            }] if matches!(params.as_slice(), [FolType::Int { .. }])
                        )
                    )
                })
                .count();
            assert_eq!(count, 1, "Single-quoted named function types should parse");
        }
        _ => panic!("Expected program node"),
    }
}
