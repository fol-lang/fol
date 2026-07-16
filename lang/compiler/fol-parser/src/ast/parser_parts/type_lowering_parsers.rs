use super::*;

impl AstParser {
    pub(super) fn fol_type_label(typ: &FolType) -> String {
        match typ {
            FolType::Limited { base, .. } => Self::fol_type_label(base),
            FolType::Channel { .. } => "chn".to_string(),
            FolType::Named { name, .. } => name.clone(),
            FolType::QualifiedNamed { path } => path.joined(),
            FolType::Package { name } => {
                if name.is_empty() {
                    "pkg".to_string()
                } else {
                    name.clone()
                }
            }
            FolType::Location { name } => {
                if name.is_empty() {
                    "loc".to_string()
                } else {
                    name.clone()
                }
            }
            FolType::Never => "nev".to_string(),
            FolType::Union { .. } => "uni".to_string(),
            _ => format!("{:?}", typ),
        }
    }

    pub(super) fn lower_bare_scalar_type_name(name: &str) -> Option<FolType> {
        match name {
            "int" => Some(FolType::Int {
                size: None,
                signed: true,
            }),
            "flt" | "float" => Some(FolType::Float { size: None }),
            "bol" | "bool" => Some(FolType::Bool),
            "chr" | "char" => Some(FolType::Char {
                encoding: CharEncoding::Utf8,
            }),
            "i8" => Some(FolType::Int {
                size: Some(IntSize::I8),
                signed: true,
            }),
            "i16" => Some(FolType::Int {
                size: Some(IntSize::I16),
                signed: true,
            }),
            "i32" => Some(FolType::Int {
                size: Some(IntSize::I32),
                signed: true,
            }),
            "i64" => Some(FolType::Int {
                size: Some(IntSize::I64),
                signed: true,
            }),
            "i128" => Some(FolType::Int {
                size: Some(IntSize::I128),
                signed: true,
            }),
            "u8" => Some(FolType::Int {
                size: Some(IntSize::I8),
                signed: false,
            }),
            "u16" => Some(FolType::Int {
                size: Some(IntSize::I16),
                signed: false,
            }),
            "u32" => Some(FolType::Int {
                size: Some(IntSize::I32),
                signed: false,
            }),
            "u64" => Some(FolType::Int {
                size: Some(IntSize::I64),
                signed: false,
            }),
            "u128" => Some(FolType::Int {
                size: Some(IntSize::I128),
                signed: false,
            }),
            "arch" => Some(FolType::Int {
                size: Some(IntSize::Arch),
                signed: true,
            }),
            "uarch" => Some(FolType::Int {
                size: Some(IntSize::Arch),
                signed: false,
            }),
            "f32" => Some(FolType::Float {
                size: Some(FloatSize::F32),
            }),
            "f64" => Some(FolType::Float {
                size: Some(FloatSize::F64),
            }),
            _ => None,
        }
    }

    pub(super) fn lower_integer_option(option: &str) -> Option<(IntSize, bool)> {
        match option {
            "8" | "i8" => Some((IntSize::I8, true)),
            "16" | "i16" => Some((IntSize::I16, true)),
            "32" | "i32" => Some((IntSize::I32, true)),
            "64" | "i64" => Some((IntSize::I64, true)),
            "128" | "i128" => Some((IntSize::I128, true)),
            "arch" => Some((IntSize::Arch, true)),
            "u8" => Some((IntSize::I8, false)),
            "u16" => Some((IntSize::I16, false)),
            "u32" => Some((IntSize::I32, false)),
            "u64" => Some((IntSize::I64, false)),
            "u128" => Some((IntSize::I128, false)),
            "uarch" => Some((IntSize::Arch, false)),
            _ => None,
        }
    }

    pub(super) fn lower_float_option(option: &str) -> Option<FloatSize> {
        match option {
            "32" | "f32" => Some(FloatSize::F32),
            "64" | "f64" => Some(FloatSize::F64),
            "arch" => Some(FloatSize::Arch),
            _ => None,
        }
    }

    pub(super) fn lower_char_option(option: &str) -> Option<CharEncoding> {
        match option {
            "utf8" => Some(CharEncoding::Utf8),
            "utf16" => Some(CharEncoding::Utf16),
            "utf32" => Some(CharEncoding::Utf32),
            _ => None,
        }
    }

    pub(super) fn parse_type_reference_tokens(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<FolType, ParseError> {
        let _nesting = self.enter_nesting(tokens)?;
        self.skip_ignorable(tokens)?;
        let token = tokens.curr(false)?;

        if token.key().is_illegal() {
            return Err(ParseError::from_token(
                &token,
                format!("Parser encountered illegal token '{}'", token.con()),
            ));
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Query)) {
            let _ = tokens.bump();
            let inner = self.parse_type_reference_tokens(tokens)?;
            return Ok(FolType::Optional {
                inner: Box::new(inner),
            });
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::At)) {
            let _ = tokens.bump();
            let inner = self.parse_type_reference_tokens(tokens)?;
            return Ok(FolType::Owned {
                inner: Box::new(inner),
            });
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Bang)) {
            let _ = tokens.bump();
            let _ = self.parse_type_reference_tokens(tokens)?;
            return Ok(FolType::Never);
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return self.parse_function_type_reference(tokens);
        }

        let first_name = Self::token_to_named_label(&token)
            .ok_or_else(|| ParseError::from_token(&token, "Expected type reference".to_string()))?;
        let mut path =
            QualifiedPath::with_syntax_id(vec![first_name], self.record_syntax_origin(&token));
        let mut name = path.joined();
        let _ = tokens.bump();

        for _ in 0..64 {
            self.skip_ignorable(tokens)?;
            let separator = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            let is_path = matches!(separator.key(), KEYWORD::Operator(OPERATOR::Path))
                || matches!(separator.key(), KEYWORD::Symbol(SYMBOL::Colon))
                    && matches!(
                        self.next_significant_key_from_window(tokens),
                        Some(KEYWORD::Symbol(SYMBOL::Colon))
                    );

            if !is_path {
                break;
            }

            if matches!(separator.key(), KEYWORD::Operator(OPERATOR::Path)) {
                let _ = tokens.bump();
            } else {
                self.consume_significant_token(tokens);
                self.consume_significant_token(tokens);
            }

            self.skip_ignorable(tokens)?;
            let segment = tokens.curr(false)?;
            let segment_name =
                Self::expect_named_label(&segment, "Expected type segment after '::'")?;

            path.segments.push(segment_name);
            name = path.joined();
            let _ = tokens.bump();
        }

        let base_name = name.clone();
        if !path.is_qualified() && base_name == "opt" {
            self.skip_ignorable(tokens)?;
            if matches!(
                tokens.curr(false).map(|token| token.key()),
                Ok(KEYWORD::Symbol(SYMBOL::At))
            ) {
                let inner = self.parse_type_reference_tokens(tokens)?;
                return Ok(FolType::Optional {
                    inner: Box::new(inner),
                });
            }
            if tokens
                .curr(false)
                .ok()
                .is_some_and(|token| token.con().trim() == "ptr")
            {
                let inner = self.parse_type_reference_tokens(tokens)?;
                return Ok(FolType::Optional {
                    inner: Box::new(inner),
                });
            }
        }
        if !path.is_qualified() && base_name == "url" {
            return Err(ParseError::from_token(
                &token,
                "Legacy source kind 'url' was removed; use 'pkg' instead".to_string(),
            ));
        }

        let mut has_suffix = false;
        for _ in 0..32 {
            self.skip_ignorable(tokens)?;
            let open = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
                break;
            }

            if matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Symbol(SYMBOL::Dot))
            ) {
                break;
            }

            if let Some(parsed) = self.try_parse_special_type_suffix(tokens, &base_name)? {
                return self.parse_trailing_type_limits(tokens, parsed);
            }

            has_suffix = true;
            name.push_str(&self.parse_balanced_type_suffix(
                tokens,
                KEYWORD::Symbol(SYMBOL::SquarO),
                KEYWORD::Symbol(SYMBOL::SquarC),
                "Expected closing ']' in type reference",
            )?);
        }

        let base_type = if name == "mod" {
            FolType::Module {
                name: String::new(),
            }
        } else if name == "blk" {
            FolType::Block {
                name: String::new(),
            }
        } else if name == "any" {
            FolType::Any
        } else if name == "nev" {
            FolType::Never
        } else if matches!(name.as_str(), "non" | "none") {
            FolType::None
        } else if let Some(lowered) = Self::lower_bare_scalar_type_name(&name) {
            lowered
        } else if let Some(lowered) = Self::lower_bare_source_kind_type_name(&name) {
            lowered
        } else if path.is_qualified() && !has_suffix {
            FolType::QualifiedNamed { path }
        } else {
            FolType::Named {
                syntax_id: path.syntax_id(),
                name,
            }
        };

        self.parse_trailing_type_limits(tokens, base_type)
    }
}
