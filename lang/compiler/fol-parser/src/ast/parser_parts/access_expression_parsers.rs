use super::*;

impl AstParser {
    fn channel_endpoint_from_pattern(
        &self,
        pattern: &AstNode,
    ) -> Option<crate::ast::ChannelEndpoint> {
        match pattern {
            AstNode::Identifier { name, .. } if name == "tx" => {
                Some(crate::ast::ChannelEndpoint::Tx)
            }
            AstNode::Identifier { name, .. } if name == "rx" => {
                Some(crate::ast::ChannelEndpoint::Rx)
            }
            _ => None,
        }
    }

    fn parse_access_pattern(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        self.skip_ignorable(tokens)?;

        let pattern = if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Star))
        ) {
            let _ = tokens.bump();
            AstNode::PatternWildcard
        } else {
            self.parse_logical_expression(tokens)?
        };

        self.skip_ignorable(tokens)?;
        let token = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(pattern),
        };

        if !matches!(token.key(), KEYWORD::Operator(OPERATOR::Flow)) {
            return Ok(pattern);
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let binding_token = tokens.curr(false)?;
        let binding = Self::expect_named_label(
            &binding_token,
            "Expected binding name after '=>' in access pattern",
        )?;
        let _ = tokens.bump();

        Ok(AstNode::PatternCapture {
            pattern: Box::new(pattern),
            binding,
        })
    }

    fn pattern_access_from_patterns(&self, node: AstNode, mut patterns: Vec<AstNode>) -> AstNode {
        if patterns.len() == 1
            && !matches!(
                patterns.first(),
                Some(AstNode::PatternWildcard | AstNode::PatternCapture { .. })
            )
        {
            if let Some(endpoint) =
                self.channel_endpoint_from_pattern(patterns.first().expect("single pattern exists"))
            {
                return AstNode::ChannelAccess {
                    channel: Box::new(node),
                    endpoint,
                };
            }

            return AstNode::IndexAccess {
                container: Box::new(node),
                index: Box::new(patterns.pop().expect("single pattern")),
            };
        }

        AstNode::PatternAccess {
            container: Box::new(node),
            patterns,
        }
    }

    pub(super) fn token_can_start_path_expression(
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> bool {
        if Self::token_to_named_label(token).is_none() {
            return false;
        }

        let text = token.con().trim();
        let numeric = text
            .strip_prefix(['+', '-'])
            .unwrap_or(text)
            .chars()
            .all(|c| c.is_ascii_digit());

        !numeric
    }

    fn consume_slice_separator(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Option<bool>, ParseError> {
        self.skip_ignorable(tokens)?;
        let token = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(None),
        };

        if matches!(token.key(), KEYWORD::Operator(OPERATOR::Path)) {
            let _ = tokens.bump();
            return Ok(Some(true));
        }

        if !matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
            return Ok(None);
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Colon) | KEYWORD::Operator(OPERATOR::Path))
        ) {
            let _ = tokens.bump();
            return Ok(Some(true));
        }

        Ok(Some(false))
    }

    fn parse_optional_slice_end(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Option<Box<AstNode>>, ParseError> {
        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::SquarC))
        ) {
            Ok(None)
        } else {
            Ok(Some(Box::new(self.parse_logical_expression(tokens)?)))
        }
    }

    pub(super) fn parse_qualified_path(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        expected_root_error: &str,
        expected_segment_error: &str,
    ) -> Result<QualifiedPath, ParseError> {
        let root = tokens.curr(false)?;
        let syntax_id = self.record_syntax_origin(&root);
        let mut segments = vec![Self::expect_named_label(&root, expected_root_error)?];
        let _ = tokens.bump();

        loop {
            self.skip_ignorable(tokens)?;
            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if !matches!(token.key(), KEYWORD::Operator(OPERATOR::Path)) {
                break;
            }

            // Turbofish escape: `ident::[TypeArgs](args)` is an explicit
            // generic call, not a continuation of the qualified path. Leave
            // `::` in place so the postfix expression parser can take it.
            if matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::SquarO))
            ) {
                break;
            }

            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let segment = tokens.curr(false)?;
            let segment_name = Self::expect_named_label(&segment, expected_segment_error)?;
            segments.push(segment_name);
            let _ = tokens.bump();
        }

        Ok(QualifiedPath::with_syntax_id(segments, syntax_id))
    }

    pub(super) fn parse_index_or_slice_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        node: AstNode,
    ) -> Result<AstNode, ParseError> {
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        if let Some(reverse) = self.consume_slice_separator(tokens)? {
            let end = self.parse_optional_slice_end(tokens)?;
            self.skip_ignorable(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' for slice expression".to_string(),
                ));
            }

            let _ = tokens.bump();
            return Ok(AstNode::SliceAccess {
                container: Box::new(node),
                start: None,
                end,
                reverse,
            });
        }

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::SquarC))
        ) {
            let _ = tokens.bump();
            return Ok(AstNode::PatternAccess {
                container: Box::new(node),
                patterns: Vec::new(),
            });
        }

        let start = self.parse_access_pattern(tokens)?;
        self.skip_ignorable(tokens)?;

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi))
        ) {
            let mut patterns = vec![start.clone()];
            for _ in 0..64 {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::SquarC))
                ) {
                    let _ = tokens.bump();
                    return Ok(AstNode::PatternAccess {
                        container: Box::new(node),
                        patterns,
                    });
                }
                patterns.push(self.parse_access_pattern(tokens)?);
                self.skip_ignorable(tokens)?;

                let token = tokens.curr(false)?;
                if matches!(
                    token.key(),
                    KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
                ) {
                    continue;
                }
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                    let _ = tokens.bump();
                    return Ok(AstNode::PatternAccess {
                        container: Box::new(node),
                        patterns,
                    });
                }

                return Err(ParseError::from_token(
                    &token,
                    "Expected ',', ';', or ']' in pattern access".to_string(),
                ));
            }
        }

        if let Some(reverse) = self.consume_slice_separator(tokens)? {
            let end = self.parse_optional_slice_end(tokens)?;
            self.skip_ignorable(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' for slice expression".to_string(),
                ));
            }

            let _ = tokens.bump();
            return Ok(AstNode::SliceAccess {
                container: Box::new(node),
                start: Some(Box::new(start)),
                end,
                reverse,
            });
        }

        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing ']' for index expression".to_string(),
            ));
        }

        let _ = tokens.bump();
        Ok(self.pattern_access_from_patterns(node, vec![start]))
    }

    pub(super) fn parse_index_or_slice_assignment_target(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        node: AstNode,
    ) -> Result<AstNode, ParseError> {
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        if let Some(reverse) = self.consume_slice_separator(tokens)? {
            let end = self.parse_optional_slice_end(tokens)?;
            self.skip_ignorable(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' for slice assignment target".to_string(),
                ));
            }

            let _ = tokens.bump();
            return Ok(AstNode::SliceAccess {
                container: Box::new(node),
                start: None,
                end,
                reverse,
            });
        }

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::SquarC))
        ) {
            let _ = tokens.bump();
            return Ok(AstNode::PatternAccess {
                container: Box::new(node),
                patterns: Vec::new(),
            });
        }

        let start = self.parse_access_pattern(tokens)?;
        self.skip_ignorable(tokens)?;

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi))
        ) {
            let mut patterns = vec![start.clone()];
            for _ in 0..64 {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::SquarC))
                ) {
                    let _ = tokens.bump();
                    return Ok(AstNode::PatternAccess {
                        container: Box::new(node),
                        patterns,
                    });
                }
                patterns.push(self.parse_access_pattern(tokens)?);
                self.skip_ignorable(tokens)?;

                let token = tokens.curr(false)?;
                if matches!(
                    token.key(),
                    KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
                ) {
                    continue;
                }
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                    let _ = tokens.bump();
                    return Ok(AstNode::PatternAccess {
                        container: Box::new(node),
                        patterns,
                    });
                }

                return Err(ParseError::from_token(
                    &token,
                    "Expected ',', ';', or ']' in pattern assignment target".to_string(),
                ));
            }
        }

        if let Some(reverse) = self.consume_slice_separator(tokens)? {
            let end = self.parse_optional_slice_end(tokens)?;
            self.skip_ignorable(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' for slice assignment target".to_string(),
                ));
            }

            let _ = tokens.bump();
            return Ok(AstNode::SliceAccess {
                container: Box::new(node),
                start: Some(Box::new(start)),
                end,
                reverse,
            });
        }

        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing ']' for index assignment target".to_string(),
            ));
        }

        let _ = tokens.bump();
        Ok(self.pattern_access_from_patterns(node, vec![start]))
    }

    pub(super) fn parse_prefix_availability_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        node: AstNode,
    ) -> Result<AstNode, ParseError> {
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' after ':' in availability expression".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let mut patterns = Vec::new();
        for _ in 0..64 {
            if matches!(
                tokens.curr(false).map(|token| token.key()),
                Ok(KEYWORD::Symbol(SYMBOL::SquarC))
            ) {
                let _ = tokens.bump();
                break;
            }
            patterns.push(self.parse_access_pattern(tokens)?);
            self.skip_ignorable(tokens)?;

            let token = tokens.curr(false)?;
            if matches!(
                token.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                continue;
            }
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                break;
            }

            return Err(ParseError::from_token(
                &token,
                "Expected ',', ';', or ']' in availability expression".to_string(),
            ));
        }

        let target = self.pattern_access_from_patterns(node, patterns);

        Ok(AstNode::AvailabilityAccess {
            target: Box::new(target),
        })
    }
}
