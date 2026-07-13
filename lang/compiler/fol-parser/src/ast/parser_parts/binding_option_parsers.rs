use super::*;

impl AstParser {
    pub(super) fn parse_binding_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        default_options: Vec<VarOption>,
    ) -> Result<Vec<VarOption>, ParseError> {
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(default_options),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Ok(default_options);
        }
        let _ = tokens.bump();

        let mut parsed_options = Vec::new();
        for _ in 0..16 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            Self::reject_illegal_token(&token)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(self.merge_binding_options(default_options, parsed_options));
            }

            let option = match token.con().trim() {
                "mut" | "mutable" => VarOption::Mutable,
                "imu" | "immutable" => VarOption::Immutable,
                "exp" | "export" | "pub" | "+" => VarOption::Export,
                "hid" | "hidden" | "-" => VarOption::Hidden,
                "nor" | "normal" => VarOption::Normal,
                "sta" | "static" | "!" => VarOption::Static,
                "rac" | "reactive" | "?" => VarOption::Reactive,
                "new" => VarOption::New,
                "bor" | "borrow" | "borrowing" => VarOption::Borrowing,
                _ => {
                    return Err(ParseError::from_token(
                        &token,
                        "Unknown binding option".to_string(),
                    ));
                }
            };

            if let Some(existing) = parsed_options
                .iter()
                .find(|existing| self.binding_options_conflict(existing, &option))
            {
                return Err(ParseError::from_token(
                    &token,
                    format!(
                        "Conflicting binding option '{}' with '{}'",
                        Self::binding_option_label(existing),
                        Self::binding_option_label(&option)
                    ),
                ));
            }

            parsed_options.push(option);
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
                    return Ok(self.merge_binding_options(default_options, parsed_options));
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(self.merge_binding_options(default_options, parsed_options));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ']' in binding options".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Binding options exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Binding options exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn merge_binding_options(
        &self,
        mut base: Vec<VarOption>,
        parsed: Vec<VarOption>,
    ) -> Vec<VarOption> {
        for option in parsed {
            match option {
                VarOption::Mutable | VarOption::Immutable => {
                    base.retain(|existing| {
                        !matches!(existing, VarOption::Mutable | VarOption::Immutable)
                    });
                }
                VarOption::Export | VarOption::Hidden | VarOption::Normal => {
                    base.retain(|existing| {
                        !matches!(
                            existing,
                            VarOption::Export | VarOption::Hidden | VarOption::Normal
                        )
                    });
                }
                _ => {}
            }

            if !base.contains(&option) {
                base.push(option);
            }
        }

        base
    }

    pub(super) fn binding_options_conflict(&self, lhs: &VarOption, rhs: &VarOption) -> bool {
        lhs == rhs
            || matches!(
                (lhs, rhs),
                (VarOption::Mutable, VarOption::Immutable)
                    | (VarOption::Immutable, VarOption::Mutable)
                    | (VarOption::Export, VarOption::Hidden)
                    | (VarOption::Export, VarOption::Normal)
                    | (VarOption::Hidden, VarOption::Export)
                    | (VarOption::Hidden, VarOption::Normal)
                    | (VarOption::Normal, VarOption::Export)
                    | (VarOption::Normal, VarOption::Hidden)
            )
    }

    pub(super) fn binding_option_label(option: &VarOption) -> &'static str {
        match option {
            VarOption::Mutable => "mut",
            VarOption::Immutable => "imu",
            VarOption::Static => "sta",
            VarOption::Reactive => "rac",
            VarOption::Export => "exp",
            VarOption::Normal => "nor",
            VarOption::Hidden => "hid",
            VarOption::New => "new",
            VarOption::Borrowing => "bor",
        }
    }
}
