use super::*;

impl AstParser {
    pub(super) fn lookahead_parenthesized_generic_header_before_colon(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return false,
        };
        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return false;
        }

        let mut depth = 1usize;
        let mut header_closed = false;
        let mut saw_colon_after_header = false;
        let mut saw_assign_after_header = false;
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };
            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if !header_closed {
                match key {
                    KEYWORD::Symbol(SYMBOL::RoundO) => depth += 1,
                    KEYWORD::Symbol(SYMBOL::RoundC) => {
                        if depth == 0 {
                            return false;
                        }
                        depth -= 1;
                        if depth == 0 {
                            header_closed = true;
                        }
                    }
                    KEYWORD::Symbol(SYMBOL::CurlyO) => return false,
                    _ => {}
                }
                continue;
            }

            match key {
                KEYWORD::Symbol(SYMBOL::Colon) => {
                    saw_colon_after_header = true;
                }
                KEYWORD::Symbol(SYMBOL::Equal) if saw_colon_after_header => {
                    saw_assign_after_header = true;
                }
                KEYWORD::Operator(OPERATOR::Flow) if saw_colon_after_header => return false,
                KEYWORD::Symbol(SYMBOL::RoundO) if saw_assign_after_header => return true,
                KEYWORD::Symbol(SYMBOL::CurlyO) if saw_assign_after_header => return false,
                _ => {}
            }
        }

        false
    }

    pub(super) fn parse_def_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let def_token = tokens.curr(false)?;
        if !matches!(def_token.key(), KEYWORD::Keyword(BUILDIN::Def)) {
            return Err(ParseError::from_token(
                &def_token,
                "Expected 'def' declaration".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_decl_visibility_options(tokens, "definition")?;
        self.skip_ignorable(tokens)?;

        let name_token = tokens.curr(false)?;
        let name = Self::expect_named_label(&name_token, "Expected definition name")?;
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let params = self.parse_definition_parameter_header(tokens)?;
        self.skip_ignorable(tokens)?;
        let colon = tokens.curr(false)?;
        if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
            return Err(ParseError::from_token(
                &colon,
                "Expected ':' after definition name".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let def_type_token = tokens.curr(false)?;
        let def_type = self.parse_type_reference_tokens(tokens)?;
        if !self.is_supported_definition_type(&def_type) {
            return Err(ParseError::from_token(
                &def_type_token,
                format!(
                    "Definition declarations currently support only mod[...], blk[...], tst[...], pkg, loc, mac, alt, or def[] types, found '{}'",
                    Self::fol_type_label(&def_type)
                ),
            ));
        }
        if !params.is_empty() && !self.definition_supports_params(&def_type) {
            return Err(ParseError::from_token(
                &def_type_token,
                "Definition parameters are currently supported only for mac definitions"
                    .to_string(),
            ));
        }

        self.skip_ignorable(tokens)?;
        let next = tokens.curr(false)?;
        if !matches!(next.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
            if matches!(def_type, FolType::Block { .. }) {
                return Ok(AstNode::DefDecl {
                    options,
                    name,
                    params,
                    def_type,
                    body: Vec::new(),
                });
            }

            return Err(ParseError::from_token(
                &next,
                "Expected '=' before definition body".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let body = if self.definition_uses_block_body(&def_type) {
            let open_body = tokens.curr(false)?;
            if !matches!(open_body.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
                return Err(ParseError::from_token(
                    &open_body,
                    "Expected '{' to start definition body".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.parse_block_body(tokens, "Expected '}' to close definition body")?
        } else {
            vec![self.parse_logical_expression(tokens)?]
        };

        Ok(AstNode::DefDecl {
            options,
            name,
            params,
            def_type,
            body,
        })
    }

    pub(super) fn parse_alias_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<AstNode, ParseError> {
        let ali_token = tokens.curr(false)?;
        if !matches!(ali_token.key(), KEYWORD::Keyword(BUILDIN::Ali)) {
            return Err(ParseError::from_token(
                &ali_token,
                "Expected 'ali' declaration".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_type_options(tokens)?;
        if options
            .iter()
            .any(|option| !matches!(option, TypeOption::Export))
        {
            let token = tokens.curr(false)?;
            return Err(ParseError::from_token(
                &token,
                "Alias declarations support only the export option".to_string(),
            ));
        }
        self.skip_ignorable(tokens)?;

        let name_token = tokens.curr(false)?;
        let name = Self::expect_named_label(&name_token, "Expected alias declaration name")?;
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let colon = tokens.curr(false)?;
        if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
            return Err(ParseError::from_token(
                &colon,
                "Expected ':' after alias name".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let target = self.parse_type_reference_tokens(tokens)?;

        Ok(AstNode::AliasDecl {
            options,
            name,
            target,
        })
    }

    pub(super) fn parse_type_decl(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let typ_token = tokens.curr(false)?;
        if !matches!(typ_token.key(), KEYWORD::Keyword(BUILDIN::Typ)) {
            return Err(ParseError::from_token(
                &typ_token,
                "Expected 'typ' declaration".to_string(),
            ));
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let options = self.parse_type_options(tokens)?;
        self.skip_ignorable(tokens)?;

        if matches!(
            tokens.curr(false).map(|token| token.key()),
            Ok(KEYWORD::Symbol(SYMBOL::RoundO))
        ) {
            return self.parse_type_group(tokens, options);
        }

        self.parse_single_type_decl_with_options(tokens, options)
    }

    pub(super) fn parse_single_type_decl_with_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        options: Vec<TypeOption>,
    ) -> Result<Vec<AstNode>, ParseError> {
        let name_token = tokens.curr(false)?;
        let name = Self::expect_named_label(&name_token, "Expected type declaration name")?;
        let _ = tokens.bump();

        let mut names = vec![name];
        loop {
            self.skip_ignorable(tokens)?;
            let sep = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };
            if !matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                break;
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            let next_name = tokens.curr(false)?;
            let next_name =
                Self::expect_named_label(&next_name, "Expected type declaration name after ','")?;
            names.push(next_name);
            let _ = tokens.bump();
        }

        self.skip_ignorable(tokens)?;
        let generics = self.parse_type_generic_header(tokens)?;
        self.skip_ignorable(tokens)?;
        let explicit_contracts = self.parse_type_contract_header(tokens)?;
        self.skip_ignorable(tokens)?;
        let colon = tokens.curr(false)?;
        if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
            return Err(ParseError::from_token(
                &colon,
                "Expected ':' after type name".to_string(),
            ));
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let mut type_defs = vec![self.parse_type_definition(tokens)?];
        while type_defs.len() < names.len() {
            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if !matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                break;
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            type_defs.push(self.parse_type_definition(tokens)?);
        }

        if names.len() > 1 && (!generics.is_empty() || !explicit_contracts.is_empty()) {
            let token = tokens.curr(false).unwrap_or(name_token);
            return Err(ParseError::from_token(
                &token,
                "Type generics and explicit contracts are currently supported only on single-name type declarations".to_string(),
            ));
        }

        let assigned_type_defs = match type_defs.len() {
            1 => vec![type_defs[0].clone(); names.len()],
            n if n == names.len() => type_defs,
            _ => {
                let token = tokens.curr(false).unwrap_or(name_token);
                return Err(ParseError::from_token(
                    &token,
                    "Type definition count must match declared names or provide a single shared definition".to_string(),
                ));
            }
        };

        let mut nodes = Vec::new();
        for (name, type_def) in names.into_iter().zip(assigned_type_defs) {
            let mut contracts = self.type_contracts_from_generics(&generics, &type_def);
            contracts.extend(explicit_contracts.clone());
            nodes.push(AstNode::TypeDecl {
                options: options.clone(),
                generics: generics.clone(),
                contracts,
                explicit_contracts: explicit_contracts.clone(),
                name,
                type_def,
            });
        }

        Ok(nodes)
    }

    pub(super) fn parse_type_definition(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<TypeDefinition, ParseError> {
        let marker = tokens.curr(false)?.con().trim().to_string();
        if marker == "ent" {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            self.parse_empty_type_marker_brackets(tokens, "entry")?;
            self.skip_ignorable(tokens)?;

            let assign = tokens.curr(false)?;
            if !matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Err(ParseError::from_token(
                    &assign,
                    "Expected '=' after entry type marker".to_string(),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            return self.parse_entry_type_definition(tokens);
        }

        if marker == "rec" || marker == "obj" {
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            self.parse_empty_type_marker_brackets(
                tokens,
                if marker == "obj" { "object" } else { "record" },
            )?;
            self.skip_ignorable(tokens)?;

            let assign = tokens.curr(false)?;
            if marker == "obj" && !matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Ok(TypeDefinition::Record {
                    fields: HashMap::new(),
                    field_meta: HashMap::new(),
                    members: Vec::new(),
                });
            }
            if !matches!(assign.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                return Err(ParseError::from_token(
                    &assign,
                    format!(
                        "Expected '=' after {} type marker",
                        if marker == "obj" { "object" } else { "record" }
                    ),
                ));
            }
            let _ = tokens.bump();
            self.skip_ignorable(tokens)?;
            return self.parse_record_type_definition(tokens);
        }

        if matches!(tokens.curr(false)?.key(), KEYWORD::Symbol(SYMBOL::CurlyO))
            && !matches!(
                self.next_significant_key_from_window(tokens),
                Some(KEYWORD::Keyword(BUILDIN::Fun))
            )
        {
            return self.parse_record_type_definition(tokens);
        }

        let target = self.parse_type_reference_tokens(tokens)?;
        Ok(TypeDefinition::Alias { target })
    }

    pub(super) fn parse_empty_type_marker_brackets(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        marker_name: &str,
    ) -> Result<(), ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(()),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::SquarO)) {
            return Ok(());
        }
        let _ = tokens.bump();

        self.skip_ignorable(tokens)?;
        let token = tokens.curr(false)?;
        Self::reject_illegal_token(&token)?;
        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
            let _ = tokens.bump();
            return Ok(());
        }

        Err(ParseError::from_token(
            &token,
            format!("Unknown {} type marker option", marker_name),
        ))
    }

    pub(super) fn parse_type_generic_header(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Generic>, ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        let close_symbol = match open.key() {
            KEYWORD::Symbol(SYMBOL::RoundO) => SYMBOL::RoundC,
            KEYWORD::Symbol(SYMBOL::SquarO) => SYMBOL::SquarC,
            _ => return Ok(Vec::new()),
        };
        let close_label = match close_symbol {
            SYMBOL::RoundC => ")",
            SYMBOL::SquarC => "]",
            _ => unreachable!("close_symbol is always RoundC or SquarC"),
        };
        if !matches!(
            open.key(),
            KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::SquarO)
        ) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();

        self.parse_generic_list_with_close(tokens, close_symbol, close_label)
    }

    pub(super) fn parse_type_contract_header(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<FolType>, ParseError> {
        self.skip_ignorable(tokens)?;
        let open = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();

        let mut contracts = Vec::new();
        for _ in 0..64 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(contracts);
            }

            contracts.push(self.parse_type_reference_tokens(tokens)?);
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
                    Ok(KEYWORD::Symbol(SYMBOL::RoundC))
                ) {
                    let _ = tokens.bump();
                    return Ok(contracts);
                }
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::RoundC)) {
                let _ = tokens.bump();
                return Ok(contracts);
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or ')' in type contracts".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Type contracts exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Type contracts exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_type_options(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<TypeOption>, ParseError> {
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
        for _ in 0..16 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;
            Self::reject_illegal_token(&token)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::SquarC)) {
                let _ = tokens.bump();
                return Ok(options);
            }

            let option = match token.con().trim() {
                "+" | "pub" | "exp" | "export" => TypeOption::Export,
                "set" => TypeOption::Set,
                "get" => TypeOption::Get,
                "nothing" | "non" => TypeOption::Nothing,
                "ext" => TypeOption::Extension,
                _ => {
                    return Err(ParseError::from_token(
                        &token,
                        "Unknown type option".to_string(),
                    ))
                }
            };
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
                "Expected ',', ';', or ']' in type options".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Type options exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Type options exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    fn type_contracts_from_generics(
        &self,
        generics: &[Generic],
        type_def: &TypeDefinition,
    ) -> Vec<FolType> {
        if !matches!(
            type_def,
            TypeDefinition::Record { .. } | TypeDefinition::Entry { .. }
        ) {
            return Vec::new();
        }

        generics
            .iter()
            .filter(|generic| generic.constraints.is_empty())
            .map(|generic| FolType::Named {
                syntax_id: None,
                name: generic.name.clone(),
            })
            .collect()
    }

    fn parse_definition_parameter_header(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<Parameter>, ParseError> {
        self.skip_ignorable(tokens)?;
        let current = match tokens.curr(false) {
            Ok(token) => token,
            Err(_) => return Ok(Vec::new()),
        };

        if !matches!(current.key(), KEYWORD::Symbol(SYMBOL::RoundO)) {
            return Ok(Vec::new());
        }
        let _ = tokens.bump();
        self.parse_parameter_list(tokens)
    }

    fn is_supported_definition_type(&self, def_type: &FolType) -> bool {
        match def_type {
            FolType::Module { .. }
            | FolType::Block { .. }
            | FolType::Test { .. }
            | FolType::Package { .. }
            | FolType::Location { .. } => true,
            other => matches!(
                other.named_text().as_deref(),
                Some("mac" | "alt" | "def[]" | "def")
            ),
        }
    }

    fn definition_uses_block_body(&self, def_type: &FolType) -> bool {
        matches!(
            def_type,
            FolType::Module { .. } | FolType::Block { .. } | FolType::Test { .. }
        )
    }

    fn definition_supports_params(&self, def_type: &FolType) -> bool {
        matches!(def_type.named_text().as_deref(), Some("mac"))
    }
}
