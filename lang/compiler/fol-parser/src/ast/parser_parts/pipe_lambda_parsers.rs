use super::*;

impl AstParser {
    fn parse_pipe_lambda_params(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Parameter>, ParseError> {
        let mut params = Vec::new();

        self.skip_ignorable(tokens)?;
        let close = tokens.curr(false)?;
        if matches!(close.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
            return Ok(params);
        }

        loop {
            let mut names = Vec::new();
            loop {
                let name_token = tokens.curr(false)?;
                let name = Self::expect_named_label(&name_token, "Expected lambda parameter name")?;
                // Record the lambda parameter NAME token so tooling can locate
                // its declaration; the resolver derives the origin from this id.
                let name_syntax_id = self.record_syntax_origin(&name_token);
                names.push((name, name_syntax_id));
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let token = tokens.curr(false)?;
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                    let Some(next_key) = self.next_significant_key_from_window(tokens) else {
                        return Err(ParseError::from_token(
                            &token,
                            "Expected lambda parameter name after ','".to_string(),
                        ));
                    };

                    if matches!(next_key, KEYWORD::Symbol(SYMBOL::Pipe)) {
                        return Err(ParseError::from_token(
                            &token,
                            "Expected lambda parameter name after ','".to_string(),
                        ));
                    }

                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    continue;
                }
                break;
            }

            self.skip_ignorable(tokens)?;
            let mut param_type = FolType::Any;
            let mut is_variadic = false;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    if matches!(
                        tokens.curr(false)?.key(),
                        KEYWORD::Operator(OPERATOR::Dotdotdot)
                    ) {
                        let _ = tokens.bump();
                        self.skip_ignorable(tokens)?;
                        is_variadic = true;
                        param_type = FolType::Sequence {
                            element_type: Box::new(self.parse_type_reference_tokens(tokens)?),
                        };
                    } else {
                        param_type = self.parse_type_reference_tokens(tokens)?;
                    }
                }
            }

            self.skip_ignorable(tokens)?;
            let mut default = None;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    default = Some(self.parse_logical_or_expression(tokens)?);
                }
            }

            if is_variadic && default.is_some() {
                let token = tokens.curr(false)?;
                return Err(ParseError::from_token(
                    &token,
                    "Variadic parameters cannot have default values".to_string(),
                ));
            }

            for (name, name_syntax_id) in names {
                params.push(Parameter {
                    is_borrowable: name.chars().all(|ch| {
                        !ch.is_ascii_lowercase() && (ch.is_ascii_alphanumeric() || ch == '_')
                    }),
                    is_mutex: false,
                    is_variadic,
                    name,
                    param_type: param_type.clone(),
                    default: default.clone(),
                    syntax_id: name_syntax_id,
                });
            }

            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if matches!(
                token.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                if is_variadic {
                    return Err(ParseError::from_token(
                        &token,
                        "Variadic parameter must be the last parameter".to_string(),
                    ));
                }
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::Pipe))
                ) {
                    return Ok(params);
                }
                continue;
            }
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
                return Ok(params);
            }

            return Err(ParseError::from_token(
                &token,
                "Expected ',', ';', or '|' after lambda parameters".to_string(),
            ));
        }
    }

    pub(super) fn parse_pipe_lambda_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '|' to start lambda expression".to_string(),
            ));
        }
        let syntax_id = self.record_syntax_origin(&open);
        let _ = tokens.bump();

        let params = self.parse_pipe_lambda_params(tokens)?;

        self.ensure_unique_parameter_names(&params, "parameter", tokens)?;

        self.skip_ignorable(tokens)?;
        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing '|' after lambda parameters".to_string(),
            ));
        }
        let _ = tokens.bump();

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
        let (body, inquiries) = if matches!(
            tokens.curr(false)?.key(),
            KEYWORD::Symbol(SYMBOL::CurlyO) | KEYWORD::Operator(OPERATOR::Flow)
        ) {
            self.parse_named_routine_body(
                tokens,
                "Expected '{', '=>', or expression after lambda parameters",
                "Expected '}' to close lambda body",
            )?
        } else {
            let body = vec![AstNode::Return {
                value: Some(Box::new(self.parse_logical_expression(tokens)?)),
            }];
            let mut inquiries = Vec::new();
            let mut inquiry_targets = HashSet::new();
            loop {
                self.skip_ignorable(tokens)?;
                let parsed = self.parse_optional_inquiry_clause(tokens)?;
                if parsed.is_empty() {
                    break;
                }

                for inquiry in parsed {
                    let target = match &inquiry {
                        AstNode::Inquiry { target, .. } => target.duplicate_key(),
                        _ => String::new(),
                    };
                    if !inquiry_targets.insert(canonical_identifier_key(&target)) {
                        let token = tokens.curr(false)?;
                        return Err(ParseError::from_token(
                            &token,
                            format!("Duplicate inquiry clause for '{}'", target),
                        ));
                    }
                    inquiries.push(inquiry);
                }
            }
            (body, inquiries)
        };

        Ok(AstNode::AnonymousFun {
            syntax_id,
            options: vec![FunOption::Mutable],
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
        })
    }
}
