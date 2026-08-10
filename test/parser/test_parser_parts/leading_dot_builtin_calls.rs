use super::*;
use fol_parser::ast::{CallSurface, WhenCase};

#[test]
fn test_top_level_leading_dot_builtin_call_statement() {
    let mut file_stream = FileStream::from_file("test/parser/simple_top_level_dot_echo.fol")
        .expect("Should read top-level leading-dot builtin fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept top-level leading-dot builtin calls");

    assert!(
        matches!(
            ast,
            AstNode::Program { declarations }
                if declarations.iter().any(|node| matches!(
                    node,
                    AstNode::FunctionCall { surface, name, args, .. }
                        if name == "echo"
                            && *surface == CallSurface::DotIntrinsic
                            && matches!(args.as_slice(), [AstNode::Literal(Literal::String(_))])
                ))
        ),
        "Expected top-level leading-dot builtin call to lower as FunctionCall"
    );
}

#[test]
fn test_leading_dot_builtin_call_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_dot_len_expr.fol")
        .expect("Should read leading-dot builtin expression fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept leading-dot builtin calls in expression position");

    let has_dot_expr = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl {
                            name,
                            value: Some(value),
                            ..
                        } if name == "size"
                            && matches!(
                                value.as_ref(),
                                AstNode::FunctionCall { surface, name, args, .. }
                                    if name == "len"
                                        && *surface == CallSurface::DotIntrinsic
                                        && matches!(args.as_slice(), [AstNode::Identifier { name, .. }] if name == "items")
                            )
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_dot_expr,
        "Expected leading-dot builtin expression to lower as FunctionCall"
    );
}

#[test]
fn test_leading_dot_not_builtin_call_expression() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_dot_not_expr.fol")
        .expect("Should read leading-dot not builtin expression fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept leading-dot not builtin calls in expression position");

    let has_dot_expr = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::VarDecl {
                            name,
                            value: Some(value),
                            ..
                        } if name == "inverted"
                            && matches!(
                                value.as_ref(),
                                AstNode::FunctionCall { surface, name, args, .. }
                                    if name == "not"
                                        && *surface == CallSurface::DotIntrinsic
                                        && matches!(args.as_slice(), [AstNode::Identifier { name, .. }] if name == "flag")
                            )
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_dot_expr,
        "Expected leading-dot not builtin expression to lower as FunctionCall"
    );
}

#[test]
fn test_leading_dot_builtin_calls_in_inquiry_bodies() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_dot_builtin_inquiry.fol")
            .expect("Should read leading-dot inquiry fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept leading-dot builtin calls in inquiry bodies");

    let has_inquiry_dot_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { inquiries, .. }
                    if inquiries.iter().any(|inquiry| matches!(
                        inquiry,
                        AstNode::Inquiry { body, .. }
                            if matches!(body.as_slice(), [AstNode::FunctionCall { surface, name, .. }] if name == "echo" && *surface == CallSurface::DotIntrinsic)
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_inquiry_dot_call,
        "Expected inquiry body to preserve leading-dot builtin call"
    );
}

#[test]
fn test_leading_dot_builtin_calls_in_flow_bodies() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_if_dot_builtin_flow.fol")
        .expect("Should read leading-dot flow-body fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept leading-dot builtin calls in flow bodies");

    let has_flow_dot_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::When { cases, .. }
                            if matches!(cases.as_slice(), [WhenCase::Case { body, .. }] if matches!(body.as_slice(), [AstNode::FunctionCall { surface, name, .. }] if name == "echo" && *surface == CallSurface::DotIntrinsic))
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_flow_dot_call,
        "Expected flow body to preserve leading-dot builtin call"
    );
}

#[test]
fn test_leading_dot_builtin_calls_in_pipe_stages() {
    let mut file_stream =
        FileStream::from_file("test/parser/simple_fun_pipe_dot_builtin_stage.fol")
            .expect("Should read leading-dot pipe-stage fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept leading-dot builtin calls in pipe stages");

    let has_pipe_dot_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value), .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::BinaryOp { right, .. }
                                    if matches!(right.as_ref(), AstNode::FunctionCall { surface, name, .. } if name == "echo" && *surface == CallSurface::DotIntrinsic)
                            )
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_pipe_dot_call,
        "Expected pipe stage to preserve leading-dot builtin call"
    );
}

#[test]
fn test_ordinary_function_calls_remain_plain_surface_calls() {
    let mut file_stream = FileStream::from_file("test/parser/simple_fun_call_expr.fol")
        .expect("Should read ordinary function call fixture");
    let mut tokens = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();

    let ast = parser
        .parse(&mut tokens)
        .expect("Parser should accept ordinary function calls");

    let has_plain_call = match ast {
        AstNode::Program { declarations } => declarations.iter().any(|node| {
            matches!(
                node,
                AstNode::FunDecl { body, .. }
                    if body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Assignment { value, .. }
                            if matches!(
                                value.as_ref(),
                                AstNode::FunctionCall { surface, name, .. }
                                    if name == "compute" && *surface == CallSurface::Plain
                            )
                    ))
            )
        }),
        _ => false,
    };

    assert!(
        has_plain_call,
        "Expected ordinary function calls to retain the plain call surface"
    );
}
