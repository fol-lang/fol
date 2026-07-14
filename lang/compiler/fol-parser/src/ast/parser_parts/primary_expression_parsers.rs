use super::*;

impl AstParser {
    fn lookahead_is_record_init_field(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !(Self::token_to_named_label(&current).is_some() || current.key().is_illegal()) {
            return false;
        }

        matches!(
            self.next_significant_key_from_window(tokens),
            Some(KEYWORD::Symbol(SYMBOL::Equal))
        )
    }

    fn parse_record_init_fields_after_open(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        open_token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<AstNode, ParseError> {
        let mut fields = Vec::new();
        let mut closed = false;
        for _ in 0..256 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                closed = true;
                break;
            }

            let name =
                Self::expect_named_label(&token, "Expected field name in record initializer")?;
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let equal = tokens.curr(false)?;
            if !matches!(equal.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Err(ParseError::from_token(
                    &equal,
                    "Expected '=' after record initializer field name".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let value = self.parse_logical_expression(tokens)?;
            fields.push(crate::ast::RecordInitField { name, value });
            self.skip_ignorable(tokens)?;

            let sep = tokens.curr(false)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
                ) {
                    let _ = tokens.bump();
                    closed = true;
                    break;
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                closed = true;
                break;
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in record initializer".to_string(),
            ));
        }

        if !closed {
            return Err(ParseError::from_token(
                open_token,
                "Record initializer exceeds maximum field count (256)".to_string(),
            ));
        }

        Ok(AstNode::RecordInit {
            syntax_id: self.record_syntax_origin(open_token),
            fields,
        })
    }

    pub(super) fn lookahead_is_spawn_expression(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return false;
        }

        matches!(
            (
                tokens.peek(0, false).map(|token| token.key()),
                tokens.peek(1, false).map(|token| token.key()),
            ),
            (
                Ok(KEYWORD::Symbol(SYMBOL::AngleC)),
                Ok(KEYWORD::Symbol(SYMBOL::SquarC)),
            )
        )
    }

    pub(super) fn parse_primary_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let leading_comments = self.collect_comment_nodes(tokens)?;
        let token = tokens.curr(false)?;

        if token.key().is_illegal() {
            return Err(ParseError::from_token(
                &token,
                format!("Parser encountered illegal token '{}'", token.con()),
            ));
        }

        if self.lookahead_is_spawn_expression(tokens) {
            self.consume_significant_token(tokens);

            let angle = tokens.curr(false)?;
            if !matches!(angle.key(), KEYWORD::Symbol(SYMBOL::AngleC)) {
                return Err(ParseError::from_token(
                    &angle,
                    "Expected '>' in spawn marker".to_string(),
                ));
            }
            self.consume_significant_token(tokens);

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' in spawn marker".to_string(),
                ));
            }
            self.consume_significant_token(tokens);
            self.skip_layout(tokens)?;

            let task = self.parse_primary_expression(tokens)?;
            return Ok(self.attach_leading_comments(
                AstNode::Spawn {
                    task: Box::new(task),
                },
                leading_comments,
            ));
        }

        if let Some((message, unary_op)) = self.unary_prefix_info(&token) {
            let operator_token = token.clone();
            let _ = tokens.bump();
            self.ensure_unary_operand(tokens, &operator_token, message)?;

            let operand = self.parse_primary_expression(tokens)?;
            if let Some(op) = unary_op {
                return Ok(self.attach_leading_comments(
                    AstNode::UnaryOp {
                        op,
                        operand: Box::new(operand),
                    },
                    leading_comments,
                ));
            }

            return Ok(self.attach_leading_comments(operand, leading_comments));
        }

        let node = if matches!(
            token.key(),
            KEYWORD::Keyword(BUILDIN::If) | KEYWORD::Keyword(BUILDIN::When)
        ) && self.lookahead_is_match_expression(tokens)
        {
            self.parse_match_expression(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Dot)) {
            self.parse_dot_builtin_call_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
            self.parse_pipe_lambda_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            self.parse_anonymous_fun_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Log)) {
            self.parse_anonymous_log_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Pro)) {
            self.parse_anonymous_pro_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Ok(self.attach_leading_comments(
                self.parse_container_expression(tokens)?,
                leading_comments,
            ));
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO))
            && self.lookahead_is_shorthand_anonymous_fun(tokens)
        {
            self.parse_shorthand_anonymous_fun_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            let _ = tokens.bump();
            let inner = self.parse_logical_expression(tokens)?;
            let inner = self.attach_trailing_comments(
                inner,
                self.collect_comments_before(tokens, |key| {
                    matches!(key, KEYWORD::Symbol(SYMBOL::RoundC))
                })?,
            );
            self.skip_layout(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ')' for parenthesized expression".to_string(),
                ));
            }

            let _ = tokens.bump();
            inner
        } else if token.key().is_textual_literal()
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::RoundO))
            )
        {
            let name = Self::token_to_named_label(&token).ok_or_else(|| {
                ParseError::from_token(&token, "Expected quoted callable name".to_string())
            })?;
            let _ = tokens.bump();
            AstNode::Identifier {
                syntax_id: self.record_syntax_origin(&token),
                name,
            }
        } else if Self::token_can_start_path_expression(&token)
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Operator(OPERATOR::Path))
            )
        {
            let path = self.parse_qualified_path(
                tokens,
                "Expected expression path root",
                "Expected name after '::' in expression path",
            )?;
            AstNode::QualifiedIdentifier { path }
        } else {
            let node = self.parse_primary(&token)?;
            let _ = tokens.bump();
            node
        };

        Ok(self.attach_leading_comments(
            self.parse_postfix_expression(tokens, node)?,
            leading_comments,
        ))
    }

    pub(super) fn parse_container_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start container expression".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_layout(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
        ) {
            let _ = tokens.bump();
            return Ok(AstNode::ContainerLiteral {
                container_type: ContainerType::Array,
                elements: Vec::new(),
            });
        }

        if self.lookahead_is_record_init_field(tokens) {
            return self.parse_record_init_fields_after_open(tokens, &open);
        }

        let mut elements = Vec::new();
        for _ in 0..256 {
            self.skip_layout(tokens)?;
            let pending_comments = self.collect_comment_nodes(tokens)?;
            let token = tokens.curr(false)?;

            if !pending_comments.is_empty()
                && matches!(
                    token.key(),
                    KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
                )
            {
                elements.extend(pending_comments);
                let _ = tokens.bump();
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                if !pending_comments.is_empty() {
                    elements.extend(pending_comments);
                }
                let _ = tokens.bump();
                break;
            }

            let mut expr = self.parse_logical_expression(tokens)?;
            expr = self.attach_leading_comments(expr, pending_comments);
            expr = self.attach_trailing_comments(
                expr,
                self.collect_comments_before(tokens, |key| {
                    matches!(
                        key,
                        KEYWORD::Keyword(BUILDIN::For)
                            | KEYWORD::Symbol(SYMBOL::Comma)
                            | KEYWORD::Symbol(SYMBOL::Semi)
                            | KEYWORD::Symbol(SYMBOL::CurlyC)
                    )
                })?,
            );
            self.skip_layout(tokens)?;

            if let Ok(next) = tokens.curr(false) {
                if matches!(next.key(), KEYWORD::Keyword(BUILDIN::For)) {
                    if !elements.is_empty() {
                        return Err(ParseError::from_token(
                            &next,
                            "Rolling expressions must contain exactly one output expression"
                                .to_string(),
                        ));
                    }
                    return self.parse_rolling_expression(tokens, expr);
                }
            }

            elements.push(expr);

            let sep = tokens.curr(false)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
                ) {
                    let _ = tokens.bump();
                    break;
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                break;
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in container expression".to_string(),
            ));
        }

        if elements.len() == 1 {
            if let Some(range) = elements.pop() {
                if matches!(range, AstNode::Range { .. }) {
                    return Ok(range);
                }
                return Ok(AstNode::ContainerLiteral {
                    container_type: ContainerType::Array,
                    elements: vec![range],
                });
            }
        }

        Ok(AstNode::ContainerLiteral {
            container_type: ContainerType::Array,
            elements,
        })
    }
}
