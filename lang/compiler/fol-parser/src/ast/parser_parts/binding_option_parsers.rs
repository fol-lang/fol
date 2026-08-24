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
                if parsed_options.contains(&VarOption::Borrowing)
                    && !parsed_options
                        .iter()
                        .any(|option| matches!(option, VarOption::Mutable | VarOption::Immutable))
                {
                    parsed_options.push(VarOption::Immutable);
                }
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
                    if parsed_options.contains(&VarOption::Borrowing)
                        && !parsed_options.iter().any(|option| {
                            matches!(option, VarOption::Mutable | VarOption::Immutable)
                        })
                    {
                        parsed_options.push(VarOption::Immutable);
                    }
                    return Ok(self.merge_binding_options(default_options, parsed_options));
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                if parsed_options.contains(&VarOption::Borrowing)
                    && !parsed_options
                        .iter()
                        .any(|option| matches!(option, VarOption::Mutable | VarOption::Immutable))
                {
                    parsed_options.push(VarOption::Immutable);
                }
                return Ok(self.merge_binding_options(default_options, parsed_options));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ']' in binding options".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(&token, "Binding options exceeded parser limit".to_string())
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

    /// Binding options on an entry variant, plus the ABI discriminant.
    ///
    /// `[tag = N]` is its own bracket group rather than one option among
    /// several: it takes a value, and the option list is a list of bare names.
    pub(super) fn parse_entry_variant_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        default_options: Vec<VarOption>,
    ) -> Result<(Vec<VarOption>, Option<i64>), ParseError> {
        if !self.looks_like_entry_variant_tag(tokens) {
            return Ok((self.parse_binding_options(tokens, default_options)?, None));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let equal = tokens.curr(false)?;
        if !matches!(equal.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            return Err(ParseError::from_token(
                &equal,
                "Expected '=' after 'tag' in an entry variant option".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let mut negative = false;
        let mut token = tokens.curr(false)?;
        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Minus)) {
            negative = true;
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            token = tokens.curr(false)?;
        }
        let digits = token.con();
        let magnitude = digits.trim().parse::<i64>().map_err(|_| {
            ParseError::from_token(
                &token,
                "Expected an integer literal for an entry variant tag".to_string(),
            )
        })?;
        let tag = if negative { -magnitude } else { magnitude };
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected ']' to close an entry variant tag; a tag is written \
                 alone as '[tag = N]'"
                    .to_string(),
            ));
        }
        let _ = tokens.bump();
        Ok((default_options, Some(tag)))
    }

    fn looks_like_entry_variant_tag(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        let Ok(open) = tokens.curr(false) else {
            return false;
        };
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return false;
        }
        self.next_significant_token_from_window(tokens)
            .is_some_and(|next| next.con().trim() == "tag")
    }
}
