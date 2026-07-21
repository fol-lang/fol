use super::*;

impl AstParser {
    fn lookahead_is_record_init_field(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !(Self::token_to_named_label(&current).is_some() || current.key().is_illegal()) {
            return false;
        }

        matches!(
            self.next_significant_key_from_window(tokens),
            Some(KEYWORD::Symbol(SYMBOL::Equal))
        )
    }

    fn parse_record_init_fields_after_open(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        open_token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<AstNode, ParseError> {
        let mut fields = Vec::new();
        let mut closed = false;
        for _ in 0..256 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                closed = true;
                break;
            }

            let name =
                Self::expect_named_label(&token, "Expected field name in record initializer")?;
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let equal = tokens.curr(false)?;
            if !matches!(equal.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Err(ParseError::from_token(
                    &equal,
                    "Expected '=' after record initializer field name".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;

            let value = self.parse_logical_expression(tokens)?;
            fields.push(crate::ast::RecordInitField { name, value });
            self.skip_ignorable(tokens)?;

            let sep = tokens.curr(false)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
                ) {
                    let _ = tokens.bump();
                    closed = true;
                    break;
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                closed = true;
                break;
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in record initializer".to_string(),
            ));
        }

        if !closed {
            return Err(ParseError::from_token(
                open_token,
                "Record initializer exceeds maximum field count (256)".to_string(),
            ));
        }

        Ok(AstNode::RecordInit {
            syntax_id: self.record_syntax_origin(open_token),
            fields,
        })
    }

    pub(super) fn lookahead_is_spawn_expression(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return false;
        }

        // `[>]` shorthand marker.
        if matches!(
            (
                tokens.peek(0, false).map(|token| token.key()),
                tokens.peek(1, false).map(|token| token.key()),
            ),
            (
                Ok(KEYWORD::Symbol(SYMBOL::AngleC)),
                Ok(KEYWORD::Symbol(SYMBOL::SquarC)),
            )
        ) {
            return true;
        }

        // `[spn]` canonical scoped-spawn marker (`[>]` is its shorthand), and
        // `[spn, det]` detached-spawn marker (V3_PROC).
        let Ok(first) = tokens.peek(0, true) else {
            return false;
        };
        if first.con().trim() != "spn" {
            return false;
        }
        matches!(
            tokens.peek(1, true).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::SquarC | SYMBOL::Comma))
        )
    }

    /// A prefix ownership operation is `[opt, ...]operand` where the bracket
    /// holds only canonical ownership options (`mov`, `cpy`, `cln`, `bor`,
    /// `mut`, `new`, `weak`, `upg`, `fin`). It is distinguished from a container
    /// literal by the first inner token being an option keyword followed by `]`,
    /// `,`, or `;`, which no plain container literal element produces.
    pub(super) fn lookahead_is_ownership_operation(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let Ok(current) = tokens.curr(false) else {
            return false;
        };
        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return false;
        }
        let Ok(first) = tokens.peek(0, true) else {
            return false;
        };
        if crate::ast::options::OwnershipOption::from_keyword(first.con().trim()).is_none()
            && Self::unary_bracket_operator(first.con().trim()).is_none()
        {
            return false;
        }
        let Ok(second) = tokens.peek(1, true) else {
            return false;
        };
        matches!(
            second.key(),
            KEYWORD::Symbol(SYMBOL::SquarC | SYMBOL::Comma | SYMBOL::Semi)
        )
    }

    /// Bracket unary operations (V3: no raw symbols on values). Each is a
    /// standalone single-option bracket that maps to an ordinary unary operator:
    /// `[uwp]x` shell unwrap, `[drf]x` dereference, `[ref]x` reference, `[end]x`
    /// give-back a borrow. They are NOT composable with ownership options.
    fn unary_bracket_operator(keyword: &str) -> Option<UnaryOperator> {
        match keyword {
            "uwp" => Some(UnaryOperator::Unwrap),
            "drf" => Some(UnaryOperator::Deref),
            "ref" => Some(UnaryOperator::Ref),
            "end" => Some(UnaryOperator::GiveBack),
            _ => None,
        }
    }

    pub(super) fn parse_ownership_operation(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open = tokens.curr(false)?;
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        // A standalone bracket unary op `[uwp|drf|ref|end]operand` parses to a
        // plain unary operation rather than an ownership operation.
        let first = tokens.curr(false)?;
        if let Some(unary_op) = Self::unary_bracket_operator(first.con().trim()) {
            if matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::SquarC))
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                let operand = self.parse_primary_expression(tokens)?;
                return Ok(Self::rebase_unary_over_method_receiver(unary_op, operand));
            }
        }
        let mut options: Vec<crate::ast::options::OwnershipOption> = Vec::new();
        for _ in 0..12 {
            self.skip_ignorable(tokens)?;
            let option = tokens.curr(false)?;
            if matches!(option.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &option,
                    "empty ownership operation '[]'; expected an option like 'mov'".to_string(),
                ));
            }
            let Some(op) = crate::ast::options::OwnershipOption::from_keyword(option.con().trim())
            else {
                return Err(ParseError::from_token(
                    &option,
                    format!(
                        "Unknown ownership option '{}'; expected mov, cpy, cln, bor, mut, new, weak, upg, or fin",
                        option.con().trim()
                    ),
                ));
            };
            if options.contains(&op) {
                return Err(ParseError::from_token(
                    &option,
                    format!("Duplicate ownership option '{}'", op.canonical()),
                ));
            }
            options.push(op);
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let separator = tokens.curr(false)?;
            if matches!(separator.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                let operand = self.parse_primary_expression(tokens)?;
                return Ok(Self::rebase_ownership_over_method_receiver(
                    options, operand,
                ));
            }
            if matches!(
                separator.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                continue;
            }
            return Err(ParseError::from_token(
                &separator,
                "Expected ',', ';', or ']' in ownership operation".to_string(),
            ));
        }
        Err(ParseError::from_token(
            &open,
            "ownership operation exceeded parser limit".to_string(),
        ))
    }

    /// The unary-bracket sibling of [`Self::rebase_ownership_over_method_receiver`]:
    /// `[drf]`/`[uwp]`/`[ref]`/`[end]` bind the same way. A trailing method call
    /// rebases onto the receiver — `[drf]ptr.method()` groups as
    /// `([drf]ptr).method()` — while a pure place chain keeps the op over the
    /// place, so `[drf]holder.link` still dereferences the pointer *field*
    /// `holder.link` (not the record `holder`). Recurses through `Commented`.
    fn rebase_unary_over_method_receiver(op: UnaryOperator, operand: AstNode) -> AstNode {
        match operand {
            AstNode::MethodCall {
                syntax_id,
                object,
                method,
                args,
            } => AstNode::MethodCall {
                syntax_id,
                object: Box::new(AstNode::UnaryOp {
                    op,
                    operand: object,
                }),
                method,
                args,
            },
            AstNode::Commented {
                leading_comments,
                node,
                trailing_comments,
            } => AstNode::Commented {
                leading_comments,
                node: Box::new(Self::rebase_unary_over_method_receiver(op, *node)),
                trailing_comments,
            },
            other => AstNode::UnaryOp {
                op,
                operand: Box::new(other),
            },
        }
    }

    /// An ownership op annotates a *place*; a trailing method call consumes that
    /// place as its receiver. So `[op]recv.method(args)` groups as
    /// `([op]recv).method(args)`, not `[op](recv.method(args))` (which would
    /// borrow/move the call's return value). Pure place chains (`.field`, `[i]`)
    /// and non-method operands keep the op wrapping the whole operand, so
    /// partial moves like `[mov]bundle.held` and element moves like `[mov]arr[i]`
    /// are unchanged. Recurses through `Commented` wrappers so a comment between
    /// the op and its operand does not defeat the rebase.
    fn rebase_ownership_over_method_receiver(
        options: Vec<crate::ast::options::OwnershipOption>,
        operand: AstNode,
    ) -> AstNode {
        match operand {
            AstNode::MethodCall {
                syntax_id,
                object,
                method,
                args,
            } => AstNode::MethodCall {
                syntax_id,
                object: Box::new(AstNode::OwnershipOp {
                    syntax_id: None,
                    options,
                    operand: object,
                }),
                method,
                args,
            },
            AstNode::Commented {
                leading_comments,
                node,
                trailing_comments,
            } => AstNode::Commented {
                leading_comments,
                node: Box::new(Self::rebase_ownership_over_method_receiver(options, *node)),
                trailing_comments,
            },
            other => AstNode::OwnershipOp {
                syntax_id: None,
                options,
                operand: Box::new(other),
            },
        }
    }

    pub(super) fn parse_primary_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let leading_comments = self.collect_comment_nodes(tokens)?;
        let token = tokens.curr(false)?;

        if token.key().is_illegal() {
            return Err(ParseError::from_token(
                &token,
                format!("Parser encountered illegal token '{}'", token.con()),
            ));
        }

        if self.lookahead_is_spawn_expression(tokens) {
            self.consume_significant_token(tokens);
            self.skip_ignorable(tokens)?;

            // The marker is `>` (shorthand) or `spn` (canonical scoped spawn).
            let marker = tokens.curr(false)?;
            let is_spn = if matches!(marker.key(), KEYWORD::Symbol(SYMBOL::AngleC)) {
                self.consume_significant_token(tokens);
                false
            } else if marker.con().trim() == "spn" {
                self.consume_significant_token(tokens);
                true
            } else {
                return Err(ParseError::from_token(
                    &marker,
                    "Expected '>' or 'spn' in spawn marker".to_string(),
                ));
            };
            self.skip_ignorable(tokens)?;

            // `[spn, det]` marks a detached task (not exit-joined). `det` is only
            // valid after the canonical `spn` marker, never the `[>]` shorthand.
            let mut detached = false;
            if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                if !is_spn {
                    return Err(ParseError::from_token(
                        &tokens.curr(false)?,
                        "detached spawn requires the canonical marker: write '[spn, det]'"
                            .to_string(),
                    ));
                }
                self.consume_significant_token(tokens);
                self.skip_ignorable(tokens)?;
                let det = tokens.curr(false)?;
                if det.con().trim() != "det" {
                    return Err(ParseError::from_token(
                        &det,
                        "Expected 'det' after 'spn,' in a detached spawn marker".to_string(),
                    ));
                }
                self.consume_significant_token(tokens);
                self.skip_ignorable(tokens)?;
                detached = true;
            }

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ']' in spawn marker".to_string(),
                ));
            }
            self.consume_significant_token(tokens);
            self.skip_layout(tokens)?;

            let task = self.parse_primary_expression(tokens)?;
            return Ok(self.attach_leading_comments(
                AstNode::Spawn {
                    task: Box::new(task),
                    detached,
                },
                leading_comments,
            ));
        }

        if self.lookahead_is_ownership_operation(tokens) {
            let node = self.parse_ownership_operation(tokens)?;
            return Ok(self.attach_leading_comments(node, leading_comments));
        }

        if let Some((message, unary_op)) = self.unary_prefix_info(&token) {
            let operator_token = token.clone();
            let _ = tokens.bump();
            self.ensure_unary_operand(tokens, &operator_token, message)?;

            let operand = self.parse_primary_expression(tokens)?;
            if let Some(op) = unary_op {
                return Ok(self.attach_leading_comments(
                    AstNode::UnaryOp {
                        op,
                        operand: Box::new(operand),
                    },
                    leading_comments,
                ));
            }

            return Ok(self.attach_leading_comments(operand, leading_comments));
        }

        let node = if matches!(
            token.key(),
            KEYWORD::Keyword(BUILDIN::If) | KEYWORD::Keyword(BUILDIN::When)
        ) && self.lookahead_is_match_expression(tokens)
        {
            self.parse_match_expression(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Dot)) {
            self.parse_dot_builtin_call_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Pipe)) {
            self.parse_pipe_lambda_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Fun)) {
            self.parse_anonymous_fun_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Log)) {
            self.parse_anonymous_log_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Pro)) {
            self.parse_anonymous_pro_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Ok(self.attach_leading_comments(
                self.parse_container_expression(tokens)?,
                leading_comments,
            ));
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO))
            && self.lookahead_is_shorthand_anonymous_fun(tokens)
        {
            self.parse_shorthand_anonymous_fun_expr(tokens)?
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            let _ = tokens.bump();
            let inner = self.parse_logical_expression(tokens)?;
            let inner = self.attach_trailing_comments(
                inner,
                self.collect_comments_before(tokens, |key| {
                    matches!(key, KEYWORD::Symbol(SYMBOL::RoundC))
                })?,
            );
            self.skip_layout(tokens)?;

            let close = tokens.curr(false)?;
            if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                return Err(ParseError::from_token(
                    &close,
                    "Expected closing ')' for parenthesized expression".to_string(),
                ));
            }

            let _ = tokens.bump();
            inner
        } else if token.key().is_textual_literal()
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::RoundO))
            )
        {
            let name = Self::token_to_named_label(&token).ok_or_else(|| {
                ParseError::from_token(&token, "Expected quoted callable name".to_string())
            })?;
            let _ = tokens.bump();
            AstNode::Identifier {
                syntax_id: self.record_syntax_origin(&token),
                name,
            }
        } else if Self::token_can_start_path_expression(&token)
            && matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Operator(OPERATOR::Path))
            )
        {
            let path = self.parse_qualified_path(
                tokens,
                "Expected expression path root",
                "Expected name after '::' in expression path",
            )?;
            AstNode::QualifiedIdentifier { path }
        } else {
            let node = self.parse_primary(&token)?;
            let _ = tokens.bump();
            node
        };

        Ok(self.attach_leading_comments(
            self.parse_postfix_expression(tokens, node)?,
            leading_comments,
        ))
    }

    pub(super) fn parse_container_expression(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start container expression".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_layout(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
        ) {
            let _ = tokens.bump();
            return Ok(AstNode::ContainerLiteral {
                container_type: ContainerType::Array,
                elements: Vec::new(),
            });
        }

        if self.lookahead_is_record_init_field(tokens) {
            return self.parse_record_init_fields_after_open(tokens, &open);
        }

        let mut elements = Vec::new();
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
                elements.extend(pending_comments);
                let _ = tokens.bump();
                continue;
            }

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                if !pending_comments.is_empty() {
                    elements.extend(pending_comments);
                }
                let _ = tokens.bump();
                break;
            }

            let mut expr = self.parse_logical_expression(tokens)?;
            expr = self.attach_leading_comments(expr, pending_comments);
            expr = self.attach_trailing_comments(
                expr,
                self.collect_comments_before(tokens, |key| {
                    matches!(
                        key,
                        KEYWORD::Keyword(BUILDIN::For)
                            | KEYWORD::Symbol(SYMBOL::Comma)
                            | KEYWORD::Symbol(SYMBOL::Semi)
                            | KEYWORD::Symbol(SYMBOL::CurlyC)
                    )
                })?,
            );
            self.skip_layout(tokens)?;

            if let Ok(next) = tokens.curr(false) {
                if matches!(next.key(), KEYWORD::Keyword(BUILDIN::For)) {
                    if !elements.is_empty() {
                        return Err(ParseError::from_token(
                            &next,
                            "Rolling expressions must contain exactly one output expression"
                                .to_string(),
                        ));
                    }
                    return self.parse_rolling_expression(tokens, expr);
                }
            }

            elements.push(expr);

            let sep = tokens.curr(false)?;
            if matches!(
                sep.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::CurlyC))
                ) {
                    let _ = tokens.bump();
                    break;
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                break;
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in container expression".to_string(),
            ));
        }

        if elements.len() == 1 {
            if let Some(range) = elements.pop() {
                if matches!(range, AstNode::Range { .. }) {
                    return Ok(range);
                }
                return Ok(AstNode::ContainerLiteral {
                    container_type: ContainerType::Array,
                    elements: vec![range],
                });
            }
        }

        Ok(AstNode::ContainerLiteral {
            container_type: ContainerType::Array,
            elements,
        })
    }
}
