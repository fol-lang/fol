use super::*;

#[test]
fn test_book_keyword_call_example_parses() {
    let mut file_stream = FileStream::from_file("test/parser/simple_book_keyword_call_example.fol")
        .expect("Should read book keyword call example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book keyword call example");

    let has_keyword_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::ProDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 3
                        && matches!(&args[0], AstNode::NamedArgument { name, .. } if name == "el3")
                        && matches!(&args[1], AstNode::NamedArgument { name, .. } if name == "el2")
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el1")
                ))
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_keyword_call,
        "Book keyword call example should preserve named call arguments"
    );
}

#[test]
fn test_book_mixed_keyword_call_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_mixed_keyword_call_example.fol")
            .expect("Should read book mixed keyword call example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book mixed keyword call example");

    let has_mixed_keyword_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::ProDecl { body, .. }
                if body.iter().any(|stmt| matches!(
                    stmt,
                    AstNode::FunctionCall { name, args, .. }
                    if name == "calc"
                        && args.len() == 5
                        && !matches!(&args[0], AstNode::NamedArgument { .. })
                        && !matches!(&args[1], AstNode::NamedArgument { .. })
                        && matches!(&args[2], AstNode::NamedArgument { name, .. } if name == "el5")
                        && matches!(&args[3], AstNode::NamedArgument { name, .. } if name == "el4")
                        && matches!(&args[4], AstNode::NamedArgument { name, .. } if name == "el3")
                ))
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_mixed_keyword_call,
        "Book mixed keyword call example should preserve positional and named call arguments"
    );
}

#[test]
fn test_book_higher_order_function_parameter_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_higher_order_param_example.fol")
            .expect("Should read book higher-order parameter example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book higher-order parameter example");

    let has_higher_order_param = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, params, .. }
                if name == "add1"
                    && params.len() == 1
                    && matches!(params[0].param_type, FolType::Function { .. })
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_higher_order_param,
        "Book higher-order parameter example should keep the function-type parameter"
    );
}

#[test]
fn test_book_higher_order_function_return_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_higher_order_return_example.fol")
            .expect("Should read book higher-order return example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book higher-order return example");

    let has_higher_order_return = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, return_type: Some(FolType::Function { .. }), .. }
                if name == "add2"
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_higher_order_return,
        "Book higher-order return example should keep the function return type"
    );
}

#[test]
fn test_book_generator_example_parses() {
    let mut file_stream = FileStream::from_file("test/parser/simple_book_generator_example.fol")
        .expect("Should read book generator example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book generator example");

    let has_generator = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, body, .. }
                if name == "someIter"
                    && body.iter().any(|stmt| matches!(stmt, AstNode::Loop { .. }))
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_generator,
        "Book generator example should keep the loop/yield routine body"
    );
}

#[test]
fn test_book_variadic_routine_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_variadic_routine_example.fol")
            .expect("Should read book variadic routine example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book variadic routine example");

    let has_variadic_routine = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, params, .. }
                if name == "calc"
                    && params.len() == 2
                    && matches!(params[1].param_type, FolType::Sequence { .. })
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_variadic_routine,
        "Book variadic routine example should keep the variadic sequence parameter"
    );
}

#[test]
fn test_book_default_argument_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_default_argument_example.fol")
            .expect("Should read book default-argument example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book default-argument example");

    let has_default_param = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, params, .. }
                if name == "calc"
                    && params.iter().any(|param| param.name == "rise" && param.default.is_some())
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_default_param,
        "Book default-argument example should keep the defaulted parameter"
    );
}

#[test]
fn test_book_short_return_form_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_short_return_form_example.fol")
            .expect("Should read book short-return-form example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book short-return-form example");

    let has_short_return_fun = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { name, return_type: None, .. }
                if name == "add"
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_short_return_fun,
        "Book short-return-form example should keep the omitted return type"
    );
}

#[test]
fn test_book_variadic_call_example_parses() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_book_variadic_call_example.fol")
            .expect("Should read book variadic call example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book variadic call example");

    let has_variadic_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::ProDecl { name, body, .. }
                if name == "main"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::FunctionCall { name, args, .. }
                        if name == "calc" && args.len() == 5
                    ))
            )
        }),
        _ => panic!("Expected program node"),
    };

    assert!(
        has_variadic_call,
        "Book variadic call example should keep the expanded positional call"
    );
}

#[test]
fn test_book_dispatch_example_parses() {
    let mut file_stream = FileStream::from_file("test/parser/simple_book_dispatch_example.fol")
        .expect("Should read book dispatch example");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should accept the book dispatch example");

    let has_dispatch_routines = match ast {
        AstNode::Program { declarations } => {
            let mut has_call_bar = false;
            let mut has_bar_call = false;
            for node in declarations {
                if let AstNode::ProDecl { name, params, .. } = node {
                    if name == "callBar" && params.len() == 1 {
                        has_call_bar = true;
                    }
                    if name == "barCall" && params.len() == 1 {
                        has_bar_call = true;
                    }
                }
            }
            has_call_bar && has_bar_call
        }
        _ => panic!("Expected program node"),
    };

    assert!(
        has_dispatch_routines,
        "Book dispatch example should keep both dispatch routines"
    );
}
