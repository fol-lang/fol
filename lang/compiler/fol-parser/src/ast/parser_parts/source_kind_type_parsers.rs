use super::*;

impl AstParser {
    pub(super) fn try_parse_source_kind_type_suffix(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        base_name: &str,
    ) -> Result<Option<FolType>, ParseError> {
        match base_name {
            "pkg" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() > 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero or one type argument for pkg[...]".to_string(),
                    ));
                }
                let name = match args.into_iter().next() {
                    None => String::new(),
                    Some(other) => other
                        .named_text()
                        .unwrap_or_else(|| Self::fol_type_label(&other)),
                };
                Ok(Some(FolType::Package { name }))
            }
            "loc" => {
                let args = self.parse_type_argument_list(tokens)?;
                if args.len() > 1 {
                    let token = tokens.curr(false)?;
                    return Err(ParseError::from_token(
                        &token,
                        "Expected zero or one type argument for loc[...]".to_string(),
                    ));
                }
                let name = match args.into_iter().next() {
                    None => String::new(),
                    Some(other) => other
                        .named_text()
                        .unwrap_or_else(|| Self::fol_type_label(&other)),
                };
                Ok(Some(FolType::Location { name }))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn lower_bare_source_kind_type_name(name: &str) -> Option<FolType> {
        match name {
            "pkg" => Some(FolType::Package {
                name: String::new(),
            }),
            "loc" => Some(FolType::Location {
                name: String::new(),
            }),
            _ => None,
        }
    }
}
