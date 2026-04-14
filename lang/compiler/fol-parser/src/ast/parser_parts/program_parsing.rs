use super::*;
use crate::{ParsedPackage, ParsedTopLevel, SyntaxIndex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootSurface {
    MixedProgram,
    DeclarationOnly,
}

impl AstParser {
    pub fn new() -> Self {
        Self {
            routine_depth: Cell::new(0),
            loop_depth: Cell::new(0),
            syntax_index: std::cell::RefCell::new(None),
        }
    }

    pub fn parse_package(
        &mut self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParsedPackage, Vec<fol_diagnostics::Diagnostic>> {
        let sources = tokens.sources().to_vec();
        let (entries, syntax_index) = self
            .parse_top_level_entries_with_surface(tokens, RootSurface::DeclarationOnly)
            .map_err(Self::glitch_vec_to_diagnostics)?;
        Ok(ParsedPackage::from_sources_and_entries(
            &sources,
            entries,
            syntax_index,
        ))
    }

    pub fn parse_script_package(
        &mut self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParsedPackage, Vec<fol_diagnostics::Diagnostic>> {
        let sources = tokens.sources().to_vec();
        let (entries, syntax_index) = self
            .parse_top_level_entries_with_surface(tokens, RootSurface::MixedProgram)
            .map_err(Self::glitch_vec_to_diagnostics)?;
        Ok(ParsedPackage::from_sources_and_entries(
            &sources,
            entries,
            syntax_index,
        ))
    }

    pub fn parse_decl_package(
        &mut self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParsedPackage, Vec<fol_diagnostics::Diagnostic>> {
        self.parse_package(tokens)
    }

    fn glitch_vec_to_diagnostics(errors: Vec<ParseError>) -> Vec<fol_diagnostics::Diagnostic> {
        errors.into_iter().map(|e| {
            use fol_diagnostics::ToDiagnostic;
            e.to_diagnostic()
        }).collect()
    }

    fn push_top_level_entry(
        &self,
        entries: &mut Vec<ParsedTopLevel>,
        token: &fol_lexer::lexer::stage3::element::Element,
        node: AstNode,
    ) {
        let node_id = self
            .record_syntax_origin(token)
            .expect("top-level parsing should have active syntax tracking");
        entries.push(ParsedTopLevel {
            node_id,
            node,
            meta: crate::ParsedTopLevelMeta::default(),
        });
    }

    fn reject_file_root_form<F>(
        &mut self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        token: &fol_lexer::lexer::stage3::element::Element,
        before: (usize, usize, String),
        message: &str,
        errors: &mut Vec<ParseError>,
        parse: F,
    ) -> bool
    where
        F: FnOnce(
            &mut Self,
            &mut fol_lexer::lexer::stage3::Elements,
        ) -> Result<(), ParseError>,
    {
        match parse(self, tokens) {
            Ok(()) => errors.push(ParseError::from_token_with_kind(token, ParseErrorKind::FileRoot, message.to_string())),
            Err(error) => errors.push(error),
        }
        self.bump_if_no_progress(tokens, before);
        tokens.curr(false).is_err()
    }

    fn extend_top_level_entries(
        &self,
        entries: &mut Vec<ParsedTopLevel>,
        token: &fol_lexer::lexer::stage3::element::Element,
        nodes: Vec<AstNode>,
    ) {
        for node in nodes {
            self.push_top_level_entry(entries, token, node);
        }
    }

    fn parse_top_level_entries_with_surface(
        &mut self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        surface: RootSurface,
    ) -> Result<(Vec<ParsedTopLevel>, SyntaxIndex), Vec<ParseError>> {
        self.start_syntax_tracking();
        let mut entries = Vec::new();
        let mut errors: Vec<ParseError> = Vec::new();

        for _ in 0..8_192 {
            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(error) => {
                    errors.push(error.into());
                    break;
                }
            };

            let key = token.key();

            if key.is_eof() {
                break;
            }

            if key.is_illegal() {
                errors.push(ParseError::from_token(
                    &token,
                    format!("Parser encountered illegal token '{}'", token.con()),
                ));
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            if key.is_comment() {
                match self.parse_comment_token(&token) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                if tokens.bump().is_none() {
                    break;
                }
                self.skip_layout(tokens).map_err(|e| vec![e])?;
                continue;
            }

            if Self::key_is_layout_ignorable(&key) || key.is_boundary() {
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            if self.lookahead_binding_alternative(tokens).is_some() {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_binding_alternative_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Var)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_var_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Let)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_let_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Con)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_con_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Lab)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_lab_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Use)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_use_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Seg)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_seg_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }


            if matches!(key, KEYWORD::Keyword(BUILDIN::Std)) && self.lookahead_is_std_decl(tokens) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_std_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Def)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_def_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Ali)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_alias_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Typ)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_type_decl(tokens) {
                    Ok(nodes) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.extend_top_level_entries(&mut entries, &token, nodes);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Fun)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_fun_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Log)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_log_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Pro)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_pro_decl(tokens) {
                    Ok(node) => {
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.bump_if_no_progress(tokens, before);
                    }
                    Err(error) => {
                        errors.push(error);
                        self.sync_to_next_declaration(tokens);
                    }
                }
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && self.lookahead_is_call(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                if self.reject_file_root_form(
                    tokens,
                    &token,
                    before,
                    "Executable calls are not allowed at file root",
                    &mut errors,
                    |parser, tokens| {
                        parser.parse_call_stmt(tokens)?;
                        parser.consume_required_semicolon(tokens)?;
                        Ok(())
                    },
                ) {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && matches!(key, KEYWORD::Symbol(SYMBOL::Dot))
                && self.lookahead_is_dot_builtin_call(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                if self.reject_file_root_form(
                    tokens,
                    &token,
                    before,
                    "Executable calls are not allowed at file root",
                    &mut errors,
                    |parser, tokens| {
                        parser.parse_dot_builtin_call_expr(tokens)?;
                        parser.consume_required_semicolon(tokens)?;
                        Ok(())
                    },
                ) {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && (matches!(
                    key,
                    KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::Dot)
                ) || AstParser::token_can_be_logical_name(&key)
                    || key.is_textual_literal())
                && self.lookahead_is_general_invoke(
                    tokens,
                    matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)),
                )
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                if self.reject_file_root_form(
                    tokens,
                    &token,
                    before,
                    "Executable calls are not allowed at file root",
                    &mut errors,
                    |parser, tokens| {
                        parser.parse_invoke_stmt(tokens)?;
                        parser.consume_required_semicolon(tokens)?;
                        Ok(())
                    },
                ) {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && matches!(
                    key,
                    KEYWORD::Keyword(BUILDIN::Panic)
                        | KEYWORD::Keyword(BUILDIN::Report)
                        | KEYWORD::Keyword(BUILDIN::Check)
                        | KEYWORD::Keyword(BUILDIN::Assert)
                )
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                if self.reject_file_root_form(
                    tokens,
                    &token,
                    before,
                    "Executable calls are not allowed at file root",
                    &mut errors,
                    |parser, tokens| {
                        parser.parse_builtin_call_stmt(tokens)?;
                        parser.consume_required_semicolon(tokens)?;
                        Ok(())
                    },
                ) {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && self.lookahead_is_assignment(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                if self.reject_file_root_form(
                    tokens,
                    &token,
                    before,
                    "Assignments are not allowed at file root",
                    &mut errors,
                    |parser, tokens| {
                        parser.parse_assignment_stmt(tokens)?;
                        parser.consume_required_semicolon(tokens)?;
                        Ok(())
                    },
                ) {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && matches!(
                    key,
                    KEYWORD::Keyword(BUILDIN::Return)
                        | KEYWORD::Keyword(BUILDIN::Break)
                        | KEYWORD::Keyword(BUILDIN::Yield)
                        | KEYWORD::Keyword(BUILDIN::Defer)
                        | KEYWORD::Keyword(BUILDIN::When)
                        | KEYWORD::Keyword(BUILDIN::If)
                        | KEYWORD::Keyword(BUILDIN::Select)
                        | KEYWORD::Keyword(BUILDIN::While)
                        | KEYWORD::Keyword(BUILDIN::Loop)
                        | KEYWORD::Keyword(BUILDIN::For)
                        | KEYWORD::Keyword(BUILDIN::Each)
                )
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                let hit_eof = if matches!(key, KEYWORD::Keyword(BUILDIN::Return)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_return_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::Break)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_break_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::Yield)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_yield_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::Defer)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_defer_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::When)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_when_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::If)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_if_stmt(tokens).map(|_| ()),
                    )
                } else if matches!(key, KEYWORD::Keyword(BUILDIN::Select)) {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_select_stmt(tokens).map(|_| ()),
                    )
                } else {
                    self.reject_file_root_form(
                        tokens,
                        &token,
                        before,
                        "Control-flow statements are not allowed at file root",
                        &mut errors,
                        |parser, tokens| parser.parse_loop_stmt(tokens).map(|_| ()),
                    )
                };
                if hit_eof {
                    break;
                }
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly)
                && (key.is_literal()
                    || matches!(
                        key,
                        KEYWORD::Keyword(BUILDIN::True) | KEYWORD::Keyword(BUILDIN::False)
                    )
                    || (key.is_ident() && token.con().trim() == "nil"))
            {
                errors.push(ParseError::from_token_with_kind(
                    &token,
                    ParseErrorKind::FileRoot,
                    "Literal expressions are not allowed at file root".to_string(),
                ));
                if tokens.bump().is_none() {
                    break;
                }
                self.skip_layout(tokens).map_err(|e| vec![e])?;
                continue;
            }

            if matches!(surface, RootSurface::DeclarationOnly) {
                errors.push(ParseError::from_token_with_kind(
                    &token,
                    ParseErrorKind::FileRoot,
                    "Expected declaration or standalone comment at file root".to_string(),
                ));
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Return)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_return_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && self.lookahead_is_call(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_call_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::Dot))
                && self.lookahead_is_dot_builtin_call(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_dot_builtin_call_expr(tokens) {
                    Ok(node) => {
                        self.push_top_level_entry(&mut entries, &token, node);
                        self.consume_required_semicolon(tokens).map_err(|e| vec![e])?;
                    }
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if (matches!(
                key,
                KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::Dot)
            ) || AstParser::token_can_be_logical_name(&key)
                || key.is_textual_literal())
                && self.lookahead_is_general_invoke(
                    tokens,
                    matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)),
                )
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_invoke_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Break)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_break_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Yield)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_yield_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(
                key,
                KEYWORD::Keyword(BUILDIN::Panic)
                    | KEYWORD::Keyword(BUILDIN::Report)
                    | KEYWORD::Keyword(BUILDIN::Check)
                    | KEYWORD::Keyword(BUILDIN::Assert)
            ) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_builtin_call_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::When)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_when_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::If)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_if_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(key, KEYWORD::Keyword(BUILDIN::Select)) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_select_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if matches!(
                key,
                KEYWORD::Keyword(BUILDIN::While)
                    | KEYWORD::Keyword(BUILDIN::Loop)
                    | KEYWORD::Keyword(BUILDIN::For)
                    | KEYWORD::Keyword(BUILDIN::Each)
            ) {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_loop_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
                && self.lookahead_is_assignment(tokens)
                && self.can_start_assignment(tokens)
            {
                let before = (
                    token.loc().row(),
                    token.loc().col(),
                    token.con().to_string(),
                );
                match self.parse_assignment_stmt(tokens) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
                self.bump_if_no_progress(tokens, before);
                if tokens.curr(false).is_err() {
                    break;
                }
                continue;
            }

            if key.is_literal() {
                match self.parse_lexer_literal(&token) {
                    Ok(node) => self.push_top_level_entry(&mut entries, &token, node),
                    Err(error) => errors.push(error),
                }
            } else if matches!(key, KEYWORD::Keyword(BUILDIN::True)) {
                self.push_top_level_entry(
                    &mut entries,
                    &token,
                    AstNode::Literal(Literal::Boolean(true)),
                );
            } else if matches!(key, KEYWORD::Keyword(BUILDIN::False)) {
                self.push_top_level_entry(
                    &mut entries,
                    &token,
                    AstNode::Literal(Literal::Boolean(false)),
                );
            } else if key.is_ident() && token.con().trim() == "nil" {
                self.push_top_level_entry(&mut entries, &token, AstNode::Literal(Literal::Nil));
            } else if AstParser::token_can_be_logical_name(&key) {
                self.push_top_level_entry(
                    &mut entries,
                    &token,
                    AstNode::Identifier {
                        syntax_id: self.record_syntax_origin(&token),
                        name: token.con().trim().to_string(),
                    },
                );
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            if tokens.bump().is_none() {
                break;
            }
        }

        let syntax_index = self.finish_syntax_tracking();
        if errors.is_empty() {
            Ok((entries, syntax_index))
        } else {
            Err(errors)
        }
    }
}
