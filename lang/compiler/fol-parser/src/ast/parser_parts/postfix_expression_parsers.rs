use super::*;

impl AstParser {
    pub(super) fn parse_postfix_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        mut node: AstNode,
    ) -> Result<AstNode, ParseError> {
        for _ in 0..256 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Symbol(SYMBOL::RoundO)
                        | KEYWORD::Symbol(SYMBOL::Dot)
                        | KEYWORD::Symbol(SYMBOL::SquarO)
                        | KEYWORD::Symbol(SYMBOL::Colon)
                        | KEYWORD::Symbol(SYMBOL::Bang)
                        | KEYWORD::Symbol(SYMBOL::Dollar)
                )
            })?;

            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(node),
            };

            match token.key() {
                KEYWORD::Symbol(SYMBOL::RoundO) => {
                    let _ = tokens.bump();
                    let args = self.parse_call_args(tokens)?;
                    node = self.attach_leading_comments(
                        match node {
                            AstNode::Identifier { name, syntax_id } => AstNode::FunctionCall {
                                syntax_id,
                                surface: crate::ast::CallSurface::Plain,
                                name,
                                args,
                            },
                            AstNode::QualifiedIdentifier { path } => {
                                AstNode::QualifiedFunctionCall { path, args }
                            }
                            callee => AstNode::Invoke {
                                callee: Box::new(callee),
                                args,
                            },
                        },
                        leading_comments,
                    );
                }
                KEYWORD::Symbol(SYMBOL::Dot) => {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let member_token = tokens.curr(false)?;
                    let member = Self::expect_named_label(
                        &member_token,
                        "Expected field or method name after '.'",
                    )?;
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let is_method_call = matches!(
                        tokens.curr(false).map(|token| token.key()),
                        Ok(KEYWORD::Symbol(SYMBOL::RoundO))
                    );

                    if is_method_call {
                        let _ = tokens.bump();
                        let args = self.parse_call_args(tokens)?;
                        node = self.attach_leading_comments(
                            AstNode::MethodCall {
                                syntax_id: self.record_syntax_origin(&member_token),
                                object: Box::new(node),
                                method: member,
                                args,
                            },
                            leading_comments,
                        );
                    } else {
                        node = self.attach_leading_comments(
                            AstNode::FieldAccess {
                                object: Box::new(node),
                                field: member,
                            },
                            leading_comments,
                        );
                    }
                }
                KEYWORD::Symbol(SYMBOL::SquarO) => {
                    node = self.attach_leading_comments(
                        self.parse_index_or_slice_expression(tokens, node)?,
                        leading_comments,
                    );
                }
                KEYWORD::Symbol(SYMBOL::Colon) => {
                    let next_key = self.next_significant_key_from_window(tokens);
                    if matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::SquarO))) {
                        node = self.attach_leading_comments(
                            self.parse_prefix_availability_expression(tokens, node)?,
                            leading_comments,
                        );
                    } else if matches!(
                        node,
                        AstNode::IndexAccess { .. }
                            | AstNode::SliceAccess { .. }
                            | AstNode::PatternAccess { .. }
                    ) {
                        let _ = tokens.bump();
                        node = self.attach_leading_comments(
                            AstNode::AvailabilityAccess {
                                target: Box::new(node),
                            },
                            leading_comments,
                        );
                    } else {
                        break;
                    }
                }
                KEYWORD::Symbol(SYMBOL::Bang) => {
                    if matches!(
                        self.next_significant_key_from_window(tokens),
                        Some(KEYWORD::Symbol(SYMBOL::Equal))
                    ) {
                        break;
                    }

                    let _ = tokens.bump();
                    node = self.attach_leading_comments(
                        AstNode::UnaryOp {
                            op: UnaryOperator::Unwrap,
                            operand: Box::new(node),
                        },
                        leading_comments,
                    );
                }
                KEYWORD::Symbol(SYMBOL::Dollar) => {
                    let _ = tokens.bump();
                    node = self.attach_leading_comments(
                        AstNode::TemplateCall {
                            object: Box::new(node),
                            template: "$".to_string(),
                        },
                        leading_comments,
                    );
                }
                _ => break,
            }
        }

        Ok(node)
    }
}
