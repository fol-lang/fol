use super::*;

impl AstParser {
    pub(super) fn lookahead_binding_alternative(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Option<(&'static str, Vec<VarOption>)> {
        let current = tokens.curr(false).ok()?;
        let prefix_option = match current.key() {
            KEYWORD::Symbol(SYMBOL::Plus) => Some(VarOption::Export),
            KEYWORD::Symbol(SYMBOL::Minus) => Some(VarOption::Hidden),
            KEYWORD::Symbol(SYMBOL::Home) => Some(VarOption::Mutable),
            KEYWORD::Symbol(SYMBOL::Bang) => Some(VarOption::Static),
            KEYWORD::Symbol(SYMBOL::Query) => Some(VarOption::Reactive),
            KEYWORD::Symbol(SYMBOL::At) => Some(VarOption::New),
            _ => None,
        }?;

        let next = self.next_significant_token_from_window(tokens)?;
        let (keyword, default_options) = match next.key() {
            KEYWORD::Keyword(BUILDIN::Var) => ("var", vec![VarOption::Mutable, VarOption::Normal]),
            KEYWORD::Keyword(BUILDIN::Con) => {
                ("con", vec![VarOption::Immutable, VarOption::Normal])
            }
            _ => return None,
        };

        Some((
            keyword,
            self.merge_binding_options(default_options, vec![prefix_option]),
        ))
    }

    pub(super) fn parse_binding_alternative_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let (keyword, options) = self.lookahead_binding_alternative(tokens).ok_or_else(|| {
            let error: ParseError = if let Ok(token) = tokens.curr(false) {
                ParseError::from_token(&token, "Expected binding alternative".to_string())
            } else {
                ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: "Expected binding alternative".to_string(),
                    file: None,
                    line: 0,
                    column: 0,
                    length: 0,
                }
            };
            error
        })?;

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        self.parse_binding_decl(tokens, keyword, options)
    }

    pub(super) fn next_significant_token_from_window(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Option<fol_lexer::lexer::stage3::element::Element> {
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            return Some(token);
        }

        None
    }
}
