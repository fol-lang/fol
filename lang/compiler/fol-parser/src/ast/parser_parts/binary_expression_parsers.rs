use super::*;

impl AstParser {
    pub(super) fn parse_logical_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        self.parse_pipe_expression(tokens)
    }

    pub(super) fn parse_logical_or_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_logical_xor_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Keyword(BUILDIN::Or) | KEYWORD::Keyword(BUILDIN::Nor)
                )
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            if self.token_is_word(&op_token, "or") {
                self.consume_significant_token(tokens);
                let rhs = self.parse_logical_xor_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op: BinaryOperator::Or,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            if self.token_is_word(&op_token, "nor") {
                self.consume_significant_token(tokens);
                let rhs = self.parse_logical_xor_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::UnaryOp {
                        op: UnaryOperator::Not,
                        operand: Box::new(AstNode::BinaryOp {
                            op: BinaryOperator::Or,
                            left: Box::new(lhs),
                            right: Box::new(rhs),
                        }),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_logical_xor_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_logical_and_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(key, KEYWORD::Keyword(BUILDIN::Xor))
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            if self.token_is_word(&op_token, "xor") {
                self.consume_significant_token(tokens);
                let rhs = self.parse_logical_and_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op: BinaryOperator::Xor,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_logical_and_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_comparison_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Keyword(BUILDIN::And) | KEYWORD::Keyword(BUILDIN::Nand)
                )
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            if self.token_is_word(&op_token, "and") {
                self.consume_significant_token(tokens);
                let rhs = self.parse_comparison_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op: BinaryOperator::And,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            if self.token_is_word(&op_token, "nand") {
                self.consume_significant_token(tokens);
                let rhs = self.parse_comparison_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::UnaryOp {
                        op: UnaryOperator::Not,
                        operand: Box::new(AstNode::BinaryOp {
                            op: BinaryOperator::And,
                            left: Box::new(lhs),
                            right: Box::new(rhs),
                        }),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_comparison_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_range_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Keyword(BUILDIN::Cast)
                        | KEYWORD::Keyword(BUILDIN::As)
                        | KEYWORD::Keyword(BUILDIN::Is)
                        | KEYWORD::Keyword(BUILDIN::Has)
                        | KEYWORD::Keyword(BUILDIN::In)
                        | KEYWORD::Operator(OPERATOR::Equal)
                        | KEYWORD::Operator(OPERATOR::Noteq)
                        | KEYWORD::Operator(OPERATOR::Greateq)
                        | KEYWORD::Operator(OPERATOR::Lesseq)
                        | KEYWORD::Symbol(SYMBOL::AngleC)
                        | KEYWORD::Symbol(SYMBOL::AngleO)
                        | KEYWORD::Symbol(SYMBOL::Equal)
                        | KEYWORD::Symbol(SYMBOL::Bang)
                )
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            let next_key = self.next_significant_key_from_window(tokens);
            let op_text = op_token.con().trim().to_string();
            let (binary_op, consume_count) = match op_token.key() {
                KEYWORD::Keyword(BUILDIN::Cast) => (Some(BinaryOperator::Cast), 1),
                KEYWORD::Keyword(BUILDIN::As) => (Some(BinaryOperator::As), 1),
                KEYWORD::Keyword(BUILDIN::Is) => (Some(BinaryOperator::Is), 1),
                KEYWORD::Keyword(BUILDIN::Has) => (Some(BinaryOperator::Has), 1),
                KEYWORD::Keyword(BUILDIN::In) => (Some(BinaryOperator::In), 1),
                KEYWORD::Operator(OPERATOR::Equal) => (Some(BinaryOperator::Eq), 1),
                KEYWORD::Operator(OPERATOR::Noteq) => (Some(BinaryOperator::Ne), 1),
                KEYWORD::Operator(OPERATOR::Greateq) => (Some(BinaryOperator::Ge), 1),
                KEYWORD::Operator(OPERATOR::Lesseq) => (Some(BinaryOperator::Le), 1),
                KEYWORD::Symbol(SYMBOL::AngleC) => {
                    if matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::Equal))) {
                        (Some(BinaryOperator::Ge), 2)
                    } else {
                        (Some(BinaryOperator::Gt), 1)
                    }
                }
                KEYWORD::Symbol(SYMBOL::AngleO) => {
                    if matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::Equal))) {
                        (Some(BinaryOperator::Le), 2)
                    } else {
                        (Some(BinaryOperator::Lt), 1)
                    }
                }
                KEYWORD::Symbol(SYMBOL::Equal) => {
                    if matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::Equal))) {
                        (Some(BinaryOperator::Eq), 2)
                    } else {
                        (None, 0)
                    }
                }
                KEYWORD::Symbol(SYMBOL::Bang) => {
                    if matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::Equal))) {
                        (Some(BinaryOperator::Ne), 2)
                    } else {
                        (None, 0)
                    }
                }
                _ => match op_text.as_str() {
                    ">=" => (Some(BinaryOperator::Ge), 1),
                    "<=" => (Some(BinaryOperator::Le), 1),
                    "==" => (Some(BinaryOperator::Eq), 1),
                    "!=" => (Some(BinaryOperator::Ne), 1),
                    ">" => (Some(BinaryOperator::Gt), 1),
                    "<" => (Some(BinaryOperator::Lt), 1),
                    _ => (None, 0),
                },
            };

            if let Some(op) = binary_op {
                for _ in 0..consume_count {
                    self.consume_significant_token(tokens);
                }
                let rhs = self.parse_range_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_range_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let leading_comments = self.collect_comment_nodes(tokens)?;
        if let Ok(token) = tokens.curr(false) {
            let is_open_start_range = matches!(
                token.key(),
                KEYWORD::Operator(OPERATOR::Dotdot) | KEYWORD::Operator(OPERATOR::Dotdotdot)
            ) || matches!(token.con().trim(), ".." | "...");
            let inclusive = !matches!(token.key(), KEYWORD::Operator(OPERATOR::Dotdotdot))
                && token.con().trim() != "...";
            if is_open_start_range {
                let operator_token = token.clone();
                let _ = tokens.bump();
                self.skip_layout(tokens)?;

                let next = tokens.curr(false)?;
                if next.key().is_terminal()
                    || matches!(
                        next.key(),
                        KEYWORD::Symbol(SYMBOL::Comma)
                            | KEYWORD::Symbol(SYMBOL::RoundC)
                            | KEYWORD::Symbol(SYMBOL::CurlyC)
                            | KEYWORD::Symbol(SYMBOL::SquarC)
                    )
                {
                    return Err(ParseError::from_token(
                        &operator_token,
                        "Expected expression after '..'".to_string(),
                    ));
                }

                let rhs = self.parse_add_sub_expression(tokens)?;
                return Ok(self.attach_leading_comments(
                    AstNode::Range {
                        start: None,
                        end: Some(Box::new(rhs)),
                        inclusive,
                    },
                    leading_comments,
                ));
            }
        }

        let lhs = self.parse_add_sub_expression(tokens)?;
        let operator_comments = self.collect_comments_before(tokens, |key| {
            matches!(
                key,
                KEYWORD::Operator(OPERATOR::Dotdot) | KEYWORD::Operator(OPERATOR::Dotdotdot)
            )
        })?;

        let op_token = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(self.attach_leading_comments(lhs, leading_comments)),
        };

        let is_range = matches!(
            op_token.key(),
            KEYWORD::Operator(OPERATOR::Dotdot) | KEYWORD::Operator(OPERATOR::Dotdotdot)
        ) || matches!(op_token.con().trim(), ".." | "...");
        if !is_range {
            return Ok(self.attach_leading_comments(lhs, leading_comments));
        }
        let inclusive = !matches!(op_token.key(), KEYWORD::Operator(OPERATOR::Dotdotdot))
            && op_token.con().trim() != "...";

        let _ = tokens.bump();
        self.skip_layout(tokens)?;

        let next = tokens.curr(false)?;
        if matches!(
            next.key(),
            KEYWORD::Symbol(SYMBOL::Comma)
                | KEYWORD::Symbol(SYMBOL::RoundC)
                | KEYWORD::Symbol(SYMBOL::CurlyC)
                | KEYWORD::Symbol(SYMBOL::SquarC)
        ) {
            return Ok(self.attach_leading_comments(
                AstNode::Range {
                    start: Some(Box::new(lhs)),
                    end: None,
                    inclusive,
                },
                {
                    let mut comments = leading_comments;
                    comments.extend(operator_comments);
                    comments
                },
            ));
        }
        if next.key().is_terminal() {
            return Err(ParseError::from_token(
                &op_token,
                format!("Expected expression after '{}'", op_token.con().trim()),
            ));
        }

        let rhs = self.parse_add_sub_expression(tokens)?;
        Ok(self.attach_leading_comments(
            AstNode::Range {
                start: Some(Box::new(lhs)),
                end: Some(Box::new(rhs)),
                inclusive,
            },
            {
                let mut comments = leading_comments;
                comments.extend(operator_comments);
                comments
            },
        ))
    }

    pub(super) fn next_significant_key_from_window(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Option<KEYWORD> {
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            return Some(key);
        }

        None
    }

    pub(super) fn consume_significant_token(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) {
        for _ in 0..16 {
            if tokens.bump().is_none() {
                break;
            }

            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if !Self::key_is_soft_ignorable(&token.key()) {
                break;
            }
        }
    }

    pub(super) fn parse_add_sub_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_mul_div_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Operator(OPERATOR::Add)
                        | KEYWORD::Symbol(SYMBOL::Plus)
                        | KEYWORD::Operator(OPERATOR::Abstract)
                        | KEYWORD::Symbol(SYMBOL::Minus)
                )
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            let binary_op = match op_token.key() {
                KEYWORD::Operator(OPERATOR::Add) | KEYWORD::Symbol(SYMBOL::Plus) => {
                    Some(BinaryOperator::Add)
                }
                KEYWORD::Operator(OPERATOR::Abstract) | KEYWORD::Symbol(SYMBOL::Minus) => {
                    Some(BinaryOperator::Sub)
                }
                _ => None,
            };

            if let Some(op) = binary_op {
                let _ = tokens.bump();
                let rhs = self.parse_mul_div_expression(tokens)?;

                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_mul_div_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let mut lhs = self.parse_pow_expression(tokens)?;

        for _ in 0..32 {
            let leading_comments = self.collect_comments_before(tokens, |key| {
                matches!(
                    key,
                    KEYWORD::Operator(OPERATOR::Multiply)
                        | KEYWORD::Symbol(SYMBOL::Star)
                        | KEYWORD::Operator(OPERATOR::Divide)
                        | KEYWORD::Symbol(SYMBOL::Root)
                        | KEYWORD::Symbol(SYMBOL::Percent)
                )
            })?;

            let op_token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => return Ok(lhs),
            };

            let binary_op = match op_token.key() {
                KEYWORD::Operator(OPERATOR::Multiply) | KEYWORD::Symbol(SYMBOL::Star) => {
                    Some(BinaryOperator::Mul)
                }
                KEYWORD::Operator(OPERATOR::Divide) | KEYWORD::Symbol(SYMBOL::Root) => {
                    Some(BinaryOperator::Div)
                }
                KEYWORD::Symbol(SYMBOL::Percent) => Some(BinaryOperator::Mod),
                _ => None,
            };

            if let Some(op) = binary_op {
                let _ = tokens.bump();
                let rhs = self.parse_pow_expression(tokens)?;
                lhs = self.attach_leading_comments(
                    AstNode::BinaryOp {
                        op,
                        left: Box::new(lhs),
                        right: Box::new(rhs),
                    },
                    leading_comments,
                );
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    pub(super) fn parse_pow_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let lhs = self.parse_primary_expression(tokens)?;
        let leading_comments = self.collect_comments_before(tokens, |key| {
            matches!(key, KEYWORD::Symbol(SYMBOL::Carret))
        })?;

        let op_token = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(lhs),
        };

        if !matches!(op_token.key(), KEYWORD::Symbol(SYMBOL::Carret)) {
            return Ok(lhs);
        }

        let _ = tokens.bump();
        let rhs = self.parse_pow_expression(tokens)?;
        Ok(self.attach_leading_comments(
            AstNode::BinaryOp {
                op: BinaryOperator::Pow,
                left: Box::new(lhs),
                right: Box::new(rhs),
            },
            leading_comments,
        ))
    }
}
