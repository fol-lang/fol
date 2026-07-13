use super::*;

impl AstParser {
    pub(super) fn parse_function_type_signature(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(Option<String>, FolType), ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start function type".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let fun_token = tokens.curr(false)?;
        if !matches!(fun_token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            return Err(ParseError::from_token(
                &fun_token,
                "Expected 'fun' in function type".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let mut function_name = None;
        if let Ok(token) = tokens.curr(false) {
            if token.key().is_illegal() || Self::token_to_named_label(&token).is_some() {
                function_name = Some(Self::expect_named_label(
                    &token,
                    "Expected function type name after 'fun'",
                )?);
                let _ = tokens.bump();
            }
        }

        self.skip_ignorable(tokens)?;
        let open_params = tokens.curr(false)?;
        if !matches!(open_params.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_params,
                "Expected '(' in function type".to_string(),
            ));
        }
        let _ = tokens.bump();

        let params = self.parse_parameter_list(tokens)?;
        if params.iter().any(|param| param.default.is_some()) {
            return Err(ParseError::from_token(
                &fun_token,
                "Default values are not allowed in function types".to_string(),
            ));
        }

        self.skip_ignorable(tokens)?;
        let colon = tokens.curr(false)?;
        if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
            return Err(ParseError::from_token(
                &colon,
                "Expected ':' before function type return type".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let return_type = self.parse_type_reference_tokens(tokens)?;

        self.skip_ignorable(tokens)?;
        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected '}' to close function type".to_string(),
            ));
        }
        let _ = tokens.bump();

        Ok((
            function_name,
            FolType::Function {
                params: params.into_iter().map(|param| param.param_type).collect(),
                return_type: Box::new(return_type),
            },
        ))
    }

    pub(super) fn parse_function_type_reference(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<FolType, ParseError> {
        self.parse_function_type_signature(tokens)
            .map(|(_, function_type)| function_type)
    }

    pub(super) fn parse_balanced_type_suffix(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        open_key: KEYWORD,
        close_key: KEYWORD,
        missing_close_message: &str,
    ) -> Result<String, ParseError> {
        let mut depth = 0usize;
        let mut rendered = String::new();
        let mut anchor_token = None;

        for _ in 0..512 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            let key = token.key();

            if key.is_boundary() {
                return Err(ParseError::from_token(
                    &token,
                    missing_close_message.to_string(),
                ));
            }

            if key == open_key {
                if anchor_token.is_none() {
                    anchor_token = Some(token.clone());
                }
                depth += 1;
            } else if key == close_key {
                if depth == 0 {
                    return Err(ParseError::from_token(
                        &token,
                        missing_close_message.to_string(),
                    ));
                }
                depth -= 1;
            }

            let fragment = token.con().trim();
            if !fragment.is_empty() {
                rendered.push_str(fragment);
            }
            let _ = tokens.bump();

            if depth == 0 {
                return Ok(rendered);
            }
        }

        let token = match anchor_token {
            Some(token) => token,
            None => tokens.curr(false)?,
        };
        Err(ParseError::from_token(
            &token,
            missing_close_message.to_string(),
        ))
    }

    pub(super) fn parse_block_body(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_close_message: &str,
    ) -> Result<Vec<AstNode>, ParseError> {
        let _nesting = self.enter_nesting(tokens)?;
        let mut body = Vec::new();
        let mut anchor_token = None;

        for _ in 0..8_192 {
            self.skip_layout(tokens)?;

            let token = tokens.curr(false)?;
            if anchor_token.is_none() {
                anchor_token = Some(token.clone());
            }
            let key = token.key();

            if key.is_comment() {
                body.push(self.parse_comment_token(&token)?);
                let _ = tokens.bump();
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(body);
            }

            if key.is_boundary() {
                return Err(ParseError::from_token(
                    &token,
                    missing_close_message.to_string(),
                ));
            }

            if key.is_eof() {
                let anchor = anchor_token.unwrap_or(token);
                return Err(ParseError::from_token(
                    &anchor,
                    missing_close_message.to_string(),
                ));
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Return)) {
                body.push(self.parse_return_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Break)) {
                body.push(self.parse_break_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Yield)) {
                body.push(self.parse_yield_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Dfr)) {
                body.push(self.parse_dfr_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Edf)) {
                body.push(self.parse_edf_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(
                key,
                KEYWORD::Keyword(BUILDIN::Panic)
                    | KEYWORD::Keyword(BUILDIN::Report)
                    | KEYWORD::Keyword(BUILDIN::Check)
                    | KEYWORD::Keyword(BUILDIN::Assert)
            ) {
                body.push(self.parse_builtin_call_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::Dot))
                && self.lookahead_is_dot_builtin_call(tokens)
            {
                body.push(self.parse_dot_builtin_call_expr(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if self.lookahead_binding_alternative(tokens).is_some() {
                body.extend(self.parse_binding_alternative_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Var)) {
                body.extend(self.parse_var_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Let)) {
                body.extend(self.parse_let_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Con)) {
                body.extend(self.parse_con_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Lab)) {
                body.extend(self.parse_lab_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Use)) {
                body.extend(self.parse_use_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Seg)) {
                body.push(self.parse_seg_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Std)) && self.lookahead_is_std_decl(tokens) {
                body.push(self.parse_std_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Ali)) {
                body.push(self.parse_alias_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Typ)) {
                body.extend(self.parse_type_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Def)) {
                body.push(self.parse_def_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Fun)) {
                body.push(self.parse_fun_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Log)) {
                body.push(self.parse_log_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Pro)) {
                body.push(self.parse_pro_decl(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::When)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                body.push(self.parse_when_stmt(tokens)?);
                self.bump_if_no_progress(tokens, before);
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::If)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                body.push(self.parse_if_stmt(tokens)?);
                self.bump_if_no_progress(tokens, before);
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Select)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                body.push(self.parse_select_stmt(tokens)?);
                self.bump_if_no_progress(tokens, before);
                continue;
            }

            if matches!(
                key,
                KEYWORD::Keyword(BUILDIN::While)
                    | KEYWORD::Keyword(BUILDIN::Loop)
                    | KEYWORD::Keyword(BUILDIN::For)
                    | KEYWORD::Keyword(BUILDIN::Each)
            ) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                body.push(self.parse_loop_stmt(tokens)?);
                self.bump_if_no_progress(tokens, before);
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::CurlyO)) {
                body.push(self.parse_block_stmt(tokens)?);
                continue;
            }

            if self.lookahead_is_spawn_expression(tokens) {
                body.push(self.parse_logical_expression(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if self.lookahead_has_top_level_pipe(tokens) {
                body.push(self.parse_logical_expression(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if matches!(
                key,
                KEYWORD::Symbol(SYMBOL::Bang) | KEYWORD::Symbol(SYMBOL::Hash)
            ) || matches!(token.con().trim(), "!" | "#")
            {
                body.push(self.parse_logical_expression(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if (AstParser::token_can_be_logical_name(&key)
                || key.is_textual_literal()
                || matches!(key, KEYWORD::Symbol(SYMBOL::Star)))
                && self.lookahead_is_assignment(tokens)
                && self.can_start_assignment(tokens)
            {
                body.push(self.parse_assignment_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && (self.lookahead_is_call(tokens) || self.lookahead_is_method_call(tokens))
                && self.can_start_assignment(tokens)
                && !self.lookahead_has_top_level_pipe(tokens)
            {
                body.push(self.parse_call_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if (matches!(
                key,
                KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::Dot)
            ) || AstParser::token_can_be_logical_name(&key)
                || key.is_textual_literal())
                && self.lookahead_is_general_invoke(
                    tokens,
                    matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)),
                )
                && self.can_start_assignment(tokens)
            {
                body.push(self.parse_invoke_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            if AstParser::token_can_be_logical_name(&key) {
                body.push(AstNode::Identifier {
                    syntax_id: self.record_syntax_origin(&token),
                    name: token.con().trim().to_string(),
                });
            } else if key.is_literal() {
                body.push(self.parse_lexer_literal(&token)?);
            }

            if tokens.bump().is_none() {
                break;
            }
        }

        let anchor = match anchor_token {
            Some(token) => token,
            None => tokens.curr(false)?,
        };
        Err(ParseError::from_token(
            &anchor,
            missing_close_message.to_string(),
        ))
    }

    pub(super) fn parse_block_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start block".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&open);
        let _ = tokens.bump();
        let statements = self.parse_block_body(tokens, "Expected '}' to close block")?;
        Ok(AstNode::Block {
            syntax_id,
            statements,
        })
    }

    pub(super) fn parse_return_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let return_token = tokens.curr(false)?;
        if !matches!(return_token.key(), KEYWORD::Keyword(BUILDIN::Return)) {
            return Err(ParseError::from_token(
                &return_token,
                "Expected 'return' statement".to_string(),
            ));
        }

        if !self.is_inside_routine() {
            return Err(ParseError::from_token(
                &return_token,
                "'return' is only allowed inside routines".to_string(),
            ));
        }

        if tokens.bump().is_none() {
            return Ok(AstNode::Return { value: None });
        }

        self.skip_ignorable(tokens)?;

        let value = match tokens.curr(false) {
            Ok(token) if token.key().is_terminal() => None,
            Ok(_) => Some(Box::new(self.parse_logical_expression(tokens)?)),
            Err(_) => None,
        };

        Ok(AstNode::Return { value })
    }

    pub(super) fn parse_break_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let break_token = tokens.curr(false)?;
        if !matches!(break_token.key(), KEYWORD::Keyword(BUILDIN::Break)) {
            return Err(ParseError::from_token(
                &break_token,
                "Expected 'break' statement".to_string(),
            ));
        }

        if !self.is_inside_loop() {
            return Err(ParseError::from_token(
                &break_token,
                "'break' is only allowed inside loops".to_string(),
            ));
        }

        let _ = tokens.bump();

        Ok(AstNode::Break)
    }

    pub(super) fn parse_yield_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let yield_token = tokens.curr(false)?;
        if !matches!(yield_token.key(), KEYWORD::Keyword(BUILDIN::Yield)) {
            return Err(ParseError::from_token(
                &yield_token,
                "Expected 'yield' statement".to_string(),
            ));
        }

        if !(self.is_inside_routine() || self.is_inside_loop()) {
            return Err(ParseError::from_token(
                &yield_token,
                "'yield' is only allowed inside routines or loops".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let value = self.parse_logical_expression(tokens)?;

        Ok(AstNode::Yield {
            value: Box::new(value),
        })
    }

    pub(super) fn parse_dfr_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let dfr_token = tokens.curr(false)?;
        if !matches!(dfr_token.key(), KEYWORD::Keyword(BUILDIN::Dfr)) {
            return Err(ParseError::from_token(
                &dfr_token,
                "Expected 'dfr' statement".to_string(),
            ));
        }

        if !self.is_inside_routine() {
            return Err(ParseError::from_token(
                &dfr_token,
                "'dfr' is only allowed inside routines".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&dfr_token);

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start dfr block".to_string(),
            ));
        }
        let _ = tokens.bump();
        let body = self.parse_block_body(tokens, "Expected '}' to close dfr block")?;
        Ok(AstNode::Dfr { syntax_id, body })
    }

    pub(super) fn parse_edf_stmt(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let edf_token = tokens.curr(false)?;
        if !matches!(edf_token.key(), KEYWORD::Keyword(BUILDIN::Edf)) {
            return Err(ParseError::from_token(
                &edf_token,
                "Expected 'edf' statement".to_string(),
            ));
        }
        if !self.is_inside_routine() {
            return Err(ParseError::from_token(
                &edf_token,
                "'edf' is only allowed inside routines".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&edf_token);
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start edf block".to_string(),
            ));
        }
        let _ = tokens.bump();
        let body = self.parse_block_body(tokens, "Expected '}' to close edf block")?;
        Ok(AstNode::Edf { syntax_id, body })
    }
}
