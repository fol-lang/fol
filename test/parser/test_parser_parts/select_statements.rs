use super::*;

#[test]
fn test_select_statement_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_select_stmt.fol")
        .expect("Should read select statement fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should parse select statements");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::ProDecl { body, .. }
                        if body.iter().any(|stmt| matches!(
                            stmt,
                            AstNode::Select { arms, default }
                                if arms.len() == 2
                                    && arms[0].binding == "left"
                                    && arms[1].binding == "right"
                                    && default.is_none()
                        ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_select_statement_without_binding_parsing() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_select_stmt_no_binding.fol")
            .expect("Should read select statement without binding fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let error = parser
        .parse(&mut lexer)
        .expect_err("old binding-free select header must no longer parse");
    assert!(format!("{error:?}").contains("old select(channel as binding) form"));
}

#[test]
fn test_select_pipe_stage_parsing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_pipe_select_stage.fol")
        .expect("Should read pipe select-stage fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let error = parser
        .parse(&mut lexer)
        .expect_err("old select pipe stages must no longer parse");
    assert!(format!("{error:?}").contains("old select(channel as binding) form"));
}
