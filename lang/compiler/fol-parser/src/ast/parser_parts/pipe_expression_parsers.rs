use super::*;

impl AstParser {
    fn pipe_stage_from_nodes(&self, nodes: Vec<AstNode>, anchor: &fol_lexer::lexer::stage3::element::Element) -> Result<AstNode, ParseError> {
        let mut iter = nodes.into_iter();
        let Some(first) = iter.next() else {
            return Err(ParseError::from_token(
                anchor,
                "Pipe stage produced no AST nodes".to_string(),
            ));
        };

        let mut statements = vec![first];
        statements.extend(iter);
        if statements.len() == 1 {
            Ok(statements.into_iter().next().expect("one statement"))
        } else {
            Ok(AstNode::Block {
                syntax_id: None,
                statements,
            })
        }
    }

    fn lookahead_pipe_stage_has_body_after_parens(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let mut depth = 0usize;
        let mut saw_open = false;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };
            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            match key {
                KEYWORD::Symbol(SYMBOL::RoundO) => {
                    saw_open = true;
                    depth += 1;
                }
                KEYWORD::Symbol(SYMBOL::RoundC) => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                    if saw_open && depth == 0 {
                        continue;
                    }
                }
                KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow)
                    if saw_open && depth == 0 =>
                {
                    return true;
                }
                _ if saw_open && depth == 0 => {
                    return false;
                }
                _ => {}
            }
        }

        false
    }

    fn parse_pipe_stage_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        self.skip_ignorable(tokens)?;
        let token = tokens.curr(false)?;

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::If))
            && self.lookahead_pipe_stage_has_body_after_parens(tokens)
        {
            return self.parse_if_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::When))
            && self.lookahead_pipe_stage_has_body_after_parens(tokens)
        {
            return self.parse_when_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Select))
            && self.lookahead_pipe_stage_has_body_after_parens(tokens)
        {
            return self.parse_select_stmt(tokens);
        }

        if matches!(
            token.key(),
            KEYWORD::Keyword(BUILDIN::While)
                | KEYWORD::Keyword(BUILDIN::Loop)
                | KEYWORD::Keyword(BUILDIN::For)
                | KEYWORD::Keyword(BUILDIN::Each)
        ) && self.lookahead_pipe_stage_has_body_after_parens(tokens)
        {
            return self.parse_loop_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Return)) {
            return self.parse_return_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return self.parse_block_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Break)) {
            return self.parse_break_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Yield)) {
            return self.parse_yield_stmt(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Async)) {
            let _ = tokens.bump();
            return Ok(AstNode::AsyncStage);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Await)) {
            let _ = tokens.bump();
            return Ok(AstNode::AwaitStage);
        }

        if self.lookahead_binding_alternative(tokens).is_some() {
            return self.pipe_stage_from_nodes(self.parse_binding_alternative_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Var)) {
            return self.pipe_stage_from_nodes(self.parse_var_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Let)) {
            return self.pipe_stage_from_nodes(self.parse_let_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Con)) {
            return self.pipe_stage_from_nodes(self.parse_con_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Lab)) {
            return self.pipe_stage_from_nodes(self.parse_lab_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Use)) {
            return self.pipe_stage_from_nodes(self.parse_use_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Ali)) {
            return self.parse_alias_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Typ)) {
            return self.pipe_stage_from_nodes(self.parse_type_decl(tokens)?, &token);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Def)) {
            return self.parse_def_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Seg)) {
            return self.parse_seg_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Std))
            && self.lookahead_is_std_decl(tokens)
        {
            return self.parse_std_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            return self.parse_fun_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Log)) {
            return self.parse_log_decl(tokens);
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Pro)) {
            return self.parse_pro_decl(tokens);
        }

        let key = token.key();
        if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
            && self.lookahead_is_assignment(tokens)
        {
            return self.parse_assignment_stmt(tokens);
        }

        if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
            && (self.lookahead_is_call(tokens) || self.lookahead_is_method_call(tokens))
        {
            return self.parse_call_stmt(tokens);
        }

        if matches!(key, KEYWORD::Symbol(SYMBOL::Dot)) && self.lookahead_is_dot_builtin_call(tokens)
        {
            let node = self.parse_dot_builtin_call_expr(tokens)?;
            self.consume_optional_semicolon(tokens)?;
            return Ok(node);
        }

        if (matches!(
            key,
            KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::Dot)
        ) || AstParser::token_can_be_logical_name(&key)
            || key.is_textual_literal())
            && self
                .lookahead_is_general_invoke(tokens, matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)))
        {
            return self.parse_invoke_stmt(tokens);
        }

        if matches!(
            token.key(),
            KEYWORD::Keyword(BUILDIN::Panic)
                | KEYWORD::Keyword(BUILDIN::Report)
                | KEYWORD::Keyword(BUILDIN::Check)
                | KEYWORD::Keyword(BUILDIN::Assert)
        ) {
            if matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::RoundO))
            ) {
                return self.parse_logical_or_expression(tokens);
            }
            return self.parse_builtin_call_stmt(tokens);
        }

        self.parse_logical_or_expression(tokens)
    }

    pub(super) fn parse_pipe_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_logical_or_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(key, KEYWORD::Symbol(SYMBOL::Pipe))
            })?;
            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            if !matches!(op_token.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
                break;
            }

            let consume_count = if matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::Pipe))
            ) {
                2
            } else {
                1
            };

            for _ in 0..consume_count {
                self.consume_significant_token(tokens);
            }

            self.skip_layout(tokens)?;
            let next = tokens.curr(false)?;
            if next.key().is_terminal() || next.key().is_eof() {
                return Err(ParseError::from_token(
                    &next,
                    if consume_count == 2 {
                        "Expected expression after '||'".to_string()
                    } else {
                        "Expected expression after '|'".to_string()
                    },
                ));
            }

            let rhs = self.parse_pipe_stage_expression(tokens)?;
            lhs = self.attach_leading_comments(
                AstNode::BinaryOp {
                    op: if consume_count == 2 {
                        BinaryOperator::PipeOr
                    } else {
                        BinaryOperator::Pipe
                    },
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                },
                leading_comments,
            );
        }

        Ok(lhs)
    }
}
