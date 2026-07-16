use super::*;

#[test]
fn test_export_binding_alternative_parses_top_level() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_plus_var.fol").expect("Should read +var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept +var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "exported"
                        && options.contains(&fol_parser::ast::VarOption::Export)
                        && !options.contains(&fol_parser::ast::VarOption::Normal)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_mutable_binding_alternative_parses_top_level() {
    let mut file_stream = FileStream::from_file("test/parser/simple_tilde_var.fol")
        .expect("Should read ~var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept ~var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "counter"
                        && options.contains(&fol_parser::ast::VarOption::Mutable)
                        && !options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_hidden_binding_alternative_parses_top_level() {
    let mut file_stream = FileStream::from_file("test/parser/simple_minus_var.fol")
        .expect("Should read -var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept -var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "hidden"
                        && options.contains(&fol_parser::ast::VarOption::Hidden)
                        && !options.contains(&fol_parser::ast::VarOption::Normal)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_static_binding_alternative_parses_top_level() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_bang_var.fol").expect("Should read !var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept !var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "cached"
                        && options.contains(&fol_parser::ast::VarOption::Static)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_reactive_binding_alternative_parses_top_level() {
    let mut file_stream = FileStream::from_file("test/parser/simple_query_var.fol")
        .expect("Should read ?var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept ?var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "derived"
                        && options.contains(&fol_parser::ast::VarOption::Reactive)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_heap_binding_alternative_parses_top_level() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_at_var.fol").expect("Should read @var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept @var binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "heap_value"
                        && options.contains(&fol_parser::ast::VarOption::New)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_hidden_const_alternative_parses_top_level() {
    let mut file_stream = FileStream::from_file("test/parser/simple_minus_con.fol")
        .expect("Should read -con fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept -con binding alternative");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "secret"
                        && options.contains(&fol_parser::ast::VarOption::Hidden)
                        && options.contains(&fol_parser::ast::VarOption::Immutable)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_export_binding_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_plus_var.fol")
        .expect("Should read function-body +var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept +var inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "exported"
                            && options.contains(&fol_parser::ast::VarOption::Export)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_mutable_binding_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_tilde_var.fol")
        .expect("Should read function-body ~var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept ~var inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "counter"
                            && options.contains(&fol_parser::ast::VarOption::Mutable)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_static_binding_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_bang_var.fol")
        .expect("Should read function-body !var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept !var inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "cached"
                            && options.contains(&fol_parser::ast::VarOption::Static)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_reactive_binding_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_query_var.fol")
        .expect("Should read function-body ?var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept ?var inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "derived"
                            && options.contains(&fol_parser::ast::VarOption::Reactive)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_heap_binding_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_at_var.fol")
        .expect("Should read function-body @var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept @var inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "heap_value"
                            && options.contains(&fol_parser::ast::VarOption::New)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_binding_alternative_allows_whitespace_before_keyword() {
    let mut file_stream = FileStream::from_file("test/parser/simple_plus_space_var.fol")
        .expect("Should read spaced + var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept spaced binding alternatives");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "exported"
                        && options.contains(&fol_parser::ast::VarOption::Export)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_binding_alternative_composes_with_mutability_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_plus_var_mut.fol")
        .expect("Should read +var[mut] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept +var[mut] composed binding alternatives");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "exported"
                        && options.contains(&fol_parser::ast::VarOption::Export)
                        && options.contains(&fol_parser::ast::VarOption::Mutable)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_binding_alternative_composes_with_visibility_options() {
    let mut file_stream = FileStream::from_file("test/parser/simple_tilde_var_exp.fol")
        .expect("Should read ~var[exp] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept ~var[exp] composed binding alternatives");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "exported"
                        && options.contains(&fol_parser::ast::VarOption::Mutable)
                        && options.contains(&fol_parser::ast::VarOption::Export)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_heap_binding_alternative_composes_with_borrowing() {
    let mut file_stream = FileStream::from_file("test/parser/simple_at_var_bor.fol")
        .expect("Should read @var[bor] fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept @var[bor] composed binding alternatives");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::VarDecl { name, options, .. }
                    if name == "heap_borrow"
                        && options.contains(&fol_parser::ast::VarOption::New)
                        && options.contains(&fol_parser::ast::VarOption::Borrowing)
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_hidden_const_alternative_parses_in_function_body() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_minus_con.fol")
        .expect("Should read function-body -con fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept -con inside function bodies");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl { name, options, .. }
                        if name == "secret"
                            && options.contains(&fol_parser::ast::VarOption::Hidden)
                            && options.contains(&fol_parser::ast::VarOption::Immutable)
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_binding_alternatives_parse_in_nested_blocks() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_nested_plus_var.fol")
        .expect("Should read nested-block +var fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept binding alternatives in nested blocks");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| {
                matches!(
                    node,
                    AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Block { statements, .. }
                        if statements.iter().any(|nested| matches!(
                            nested,
                            AstNode::VarDecl { name, options, .. }
                            if name == "exported"
                                && options.contains(&fol_parser::ast::VarOption::Export)
                        ))
                    ))
                )
            }));
        }
        _ => panic!("Expected program node"),
    }
}
