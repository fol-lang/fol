use super::*;

impl AstParser {
    pub(super) fn parse_routine_body_with_inquiries(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_close_message: &str,
    ) -> Result<(Vec<AstNode>, Vec<AstNode>), ParseError> {
        let mut body = Vec::new();
        let mut inquiries = Vec::new();
        let mut inquiry_targets = HashSet::new();
        let mut anchor_token = None;

        for _ in 0..1024 {
            self.skip_layout(tokens)?;
            let token = tokens.curr(false)?;
            if anchor_token.is_none() {
                anchor_token = Some(token.clone());
            }

            if token.key().is_comment() {
                body.push(self.parse_comment_token(&token)?);
                let _ = tokens.bump();
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Where)) {
                let parsed = self.parse_optional_inquiry_clause(tokens)?;
                for inquiry in parsed {
                    let target = match &inquiry {
                        AstNode::Inquiry { target, .. } => target.duplicate_key(),
                        _ => String::new(),
                    };
                    if !inquiry_targets.insert(canonical_identifier_key(&target)) {
                        return Err(ParseError::from_token(
                            &token,
                            format!("Duplicate inquiry clause for '{}'", target),
                        ));
                    }
                    inquiries.push(inquiry);
                }
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok((body, inquiries));
            }

            if token.key().is_boundary() {
                return Err(ParseError::from_token(
                    &token,
                    missing_close_message.to_string(),
                ));
            }

            if token.key().is_eof() {
                let anchor = anchor_token.unwrap_or(token);
                return Err(ParseError::from_token(
                    &anchor,
                    missing_close_message.to_string(),
                ));
            }

            let key = token.key();

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

            if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
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

            if (matches!(key, KEYWORD::Symbol(SYMBOL::RoundO))
                || AstParser::token_can_be_logical_name(&key)
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

    pub(super) fn parse_optional_inquiry_clause(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.skip_ignorable(tokens)?;
        let where_token = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        if !matches!(where_token.key(), KEYWORD::Keyword(BUILDIN::Where)) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '(' after 'where'".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut targets = Vec::new();
        let mut seen_targets = HashSet::new();
        loop {
            self.skip_ignorable(tokens)?;
            let target_token = tokens.curr(false)?;
            let target = self.parse_inquiry_target(tokens)?;
            let duplicate_key = target.duplicate_key();
            if !seen_targets.insert(canonical_identifier_key(&duplicate_key)) {
                return Err(ParseError::from_token(
                    &target_token,
                    format!("Duplicate inquiry clause for '{}'", duplicate_key),
                ));
            }
            targets.push(target);

            self.skip_ignorable(tokens)?;

            self.skip_ignorable(tokens)?;
            let close = tokens.curr(false)?;
            if matches!(
                close.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                continue;
            }
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected ',', ';', or ')' after inquiry target".to_string(),
                ));
            }
            let _ = tokens.bump();
            break;
        }

        self.skip_ignorable(tokens)?;
        let open_body = tokens.curr(false)?;
        if matches!(open_body.key(), KEYWORD::Operator(OPERATOR::Flow)) {
            let expr = self.flow_nodes_to_expr(self.parse_flow_body_nodes(tokens)?);
            return Ok(targets
                .into_iter()
                .map(|target| AstNode::Inquiry {
                    target,
                    body: vec![expr.clone()],
                })
                .collect());
        }

        if !matches!(open_body.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open_body,
                "Expected '{' or '=>' to start inquiry body".to_string(),
            ));
        }
        let _ = tokens.bump();

        let body = self.parse_inquiry_body(tokens)?;
        Ok(targets
            .into_iter()
            .map(|target| AstNode::Inquiry {
                target,
                body: body.clone(),
            })
            .collect())
    }

    pub(super) fn parse_inquiry_target(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<InquiryTarget, ParseError> {
        self.skip_ignorable(tokens)?;
        let token = tokens.curr(false)?;

        if token.key().is_textual_literal() {
            let target = Self::exact_unquote_text(token.con());
            let _ = tokens.bump();
            return Ok(InquiryTarget::Quoted(target));
        }

        let first = Self::expect_named_label(&token, "Expected inquiry target name")?;
        let _ = tokens.bump();

        let mut segments = vec![first];
        for _ in 0..64 {
            self.skip_ignorable(tokens)?;
            let separator = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            let is_path = matches!(separator.key(), KEYWORD::Operator(OPERATOR::Path))
                || matches!(separator.key(), KEYWORD::Symbol(SYMBOL::Colon))
                    && matches!(
                        self.next_significant_key_from_window(tokens),
                        Some(KEYWORD::Symbol(SYMBOL::Colon))
                    );

            if !is_path {
                break;
            }

            if matches!(separator.key(), KEYWORD::Operator(OPERATOR::Path)) {
                let _ = tokens.bump();
            } else {
                self.consume_significant_token(tokens);
                self.consume_significant_token(tokens);
            }

            self.skip_ignorable(tokens)?;
            let segment = tokens.curr(false)?;
            let segment_name =
                Self::expect_named_label(&segment, "Expected name after '::' in inquiry target")?;
            segments.push(segment_name);
            let _ = tokens.bump();
        }

        Ok(match segments.as_slice() {
            [single] if single == "self" => InquiryTarget::SelfValue,
            [single] if single == "this" => InquiryTarget::ThisValue,
            [single] => InquiryTarget::Named(single.clone()),
            _ => InquiryTarget::Qualified(QualifiedPath::with_syntax_id(
                segments,
                self.record_syntax_origin(&token),
            )),
        })
    }

    fn parse_inquiry_body(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let mut body = Vec::new();
        let mut anchor_token = None;

        for _ in 0..512 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if anchor_token.is_none() {
                anchor_token = Some(token.clone());
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(body);
            }

            if token.key().is_eof() {
                let anchor = anchor_token.unwrap_or(token);
                return Err(ParseError::from_token(
                    &anchor,
                    "Expected '}' to close inquiry body".to_string(),
                ));
            }

            let key = token.key();
            if matches!(key, KEYWORD::Symbol(SYMBOL::CurlyO)) {
                body.push(self.parse_block_stmt(tokens)?);
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

            if matches!(key, KEYWORD::Keyword(BUILDIN::If)) {
                body.push(self.parse_if_stmt(tokens)?);
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::When)) {
                body.push(self.parse_when_stmt(tokens)?);
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Select)) {
                body.push(self.parse_select_stmt(tokens)?);
                continue;
            }

            if matches!(
                key,
                KEYWORD::Keyword(BUILDIN::While)
                    | KEYWORD::Keyword(BUILDIN::Loop)
                    | KEYWORD::Keyword(BUILDIN::For)
                    | KEYWORD::Keyword(BUILDIN::Each)
            ) {
                body.push(self.parse_loop_stmt(tokens)?);
                continue;
            }

            if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && self.lookahead_is_assignment(tokens)
            {
                body.push(self.parse_assignment_stmt(tokens)?);
                self.consume_required_semicolon(tokens)?;
                continue;
            }

            body.push(self.parse_logical_expression(tokens)?);
            self.consume_required_semicolon(tokens)?;
        }

        let anchor = match anchor_token {
            Some(token) => token,
            None => tokens.curr(false)?,
        };
        Err(ParseError::from_token(
            &anchor,
            "Inquiry body exceeded parser limit".to_string(),
        ))
    }
}
