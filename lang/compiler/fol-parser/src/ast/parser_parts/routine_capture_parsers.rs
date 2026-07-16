use super::*;
use crate::ast::{ChannelEndpoint, RoutineCapture};

impl AstParser {
    pub(super) fn ensure_unique_capture_names(
        &self,
        captures: &[RoutineCapture],
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<(), ParseError> {
        let mut seen = HashSet::new();
        for capture in captures {
            if !seen.insert(canonical_identifier_key(&capture.name)) {
                let error = if let Ok(token) = tokens.curr(false) {
                    ParseError::from_token(
                        &token,
                        format!("Duplicate capture name '{}'", capture.name),
                    )
                } else {
                    ParseError {
                        kind: ParseErrorKind::Syntax,
                        message: format!("Duplicate capture name '{}'", capture.name),
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

    pub(super) fn parse_optional_routine_capture_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<RoutineCapture>, ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();

        let mut captures = Vec::new();
        for _ in 0..128 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(captures);
            }

            let name =
                Self::expect_named_label(&token, "Expected capture name in routine capture list")?;
            let syntax_id = self.record_syntax_origin(&token);
            let _ = tokens.bump();

            self.skip_ignorable(tokens)?;
            let endpoint = if matches!(
                tokens.curr(false).map(|token| token.key()),
                Ok(KEYWORD::Symbol(SYMBOL::SquarO))
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let endpoint_token = tokens.curr(false)?;
                let endpoint = match Self::token_to_named_label(&endpoint_token).as_deref() {
                    Some("tx") => ChannelEndpoint::Tx,
                    Some("rx") => ChannelEndpoint::Rx,
                    _ => {
                        return Err(ParseError::from_token(
                            &endpoint_token,
                            "Expected 'tx' or 'rx' in capture endpoint".to_string(),
                        ));
                    }
                };
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let close = tokens.curr(false)?;
                if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                    return Err(ParseError::from_token(
                        &close,
                        "Expected ']' after capture endpoint".to_string(),
                    ));
                }
                let _ = tokens.bump();
                Some(endpoint)
            } else {
                None
            };
            captures.push(RoutineCapture {
                name,
                syntax_id,
                endpoint,
            });

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
                    Ok(KEYWORD::Symbol(SYMBOL::SquarC))
                ) {
                    let _ = tokens.bump();
                    return Ok(captures);
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(captures);
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ']' in routine capture list".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Routine capture parsing exceeded safety bound".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Routine capture parsing exceeded safety bound".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }
}
