use super::*;

impl AstParser {
    pub(super) fn parse_use_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<UseOption>, ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();

        let mut options = Vec::new();
        let mut saw_export = false;
        let mut saw_hidden = false;
        let mut saw_normal = false;
        for _ in 0..16 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            Self::reject_illegal_token(&token)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(options);
            }

            let option = match token.con().trim() {
                "+" | "exp" | "export" | "pub" => UseOption::Export,
                "-" | "hid" | "hidden" => UseOption::Hidden,
                "nor" | "normal" => UseOption::Normal,
                _ => {
                    return Err(ParseError::from_token(
                        &token,
                        "Unknown use option".to_string(),
                    ))
                }
            };
            match option {
                UseOption::Export => {
                    if saw_export {
                        return Err(ParseError::from_token(
                            &token,
                            "Duplicate use option 'export'".to_string(),
                        ));
                    }
                    if saw_hidden {
                        return Err(ParseError::from_token(
                            &token,
                            "Conflicting use options 'export' and 'hidden'".to_string(),
                        ));
                    }
                    saw_export = true;
                }
                UseOption::Hidden => {
                    if saw_hidden {
                        return Err(ParseError::from_token(
                            &token,
                            "Duplicate use option 'hidden'".to_string(),
                        ));
                    }
                    if saw_export {
                        return Err(ParseError::from_token(
                            &token,
                            "Conflicting use options 'export' and 'hidden'".to_string(),
                        ));
                    }
                    saw_hidden = true;
                }
                UseOption::Normal => {
                    if saw_normal {
                        return Err(ParseError::from_token(
                            &token,
                            "Duplicate use option 'normal'".to_string(),
                        ));
                    }
                    saw_normal = true;
                }
            }
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
                "Expected ',', ';', or ']' in use options".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(&token, "Use options exceeded parser limit".to_string())
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Use options exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }
}
