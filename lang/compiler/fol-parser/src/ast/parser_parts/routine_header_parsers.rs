use super::*;

impl AstParser {
    fn parse_parameter_name_group(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_name_error: &str,
        missing_group_name_error: &str,
        missing_mutex_close_error: &str,
    ) -> Result<(Vec<String>, bool), ParseError> {
        let token = tokens.curr(false)?;

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO))
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::RoundO))
            )
        {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let second_open = tokens.curr(false)?;
            if !matches!(second_open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                return Err(ParseError::from_token(
                    &second_open,
                    "Expected second '(' to start mutex parameter".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let name_token = tokens.curr(false)?;
            let name = Self::expect_named_label(&name_token, missing_name_error)?;
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let close_inner = tokens.curr(false)?;
            if !matches!(close_inner.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close_inner,
                    missing_mutex_close_error.to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let close_outer = tokens.curr(false)?;
            if !matches!(close_outer.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close_outer,
                    missing_mutex_close_error.to_string(),
                ));
            }
            let _ = tokens.bump();
            return Ok((vec![name], true));
        }

        let first_name = Self::expect_named_label(&token, missing_name_error)?;

        let mut names = vec![first_name];
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        loop {
            let next = tokens.curr(false)?;
            if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                break;
            }
            if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let name_token = tokens.curr(false)?;
                let grouped_name = Self::expect_named_label(&name_token, missing_group_name_error)?;
                names.push(grouped_name);
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                continue;
            }
            break;
        }

        Ok((names, false))
    }

    pub(super) fn parse_routine_header_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<
        (
            Vec<Parameter>,
            Option<fol_lexer::lexer::stage3::element::Element>,
        ),
        ParseError,
    > {
        let mut params = Vec::new();
        let mut first_untyped = None;

        for _ in 0..128 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok((params, first_untyped));
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
                let (function_name, function_type) = self.parse_function_type_signature(tokens)?;
                let Some(param_name) = function_name else {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected named function header in higher-order parameter".to_string(),
                    ));
                };

                params.push(Parameter {
                    name: param_name.clone(),
                    param_type: function_type,
                    is_variadic: false,
                    is_borrowable: param_name.chars().all(|ch| {
                        !ch.is_ascii_lowercase() && (ch.is_ascii_alphanumeric() || ch == '_')
                    }),
                    is_mutex: false,
                    default: None,
                });

                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    let _ = tokens.bump();
                    return Ok((params, first_untyped));
                }

                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or ')' after generic parameter".to_string(),
                ));
            }

            let (names, is_mutex) = self.parse_parameter_name_group(
                tokens,
                "Expected generic parameter name",
                "Expected parameter name after ','",
                "Expected closing '))' after mutex parameter name",
            )?;

            let mut is_variadic = false;
            let param_type = if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    if let Ok(token) = tokens.curr(false) {
                        if matches!(token.key(), KEYWORD::Operator(OPERATOR::Dotdotdot))
                            || token.con().trim() == "..."
                        {
                            is_variadic = true;
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                        }
                    }
                    let base_type = self.parse_type_reference_tokens(tokens)?;
                    if is_variadic {
                        FolType::Sequence {
                            element_type: Box::new(base_type),
                        }
                    } else {
                        base_type
                    }
                } else {
                    if first_untyped.is_none() {
                        first_untyped = Some(token.clone());
                    }
                    FolType::Named {
                        syntax_id: None,
                        name: "any".to_string(),
                    }
                }
            } else {
                FolType::Named {
                    syntax_id: None,
                    name: "any".to_string(),
                }
            };

            self.skip_ignorable(tokens)?;
            let default = if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let next = tokens.curr(false)?;
                    if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Comma))
                        || matches!(next.key(), KEYWORD::Symbol(SYMBOL::RoundC))
                        || next.key().is_terminal()
                    {
                        return Err(ParseError::from_token(
                            &next,
                            "Expected default value expression after '=' in parameter".to_string(),
                        ));
                    }

                    Some(self.parse_logical_expression(tokens)?)
                } else {
                    None
                }
            } else {
                None
            };

            if is_variadic && default.is_some() {
                let token = tokens.curr(false)?;
                return Err(ParseError::from_token(
                    &token,
                    "Variadic parameters cannot have default values".to_string(),
                ));
            }

            for name in names {
                params.push(Parameter {
                    name: name.clone(),
                    param_type: param_type.clone(),
                    is_variadic,
                    is_borrowable: name.chars().all(|ch| {
                        !ch.is_ascii_lowercase() && (ch.is_ascii_alphanumeric() || ch == '_')
                    }),
                    is_mutex,
                    default: default.clone(),
                });
            }

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
            {
                if is_variadic {
                    return Err(ParseError::from_token(
                        &sep,
                        "Variadic parameter must be the last parameter".to_string(),
                    ));
                }
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok((params, first_untyped));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ')' after generic parameter".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Generic parsing exceeded safety bound".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Generic parsing exceeded safety bound".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parameters_to_generics(
        &self,
        params: Vec<Parameter>,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Generic>, ParseError> {
        params
            .into_iter()
            .map(|param| {
                if param.default.is_some() {
                    let error = if let Ok(token) = tokens.curr(false) {
                        ParseError::from_token(
                            &token,
                            "Default values are not allowed in routine generic headers".to_string(),
                        )
                    } else {
                        ParseError {
                            kind: ParseErrorKind::Syntax,
                            message: "Default values are not allowed in routine generic headers"
                                .to_string(),
                            file: None,
                            line: 0,
                            column: 0,
                            length: 0,
                        }
                    };
                    return Err(error);
                }

                if matches!(param.param_type, FolType::Sequence { .. }) {
                    let error = if let Ok(token) = tokens.curr(false) {
                        ParseError::from_token(
                            &token,
                            "Variadic parameters are not allowed in routine generic headers".to_string(),
                        )
                    } else {
                        ParseError {
                            kind: ParseErrorKind::Syntax,
                            message: "Variadic parameters are not allowed in routine generic headers"
                                .to_string(),
                            file: None,
                            line: 0,
                            column: 0,
                            length: 0,
                        }
                    };
                    return Err(error);
                }

                let constraints = if matches!(param.param_type.named_text().as_deref(), Some("any"))
                {
                    Vec::new()
                } else {
                    vec![param.param_type]
                };

                Ok(Generic {
                    name: param.name,
                    constraints,
                })
            })
            .collect()
    }

    pub(super) fn ensure_unique_parameter_names(
        &self,
        params: &[Parameter],
        kind: &str,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<(), ParseError> {
        let mut seen_names = HashSet::new();
        for param in params {
            if !seen_names.insert(canonical_identifier_key(&param.name)) {
                let error = if let Ok(token) = tokens.curr(false) {
                    ParseError::from_token(
                        &token,
                        format!("Duplicate {} name '{}'", kind, param.name),
                    )
                } else {
                    ParseError {
                        kind: ParseErrorKind::Syntax,
                        message: format!("Duplicate {} name '{}'", kind, param.name),
                        file: None,
                        line: 0,
                        column: 0,
                        length: 0,
                    }
                };
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) fn parse_generic_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Generic>, ParseError> {
        self.parse_generic_list_with_close(tokens, SYMBOL::RoundC, ")")
    }

    pub(super) fn parse_generic_list_with_close(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        close_symbol: SYMBOL,
        close_label: &str,
    ) -> Result<Vec<Generic>, ParseError> {
        let mut generics = Vec::new();
        let mut seen_names = HashSet::new();

        for _ in 0..128 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(symbol) if symbol == close_symbol) {
                let _ = tokens.bump();
                return Ok(generics);
            }

            let name = Self::expect_named_label(&token, "Expected generic parameter name")?;
            if !seen_names.insert(canonical_identifier_key(&name)) {
                return Err(ParseError::from_token(
                    &token,
                    format!("Duplicate generic name '{}'", name),
                ));
            }
            let _ = tokens.bump();

            self.skip_ignorable(tokens)?;
            let mut constraints = Vec::new();
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    constraints.push(self.parse_type_reference_tokens(tokens)?);
                }
            }

            generics.push(Generic { name, constraints });

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
            {
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(symbol) if symbol == close_symbol) {
                let _ = tokens.bump();
                return Ok(generics);
            }

            return Err(ParseError::from_token(
                &sep,
                format!(
                    "Expected ',', ';', or '{}' after generic parameter",
                    close_label
                ),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Generic parsing exceeded safety bound".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Generic parsing exceeded safety bound".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_routine_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<FunOption>, ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(vec![FunOption::Mutable]),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Ok(vec![FunOption::Mutable]);
        }
        let _ = tokens.bump();

        let mut options = Vec::new();
        for _ in 0..16 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            Self::reject_illegal_token(&token)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(options);
            }

            let option = match token.con().trim() {
                "+" | "exp" | "export" => FunOption::Export,
                "-" | "hid" | "hidden" => FunOption::Hidden,
                "mut" | "mutable" => FunOption::Mutable,
                "itr" | "iter" | "iterator" => FunOption::Iterator,
                _ => {
                    return Err(ParseError::from_token(
                        &token,
                        "Unknown routine option".to_string(),
                    ))
                }
            };
            options.push(option);
            let _ = tokens.bump();

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            Self::reject_illegal_token(&sep)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::SquarC))
                ) {
                    let _ = tokens.bump();
                    return Ok(options);
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(options);
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ']' in routine options".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Routine options exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Routine options exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_parameter_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Parameter>, ParseError> {
        let mut params = Vec::new();
        let mut seen_names = HashSet::new();

        for _ in 0..512 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(params);
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
                let (function_name, function_type) = self.parse_function_type_signature(tokens)?;
                let Some(param_name) = function_name else {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected named function header in higher-order parameter".to_string(),
                    ));
                };
                if !seen_names.insert(canonical_identifier_key(&param_name)) {
                    return Err(ParseError::from_token(
                        &token,
                        format!("Duplicate parameter name '{}'", param_name),
                    ));
                }

                params.push(Parameter {
                    name: param_name.clone(),
                    param_type: function_type,
                    is_variadic: false,
                    is_borrowable: param_name.chars().all(|ch| {
                        !ch.is_ascii_lowercase() && (ch.is_ascii_alphanumeric() || ch == '_')
                    }),
                    is_mutex: false,
                    default: None,
                });

                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    let _ = tokens.bump();
                    return Ok(params);
                }

                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or ')' after parameter".to_string(),
                ));
            }

            let (names, is_mutex) = self.parse_parameter_name_group(
                tokens,
                "Expected parameter name",
                "Expected parameter name after ','",
                "Expected closing '))' after mutex parameter name",
            )?;
            let first_name = names[0].clone();
            if !seen_names.insert(canonical_identifier_key(&first_name)) {
                return Err(ParseError::from_token(
                    &token,
                    format!("Duplicate parameter name '{}'", first_name),
                ));
            }
            for grouped_name in names.iter().skip(1) {
                if !seen_names.insert(canonical_identifier_key(grouped_name)) {
                    let name_token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &name_token,
                        format!("Duplicate parameter name '{}'", grouped_name),
                    ));
                }
            }

            let _colon = tokens.curr(false)?;
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let mut is_variadic = false;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Operator(OPERATOR::Dotdotdot))
                    || token.con().trim() == "..."
                {
                    is_variadic = true;
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                }
            }

            let base_type = self.parse_type_reference_tokens(tokens)?;
            let param_type = if is_variadic {
                FolType::Sequence {
                    element_type: Box::new(base_type),
                }
            } else {
                base_type
            };
            self.skip_ignorable(tokens)?;

            let default = if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;

                    let next = tokens.curr(false)?;
                    if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Comma))
                        || matches!(next.key(), KEYWORD::Symbol(SYMBOL::Semi))
                        || matches!(next.key(), KEYWORD::Symbol(SYMBOL::RoundC))
                        || next.key().is_terminal()
                    {
                        return Err(ParseError::from_token(
                            &next,
                            "Expected default value expression after '=' in parameter".to_string(),
                        ));
                    }

                    Some(self.parse_logical_expression(tokens)?)
                } else {
                    None
                }
            } else {
                None
            };

            if is_variadic && default.is_some() {
                let token = tokens.curr(false)?;
                return Err(ParseError::from_token(
                    &token,
                    "Variadic parameters cannot have default values".to_string(),
                ));
            }

            for param_name in names {
                params.push(Parameter {
                    name: param_name.clone(),
                    param_type: param_type.clone(),
                    is_variadic,
                    is_borrowable: param_name.chars().all(|ch| {
                        !ch.is_ascii_lowercase() && (ch.is_ascii_alphanumeric() || ch == '_')
                    }),
                    is_mutex,
                    default: default.clone(),
                });
            }

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
            {
                if is_variadic {
                    return Err(ParseError::from_token(
                        &sep,
                        "Variadic parameter must be the last parameter".to_string(),
                    ));
                }
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(params);
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ')' after parameter".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Parameter parsing exceeded safety bound".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Parameter parsing exceeded safety bound".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_routine_name_with_optional_receiver(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_name_message: &str,
    ) -> Result<(Option<FolType>, String, fol_lexer::lexer::stage3::element::Element), ParseError> {
        let mut receiver_type = None;
        let current = tokens.curr(false)?;

        if matches!(current.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let receiver_token = tokens.curr(false)?;
            receiver_type = Some(self.parse_type_reference_tokens(tokens)?);
            match receiver_type.as_ref() {
                Some(other)
                    if matches!(
                        other.named_text().as_deref(),
                        Some(name) if !matches!(name, "any" | "none" | "non")
                    ) => {}
                Some(FolType::Int { .. })
                | Some(FolType::Float { .. })
                | Some(FolType::Bool)
                | Some(FolType::Char { .. }) => {}
                Some(FolType::Named { .. })
                | Some(FolType::QualifiedNamed { .. })
                | Some(FolType::Any)
                | Some(FolType::None) => {
                    return Err(ParseError::from_token(
                        &receiver_token,
                        "Method receiver type cannot be any, non, or none".to_string(),
                    ));
                }
                Some(_) => {}
                None => unreachable!("receiver_type is set above"),
            }

            self.skip_ignorable(tokens)?;
            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected ')' after method receiver type".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
        }

        let name_token = tokens.curr(false)?;
        let name = Self::expect_named_label(&name_token, missing_name_message)?;
        let _ = tokens.bump();
        Ok((receiver_type, name, name_token))
    }
}
