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
                                type_args: Vec::new(),
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
                KEYWORD::Operator(OPERATOR::Path) => {
                    // Explicit generic call turbofish: `ident::[TypeArgs](args)`.
                    // The qualified-path parser bails out when it peeks
                    // `::[`, leaving the `::` for this postfix branch.
                    let is_eligible_callee = match &node {
                        AstNode::Identifier { .. } => true,
                        AstNode::QualifiedIdentifier { path } => !path.is_qualified(),
                        _ => false,
                    };
                    if is_eligible_callee
                        && matches!(
                            self.next_significant_key_from_window(tokens),
                            Some(KEYWORD::Symbol(SYMBOL::SquarO))
                        )
                    {
                        let callee = match node {
                            AstNode::Identifier { .. } => node,
                            AstNode::QualifiedIdentifier { path } => AstNode::Identifier {
                                syntax_id: path.syntax_id(),
                                name: path.joined(),
                            },
                            other => other,
                        };
                        node = self.attach_leading_comments(
                            self.parse_explicit_generic_call(tokens, callee)?,
                            leading_comments,
                        );
                    } else {
                        break;
                    }
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
                    // Postfix `value!` shell unwrap is removed (V3: no raw symbols
                    // on values — use `[uwp]value`). A `!` here is not a postfix
                    // operator: stop postfix parsing so the binary `!=` (or a
                    // clean parse error for a stray `!`) is handled by the caller.
                    break;
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

    /// Parse an explicit generic call of the form `name::[TypeArgs](args)`.
    /// Assumes the caller has already positioned `tokens` at the `::`
    /// operator and verified that it is followed by `[`.
    fn parse_explicit_generic_call(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        callee: AstNode,
    ) -> Result<AstNode, ParseError> {
        let (name, syntax_id) = match callee {
            AstNode::Identifier { name, syntax_id } => (name, syntax_id),
            other => {
                return Err(ParseError::from_token(
                    &tokens.curr(false)?,
                    format!(
                        "Explicit generic calls require a plain identifier callee, got {other:?}"
                    ),
                ));
            }
        };

        let path_sep = tokens.curr(false)?;
        if !matches!(path_sep.key(), KEYWORD::Operator(OPERATOR::Path)) {
            return Err(ParseError::from_token(
                &path_sep,
                "Expected '::' before explicit generic type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open_square = tokens.curr(false)?;
        if !matches!(open_square.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open_square,
                "Expected '[' after '::' in explicit generic type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let mut type_args = Vec::new();
        loop {
            self.skip_ignorable(tokens)?;
            let peek = tokens.curr(false)?;
            if matches!(peek.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                break;
            }
            let type_arg = self.parse_type_reference_tokens(tokens)?;
            type_args.push(type_arg);
            self.skip_ignorable(tokens)?;
            let after = tokens.curr(false)?;
            match after.key() {
                KEYWORD::Symbol(SYMBOL::Comma) => {
                    let _ = tokens.bump();
                    continue;
                }
                KEYWORD::Symbol(SYMBOL::SquarC) => {
                    let _ = tokens.bump();
                    break;
                }
                _ => {
                    return Err(ParseError::from_token(
                        &after,
                        "Expected ',' or ']' in explicit generic type arguments".to_string(),
                    ));
                }
            }
        }

        if type_args.is_empty() {
            return Err(ParseError::from_token(
                &open_square,
                "Explicit generic calls require at least one type argument".to_string(),
            ));
        }

        self.skip_ignorable(tokens)?;
        let open_paren = tokens.curr(false)?;
        if !matches!(open_paren.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_paren,
                "Expected '(' after explicit generic type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();
        let args = self.parse_call_args(tokens)?;

        Ok(AstNode::FunctionCall {
            syntax_id,
            surface: crate::ast::CallSurface::Plain,
            name,
            type_args,
            args,
        })
    }
}
