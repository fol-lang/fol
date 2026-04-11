use super::*;

impl AstParser {
    pub(super) fn parse_select_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let select_token = tokens.curr(false)?;
        if !matches!(select_token.key(), KEYWORD::Keyword(BUILDIN::Select)) {
            return Err(ParseError::from_token(
                &select_token,
                "Expected 'select' statement".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '(' after 'select'".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let channel = self.parse_range_expression(tokens)?;
        self.skip_ignorable(tokens)?;

        let mut binding = None;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Keyword(BUILDIN::As))
        ) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let binding_token = tokens.curr(false)?;
            binding = Some(Self::expect_named_label(
                &binding_token,
                "Expected binding name after 'as' in select statement",
            )?);
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
        }

        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected ')' after select header".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let body = self.parse_branch_body(tokens)?;
        Ok(AstNode::Select {
            channel: Box::new(channel),
            binding,
            body,
        })
    }

    pub(super) fn parse_builtin_call_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let keyword_token = tokens.curr(false)?;
        let syntax_id = self.record_syntax_origin(&keyword_token);
        let name = match keyword_token.key() {
            KEYWORD::Keyword(BUILDIN::Panic) => "panic",
            KEYWORD::Keyword(BUILDIN::Report) => "report",
            KEYWORD::Keyword(BUILDIN::Check) => "check",
            KEYWORD::Keyword(BUILDIN::Assert) => "assert",
            _ => {
                return Err(ParseError::from_token(
                    &keyword_token,
                    "Expected builtin diagnostic statement".to_string(),
                ));
            }
        }
        .to_string();

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let mut args = Vec::new();
        if let Ok(token) = tokens.curr(false) {
            if !token.key().is_terminal() {
                let expr = self.parse_logical_expression(tokens)?;
                args.push(expr);

                loop {
                    self.skip_ignorable(tokens)?;
                    let comma = match tokens.curr(false) {
                        Ok(token) => token,
                        Err(_) => break,
                    };

                    if !matches!(comma.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                        break;
                    }

                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let next = tokens.curr(false)?;
                    if next.key().is_terminal() {
                        return Err(ParseError::from_token(
                            &next,
                            "Expected expression after ',' in builtin diagnostic statement"
                                .to_string(),
                        ));
                    }

                    let expr = self.parse_logical_expression(tokens)?;
                    args.push(expr);
                }
            }
        }

        Ok(AstNode::FunctionCall {
            syntax_id,
            surface: crate::ast::CallSurface::KeywordIntrinsic,
            name,
            args,
        })
    }

    pub(super) fn parse_when_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let when_token = tokens.curr(false)?;
        if !matches!(when_token.key(), KEYWORD::Keyword(BUILDIN::When)) {
            return Err(ParseError::from_token(
                &when_token,
                "Expected 'when' statement".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open_expr = tokens.curr(false)?;
        if !matches!(open_expr.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_expr,
                "Expected '(' after 'when'".to_string(),
            ));
        }
        let _ = tokens.bump();

        let expr = self.parse_logical_expression(tokens)?;
        self.skip_ignorable(tokens)?;

        let close_expr = tokens.curr(false)?;
        if !matches!(close_expr.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Err(ParseError::from_token(
                &close_expr,
                "Expected ')' after when expression".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let open_cases = tokens.curr(false)?;
        if !matches!(open_cases.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open_cases,
                "Expected '{' to start when cases".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut cases = Vec::new();
        let mut default = None;

        for _ in 0..1024 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                break;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Case)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_cond = tokens.curr(false)?;
                if !matches!(open_cond.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_cond,
                        "Expected '(' after case".to_string(),
                    ));
                }
                let _ = tokens.bump();

                let condition = self.parse_logical_expression(tokens)?;
                self.skip_ignorable(tokens)?;
                let close_cond = tokens.curr(false)?;
                if !matches!(close_cond.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_cond,
                        "Expected ')' after case condition".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::Case { condition, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Of)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_type = tokens.curr(false)?;
                if !matches!(open_type.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_type,
                        "Expected '(' after of".to_string(),
                    ));
                }
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let type_match = self.parse_type_reference_tokens(tokens)?;
                self.skip_ignorable(tokens)?;

                let close_type = tokens.curr(false)?;
                if !matches!(close_type.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_type,
                        "Expected ')' after of type".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::Of { type_match, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Is)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_value = tokens.curr(false)?;
                if !matches!(open_value.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_value,
                        "Expected '(' after is".to_string(),
                    ));
                }
                let _ = tokens.bump();

                let value = self.parse_logical_expression(tokens)?;
                self.skip_ignorable(tokens)?;
                let close_value = tokens.curr(false)?;
                if !matches!(close_value.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_value,
                        "Expected ')' after is value".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::Is { value, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::In)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_range = tokens.curr(false)?;
                if !matches!(open_range.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_range,
                        "Expected '(' after in".to_string(),
                    ));
                }
                let _ = tokens.bump();

                let range = self.parse_logical_expression(tokens)?;
                self.skip_ignorable(tokens)?;
                let close_range = tokens.curr(false)?;
                if !matches!(close_range.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_range,
                        "Expected ')' after in range".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::In { range, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Has)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_member = tokens.curr(false)?;
                if !matches!(open_member.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_member,
                        "Expected '(' after has".to_string(),
                    ));
                }
                let _ = tokens.bump();

                let member = self.parse_logical_expression(tokens)?;
                self.skip_ignorable(tokens)?;
                let close_member = tokens.curr(false)?;
                if !matches!(close_member.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_member,
                        "Expected ')' after has member".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::Has { member, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::On)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let open_channel = tokens.curr(false)?;
                if !matches!(open_channel.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                    return Err(ParseError::from_token(
                        &open_channel,
                        "Expected '(' after on".to_string(),
                    ));
                }
                let _ = tokens.bump();

                let channel = self.parse_logical_expression(tokens)?;
                self.skip_ignorable(tokens)?;
                let close_channel = tokens.curr(false)?;
                if !matches!(close_channel.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    return Err(ParseError::from_token(
                        &close_channel,
                        "Expected ')' after on channel".to_string(),
                    ));
                }
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                cases.push(WhenCase::On { channel, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
                let body = self.parse_branch_body(tokens)?;
                default = Some(body);
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Star)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let next = tokens.curr(false)?;
                if !matches!(
                    next.key(),
                    KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow)
                ) {
                    return Err(ParseError::from_token(
                        &next,
                        "Expected '{' after when default '*'".to_string(),
                    ));
                }
                let body = self.parse_branch_body(tokens)?;
                default = Some(body);
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Dollar)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let body = self.parse_branch_body(tokens)?;
                default = Some(body);
                continue;
            }

            let _ = tokens.bump();
        }

        Ok(AstNode::When {
            expr: Box::new(expr),
            cases,
            default,
        })
    }

    pub(super) fn parse_if_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let if_token = tokens.curr(false)?;
        if !matches!(if_token.key(), KEYWORD::Keyword(BUILDIN::If)) {
            return Err(ParseError::from_token(
                &if_token,
                "Expected 'if' statement".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open_cond = tokens.curr(false)?;
        if !matches!(open_cond.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_cond,
                "Expected '(' after 'if'".to_string(),
            ));
        }
        let _ = tokens.bump();

        let condition = self.parse_logical_expression(tokens)?;
        self.skip_ignorable(tokens)?;

        let close_cond = tokens.curr(false)?;
        if !matches!(close_cond.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Err(ParseError::from_token(
                &close_cond,
                "Expected ')' after if condition".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let then_body = self.parse_branch_body(tokens)?;

        self.skip_ignorable(tokens)?;
        let else_body = if let Ok(token) = tokens.curr(false) {
            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Else)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;

                let else_target = tokens.curr(false)?;
                if matches!(else_target.key(), KEYWORD::Keyword(BUILDIN::If)) {
                    Some(vec![self.parse_if_stmt(tokens)?])
                } else if matches!(
                    else_target.key(),
                    KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow)
                ) {
                    Some(self.parse_branch_body(tokens)?)
                } else {
                    return Err(ParseError::from_token(
                        &else_target,
                        "Expected 'if', '{', or '=>' after else".to_string(),
                    ));
                }
            } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::If)) {
                Some(vec![self.parse_if_stmt(tokens)?])
            } else if matches!(
                token.key(),
                KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow)
            ) {
                Some(self.parse_branch_body(tokens)?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(AstNode::When {
            expr: Box::new(condition.clone()),
            cases: vec![WhenCase::Case {
                condition,
                body: then_body,
            }],
            default: else_body,
        })
    }

    pub(super) fn parse_branch_body(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.skip_ignorable(tokens)?;
        let token = tokens.curr(false)?;
        if matches!(token.key(), KEYWORD::Operator(OPERATOR::Flow)) {
            return self.parse_flow_body_nodes(tokens);
        }

        if !matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &token,
                "Expected '{' or '=>' to start branch body".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.parse_block_body(tokens, "Expected '}' to close case/default body")
    }

    pub(super) fn parse_loop_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let loop_token = tokens.curr(false)?;
        let keyword_name = match loop_token.key() {
            KEYWORD::Keyword(BUILDIN::While) => "while",
            KEYWORD::Keyword(BUILDIN::Loop) => "loop",
            KEYWORD::Keyword(BUILDIN::For) => "for",
            KEYWORD::Keyword(BUILDIN::Each) => "each",
            _ => "",
        };
        if keyword_name.is_empty() {
            return Err(ParseError::from_token(
                &loop_token,
                "Expected loop-like statement".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open_cond = tokens.curr(false)?;
        if !matches!(open_cond.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_cond,
                format!("Expected '(' after '{}'", keyword_name),
            ));
        }
        let _ = tokens.bump();

        let condition = self.parse_loop_condition(tokens)?;
        self.skip_ignorable(tokens)?;

        let close_cond = tokens.curr(false)?;
        if !matches!(close_cond.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Err(ParseError::from_token(
                &close_cond,
                format!("Expected ')' after {} condition", keyword_name),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let _loop_context = self.enter_loop_context(tokens)?;
        let body = if matches!(
            loop_token.key(),
            KEYWORD::Keyword(BUILDIN::While | BUILDIN::Loop | BUILDIN::For | BUILDIN::Each)
        ) {
            self.parse_branch_body(tokens)?
        } else {
            let open_body = tokens.curr(false)?;
            if !matches!(open_body.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
                return Err(ParseError::from_token(
                    &open_body,
                    format!("Expected '{{' to start {} body", keyword_name),
                ));
            }
            let _ = tokens.bump();
            self.parse_block_body(tokens, "Expected '}' to close loop body")?
        };

        Ok(AstNode::Loop {
            condition: Box::new(condition),
            body,
        })
    }

    pub(super) fn parse_loop_condition(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<LoopCondition, ParseError> {
        self.skip_ignorable(tokens)?;

        let current = tokens.curr(false)?;
        if matches!(current.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Ok(LoopCondition::Condition(Box::new(AstNode::Literal(
                Literal::Boolean(true),
            ))));
        }
        let mut type_hint = None;
        let mut current_var_token = current.clone();

        if matches!(current.key(), KEYWORD::Keyword(BUILDIN::Var)) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            current_var_token = tokens.curr(false)?;
            let declared_var = if matches!(current_var_token.key(), KEYWORD::Symbol(SYMBOL::Under))
            {
                "_".to_string()
            } else {
                Self::expect_named_label(
                    &current_var_token,
                    "Expected iteration binder name after 'var'",
                )?
            };
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let colon = tokens.curr(false)?;
            if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                return Err(ParseError::from_token(
                    &colon,
                    "Expected ':' after typed iteration binder name".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            type_hint = Some(self.parse_type_reference_tokens(tokens)?);
            self.skip_ignorable(tokens)?;

            let semi = tokens.curr(false)?;
            if !matches!(semi.key(), KEYWORD::Symbol(SYMBOL::Semi)) {
                return Err(ParseError::from_token(
                    &semi,
                    "Expected ';' after typed iteration binder declaration".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            current_var_token = tokens.curr(false)?;
            let iteration_var = if matches!(current_var_token.key(), KEYWORD::Symbol(SYMBOL::Under))
            {
                "_".to_string()
            } else {
                Self::expect_named_label(
                    &current_var_token,
                    "Expected typed iteration binder usage before 'in'",
                )?
            };

            if iteration_var != declared_var {
                return Err(ParseError::from_token(
                    &current_var_token,
                    format!(
                        "Typed iteration binder '{}' must match the iteration variable before 'in'",
                        declared_var
                    ),
                ));
            }
        }

        if (Self::token_to_named_label(&current_var_token).is_some()
            || current_var_token.key().is_illegal()
            || matches!(current_var_token.key(), KEYWORD::Symbol(SYMBOL::Under)))
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Keyword(BUILDIN::In))
            )
        {
            let var = if matches!(current_var_token.key(), KEYWORD::Symbol(SYMBOL::Under)) {
                "_".to_string()
            } else {
                Self::expect_named_label(
                    &current_var_token,
                    "Expected iteration binder before 'in'",
                )?
            };
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let in_token = tokens.curr(false)?;
            if !matches!(in_token.key(), KEYWORD::Keyword(BUILDIN::In)) {
                return Err(ParseError::from_token(
                    &in_token,
                    "Expected 'in' in loop iteration condition".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let iterable = self.parse_logical_expression(tokens)?;
            self.skip_ignorable(tokens)?;

            let condition = if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Keyword(BUILDIN::When)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    Some(Box::new(self.parse_logical_expression(tokens)?))
                } else {
                    None
                }
            } else {
                None
            };

            return Ok(LoopCondition::Iteration {
                var,
                type_hint,
                iterable: Box::new(iterable),
                condition,
            });
        }

        let condition_expr = self.parse_logical_expression(tokens)?;
        Ok(LoopCondition::Condition(Box::new(condition_expr)))
    }

    pub(super) fn parse_assignment_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let target = self.parse_assignment_target(tokens)?;
        self.skip_ignorable(tokens)?;

        let assign_token = tokens.curr(false)?;
        let mut compound_op = self.compound_assignment_op(&assign_token.key());
        let mut is_simple_assign = matches!(assign_token.key(), KEYWORD::Symbol(SYMBOL::Equal));

        if let Some(symbol_op) = self.compound_assignment_symbol_op(&assign_token.key()) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let eq_token = tokens.curr(false)?;
            if matches!(eq_token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                compound_op = Some(symbol_op);
                is_simple_assign = false;
            } else {
                return Err(ParseError::from_token(
                    &eq_token,
                    "Expected '=' after operator in compound assignment".to_string(),
                ));
            }
        }

        if !is_simple_assign && compound_op.is_none() {
            return Err(ParseError::from_token(
                &assign_token,
                "Expected assignment operator in assignment statement".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let parsed_value = self.parse_logical_expression(tokens)?;
        let value = if let Some(op) = compound_op {
            AstNode::BinaryOp {
                op,
                left: Box::new(target.clone()),
                right: Box::new(parsed_value),
            }
        } else {
            parsed_value
        };

        Ok(AstNode::Assignment {
            target: Box::new(target),
            value: Box::new(value),
        })
    }

    pub(super) fn parse_assignment_target(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let path = self.parse_qualified_path(
            tokens,
            "Expected assignment target",
            "Expected name after '::' in assignment target",
        )?;
        let mut target = if path.is_qualified() {
            AstNode::QualifiedIdentifier { path }
        } else {
            AstNode::Identifier {
                syntax_id: path.syntax_id(),
                name: path.joined(),
            }
        };

        for _ in 0..128 {
            self.skip_ignorable(tokens)?;
            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(target),
            };

            match token.key() {
                KEYWORD::Symbol(SYMBOL::RoundO) => {
                    return Err(ParseError::from_token(
                        &token,
                        "Function call cannot be used as an assignment target".to_string(),
                    ));
                }
                KEYWORD::Symbol(SYMBOL::Dot) => {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let field_token = tokens.curr(false)?;
                    let field = Self::expect_named_label(
                        &field_token,
                        "Expected field name after '.' in assignment target",
                    )?;
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    if matches!(
                        tokens.curr(false).map(|token| token.key()),
                        Ok(KEYWORD::Symbol(SYMBOL::RoundO))
                    ) {
                        return Err(ParseError::from_token(
                            &field_token,
                            "Method call cannot be used as an assignment target".to_string(),
                        ));
                    }

                    target = AstNode::FieldAccess {
                        object: Box::new(target),
                        field,
                    };
                }
                KEYWORD::Symbol(SYMBOL::SquarO) => {
                    target = self.parse_index_or_slice_assignment_target(tokens, target)?;
                }
                _ => break,
            }
        }

        Ok(target)
    }

    pub(super) fn parse_call_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let call = if self.lookahead_is_method_call(tokens) {
            self.parse_method_call_expr(tokens)?
        } else {
            self.parse_call_expr(tokens)?
        };

        Ok(call)
    }

    pub(super) fn parse_invoke_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let start_token = tokens.curr(false)?;
        let expr = self.parse_logical_expression(tokens)?;
        if !matches!(
            expr,
            AstNode::FunctionCall { .. }
                | AstNode::QualifiedFunctionCall { .. }
                | AstNode::MethodCall { .. }
                | AstNode::Invoke { .. }
        ) {
            return Err(ParseError::from_token(
                &start_token,
                "Expected invocable statement expression".to_string(),
            ));
        }

        Ok(expr)
    }

    pub(super) fn parse_call_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let path = self.parse_qualified_path(
            tokens,
            "Expected identifier for function call",
            "Expected name after '::' in function call",
        )?;
        let args =
            self.parse_open_paren_and_call_args(tokens, "Expected '(' after function name")?;

        Ok(if path.is_qualified() {
            AstNode::QualifiedFunctionCall { path, args }
        } else {
            AstNode::FunctionCall {
                syntax_id: path.syntax_id(),
                surface: crate::ast::CallSurface::Plain,
                name: path.joined(),
                args,
            }
        })
    }

    pub(super) fn parse_method_call_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let path = self.parse_qualified_path(
            tokens,
            "Expected object identifier for method call",
            "Expected name after '::' in method call",
        )?;
        let object = if path.is_qualified() {
            AstNode::QualifiedIdentifier { path }
        } else {
            AstNode::Identifier {
                syntax_id: path.syntax_id(),
                name: path.joined(),
            }
        };
        self.skip_ignorable(tokens)?;

        let dot = tokens.curr(false)?;
        if !matches!(dot.key(), KEYWORD::Symbol(SYMBOL::Dot)) {
            return Err(ParseError::from_token(
                &dot,
                "Expected '.' after object identifier".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let method_token = tokens.curr(false)?;
        let method = Self::expect_named_label(&method_token, "Expected method name after '.'")?;
        let _ = tokens.bump();
        let args = self.parse_open_paren_and_call_args(tokens, "Expected '(' after method name")?;

        Ok(AstNode::MethodCall {
            syntax_id: self.record_syntax_origin(&method_token),
            object: Box::new(object),
            method,
            args,
        })
    }

    pub(super) fn parse_open_paren_and_call_args(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        expected_open_error: &str,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.skip_ignorable(tokens)?;

        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open,
                expected_open_error.to_string(),
            ));
        }

        let _ = tokens.bump();
        self.parse_call_args(tokens)
    }
}
