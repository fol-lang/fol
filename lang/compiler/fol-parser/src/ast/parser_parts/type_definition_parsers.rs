use super::*;

impl AstParser {
    pub(super) fn parse_entry_type_definition(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<TypeDefinition, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start type entry definition".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut variants = HashMap::new();
        let mut variant_meta = HashMap::new();
        let mut members = Vec::new();
        let mut seen_variant_names = HashSet::new();
        let mut seen_members = HashSet::new();
        for _ in 0..256 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(TypeDefinition::Entry {
                    variants,
                    variant_meta,
                    members,
                });
            }

            if let Some(member) = self.parse_prefixed_type_routine_member(tokens)? {
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Entry {
                        variants,
                        variant_meta,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type entry definition".to_string(),
                ));
            }

            if self.looks_like_type_routine_member(tokens)? {
                let member = self.parse_standard_routine_signature(tokens)?;
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Entry {
                        variants,
                        variant_meta,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type entry definition".to_string(),
                ));
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Ali)) {
                let member = self.parse_alias_decl(tokens)?;
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Entry {
                        variants,
                        variant_meta,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type entry definition".to_string(),
                ));
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Typ)) {
                for member in self.parse_type_decl(tokens)? {
                    let key = self.type_member_key(&member);
                    if !seen_members.insert(canonical_identifier_key(&key)) {
                        return Err(self.duplicate_type_member_error(&token, &key));
                    }
                    members.push(member);
                }
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Entry {
                        variants,
                        variant_meta,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type entry definition".to_string(),
                ));
            }

            let default_options = if let Some((keyword, options)) =
                self.lookahead_binding_alternative(tokens)
            {
                match keyword {
                    "var" | "con" => {
                        let _ = tokens.bump();
                        self.skip_ignorable(tokens)?;
                        let _ = tokens.bump();
                        self.skip_ignorable(tokens)?;
                        options
                    }
                    _ => {
                        return Err(ParseError::from_token(
                            &token,
                            "Expected 'var', 'lab', or 'con' in type entry definition".to_string(),
                        ))
                    }
                }
            } else {
                match token.key() {
                    KEYWORD::Keyword(BUILDIN::Var) => {
                        let _ = tokens.bump();
                        self.parse_binding_options(
                            tokens,
                            vec![VarOption::Mutable, VarOption::Normal],
                        )?
                    }
                    KEYWORD::Keyword(BUILDIN::Lab) => {
                        let _ = tokens.bump();
                        self.parse_binding_options(
                            tokens,
                            vec![VarOption::Immutable, VarOption::Normal],
                        )?
                    }
                    KEYWORD::Keyword(BUILDIN::Con) => {
                        let _ = tokens.bump();
                        self.parse_binding_options(
                            tokens,
                            vec![VarOption::Immutable, VarOption::Normal],
                        )?
                    }
                    _ => {
                        return Err(ParseError::from_token(
                            &token,
                            "Expected 'var', 'lab', or 'con' in type entry definition".to_string(),
                        ))
                    }
                }
            };

            self.skip_ignorable(tokens)?;

            let mut names = Vec::new();
            for _ in 0..64 {
                self.skip_ignorable(tokens)?;
                let name_token = tokens.curr(false)?;
                let name = Self::expect_named_label(&name_token, "Expected entry variant name")?;
                names.push((name, name_token));
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                    let _ = tokens.bump();
                    continue;
                }
                break;
            }

            self.skip_ignorable(tokens)?;
            let mut variant_type = None;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    variant_type = Some(self.parse_type_reference_tokens(tokens)?);
                }
            }

            self.skip_ignorable(tokens)?;
            let mut default = None;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    default = Some(self.parse_logical_expression(tokens)?);
                }
            }

            for (name, name_token) in names {
                if !seen_variant_names.insert(canonical_identifier_key(&name)) {
                    return Err(ParseError::from_token(
                        &name_token,
                        format!("Duplicate entry variant '{}'", name),
                    ));
                }
                let _ = variants.insert(name.clone(), variant_type.clone());
                if !seen_members.insert(canonical_identifier_key(&name)) {
                    return Err(self.duplicate_type_member_error(&name_token, &name));
                }
                variant_meta.insert(
                    name,
                    EntryVariantMeta {
                        default: default.clone(),
                        options: default_options.clone(),
                    },
                );
            }

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
            {
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(TypeDefinition::Entry {
                    variants,
                    variant_meta,
                    members,
                });
            }
            if sep.key().is_terminal() || matches!(sep.key(), KEYWORD::Void(_)) {
                return Err(ParseError::from_token(
                    &sep,
                    "Expected '}' to close type entry definition".to_string(),
                ));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in type entry definition".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Type entry definition exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Type entry definition exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    pub(super) fn parse_record_type_definition(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<TypeDefinition, ParseError> {
        let open = tokens.curr(false)?;
        if !matches!(open.key(), KEYWORD::Symbol(SYMBOL::CurlyO)) {
            return Err(ParseError::from_token(
                &open,
                "Expected '{' to start type record definition".to_string(),
            ));
        }
        let _ = tokens.bump();

        let mut fields = HashMap::new();
        let mut field_meta = HashMap::new();
        let mut field_order = Vec::new();
        let mut members = Vec::new();
        let mut seen_field_names = HashSet::new();
        let mut seen_members = HashSet::new();
        for _ in 0..256 {
            self.skip_ignorable(tokens)?;
            let token = tokens.curr(false)?;

            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(TypeDefinition::Record {
                    fields,
                    field_meta,
                    field_order,
                    members,
                });
            }

            if token.key().is_terminal() || matches!(token.key(), KEYWORD::Void(_)) {
                return Err(ParseError::from_token(
                    &token,
                    "Expected '}' to close type record definition".to_string(),
                ));
            }

            if let Some(member) = self.parse_prefixed_type_routine_member(tokens)? {
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Record {
                        fields,
                        field_meta,
                        field_order,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type record definition".to_string(),
                ));
            }

            if self.looks_like_type_routine_member(tokens)? {
                let member = self.parse_standard_routine_signature(tokens)?;
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Record {
                        fields,
                        field_meta,
                        field_order,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type record definition".to_string(),
                ));
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Ali)) {
                let member = self.parse_alias_decl(tokens)?;
                let key = self.type_member_key(&member);
                if !seen_members.insert(canonical_identifier_key(&key)) {
                    return Err(self.duplicate_type_member_error(&token, &key));
                }
                members.push(member);
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Record {
                        fields,
                        field_meta,
                        field_order,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type record definition".to_string(),
                ));
            }

            if matches!(token.key(), KEYWORD::Keyword(BUILDIN::Typ)) {
                for member in self.parse_type_decl(tokens)? {
                    let key = self.type_member_key(&member);
                    if !seen_members.insert(canonical_identifier_key(&key)) {
                        return Err(self.duplicate_type_member_error(&token, &key));
                    }
                    members.push(member);
                }
                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                    || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
                {
                    let _ = tokens.bump();
                    continue;
                }
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                    let _ = tokens.bump();
                    return Ok(TypeDefinition::Record {
                        fields,
                        field_meta,
                        field_order,
                        members,
                    });
                }
                return Err(ParseError::from_token(
                    &sep,
                    "Expected ',', ';', or '}' in type record definition".to_string(),
                ));
            }

            let options =
                if let Some((keyword, options)) = self.lookahead_binding_alternative(tokens) {
                    match keyword {
                        "var" | "con" => {
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                            options
                        }
                        _ => Vec::new(),
                    }
                } else {
                    match token.key() {
                        KEYWORD::Keyword(BUILDIN::Var) => {
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                            self.parse_binding_options(
                                tokens,
                                vec![VarOption::Mutable, VarOption::Normal],
                            )?
                        }
                        KEYWORD::Keyword(BUILDIN::Lab) => {
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                            self.parse_binding_options(
                                tokens,
                                vec![VarOption::Immutable, VarOption::Normal],
                            )?
                        }
                        KEYWORD::Keyword(BUILDIN::Con) => {
                            let _ = tokens.bump();
                            self.skip_ignorable(tokens)?;
                            self.parse_binding_options(
                                tokens,
                                vec![VarOption::Immutable, VarOption::Normal],
                            )?
                        }
                        _ => Vec::new(),
                    }
                };

            self.skip_ignorable(tokens)?;

            let mut field_names = Vec::new();
            loop {
                let name_token = tokens.curr(false)?;
                let field_name = Self::expect_named_label(
                    &name_token,
                    "Expected field name in type record definition",
                )?;
                field_names.push((field_name, name_token));
                let _ = tokens.bump();

                self.skip_ignorable(tokens)?;
                let sep = tokens.curr(false)?;
                if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma)) {
                    let _ = tokens.bump();
                    self.skip_ignorable(tokens)?;
                    continue;
                }
                break;
            }
            self.skip_ignorable(tokens)?;

            let colon = tokens.curr(false)?;
            if !matches!(colon.key(), KEYWORD::Symbol(SYMBOL::Colon)) {
                return Err(ParseError::from_token(
                    &colon,
                    "Expected ':' after record field name".to_string(),
                ));
            }
            let _ = tokens.bump();

            self.skip_ignorable(tokens)?;
            let field_type = self.parse_type_reference_tokens(tokens)?;

            self.skip_ignorable(tokens)?;
            let mut default = None;
            if let Ok(token) = tokens.curr(false) {
                if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Equal)) {
                    let _ = tokens.bump();
                    default = Some(self.parse_logical_expression(tokens)?);
                    self.skip_ignorable(tokens)?;
                }
            }

            for (field_name, name_token) in field_names {
                if !seen_field_names.insert(canonical_identifier_key(&field_name)) {
                    return Err(ParseError::from_token(
                        &name_token,
                        format!("Duplicate record field '{}'", field_name),
                    ));
                }
                let _ = fields.insert(field_name.clone(), field_type.clone());
                if !seen_members.insert(canonical_identifier_key(&field_name)) {
                    return Err(self.duplicate_type_member_error(&name_token, &field_name));
                }
                field_order.push(field_name.clone());
                field_meta.insert(
                    field_name,
                    RecordFieldMeta {
                        default: default.clone(),
                        options: options.clone(),
                    },
                );
            }

            self.skip_ignorable(tokens)?;
            let sep = tokens.curr(false)?;
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Comma))
                || matches!(sep.key(), KEYWORD::Symbol(SYMBOL::Semi))
            {
                let _ = tokens.bump();
                continue;
            }
            if matches!(sep.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
                let _ = tokens.bump();
                return Ok(TypeDefinition::Record {
                    fields,
                    field_meta,
                    field_order,
                    members,
                });
            }
            if sep.key().is_terminal() || matches!(sep.key(), KEYWORD::Void(_)) {
                return Err(ParseError::from_token(
                    &sep,
                    "Expected '}' to close type record definition".to_string(),
                ));
            }

            return Err(ParseError::from_token(
                &sep,
                "Expected ',', ';', or '}' in type record definition".to_string(),
            ));
        }

        let error = if let Ok(token) = tokens.curr(false) {
            ParseError::from_token(
                &token,
                "Type record definition exceeded parser limit".to_string(),
            )
        } else {
            ParseError {
                kind: ParseErrorKind::Syntax,
                message: "Type record definition exceeded parser limit".to_string(),
                file: None,
                line: 0,
                column: 0,
                length: 0,
            }
        };
        Err(error)
    }

    fn type_member_key(&self, node: &AstNode) -> String {
        match node {
            AstNode::FunDecl { name, params, .. }
            | AstNode::LogDecl { name, params, .. }
            | AstNode::ProDecl { name, params, .. } => {
                format!("{}#{}", name, params.len())
            }
            AstNode::AliasDecl { name, .. }
            | AstNode::TypeDecl { name, .. }
            | AstNode::VarDecl { name, .. }
            | AstNode::LabDecl { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    fn duplicate_type_member_error(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
        key: &str,
    ) -> ParseError {
        ParseError::from_token(token, format!("Duplicate type member '{}'", key))
    }

    fn parse_prefixed_type_routine_member(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Option<AstNode>, ParseError> {
        let token = tokens.curr(false)?;
        if !matches!(token.key(), KEYWORD::Symbol(SYMBOL::Plus)) {
            return Ok(None);
        }

        let Some(next_key) = self.next_significant_key_from_window(tokens) else {
            return Ok(None);
        };
        if !matches!(
            next_key,
            KEYWORD::Keyword(BUILDIN::Fun)
                | KEYWORD::Keyword(BUILDIN::Pro)
                | KEYWORD::Keyword(BUILDIN::Log)
        ) {
            return Ok(None);
        }

        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;
        let mut member = self.parse_standard_routine_signature(tokens)?;
        match &mut member {
            AstNode::FunDecl { options, .. }
            | AstNode::LogDecl { options, .. }
            | AstNode::ProDecl { options, .. } => {
                if !options.contains(&FunOption::Export) {
                    options.insert(0, FunOption::Export);
                }
            }
            _ => {}
        }
        Ok(Some(member))
    }

    fn looks_like_type_routine_member(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<bool, ParseError> {
        let token = tokens.curr(false)?;
        if !matches!(
            token.key(),
            KEYWORD::Keyword(BUILDIN::Fun)
                | KEYWORD::Keyword(BUILDIN::Pro)
                | KEYWORD::Keyword(BUILDIN::Log)
        ) {
            return Ok(false);
        }

        Ok(!matches!(
            self.next_significant_key_from_window(tokens),
            Some(KEYWORD::Symbol(SYMBOL::Colon))
        ))
    }
}
