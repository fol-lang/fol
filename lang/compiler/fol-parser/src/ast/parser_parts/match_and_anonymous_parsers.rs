use super::*;

#[derive(Clone, Copy)]
enum AnonymousRoutineKind {
    Function,
    Procedure,
    Logical,
}

impl AnonymousRoutineKind {
    fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Procedure => "procedure",
            Self::Logical => "logical",
        }
    }

    fn body_close_message(self) -> &'static str {
        match self {
            Self::Function => "Expected '}' to close anonymous function body",
            Self::Procedure => "Expected '}' to close anonymous procedure body",
            Self::Logical => "Expected '}' to close anonymous logical body",
        }
    }
}

impl AstParser {
    pub(super) fn lookahead_is_match_expression(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !matches!(
            current.key(),
            KEYWORD::Keyword(BUILDIN::If) | KEYWORD::Keyword(BUILDIN::When)
        ) {
            return false;
        }

        let mut round_depth = 0usize;
        let mut saw_open_round = false;
        let mut saw_close_round = false;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if !saw_open_round {
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    saw_open_round = true;
                    round_depth = 1;
                    continue;
                }
                return false;
            }

            if !saw_close_round {
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    round_depth += 1;
                    continue;
                }
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundC)) {
                    round_depth -= 1;
                    if round_depth == 0 {
                        saw_close_round = true;
                    }
                    continue;
                }
                continue;
            }

            return matches!(key, KEYWORD::Symbol(SYMBOL::CurlyO));
        }

        false
    }

    fn parse_match_arrow_body(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.skip_ignorable(tokens)?;
        let flow = tokens.curr(false)?;
        if !matches!(
            flow.key(),
            KEYWORD::Operator(OPERATOR::Flow2) | KEYWORD::Operator(OPERATOR::Flow)
        ) {
            return Err(ParseError::from_token(
                &flow,
                "Expected '->' or '=>' in matching expression".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let expr = self.parse_logical_expression(tokens)?;
        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Semi))
        ) {
            let _ = tokens.bump();
        }

        Ok(vec![expr])
    }

    fn parse_match_expression_has_members(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut members = Vec::new();
        for _ in 0..64 {
            members.push(self.parse_logical_expression(tokens)?);
            self.skip_ignorable(tokens)?;

            let next = tokens.curr(false)?;
            if matches!(
                next.key(),
                KEYWORD::Operator(OPERATOR::Flow2) | KEYWORD::Operator(OPERATOR::Flow)
            ) {
                break;
            }
            if matches!(
                next.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                continue;
            }

            return Err(ParseError::from_token(
                &next,
                "Expected ',', ';', or '->' in matching has-case".to_string(),
            ));
        }

        if members.len() == 1 {
            Ok(members.into_iter().next().expect("single has member"))
        } else {
            Ok(AstNode::ContainerLiteral {
                container_type: ContainerType::Set,
                elements: members,
            })
        }
    }

    pub(super) fn parse_match_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let keyword = tokens.curr(false)?;
        if !matches!(
            keyword.key(),
            KEYWORD::Keyword(BUILDIN::If) | KEYWORD::Keyword(BUILDIN::When)
        ) {
            return Err(ParseError::from_token(
                &keyword,
                "Expected matching expression".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '(' after matching keyword".to_string(),
            ));
        }
        let _ = tokens.bump();

        let expr = self.parse_logical_expression(tokens)?;
        self.skip_ignorable(tokens)?;
        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected ')' after matching expression condition".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open_cases = tokens.curr(false)?;
        if !matches!(open_cases.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open_cases,
                "Expected '{' to start matching expression cases".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut cases = Vec::new();
        let mut default = None;

        for _ in 0..256 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                break;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::In)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let range = self.parse_logical_expression(tokens)?;
                let body = self.parse_match_arrow_body(tokens)?;
                cases.push(WhenCase::In { range, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Is)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let value = self.parse_logical_expression(tokens)?;
                let body = self.parse_match_arrow_body(tokens)?;
                cases.push(WhenCase::Is { value, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Has)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let member = self.parse_match_expression_has_members(tokens)?;
                let body = self.parse_match_arrow_body(tokens)?;
                cases.push(WhenCase::Has { member, body });
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Star)) {
                let _ = tokens.bump();
                default = Some(self.parse_match_arrow_body(tokens)?);
                continue;
            }

            return Err(ParseError::from_token(
                &token,
                "Expected in/is/has/* case in matching expression".to_string(),
            ));
        }

        Ok(AstNode::When {
            expr: Box::new(expr),
            cases,
            default,
        })
    }

    pub(super) fn lookahead_is_shorthand_anonymous_fun(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let mut params_depth = 1usize;
        let mut params_closed = false;
        let mut capture_depth = 0usize;
        let mut in_type_clause = false;
        let mut type_round_depth = 0usize;
        let mut type_square_depth = 0usize;
        let mut type_curly_depth = 0usize;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if !params_closed {
                match key {
                    KEYWORD::Symbol(SYMBOL::RoundO) => {
                        params_depth += 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::RoundC) => {
                        if params_depth == 0 {
                            return false;
                        }
                        params_depth -= 1;
                        if params_depth == 0 {
                            params_closed = true;
                        }
                        continue;
                    }
                    _ => continue,
                }
            }

            if capture_depth > 0 {
                match key {
                    KEYWORD::Symbol(SYMBOL::SquarO) => capture_depth += 1,
                    KEYWORD::Symbol(SYMBOL::SquarC) => capture_depth -= 1,
                    _ => {}
                }
                continue;
            }

            if in_type_clause {
                match key {
                    KEYWORD::Symbol(SYMBOL::RoundO) => {
                        type_round_depth += 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::RoundC) if type_round_depth > 0 => {
                        type_round_depth -= 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::SquarO) => {
                        type_square_depth += 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::SquarC) if type_square_depth > 0 => {
                        type_square_depth -= 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::CurlyO) => {
                        if type_round_depth == 0
                            && type_square_depth == 0
                            && type_curly_depth == 0
                            && !matches!(
                                self.next_significant_key_from_window(tokens),
                                Some(KEYWORD::Keyword(BUILDIN::Fun))
                            )
                        {
                            return true;
                        }
                        type_curly_depth += 1;
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::CurlyC) if type_curly_depth > 0 => {
                        type_curly_depth -= 1;
                        continue;
                    }
                    KEYWORD::Operator(OPERATOR::Flow)
                        if type_round_depth == 0
                            && type_square_depth == 0
                            && type_curly_depth == 0 =>
                    {
                        return true;
                    }
                    KEYWORD::Symbol(SYMBOL::Colon)
                        if type_round_depth == 0
                            && type_square_depth == 0
                            && type_curly_depth == 0 =>
                    {
                        continue;
                    }
                    KEYWORD::Symbol(SYMBOL::Root) | KEYWORD::Operator(OPERATOR::Divide)
                        if type_round_depth == 0
                            && type_square_depth == 0
                            && type_curly_depth == 0 =>
                    {
                        continue;
                    }
                    _ => continue,
                }
            }

            match key {
                KEYWORD::Symbol(SYMBOL::SquarO) => {
                    capture_depth = 1;
                }
                KEYWORD::Symbol(SYMBOL::Colon) => {
                    in_type_clause = true;
                }
                KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow) => {
                    return true;
                }
                _ => {
                    return false;
                }
            }
        }

        params_closed && in_type_clause
    }

    pub(super) fn parse_anonymous_fun_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let routine_token = tokens.curr(false)?;
        if !matches!(routine_token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            return Err(ParseError::from_token(
                &routine_token,
                "Expected anonymous function expression".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&routine_token);

        let _ = tokens.bump();
        self.parse_anonymous_routine_after_keyword(
            tokens,
            AnonymousRoutineKind::Function,
            syntax_id,
        )
    }

    pub(super) fn parse_anonymous_pro_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let routine_token = tokens.curr(false)?;
        if !matches!(routine_token.key(), KEYWORD::Keyword(BUILDIN::Pro)) {
            return Err(ParseError::from_token(
                &routine_token,
                "Expected anonymous procedure expression".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&routine_token);

        let _ = tokens.bump();
        self.parse_anonymous_routine_after_keyword(
            tokens,
            AnonymousRoutineKind::Procedure,
            syntax_id,
        )
    }

    pub(super) fn parse_anonymous_log_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let routine_token = tokens.curr(false)?;
        if !matches!(routine_token.key(), KEYWORD::Keyword(BUILDIN::Log)) {
            return Err(ParseError::from_token(
                &routine_token,
                "Expected anonymous logical expression".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&routine_token);

        let _ = tokens.bump();
        self.parse_anonymous_routine_after_keyword(tokens, AnonymousRoutineKind::Logical, syntax_id)
    }

    fn parse_anonymous_routine_after_keyword(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        kind: AnonymousRoutineKind,
        syntax_id: Option<SyntaxNodeId>,
    ) -> Result<AstNode, ParseError> {
        self.skip_ignorable(tokens)?;
        let options = self.parse_routine_options(tokens)?;
        self.skip_ignorable(tokens)?;

        let open_params = tokens.curr(false)?;
        if !matches!(open_params.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_params,
                format!("Expected '(' after anonymous {}", kind.label()),
            ));
        }
        let _ = tokens.bump();

        let (params, first_untyped) = self.parse_routine_header_list(tokens)?;
        if let Some(token) = first_untyped {
            return Err(ParseError::from_token(
                &token,
                format!("Expected ':' after {} parameter name", kind.label()),
            ));
        }
        self.ensure_unique_parameter_names(&params, "parameter", tokens)?;

        let captures = self.parse_optional_routine_capture_list(tokens)?;
        self.ensure_unique_capture_names(&captures, tokens)?;

        self.skip_ignorable(tokens)?;
        let mut return_type = None;
        let mut error_type = None;
        if let Ok(token) = tokens.curr(false) {
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                return_type = Some(self.parse_type_reference_tokens(tokens)?);

                error_type = self.parse_optional_error_type_after_return_type(tokens)?;
            }
        }

        self.skip_ignorable(tokens)?;
        let assign = tokens.curr(false)?;
        if !matches!(
            assign.key(),
            KEYWORD::Symbol(SYMBOL::Equal) | KEYWORD::Operator(OPERATOR::Flow)
        ) {
            return Err(ParseError::from_token(
                &assign,
                format!(
                    "Expected '=' or '=>' before anonymous {} body",
                    kind.label()
                ),
            ));
        }
        if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            let _ = tokens.bump();
        }

        let (body, inquiries) = self.parse_named_routine_body(
            tokens,
            "Expected '{' or '=>' to start anonymous routine body",
            kind.body_close_message(),
        )?;

        match kind {
            AnonymousRoutineKind::Function => Ok(AstNode::AnonymousFun {
                syntax_id,
                options,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            }),
            AnonymousRoutineKind::Procedure => Ok(AstNode::AnonymousPro {
                syntax_id,
                options,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            }),
            AnonymousRoutineKind::Logical => Ok(AstNode::AnonymousLog {
                syntax_id,
                options,
                captures,
                params,
                return_type: Some(return_type.unwrap_or(FolType::Bool)),
                error_type,
                body,
                inquiries,
            }),
        }
    }

    pub(super) fn parse_shorthand_anonymous_fun_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open_params = tokens.curr(false)?;
        if !matches!(open_params.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_params,
                "Expected '(' to start shorthand anonymous function".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&open_params);
        let _ = tokens.bump();

        let (params, first_untyped) = self.parse_routine_header_list(tokens)?;
        if let Some(token) = first_untyped {
            return Err(ParseError::from_token(
                &token,
                "Expected ':' after function parameter name".to_string(),
            ));
        }
        self.ensure_unique_parameter_names(&params, "parameter", tokens)?;

        let captures = self.parse_optional_routine_capture_list(tokens)?;
        self.ensure_unique_capture_names(&captures, tokens)?;

        self.skip_ignorable(tokens)?;
        let mut return_type = None;
        let mut error_type = None;
        if let Ok(token) = tokens.curr(false) {
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                return_type = Some(self.parse_type_reference_tokens(tokens)?);

                error_type = self.parse_optional_error_type_after_return_type(tokens)?;
            }
        }

        let (body, inquiries) = self.parse_named_routine_body(
            tokens,
            "Expected '{' or '=>' to start shorthand anonymous function body",
            "Expected '}' to close shorthand anonymous function body",
        )?;

        Ok(AstNode::AnonymousFun {
            syntax_id,
            options: Vec::new(),
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
        })
    }
}
