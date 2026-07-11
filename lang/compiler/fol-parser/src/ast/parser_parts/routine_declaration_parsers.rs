use super::*;

impl AstParser {
    pub(super) fn parse_optional_error_type_after_return_type(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Option<FolType>, ParseError> {
        self.skip_ignorable(tokens)?;
        let Ok(separator) = tokens.curr(false) else {
            return Ok(None);
        };

        match separator.key() {
            KEYWORD::Symbol(SYMBOL::Root) | KEYWORD::Operator(OPERATOR::Divide) => {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                Ok(Some(self.parse_type_reference_tokens(tokens)?))
            }
            KEYWORD::Symbol(SYMBOL::Colon) => Err(ParseError::from_token(
                &separator,
                "Expected '/' before routine error type".to_string(),
            )),
            _ => Ok(None),
        }
    }

    pub(super) fn parse_fun_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let fun_token = tokens.curr(false)?;
        if !matches!(fun_token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            return Err(ParseError::from_token(
                &fun_token,
                "Expected 'fun' declaration".to_string(),
            ));
        }
        let decl_line = fun_token.loc().row();

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_routine_options(tokens)?;
        self.skip_ignorable(tokens)?;

        // Guard: if skip_ignorable jumped past the declaration into a
        // different declaration, bail early pointing at the original keyword.
        self.guard_decl_boundary(&fun_token, tokens, decl_line, "Incomplete function declaration")?;

        let (receiver_type, name, name_token) = self.parse_routine_name_with_optional_receiver(
            tokens,
            "Expected function name after 'fun'",
        )?;

        self.skip_ignorable(tokens)?;
        self.guard_decl_boundary(&fun_token, tokens, decl_line, "Incomplete function declaration")?;

        let alt_generics = if self.lookahead_parenthesized_generic_header_before_colon(tokens) {
            let open = tokens.curr(false)?;
            if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                Vec::new()
            } else {
                let _ = tokens.bump();
                self.parse_generic_list(tokens)?
            }
        } else {
            Vec::new()
        };

        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key().clone()),
            Ok(KEYWORD::Symbol(SYMBOL::Colon))
        ) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let return_type = Some(self.parse_type_reference_tokens(tokens)?);
            let error_type = self.parse_optional_error_type_after_return_type(tokens)?;

            self.skip_ignorable(tokens)?;
            let assign = tokens.curr(false)?;
            if !matches!(
                assign.key(),
                KEYWORD::Symbol(SYMBOL::Equal) | KEYWORD::Operator(OPERATOR::Flow)
            ) {
                return Err(ParseError::from_token(
                    &assign,
                    "Expected '=' or '=>' before function body".to_string(),
                ));
            }
            if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                let _ = tokens.bump();
            }

            self.skip_ignorable(tokens)?;
            let mut params = Vec::new();
            if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                let _ = tokens.bump();
                let (parsed_params, first_untyped) = self.parse_routine_header_list(tokens)?;
                if let Some(token) = first_untyped {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected ':' after function parameter name".to_string(),
                    ));
                }
                self.ensure_unique_parameter_names(&parsed_params, "parameter", tokens)?;
                params = parsed_params;
            }

            self.skip_ignorable(tokens)?;
            let captures = self.parse_optional_routine_capture_list(tokens)?;
            self.ensure_unique_capture_names(&captures, tokens)?;

            let (body, inquiries) = self.parse_named_routine_body(
                tokens,
                "Expected '{' or '=>' to start function body",
                "Expected '}' to close function body",
            )?;

            return Ok(AstNode::FunDecl {
                syntax_id: self.record_syntax_origin(&name_token),
                options,
                generics: alt_generics,
                name,
                receiver_type,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            });
        }

        let (generics, params) =
            self.parse_routine_generics_and_params(tokens, "Expected '(' after function name")?;
        self.skip_ignorable(tokens)?;
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
                "Expected '=' or '=>' before function body".to_string(),
            ));
        }
        if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            let _ = tokens.bump();
        }

        let (body, inquiries) = self.parse_named_routine_body(
            tokens,
            "Expected '{' or '=>' to start function body",
            "Expected '}' to close function body",
        )?;

        Ok(AstNode::FunDecl {
            syntax_id: self.record_syntax_origin(&name_token),
            options,
            generics,
            name,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
        })
    }

    pub(super) fn parse_log_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let log_token = tokens.curr(false)?;
        if !matches!(log_token.key(), KEYWORD::Keyword(BUILDIN::Log)) {
            return Err(ParseError::from_token(
                &log_token,
                "Expected 'log' declaration".to_string(),
            ));
        }

        let decl_line = log_token.loc().row();

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_routine_options(tokens)?;
        self.skip_ignorable(tokens)?;

        self.guard_decl_boundary(&log_token, tokens, decl_line, "Incomplete logical declaration")?;

        let (receiver_type, name, name_token) = self.parse_routine_name_with_optional_receiver(
            tokens,
            "Expected logical name after 'log'",
        )?;

        self.skip_ignorable(tokens)?;
        self.guard_decl_boundary(&log_token, tokens, decl_line, "Incomplete logical declaration")?;

        let alt_generics = if self.lookahead_parenthesized_generic_header_before_colon(tokens) {
            let open = tokens.curr(false)?;
            if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                Vec::new()
            } else {
                let _ = tokens.bump();
                self.parse_generic_list(tokens)?
            }
        } else {
            Vec::new()
        };

        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key().clone()),
            Ok(KEYWORD::Symbol(SYMBOL::Colon))
        ) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let return_type = Some(self.parse_type_reference_tokens(tokens)?);
            let error_type = self.parse_optional_error_type_after_return_type(tokens)?;

            self.skip_ignorable(tokens)?;
            let assign = tokens.curr(false)?;
            if !matches!(
                assign.key(),
                KEYWORD::Symbol(SYMBOL::Equal) | KEYWORD::Operator(OPERATOR::Flow)
            ) {
                return Err(ParseError::from_token(
                    &assign,
                    "Expected '=' or '=>' before logical body".to_string(),
                ));
            }
            if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                let _ = tokens.bump();
            }

            self.skip_ignorable(tokens)?;
            let mut params = Vec::new();
            if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                let _ = tokens.bump();
                let (parsed_params, first_untyped) = self.parse_routine_header_list(tokens)?;
                if let Some(token) = first_untyped {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected ':' after function parameter name".to_string(),
                    ));
                }
                self.ensure_unique_parameter_names(&parsed_params, "parameter", tokens)?;
                params = parsed_params;
            }

            self.skip_ignorable(tokens)?;
            let captures = self.parse_optional_routine_capture_list(tokens)?;
            self.ensure_unique_capture_names(&captures, tokens)?;

            let (body, inquiries) = self.parse_named_routine_body(
                tokens,
                "Expected '{' or '=>' to start logical body",
                "Expected '}' to close logical body",
            )?;

            return Ok(AstNode::LogDecl {
                syntax_id: self.record_syntax_origin(&name_token),
                options,
                generics: alt_generics,
                name,
                receiver_type,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            });
        }

        let (generics, params) =
            self.parse_routine_generics_and_params(tokens, "Expected '(' after logical name")?;
        self.skip_ignorable(tokens)?;
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
                "Expected '=' or '=>' before logical body".to_string(),
            ));
        }
        if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            let _ = tokens.bump();
        }

        let (body, inquiries) = self.parse_named_routine_body(
            tokens,
            "Expected '{' or '=>' to start logical body",
            "Expected '}' to close logical body",
        )?;

        Ok(AstNode::LogDecl {
            syntax_id: self.record_syntax_origin(&name_token),
            options,
            generics,
            name,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
        })
    }

    pub(super) fn parse_pro_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let pro_token = tokens.curr(false)?;
        if !matches!(pro_token.key(), KEYWORD::Keyword(BUILDIN::Pro)) {
            return Err(ParseError::from_token(
                &pro_token,
                "Expected 'pro' declaration".to_string(),
            ));
        }

        let decl_line = pro_token.loc().row();

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_routine_options(tokens)?;
        self.skip_ignorable(tokens)?;

        self.guard_decl_boundary(&pro_token, tokens, decl_line, "Incomplete procedure declaration")?;

        let (receiver_type, name, name_token) = self.parse_routine_name_with_optional_receiver(
            tokens,
            "Expected procedure name after 'pro'",
        )?;

        self.skip_ignorable(tokens)?;
        self.guard_decl_boundary(&pro_token, tokens, decl_line, "Incomplete procedure declaration")?;

        let alt_generics = if self.lookahead_parenthesized_generic_header_before_colon(tokens) {
            let open = tokens.curr(false)?;
            if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                Vec::new()
            } else {
                let _ = tokens.bump();
                self.parse_generic_list(tokens)?
            }
        } else {
            Vec::new()
        };

        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key().clone()),
            Ok(KEYWORD::Symbol(SYMBOL::Colon))
        ) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let return_type = Some(self.parse_type_reference_tokens(tokens)?);
            let error_type = self.parse_optional_error_type_after_return_type(tokens)?;

            self.skip_ignorable(tokens)?;
            let assign = tokens.curr(false)?;
            if !matches!(
                assign.key(),
                KEYWORD::Symbol(SYMBOL::Equal) | KEYWORD::Operator(OPERATOR::Flow)
            ) {
                return Err(ParseError::from_token(
                    &assign,
                    "Expected '=' or '=>' before procedure body".to_string(),
                ));
            }
            if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                let _ = tokens.bump();
            }

            self.skip_ignorable(tokens)?;
            let mut params = Vec::new();
            if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                let _ = tokens.bump();
                let (parsed_params, first_untyped) = self.parse_routine_header_list(tokens)?;
                if let Some(token) = first_untyped {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected ':' after function parameter name".to_string(),
                    ));
                }
                self.ensure_unique_parameter_names(&parsed_params, "parameter", tokens)?;
                params = parsed_params;
            }

            self.skip_ignorable(tokens)?;
            let captures = self.parse_optional_routine_capture_list(tokens)?;
            self.ensure_unique_capture_names(&captures, tokens)?;

            let (body, inquiries) = self.parse_named_routine_body(
                tokens,
                "Expected '{' or '=>' to start procedure body",
                "Expected '}' to close procedure body",
            )?;

            return Ok(AstNode::ProDecl {
                syntax_id: self.record_syntax_origin(&name_token),
                options,
                generics: alt_generics,
                name,
                receiver_type,
                captures,
                params,
                return_type,
                error_type,
                body,
                inquiries,
            });
        }

        let (generics, params) =
            self.parse_routine_generics_and_params(tokens, "Expected '(' after procedure name")?;
        self.skip_ignorable(tokens)?;
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
                "Expected '=' or '=>' before procedure body".to_string(),
            ));
        }
        if matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            let _ = tokens.bump();
        }

        let (body, inquiries) = self.parse_named_routine_body(
            tokens,
            "Expected '{' or '=>' to start procedure body",
            "Expected '}' to close procedure body",
        )?;

        Ok(AstNode::ProDecl {
            syntax_id: self.record_syntax_origin(&name_token),
            options,
            generics,
            name,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
        })
    }

    pub(super) fn parse_routine_generics_and_params(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_open_message: &str,
    ) -> Result<(Vec<Generic>, Vec<Parameter>), ParseError> {
        self.skip_ignorable(tokens)?;
        let open_paren = tokens.curr(false)?;
        if !matches!(open_paren.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open_paren,
                missing_open_message.to_string(),
            ));
        }
        let _ = tokens.bump();

        let (first_list, first_untyped) = self.parse_routine_header_list(tokens)?;
        self.skip_ignorable(tokens)?;

        let next = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => {
                if let Some(token) = first_untyped {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected ':' after parameter name".to_string(),
                    ));
                }
                self.ensure_unique_parameter_names(&first_list, "parameter", tokens)?;
                return Ok((Vec::new(), first_list));
            }
        };

        if !matches!(next.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            if let Some(token) = first_untyped {
                return Err(ParseError::from_token(
                    &token,
                    "Expected ':' after parameter name".to_string(),
                ));
            }
            self.ensure_unique_parameter_names(&first_list, "parameter", tokens)?;
            return Ok((Vec::new(), first_list));
        }

        self.ensure_unique_parameter_names(&first_list, "generic", tokens)?;
        let generics = self.parameters_to_generics(first_list, tokens)?;
        let _ = tokens.bump();
        let params = self.parse_parameter_list(tokens)?;
        Ok((generics, params))
    }
}
