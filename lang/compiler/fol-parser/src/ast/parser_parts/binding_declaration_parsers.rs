use super::*;
use crate::ast::BindingPattern;

impl AstParser {
    pub(super) fn parse_lexer_literal(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<AstNode, ParseError> {
        let raw = token.con().trim();
        let wrap_err =
            |error: ParseError| -> ParseError { ParseError::from_token(token, error.to_string()) };

        match token.key() {
            fol_lexer::token::KEYWORD::Literal(LITERAL::CookedQuoted)
            | fol_lexer::token::KEYWORD::Literal(LITERAL::RawQuoted) => {
                self.parse_literal(raw).map_err(wrap_err)
            }
            fol_lexer::token::KEYWORD::Literal(LITERAL::Decimal)
            | fol_lexer::token::KEYWORD::Literal(LITERAL::Float)
            | fol_lexer::token::KEYWORD::Literal(LITERAL::Hexadecimal)
            | fol_lexer::token::KEYWORD::Literal(LITERAL::Octal)
            | fol_lexer::token::KEYWORD::Literal(LITERAL::Binary) => {
                self.parse_literal(raw).map_err(wrap_err)
            }
            _ => self.parse_literal(raw).map_err(wrap_err),
        }
    }

    pub(super) fn parse_var_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.parse_binding_decl(tokens, "var", vec![VarOption::Mutable, VarOption::Normal])
    }

    pub(super) fn parse_con_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.parse_binding_decl(tokens, "con", vec![VarOption::Immutable, VarOption::Normal])
    }

    pub(super) fn parse_lab_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let nodes =
            self.parse_binding_decl(tokens, "lab", vec![VarOption::Immutable, VarOption::Normal])?;
        Ok(nodes
            .into_iter()
            .map(|node| match node {
                AstNode::VarDecl {
                    options,
                    name,
                    type_hint,
                    value,
                    syntax_id,
                } => AstNode::LabDecl {
                    options,
                    name,
                    type_hint,
                    value,
                    syntax_id,
                },
                AstNode::DestructureDecl {
                    options,
                    pattern,
                    type_hint,
                    value,
                    ..
                } => AstNode::DestructureDecl {
                    options,
                    is_label: true,
                    pattern,
                    type_hint,
                    value,
                },
                other => other,
            })
            .collect())
    }

    pub(super) fn parse_binding_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        keyword: &str,
        default_options: Vec<VarOption>,
    ) -> Result<Vec<AstNode>, ParseError> {
        if tokens.bump().is_none() {
            let error = if let Ok(token) = tokens.curr(false) {
                ParseError::from_token(
                    &token,
                    format!("Unexpected EOF after '{}' declaration", keyword),
                )
            } else {
                ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: format!("Unexpected EOF after '{}' declaration", keyword),
                    file: None,
                    line: 0,
                    column: 0,
                    length: 0,
                }
            };
            return Err(error);
        }
        self.skip_ignorable(tokens)?;
        let options = self.parse_binding_options(tokens, default_options)?;
        self.skip_ignorable(tokens)?;

        if let Ok(open_group) = tokens.curr(false) {
            if matches!(open_group.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
                return self.parse_binding_group(tokens, options);
            }
        }

        let mut nodes = Vec::new();
        for _ in 0..256 {
            let patterns = self.parse_binding_pattern_list(tokens, keyword)?;
            let is_destructuring = patterns.iter().any(BindingPattern::is_destructuring);
            self.skip_ignorable(tokens)?;

            let mut type_hint = None;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    type_hint = Some(self.parse_type_reference_tokens(tokens)?);
                }
            }

            self.skip_ignorable(tokens)?;
            let mut values = Vec::new();
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    values = self
                        .parse_binding_values(tokens, !is_destructuring && patterns.len() == 1)?;
                }
            }

            if is_destructuring {
                nodes.push(self.build_destructure_binding_node(
                    options.clone(),
                    patterns,
                    type_hint,
                    values,
                    tokens,
                )?);
            } else {
                let names = patterns
                    .into_iter()
                    .map(|pattern| match pattern {
                        BindingPattern::Name(name, syntax_id) => Ok((name, syntax_id)),
                        other => {
                            let error = if let Ok(token) = tokens.curr(false) {
                                ParseError::from_token_with_kind(
                                    &token,
                                    ParseErrorKind::Unsupported,
                                    format!("Unsupported plain binding pattern: {other:?}"),
                                )
                            } else {
                                ParseError {
                                    kind: ParseErrorKind::Unsupported,
                                    message: format!(
                                        "Unsupported plain binding pattern: {other:?}"
                                    ),
                                    file: None,
                                    line: 0,
                                    column: 0,
                                    length: 0,
                                }
                            };
                            Err(error)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                nodes.extend(self.build_binding_nodes(
                    options.clone(),
                    names,
                    type_hint,
                    values,
                    tokens,
                )?);
            }

            self.skip_layout(tokens)?;
            let next = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Semi)) {
                break;
            }
            if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Comma))
                && self.lookahead_starts_binding_segment(tokens)
            {
                let _ = tokens.bump();
                self.skip_layout(tokens)?;
                continue;
            }

            break;
        }

        Ok(nodes)
    }

    pub(super) fn parse_binding_pattern_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        keyword: &str,
    ) -> Result<Vec<BindingPattern>, ParseError> {
        let mut patterns = Vec::new();

        for _ in 0..256 {
            patterns.push(self.parse_binding_pattern(tokens, keyword)?);
            self.skip_ignorable(tokens)?;

            let next = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if matches!(next.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                continue;
            }

            break;
        }

        Ok(patterns)
    }

    pub(super) fn parse_binding_pattern(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        keyword: &str,
    ) -> Result<BindingPattern, ParseError> {
        let token = tokens.curr(false)?;

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            let _ = tokens.bump();
            let mut parts = Vec::new();
            for _ in 0..256 {
                self.skip_ignorable(tokens)?;
                let current = tokens.curr(false)?;
                if matches!(current.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    let _ = tokens.bump();
                    return Ok(BindingPattern::Sequence(parts));
                }

                parts.push(self.parse_binding_pattern(tokens, keyword)?);
                self.skip_ignorable(tokens)?;

                let separator = tokens.curr(false)?;
                if matches!(separator.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(separator.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                    let _ = tokens.bump();
                    return Ok(BindingPattern::Sequence(parts));
                }

                return Err(ParseError::from_token(
                    &separator,
                    "Expected ',' or ')' in binding pattern".to_string(),
                ));
            }

            return Err(ParseError::from_token(
                &token,
                "Binding pattern exceeded parser limit".to_string(),
            ));
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Star)) {
            let star = token.clone();
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let name_token = tokens.curr(false)?;
            let name = if matches!(name_token.key(), KEYWORD::Symbol(SYMBOL::Under)) {
                "_".to_string()
            } else {
                Self::expect_named_label(&name_token, "Expected binding name after '*'")?
            };
            let _ = tokens.bump();
            if matches!(name_token.key(), KEYWORD::Symbol(SYMBOL::Star)) {
                return Err(ParseError::from_token(
                    &star,
                    "Nested '*' binding patterns are not allowed".to_string(),
                ));
            }
            return Ok(BindingPattern::Rest(name));
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Under)) {
            let _ = tokens.bump();
            return Ok(BindingPattern::Name("_".to_string(), None));
        }

        let name =
            Self::expect_named_label(&token, &format!("Expected identifier after '{}'", keyword))?;
        // Record the binding name token so tooling can locate the local
        // declaration; the resolver derives the symbol origin from this id.
        let syntax_id = self.record_syntax_origin(&token);
        let _ = tokens.bump();
        Ok(BindingPattern::Name(name, syntax_id))
    }

    pub(super) fn build_binding_nodes(
        &self,
        options: Vec<VarOption>,
        names: Vec<(String, Option<SyntaxNodeId>)>,
        type_hint: Option<FolType>,
        values: Vec<AstNode>,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        self.build_binding_nodes_with_form(options, names, type_hint, values, tokens, false)
    }

    /// `grouped` marks a segment inside a grouped declaration
    /// (`var (a, b: int = 3, ...)`), where one value is a deliberate shared
    /// initializer for every name in the segment.
    pub(super) fn build_binding_nodes_with_form(
        &self,
        options: Vec<VarOption>,
        names: Vec<(String, Option<SyntaxNodeId>)>,
        type_hint: Option<FolType>,
        values: Vec<AstNode>,
        tokens: &fol_lexer::lexer::stage3::Elements,
        grouped: bool,
    ) -> Result<Vec<AstNode>, ParseError> {
        let assigned_values = match values.len() {
            0 => vec![None; names.len()],
            // In the flat comma form, one source expression with several names
            // is positional destructuring (book 700_sugar/600_unpacking),
            // never a broadcast of the same value into every binding.
            1 if names.len() > 1 && !grouped => {
                let patterns = names
                    .into_iter()
                    .map(|(name, syntax_id)| BindingPattern::Name(name, syntax_id))
                    .collect();
                return self
                    .build_destructure_binding_node(options, patterns, type_hint, values, tokens)
                    .map(|node| vec![node]);
            }
            1 => vec![Some(values[0].clone()); names.len()],
            n if n == names.len() => values.into_iter().map(Some).collect(),
            _ => {
                let error = if let Ok(token) = tokens.curr(false) {
                    ParseError::from_token(
                        &token,
                        "Binding value count must match declared names or provide a single shared value".to_string(),
                    )
                } else {
                    ParseError {
                        kind: ParseErrorKind::Syntax,
                        message: "Binding value count must match declared names or provide a single shared value".to_string(),
                        file: None,
                        line: 0,
                        column: 0,
                        length: 0,
                    }
                };
                return Err(error);
            }
        };

        Ok(names
            .into_iter()
            .zip(assigned_values)
            .map(|((name, syntax_id), value)| AstNode::VarDecl {
                options: options.clone(),
                name,
                type_hint: type_hint.clone(),
                value: value.map(Box::new),
                syntax_id,
            })
            .collect())
    }

    pub(super) fn build_destructure_binding_node(
        &self,
        options: Vec<VarOption>,
        patterns: Vec<BindingPattern>,
        type_hint: Option<FolType>,
        values: Vec<AstNode>,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        if values.len() != 1 {
            let error = if let Ok(token) = tokens.curr(false) {
                ParseError::from_token(
                    &token,
                    "Destructuring bindings require exactly one source value".to_string(),
                )
            } else {
                ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: "Destructuring bindings require exactly one source value".to_string(),
                    file: None,
                    line: 0,
                    column: 0,
                    length: 0,
                }
            };
            return Err(error);
        }

        Ok(AstNode::DestructureDecl {
            options,
            is_label: false,
            pattern: BindingPattern::Sequence(patterns),
            type_hint,
            value: Box::new(values.into_iter().next().expect("single destructure value")),
        })
    }
}
