use super::*;
use crate::ast::{ChannelEndpoint, OwnershipOption, RoutineCapture};

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
            let (endpoint, operation) = if matches!(
                tokens.curr(false).map(|token| token.key()),
                Ok(KEYWORD::Symbol(SYMBOL::SquarO))
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let inner_token = tokens.curr(false)?;
                // The capture bracket carries either a channel endpoint
                // (`c[tx]` / `c[rx]`) or a value-capture ownership operation
                // (`data[mov]` / `data[cpy]`); the two are mutually exclusive.
                let (endpoint, operation) =
                    match Self::token_to_named_label(&inner_token).as_deref() {
                        Some("tx") => (Some(ChannelEndpoint::Tx), None),
                        Some("rx") => (Some(ChannelEndpoint::Rx), None),
                        Some("mov" | "move") => (None, Some(OwnershipOption::Move)),
                        Some("cpy" | "copy") => (None, Some(OwnershipOption::Copy)),
                        Some("cln" | "clone") => (None, Some(OwnershipOption::Clone)),
                        Some("bor" | "borrow") => (None, Some(OwnershipOption::Borrow)),
                        _ => {
                            return Err(ParseError::from_token(
                            &inner_token,
                            "Expected 'tx', 'rx', 'mov', 'cpy', 'cln', or 'bor' in capture bracket"
                                .to_string(),
                        ));
                        }
                    };
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let close = tokens.curr(false)?;
                if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                    return Err(ParseError::from_token(
                        &close,
                        "Expected ']' after capture bracket".to_string(),
                    ));
                }
                let _ = tokens.bump();
                (endpoint, operation)
            } else {
                (None, None)
            };
            captures.push(RoutineCapture {
                name,
                syntax_id,
                endpoint,
                operation,
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
