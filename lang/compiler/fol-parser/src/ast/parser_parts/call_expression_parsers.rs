use super::*;

impl AstParser {
    pub(super) fn lookahead_is_dot_builtin_call(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::Dot)) {
            return false;
        }

        let mut saw_name = false;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if !saw_name {
                if Self::token_can_be_logical_name(&key)
                    || key.is_textual_literal()
                    || key.is_illegal()
                {
                    saw_name = true;
                    continue;
                }
                return false;
            }

            return matches!(key, KEYWORD::Symbol(SYMBOL::RoundO));
        }

        false
    }

    pub(super) fn parse_dot_builtin_call_expr(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let dot = tokens.curr(false)?;
        if !matches!(dot.key(), KEYWORD::Symbol(SYMBOL::Dot)) {
            return Err(ParseError::from_token(
                &dot,
                "Expected '.' to start builtin root call".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let name_token = tokens.curr(false)?;
        let name = Self::expect_named_label(&name_token, "Expected builtin call name after '.'")?;
        let _ = tokens.bump();

        let args =
            self.parse_open_paren_and_call_args(tokens, "Expected '(' after builtin call name")?;

        Ok(AstNode::FunctionCall {
            syntax_id: self.record_syntax_origin(&name_token),
            surface: crate::ast::CallSurface::DotIntrinsic,
            name,
            type_args: Vec::new(),
            args,
        })
    }

    fn lookahead_is_named_call_arg(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !(Self::token_to_named_label(&current).is_some() || current.key().is_illegal()) {
            return false;
        }

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };
            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }
            return matches!(key, KEYWORD::Symbol(SYMBOL::Equal));
        }

        false
    }

    fn parse_call_argument(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let current = tokens.curr(false)?;
        if current.con().trim() == "..." {
            let _ = tokens.bump();
            self.skip_layout(tokens)?;

            let value_token = tokens.curr(false)?;
            if matches!(
                value_token.key(),
                KEYWORD::Symbol(SYMBOL::RoundC)
                    | KEYWORD::Symbol(SYMBOL::Comma)
                    | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                return Err(ParseError::from_token(
                    &value_token,
                    "Expected expression after '...' in call arguments".to_string(),
                ));
            }

            let value = self.parse_logical_expression(tokens)?;
            return Ok(AstNode::Unpack {
                value: Box::new(value),
            });
        }

        if self.lookahead_is_named_call_arg(tokens) {
            let name_token = tokens.curr(false)?;
            let name = Self::expect_named_label(&name_token, "Expected argument name before '='")?;
            let _ = tokens.bump();
            self.skip_layout(tokens)?;

            let equal = tokens.curr(false)?;
            if !matches!(equal.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Err(ParseError::from_token(
                    &equal,
                    "Expected '=' after named call argument".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_layout(tokens)?;

            let value = self.parse_logical_expression(tokens)?;
            return Ok(AstNode::NamedArgument {
                name,
                value: Box::new(value),
            });
        }

        self.parse_logical_expression(tokens)
    }

    fn ast_node_is_named_argument(node: &AstNode) -> bool {
        match node {
            AstNode::NamedArgument { .. } => true,
            AstNode::Commented { node, .. } => Self::ast_node_is_named_argument(node.as_ref()),
            _ => false,
        }
    }

    fn ast_node_is_unpack_argument(node: &AstNode) -> bool {
        match node {
            AstNode::Unpack { .. } => true,
            AstNode::Commented { node, .. } => Self::ast_node_is_unpack_argument(node.as_ref()),
            _ => false,
        }
    }

    pub(super) fn parse_call_args(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let mut args = Vec::new();
        let mut seen_named_arg = false;
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
                args.extend(pending_comments);
                let _ = tokens.bump();
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                if !pending_comments.is_empty() {
                    args.extend(pending_comments);
                }
                let _ = tokens.bump();
                break;
            }

            let mut arg = self.parse_call_argument(tokens)?;
            arg = self.attach_leading_comments(arg, pending_comments);
            arg = self.attach_trailing_comments(
                arg,
                self.collect_comments_before(tokens, |key| {
                    matches!(
                        key,
                        KEYWORD::Symbol(SYMBOL::Comma)
                            | KEYWORD::Symbol(SYMBOL::Semi)
                            | KEYWORD::Symbol(SYMBOL::RoundC)
                    )
                })?,
            );

            let is_named = Self::ast_node_is_named_argument(&arg);
            let is_unpack = Self::ast_node_is_unpack_argument(&arg);
            if !is_named && !is_unpack && seen_named_arg {
                return Err(ParseError::from_token(
                    &token,
                    "Positional call arguments are not allowed after named arguments".to_string(),
                ));
            }
            seen_named_arg |= is_named;
            args.push(arg);
            self.skip_layout(tokens)?;

            let sep = tokens.curr(false)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::RoundC))
                ) {
                    let _ = tokens.bump();
                    break;
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                break;
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ')' in call arguments".to_string(),
            ));
        }

        Ok(args)
    }
}
