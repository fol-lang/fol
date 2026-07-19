use super::*;

impl AstParser {
    pub(super) fn flow_nodes_to_expr(&self, nodes: Vec<AstNode>) -> AstNode {
        if nodes.len() == 1 {
            nodes.into_iter().next().expect("one node")
        } else {
            AstNode::Block {
                syntax_id: None,
                statements: nodes,
            }
        }
    }

    pub(super) fn parse_flow_body_nodes(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let flow = tokens.curr(false)?;
        if !matches!(flow.key(), KEYWORD::Operator(OPERATOR::Flow)) {
            return Err(ParseError::from_token(
                &flow,
                "Expected '=>' to start flow body".to_string(),
            ));
        }
        let _ = tokens.bump();
        self.skip_ignorable(tokens)?;

        let key = tokens.curr(false)?.key();
        if self.lookahead_binding_alternative(tokens).is_some() {
            let nodes = self.parse_binding_alternative_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Var)) {
            let nodes = self.parse_var_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Con)) {
            let nodes = self.parse_con_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Lab)) {
            let nodes = self.parse_lab_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Use)) {
            let nodes = self.parse_use_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Ali)) {
            let node = self.parse_alias_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Typ)) {
            let nodes = self.parse_type_decl(tokens)?;
            return Ok(nodes);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Def)) {
            let node = self.parse_def_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Seg)) {
            let node = self.parse_seg_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Std)) && self.lookahead_is_std_decl(tokens) {
            let node = self.parse_std_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Fun)) {
            let node = self.parse_fun_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Log)) {
            let node = self.parse_log_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Pro)) {
            let node = self.parse_pro_decl(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::If)) {
            let node = self.parse_if_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::When)) {
            let node = self.parse_when_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Select)) {
            let node = self.parse_select_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(
            key,
            KEYWORD::Keyword(BUILDIN::While)
                | KEYWORD::Keyword(BUILDIN::Loop)
                | KEYWORD::Keyword(BUILDIN::For)
                | KEYWORD::Keyword(BUILDIN::Each)
        ) {
            let node = self.parse_loop_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Symbol(SYMBOL::CurlyO)) {
            let node = self.parse_block_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(
            key,
            KEYWORD::Keyword(BUILDIN::Panic)
                | KEYWORD::Keyword(BUILDIN::Report)
                | KEYWORD::Keyword(BUILDIN::Check)
                | KEYWORD::Keyword(BUILDIN::Assert)
        ) {
            let node = self.parse_builtin_call_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Symbol(SYMBOL::Dot)) && self.lookahead_is_dot_builtin_call(tokens)
        {
            let node = self.parse_dot_builtin_call_expr(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Return)) {
            let node = self.parse_return_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Break)) {
            let node = self.parse_break_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Yield)) {
            let node = self.parse_yield_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Dfr)) {
            let node = self.parse_dfr_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if matches!(key, KEYWORD::Keyword(BUILDIN::Edf)) {
            let node = self.parse_edf_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if (AstParser::token_can_be_logical_name(&key)
            || key.is_textual_literal()
            || matches!(key, KEYWORD::Symbol(SYMBOL::Star))
            || self.peek_is_deref_bracket(tokens))
            && self.lookahead_is_assignment(tokens)
        {
            let node = self.parse_assignment_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if (AstParser::token_can_be_logical_name(&key) || key.is_textual_literal())
            && (self.lookahead_is_call(tokens) || self.lookahead_is_method_call(tokens))
            && self.can_start_assignment(tokens)
        {
            let node = self.parse_call_stmt(tokens)?;
            return Ok(vec![node]);
        }

        if (matches!(
            key,
            KEYWORD::Symbol(SYMBOL::RoundO) | KEYWORD::Symbol(SYMBOL::Dot)
        ) || AstParser::token_can_be_logical_name(&key)
            || key.is_textual_literal())
            && self
                .lookahead_is_general_invoke(tokens, matches!(key, KEYWORD::Symbol(SYMBOL::RoundO)))
            && self.can_start_assignment(tokens)
        {
            let node = self.parse_invoke_stmt(tokens)?;
            return Ok(vec![node]);
        }

        let node = self.parse_logical_expression(tokens)?;
        Ok(vec![node])
    }
}
