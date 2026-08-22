use super::*;

#[test]
fn test_routine_option_brackets_accept_semicolon_separators() {
    let mut file_stream = FileStream::from_file("test/parser/simple_routine_options_semicolon.fol")
        .expect("Should read semicolon routine options test file");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated routine options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { name, options, .. }
                    if name == "pure_add"
                        && options == &vec![fol_parser::ast::FunOption::Mutable]
                )
            }));
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { name, options, .. }
                    if name == "publish"
                        && options
                            == &vec![
                                fol_parser::ast::FunOption::Export,
                                fol_parser::ast::FunOption::Iterator
                            ]
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
