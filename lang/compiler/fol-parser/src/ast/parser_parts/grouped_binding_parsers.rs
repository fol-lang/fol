use super::*;
use crate::ast::BindingPattern;

impl AstParser {
    pub(super) fn parse_binding_group(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        options: Vec<VarOption>,
    ) -> Result<Vec<AstNode>, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '(' to start grouped bindings".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut nodes = Vec::new();
        for _ in 0..512 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(nodes);
            }

            let patterns = self.parse_binding_pattern_list(tokens, "grouped binding")?;
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
                    values = self.parse_binding_values(tokens, !is_destructuring)?;
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
                                    format!("Unsupported grouped binding pattern: {other:?}"),
                                )
                            } else {
                                ParseError {
                                    kind: ParseErrorKind::Unsupported,
                                    message: format!(
                                        "Unsupported grouped binding pattern: {other:?}"
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
                nodes.extend(self.build_binding_nodes_with_form(
                    options.clone(),
                    names,
                    type_hint,
                    values,
                    tokens,
                    true,
                )?);
            }

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(nodes);
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',' or ')' in grouped bindings".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(&token, "Grouped bindings exceeded parser limit".to_string())
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Grouped bindings exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }
}
