use super::*;
use fol_parser::ast::DeclOption;

#[test]
fn test_declarations_accept_semicolon_visibility_option_separators() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_decl_visibility_options_semicolon.fol")
            .expect("Should read semicolon declaration-option fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse semicolon-separated declaration options");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::StdDecl { name, options, .. }
                if name == "geometry" && options == &vec![DeclOption::Export]
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::DefDecl { name, options, .. }
                if name == "mark" && options == &vec![DeclOption::Export]
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::SegDecl { name, options, .. }
                if name == "core" && options == &vec![DeclOption::Export]
            )));
        }
        _ => panic!("Expected program node"),
    }
}
