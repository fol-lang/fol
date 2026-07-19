use super::*;

impl AstParser {
    pub(super) fn parse_trailing_type_limits(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        mut base: FolType,
    ) -> Result<FolType, ParseError> {
        for _ in 0..32 {
            self.skip_ignorable(tokens)?;
            let open = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
                break;
            }

            let next_key = self.next_significant_key_from_window(tokens);
            if !matches!(next_key, Some(KEYWORD::Symbol(SYMBOL::Dot))) {
                break;
            }

            let limits = self.parse_type_limit_list(tokens)?;
            base = FolType::Limited {
                base: Box::new(base),
                limits,
            };
        }

        Ok(base)
    }

    pub(super) fn parse_type_limit_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start type limits".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut limits = Vec::new();
        for _ in 0..128 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(limits);
            }

            limits.push(self.parse_logical_expression(tokens)?);
            self.skip_ignorable(tokens)?;

            let separator = tokens.curr(false)?;
            if matches!(
                separator.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                if matches!(
                    tokens.curr(false).map(|token| token.key()),
                    Ok(KEYWORD::Symbol(SYMBOL::SquarC))
                ) {
                    let _ = tokens.bump();
                    return Ok(limits);
                }
                continue;
            }

            if matches!(separator.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(limits);
            }

            return Err(ParseError::from_token(
                &separator,
                "Expected ',', ';', or ']' in type limits".to_string(),
            ));
        }

        Err(ParseError::from_token(
            &open,
            "Type limits exceeded parser limit".to_string(),
        ))
    }

    pub(super) fn is_missing_type_reference_close_token(key: &KEYWORD) -> bool {
        key.is_terminal()
            || matches!(key, KEYWORD::Void(_))
            || matches!(
                key,
                KEYWORD::Symbol(SYMBOL::RoundC)
                    | KEYWORD::Symbol(SYMBOL::CurlyC)
                    | KEYWORD::Symbol(SYMBOL::Equal)
            )
    }

    pub(super) fn try_parse_special_type_suffix(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        base_name: &str,
    ) -> Result<Option<FolType>, ParseError> {
        if let Some(parsed) = self.try_parse_source_kind_type_suffix(tokens, base_name)? {
            return Ok(Some(parsed));
        }

        match base_name {
            "opt" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() != 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected exactly one type argument for opt[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Optional {
                    inner: Box::new(args.into_iter().next().expect("opt arg exists")),
                }))
            }
            "mul" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected at least one type argument for mul[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Multiple { types: args }))
            }
            "uni" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected at least one type argument for uni[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Union { types: args }))
            }
            "nev" => {
                let args = self.parse_type_argument_list(tokens)?;
                if !args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero type arguments for nev[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Never))
            }
            "any" => {
                let args = self.parse_type_argument_list(tokens)?;
                if !args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero type arguments for any[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Any))
            }
            "non" | "none" => {
                let args = self.parse_type_argument_list(tokens)?;
                if !args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero type arguments for none[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::None))
            }
            "ptr" => {
                let args = self.parse_type_argument_list(tokens)?;
                let (qualifier, target) = match args.as_slice() {
                    [target] => (crate::ast::PointerQualifier::Unique, target.clone()),
                    [FolType::Named { name, .. }, target] if name == "shared" => {
                        (crate::ast::PointerQualifier::Shared, target.clone())
                    }
                    [FolType::Named { name, .. }, target] if name == "raw" => {
                        (crate::ast::PointerQualifier::Raw, target.clone())
                    }
                    [FolType::Named { name, .. }, target] if name == "weak" => {
                        (crate::ast::PointerQualifier::Weak, target.clone())
                    }
                    // `ptr[shared, sync, T]` — the Arc-backed thread-safe shared
                    // pointer that may cross task boundaries (V3_MEM §8.3).
                    [FolType::Named { name: shared, .. }, FolType::Named { name: sync, .. }, target]
                        if shared == "shared" && sync == "sync" =>
                    {
                        (crate::ast::PointerQualifier::SharedSync, target.clone())
                    }
                    [FolType::Named { name, .. }, _] => {
                        let token = tokens.curr(false)?;
                        return Err(ParseError::from_token(
                            &token,
                            format!(
                                "Unknown pointer qualifier '{name}'; expected 'shared', 'shared, sync', 'weak', or 'raw'"
                            ),
                        ));
                    }
                    _ => {
                        let token = tokens.curr(false)?;
                        return Err(ParseError::from_token(
                            &token,
                            "Expected ptr[T], ptr[shared, T], ptr[shared, sync, T], or ptr[raw, T]"
                                .to_string(),
                        ));
                    }
                };
                Ok(Some(FolType::Pointer {
                    qualifier,
                    target: Box::new(target),
                }))
            }
            "err" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() > 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero or one type argument for err[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Error {
                    inner: args.into_iter().next().map(Box::new),
                }))
            }
            "evt" => {
                // `evt[T]` / `evt[T / E]` elide the lexical lifetime in local
                // declarations; `evt[L, T]` / `evt[L, T / E]` name the public
                // parent-scope lifetime `L` (V3_MEM §8.1).
                let (lifetime, value_type, error_type) =
                    self.parse_eventual_type_arguments(tokens)?;
                Ok(Some(FolType::Eventual {
                    value_type: Box::new(value_type),
                    error_type: error_type.map(Box::new),
                    lifetime,
                }))
            }
            "vec" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() != 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected exactly one type argument for vec[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Vector {
                    element_type: Box::new(args.into_iter().next().expect("vec arg exists")),
                }))
            }
            "arr" => {
                let (element_type, size) = self.parse_array_type_arguments(tokens)?;
                Ok(Some(FolType::Array {
                    element_type: Box::new(element_type),
                    size: Some(size),
                }))
            }
            "mat" => {
                let (element_type, dimensions) = self.parse_matrix_type_arguments(tokens)?;
                Ok(Some(FolType::Matrix {
                    element_type: Box::new(element_type),
                    dimensions,
                }))
            }
            "seq" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() != 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected exactly one type argument for seq[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Sequence {
                    element_type: Box::new(args.into_iter().next().expect("seq arg exists")),
                }))
            }
            "set" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.is_empty() {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected at least one type argument for set[...]".to_string(),
                    ));
                }
                Ok(Some(FolType::Set { types: args }))
            }
            "map" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() != 2 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected exactly two type arguments for map[...]".to_string(),
                    ));
                }
                let mut args = args.into_iter();
                Ok(Some(FolType::Map {
                    key_type: Box::new(args.next().expect("map key exists")),
                    value_type: Box::new(args.next().expect("map value exists")),
                }))
            }
            "chn" => {
                // `chn[T]` is a full channel. `chn[tx, T]` / `chn[rx, T]` name
                // the endpoint value types (V3_MEM §8.2); the first argument is
                // the `tx`/`rx` endpoint marker.
                let args = self.parse_type_argument_list(tokens)?;
                match args.as_slice() {
                    [element_type] => Ok(Some(FolType::Channel {
                        element_type: Box::new(element_type.clone()),
                    })),
                    [FolType::Named { name, .. }, element_type] if name == "tx" => {
                        Ok(Some(FolType::ChannelSender {
                            element_type: Box::new(element_type.clone()),
                        }))
                    }
                    [FolType::Named { name, .. }, element_type] if name == "rx" => {
                        Ok(Some(FolType::ChannelReceiver {
                            element_type: Box::new(element_type.clone()),
                        }))
                    }
                    _ => {
                        let token = tokens.curr(false)?;
                        Err(ParseError::from_token(
                            &token,
                            "Expected chn[T], chn[tx, T], or chn[rx, T]".to_string(),
                        ))
                    }
                }
            }
            "mux" => {
                // `mux[T]` — a first-class managed mutex over its guarded value
                // (V3_MEM §8.3). Replaces the `name[mux]: T` parameter option.
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() != 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected exactly one type argument for mux[T]".to_string(),
                    ));
                }
                Ok(Some(FolType::Mutex {
                    inner: Box::new(args.into_iter().next().expect("mux inner exists")),
                }))
            }
            "mod" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() > 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero or one type argument for mod[...]".to_string(),
                    ));
                }
                let name = match args.into_iter().next() {
                    None => String::new(),
                    Some(other) => other
                        .named_text()
                        .unwrap_or_else(|| Self::fol_type_label(&other)),
                };
                Ok(Some(FolType::Module { name }))
            }
            "blk" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() > 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero or one type argument for blk[...]".to_string(),
                    ));
                }
                let name = match args.into_iter().next() {
                    None => String::new(),
                    Some(other) => other
                        .named_text()
                        .unwrap_or_else(|| Self::fol_type_label(&other)),
                };
                Ok(Some(FolType::Block { name }))
            }
            "tst" => {
                let (name, access) = self.parse_test_type_arguments(tokens)?;
                Ok(Some(FolType::Test { name, access }))
            }
            "int" => Ok(Some(self.parse_integer_type_reference(tokens)?)),
            "flt" | "float" => Ok(Some(self.parse_float_type_reference(tokens)?)),
            "chr" | "char" => Ok(Some(self.parse_char_type_reference(tokens)?)),
            _ => Ok(None),
        }
    }

    pub(super) fn parse_integer_type_reference(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<FolType, ParseError> {
        let args = self
            .parse_scalar_type_options(tokens, "Expected closing ']' in integer type reference")?;

        if args.len() != 1 {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                "Expected exactly one integer type option in int[...]".to_string(),
            ));
        }

        let Some((size, signed)) = Self::lower_integer_option(&args[0]) else {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                format!("Unknown integer type option '{}'", args[0]),
            ));
        };

        Ok(FolType::Int {
            size: Some(size),
            signed,
        })
    }

    pub(super) fn parse_float_type_reference(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<FolType, ParseError> {
        let args =
            self.parse_scalar_type_options(tokens, "Expected closing ']' in float type reference")?;

        if args.len() != 1 {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                "Expected exactly one float type option in flt[...]".to_string(),
            ));
        }

        let Some(size) = Self::lower_float_option(&args[0]) else {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                format!("Unknown float type option '{}'", args[0]),
            ));
        };

        Ok(FolType::Float { size: Some(size) })
    }

    pub(super) fn parse_char_type_reference(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<FolType, ParseError> {
        let args = self.parse_scalar_type_options(
            tokens,
            "Expected closing ']' in character type reference",
        )?;

        if args.len() != 1 {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                "Expected exactly one character encoding in chr[...]".to_string(),
            ));
        }

        let Some(encoding) = Self::lower_char_option(&args[0]) else {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                format!("Unknown character type option '{}'", args[0]),
            ));
        };

        Ok(FolType::Char { encoding })
    }

    pub(super) fn parse_scalar_type_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        missing_close_message: &str,
    ) -> Result<Vec<String>, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start scalar type options".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut args = Vec::new();
        for _ in 0..16 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            Self::reject_illegal_token(&token)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(args);
            }

            let option =
                if token.key().is_ident() || token.key().is_buildin() || token.key().is_number() {
                    token.con().trim().to_string()
                } else {
                    return Err(ParseError::from_token(
                        &token,
                        "Expected scalar type option".to_string(),
                    ));
                };
            args.push(option);
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
                    return Ok(args);
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(args);
            }

            if Self::is_missing_type_reference_close_token(&sep.key()) {
                return Err(ParseError::from_token(
                    &sep,
                    "Expected closing ']' in type reference".to_string(),
                ));
            }

            return Err(ParseError::from_token(
                &sep,
                missing_close_message.to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Scalar type option list exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Scalar type option list exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    /// Parse `[T]` or `[T / E]` for an eventual type, consuming both brackets.
    /// The value type is required; the recoverable error channel after `/` is
    /// optional.
    pub(super) fn parse_eventual_type_arguments(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(Option<String>, FolType, Option<FolType>), ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start eventual type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let first_type = self.parse_type_reference_tokens(tokens)?;
        self.skip_ignorable(tokens)?;

        // `evt[L, T]` / `evt[L, T / E]` names the public parent-scope lifetime
        // `L` before the value type (V3_MEM §8.1). A leading `L,` marks the
        // named form; otherwise `first_type` is the value type (elided `L`).
        let (lifetime, value_type) =
            if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                let FolType::Named { name, .. } = &first_type else {
                    return Err(ParseError::from_token(
                        &tokens.curr(false)?,
                        "an eventual lifetime must be a simple name, e.g. 'evt[L, T]'".to_string(),
                    ));
                };
                let lifetime = name.clone();
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let value_type = self.parse_type_reference_tokens(tokens)?;
                self.skip_ignorable(tokens)?;
                (Some(lifetime), value_type)
            } else {
                (None, first_type)
            };

        let separator = tokens.curr(false)?;
        let error_type = match separator.key() {
            KEYWORD::Symbol(SYMBOL::Root) | KEYWORD::Operator(OPERATOR::Divide) => {
                let _ = tokens.bump();
                self.skip_ignorable(tokens)?;
                let error = self.parse_type_reference_tokens(tokens)?;
                self.skip_ignorable(tokens)?;
                Some(error)
            }
            _ => None,
        };

        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing ']' after eventual type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();

        Ok((lifetime, value_type, error_type))
    }

    pub(super) fn parse_type_argument_list(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<FolType>, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start type argument list".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut args = Vec::new();
        for _ in 0..64 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(args);
            }

            if Self::is_missing_type_reference_close_token(&token.key()) {
                return Err(ParseError::from_token(
                    &token,
                    "Expected closing ']' in type reference".to_string(),
                ));
            }

            args.push(self.parse_type_reference_tokens(tokens)?);
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
                    return Ok(args);
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(args);
            }
            if Self::is_missing_type_reference_close_token(&sep.key()) {
                return Err(ParseError::from_token(
                    &sep,
                    "Expected closing ']' in type reference".to_string(),
                ));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or closing ']' in type reference".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Type argument list exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Type argument list exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_array_type_arguments(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(FolType, usize), ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start array type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let element_type = self.parse_type_reference_tokens(tokens)?;
        self.skip_ignorable(tokens)?;

        let comma = tokens.curr(false)?;
        if !matches!(
            comma.key(),
            KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
        ) {
            if Self::is_missing_type_reference_close_token(&comma.key()) {
                return Err(ParseError::from_token(
                    &comma,
                    "Expected closing ']' in type reference".to_string(),
                ));
            }
            return Err(ParseError::from_token(
                &comma,
                "Expected ',' or ';' after array element type".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::SquarC))
        ) {
            return Err(ParseError::from_token(
                &tokens.curr(false)?,
                "Expected decimal array size in arr[...]".to_string(),
            ));
        }

        let size_token = tokens.curr(false)?;
        let size = size_token.con().trim().parse::<usize>().map_err(|_| {
            ParseError::from_token(
                &size_token,
                "Expected decimal array size in arr[...]".to_string(),
            )
        })?;
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi))
        ) {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
        }
        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing ']' in type reference".to_string(),
            ));
        }
        let _ = tokens.bump();

        Ok((element_type, size))
    }

    pub(super) fn parse_matrix_type_arguments(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(FolType, Vec<usize>), ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '[' to start matrix type arguments".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let element_type = self.parse_type_reference_tokens(tokens)?;
        let mut dimensions = Vec::new();

        for _ in 0..8 {
            self.skip_ignorable(tokens)?;
            let comma = tokens.curr(false)?;
            if matches!(comma.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                break;
            }
            if !matches!(
                comma.key(),
                KEYWORD::Symbol(SYMBOL::Comma) | KEYWORD::Symbol(SYMBOL::Semi)
            ) {
                if Self::is_missing_type_reference_close_token(&comma.key()) {
                    return Err(ParseError::from_token(
                        &comma,
                        "Expected closing ']' in type reference".to_string(),
                    ));
                }
                return Err(ParseError::from_token(
                    &comma,
                    "Expected ',' or ';' after matrix element type".to_string(),
                ));
            }
            let _ = tokens.bump();

            self.skip_ignorable(tokens)?;
            if matches!(
                tokens.curr(false).map(|token| token.key()),
                Ok(KEYWORD::Symbol(SYMBOL::SquarC))
            ) {
                break;
            }
            let dim_token = tokens.curr(false)?;
            let dim = dim_token.con().trim().parse::<usize>().map_err(|_| {
                ParseError::from_token(
                    &dim_token,
                    "Expected decimal matrix dimension in mat[...]".to_string(),
                )
            })?;
            dimensions.push(dim);
            let _ = tokens.bump();
        }

        if dimensions.is_empty() {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                "Expected at least one matrix dimension in mat[...]".to_string(),
            ));
        }

        self.skip_ignorable(tokens)?;
        let close = tokens.curr(false)?;
        if !matches!(close.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            return Err(ParseError::from_token(
                &close,
                "Expected closing ']' in type reference".to_string(),
            ));
        }
        let _ = tokens.bump();

        Ok((element_type, dimensions))
    }
}
