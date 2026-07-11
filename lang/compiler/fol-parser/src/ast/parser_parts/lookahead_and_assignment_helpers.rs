use super::*;

impl AstParser {
    pub(super) fn lookahead_is_assignment(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let mut found_compound_symbol = false;
        let mut square_depth = 0usize;
        let mut round_depth = 0usize;
        let mut expect_member_ident = false;
        let mut expect_path_ident = false;
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::SquarO)) {
                square_depth += 1;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::SquarC)) {
                if square_depth == 0 {
                    return false;
                }
                square_depth -= 1;
                continue;
            }

            if square_depth > 0 {
                if matches!(key, KEYWORD::Symbol(SYMBOL::Equal))
                    || self.compound_assignment_op(&key).is_some()
                {
                    return true;
                }
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    round_depth += 1;
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::RoundC)) && round_depth > 0 {
                    round_depth -= 1;
                }
                continue;
            }

            if round_depth > 0 {
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    round_depth += 1;
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::RoundC)) {
                    round_depth -= 1;
                }
                continue;
            }

            if found_compound_symbol {
                return matches!(key, KEYWORD::Symbol(SYMBOL::Equal));
            }

            if expect_path_ident {
                if key.is_illegal() {
                    return true;
                }
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    expect_path_ident = false;
                    continue;
                }
                return false;
            }

            if expect_member_ident {
                if key.is_illegal() {
                    return true;
                }
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    expect_member_ident = false;
                    continue;
                }
                if matches!(key, KEYWORD::Symbol(SYMBOL::Equal))
                    || self.compound_assignment_op(&key).is_some()
                {
                    return true;
                }
                return false;
            }

            if matches!(key, KEYWORD::Operator(OPERATOR::Path)) {
                expect_path_ident = true;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::Dot)) {
                expect_member_ident = true;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                round_depth = 1;
                continue;
            }

            if self.compound_assignment_symbol_op(&key).is_some() {
                found_compound_symbol = true;
                continue;
            }

            return matches!(key, KEYWORD::Symbol(SYMBOL::Equal))
                || self.compound_assignment_op(&key).is_some();
        }

        false
    }

    pub(super) fn lookahead_is_call(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        let mut allow_path_segment = false;
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if allow_path_segment {
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    allow_path_segment = false;
                    continue;
                }
                return false;
            }

            if matches!(key, KEYWORD::Operator(OPERATOR::Path)) {
                allow_path_segment = true;
                continue;
            }

            return matches!(key, KEYWORD::Symbol(SYMBOL::RoundO));
        }

        false
    }

    pub(super) fn lookahead_is_method_call(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let mut saw_dot = false;
        let mut allow_path_segment = false;
        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if allow_path_segment {
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    allow_path_segment = false;
                    continue;
                }
                return false;
            }

            if !saw_dot {
                if matches!(key, KEYWORD::Operator(OPERATOR::Path)) {
                    allow_path_segment = true;
                    continue;
                }
                if matches!(key, KEYWORD::Symbol(SYMBOL::Dot)) {
                    saw_dot = true;
                    continue;
                }
                return false;
            }

            if key.is_illegal() {
                return true;
            }

            if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                continue;
            }

            return matches!(key, KEYWORD::Symbol(SYMBOL::RoundO));
        }

        false
    }

    pub(super) fn lookahead_is_general_invoke(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
        starts_grouped: bool,
    ) -> bool {
        let mut round_depth = if starts_grouped { 1usize } else { 0usize };
        let mut square_depth = 0usize;
        let mut saw_postfix_base = starts_grouped;
        let mut expect_path_segment = false;
        let mut expect_member_name = false;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            if round_depth > 0 {
                if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    round_depth += 1;
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::RoundC)) {
                    round_depth -= 1;
                    if round_depth == 0 {
                        saw_postfix_base = true;
                    }
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::SquarO)) {
                    square_depth = 1;
                }
                continue;
            }

            if square_depth > 0 {
                if matches!(key, KEYWORD::Symbol(SYMBOL::SquarO)) {
                    square_depth += 1;
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::SquarC)) {
                    square_depth -= 1;
                    if square_depth == 0 {
                        saw_postfix_base = true;
                    }
                } else if matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                    round_depth = 1;
                }
                continue;
            }

            if expect_path_segment {
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    expect_path_segment = false;
                    continue;
                }
                return false;
            }

            if expect_member_name {
                if Self::token_can_be_logical_name(&key) || key.is_textual_literal() {
                    expect_member_name = false;
                    continue;
                }
                return false;
            }

            if matches!(key, KEYWORD::Operator(OPERATOR::Path)) {
                expect_path_segment = true;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::Dot)) {
                expect_member_name = true;
                saw_postfix_base = true;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::Colon)) {
                saw_postfix_base = true;
                continue;
            }

            if matches!(key, KEYWORD::Symbol(SYMBOL::SquarO)) {
                square_depth = 1;
                saw_postfix_base = true;
                continue;
            }

            if saw_postfix_base && matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)) {
                return true;
            }

            return false;
        }

        false
    }

    pub(super) fn can_start_assignment(&self, tokens: &fol_lexer::lexer::stage3::Elements) -> bool {
        match self.previous_significant_key(tokens) {
            None => true,
            Some(KEYWORD::Symbol(SYMBOL::CurlyO)) => true,
            // Block-terminated statements (when/loop/for bodies) end at `}`
            // with no `;`, so a following identifier starts a new statement.
            Some(KEYWORD::Symbol(SYMBOL::CurlyC)) => true,
            Some(KEYWORD::Symbol(SYMBOL::Semi)) => true,
            Some(key) if key.is_terminal() => true,
            _ => false,
        }
    }

    pub(super) fn previous_significant_key(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Option<KEYWORD> {
        for candidate in tokens.prev_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            return Some(key);
        }

        None
    }

    pub(super) fn lookahead_has_top_level_pipe(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> bool {
        let mut round_depth = 0usize;
        let mut square_depth = 0usize;
        let mut curly_depth = 0usize;

        for candidate in tokens.next_vec() {
            let token = match candidate {
                Ok(token) => token,
                Err(_) => continue,
            };

            let key = token.key();
            if Self::key_is_soft_ignorable(&key) {
                continue;
            }

            match key {
                KEYWORD::Symbol(SYMBOL::RoundO) => round_depth += 1,
                KEYWORD::Symbol(SYMBOL::RoundC) => {
                    if round_depth > 0 {
                        round_depth -= 1;
                    } else {
                        break;
                    }
                }
                KEYWORD::Symbol(SYMBOL::SquarO) => square_depth += 1,
                KEYWORD::Symbol(SYMBOL::SquarC) => {
                    square_depth = square_depth.saturating_sub(1);
                }
                KEYWORD::Symbol(SYMBOL::CurlyO) => curly_depth += 1,
                KEYWORD::Symbol(SYMBOL::CurlyC) => {
                    if curly_depth == 0 {
                        break;
                    }
                    curly_depth -= 1;
                }
                KEYWORD::Symbol(SYMBOL::Pipe)
                    if round_depth == 0 && square_depth == 0 && curly_depth == 0 =>
                {
                    return true;
                }
                _ if round_depth == 0
                    && square_depth == 0
                    && curly_depth == 0
                    && key.is_terminal() =>
                {
                    break;
                }
                _ => {}
            }
        }

        false
    }

    /// Skip tokens until we find a declaration-start keyword or EOF.
    /// Guard against skip_ignorable jumping into a different declaration.
    /// If the current token is on a later line than the declaration start
    /// and is a declaration keyword, bail with an error pointing at the
    /// original declaration keyword.
    pub(super) fn guard_decl_boundary(
        &self,
        decl_token: &fol_lexer::lexer::stage3::element::Element,
        tokens: &fol_lexer::lexer::stage3::Elements,
        decl_line: usize,
        message: &str,
    ) -> Result<(), ParseError> {
        if let Ok(next) = tokens.curr(false) {
            if next.loc().row() > decl_line && next.key().is_assign() {
                return Err(ParseError::from_token(
                    decl_token,
                    message.to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Used for error recovery: after a failed declaration parse, advance
    /// past the junk so the main loop can re-enter on the next declaration.
    pub(super) fn sync_to_next_declaration(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) {
        for _ in 0..8_192 {
            match tokens.curr(false) {
                Ok(token) => {
                    let key = token.key();
                    if key.is_eof() {
                        break;
                    }
                    // is_assign() covers Use, Def, Seg, Var, Fun, Pro, Typ, Ali, Lab, Con.
                    // Also stop on Std, Log, and Let which are declaration starters
                    // not covered by is_assign().
                    if key.is_assign()
                        || matches!(
                            key,
                            KEYWORD::Keyword(BUILDIN::Std)
                                | KEYWORD::Keyword(BUILDIN::Log)
                                | KEYWORD::Keyword(BUILDIN::Let)
                        )
                    {
                        break;
                    }
                    tokens.bump();
                }
                Err(_) => break,
            }
        }
    }

    pub(super) fn bump_if_no_progress(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        before: (usize, usize, String),
    ) {
        if let Ok(current) = tokens.curr(false) {
            let after = (
                current.loc().row(),
                current.loc().col(),
                current.con().to_string(),
            );
            if before == after {
                let _ = tokens.bump();
            }
        }
    }

    pub(super) fn token_is_word(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
        word: &str,
    ) -> bool {
        token.con().trim() == word
            || matches!(
                (token.key(), word),
                (KEYWORD::Keyword(BUILDIN::And), "and")
                    | (KEYWORD::Keyword(BUILDIN::Or), "or")
                    | (KEYWORD::Keyword(BUILDIN::Xor), "xor")
                    | (KEYWORD::Keyword(BUILDIN::Nand), "nand")
                    | (KEYWORD::Keyword(BUILDIN::Nor), "nor")
                    | (KEYWORD::Keyword(BUILDIN::Not), "not")
            )
    }

    pub(super) fn compound_assignment_op(&self, key: &KEYWORD) -> Option<BinaryOperator> {
        match key {
            KEYWORD::Operator(OPERATOR::Addeq) => Some(BinaryOperator::Add),
            KEYWORD::Operator(OPERATOR::Subeq) => Some(BinaryOperator::Sub),
            KEYWORD::Operator(OPERATOR::Multeq) => Some(BinaryOperator::Mul),
            KEYWORD::Operator(OPERATOR::Diveq) => Some(BinaryOperator::Div),
            _ => None,
        }
    }

    pub(super) fn compound_assignment_symbol_op(&self, key: &KEYWORD) -> Option<BinaryOperator> {
        match key {
            KEYWORD::Symbol(SYMBOL::Percent) => Some(BinaryOperator::Mod),
            KEYWORD::Symbol(SYMBOL::Carret) => Some(BinaryOperator::Pow),
            _ => None,
        }
    }
}
