use super::*;
use crate::ast::CommentKind;

impl AstParser {
    pub(super) fn key_is_layout_ignorable(key: &KEYWORD) -> bool {
        matches!(
            key,
            KEYWORD::Void(VOID::Space) | KEYWORD::Void(VOID::EndLine)
        )
    }

    pub(super) fn parse_comment_token(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<AstNode, ParseError> {
        let kind = match token.key() {
            KEYWORD::Comment(fol_lexer::token::COMMENT::Backtick) => CommentKind::Backtick,
            KEYWORD::Comment(fol_lexer::token::COMMENT::Doc) => CommentKind::Doc,
            KEYWORD::Comment(fol_lexer::token::COMMENT::SlashLine) => CommentKind::SlashLine,
            KEYWORD::Comment(fol_lexer::token::COMMENT::SlashBlock) => CommentKind::SlashBlock,
            _ => {
                return Err(ParseError::from_token(
                    token,
                    "Expected comment token".to_string(),
                ));
            }
        };

        Ok(AstNode::Comment {
            kind,
            raw: token.con().to_string(),
        })
    }

    pub(super) fn key_is_soft_ignorable(key: &KEYWORD) -> bool {
        Self::key_is_layout_ignorable(key) || key.is_comment()
    }

    pub(super) fn skip_layout(&self, tokens: &mut fol_lexer::lexer::stage3::Elements) -> Result<(), ParseError> {
        let mut count = 0u32;
        loop {
            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if Self::key_is_layout_ignorable(&token.key()) {
                count += 1;
                if count > 128 {
                    return Err(ParseError::from_token(
                        &token,
                        "skip_layout exceeded 128-token limit; possible infinite layout loop".to_string(),
                    ));
                }
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            break;
        }
        Ok(())
    }

    pub(super) fn parse_primary(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<AstNode, ParseError> {
        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::True)) {
            return Ok(AstNode::Literal(Literal::Boolean(true)));
        }

        if matches!(token.key(), KEYWORD::Keyword(BUILDIN::False)) {
            return Ok(AstNode::Literal(Literal::Boolean(false)));
        }

        if token.key().is_literal() {
            return self.parse_lexer_literal(token);
        }

        if token.key().is_ident() && token.con().trim() == "nil" {
            return Ok(AstNode::Literal(Literal::Nil));
        }

        if Self::token_can_be_logical_name(&token.key()) {
            return Ok(AstNode::Identifier {
                syntax_id: self.record_syntax_origin(token),
                name: token.con().trim().to_string(),
            });
        }

        Err(ParseError::from_token_with_kind(
            token,
            ParseErrorKind::Unsupported,
            format!("Unsupported expression token '{}'", token.con()),
        ))
    }

    pub(super) fn skip_ignorable(&self, tokens: &mut fol_lexer::lexer::stage3::Elements) -> Result<(), ParseError> {
        let mut count = 0u32;
        loop {
            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if Self::key_is_soft_ignorable(&token.key()) {
                count += 1;
                if count > 128 {
                    return Err(ParseError::from_token(
                        &token,
                        "skip_ignorable exceeded 128-token limit; possible infinite ignorable loop".to_string(),
                    ));
                }
                if tokens.bump().is_none() {
                    break;
                }
                continue;
            }

            break;
        }
        Ok(())
    }

    pub(super) fn token_can_be_logical_name(key: &KEYWORD) -> bool {
        key.is_ident()
            || matches!(
                key,
                KEYWORD::Keyword(BUILDIN::Use)
                    | KEYWORD::Keyword(BUILDIN::Def)
                    | KEYWORD::Keyword(BUILDIN::Seg)
                    | KEYWORD::Keyword(BUILDIN::Var)
                    | KEYWORD::Keyword(BUILDIN::Log)
                    | KEYWORD::Keyword(BUILDIN::Con)
                    | KEYWORD::Keyword(BUILDIN::Fun)
                    | KEYWORD::Keyword(BUILDIN::Pro)
                    | KEYWORD::Keyword(BUILDIN::Typ)
                    | KEYWORD::Keyword(BUILDIN::Std)
                    | KEYWORD::Keyword(BUILDIN::Ali)
                    | KEYWORD::Keyword(BUILDIN::Lab)
                    | KEYWORD::Keyword(BUILDIN::This)
                    | KEYWORD::Keyword(BUILDIN::Selfi)
                    | KEYWORD::Keyword(BUILDIN::If)
                    | KEYWORD::Keyword(BUILDIN::Go)
                    | KEYWORD::Keyword(BUILDIN::Get)
                    | KEYWORD::Keyword(BUILDIN::Let)
                    | KEYWORD::Keyword(BUILDIN::Not)
                    | KEYWORD::Keyword(BUILDIN::Check)
                    | KEYWORD::Keyword(BUILDIN::Panic)
                    | KEYWORD::Keyword(BUILDIN::Report)
                    | KEYWORD::Keyword(BUILDIN::Assert)
            )
    }

    pub(super) fn token_to_named_label(
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Option<String> {
        if Self::token_can_be_logical_name(&token.key()) {
            return Some(token.con().trim().to_string());
        }

        match token.key() {
            KEYWORD::Literal(LITERAL::CookedQuoted) | KEYWORD::Literal(LITERAL::RawQuoted) => {
                Some(Self::exact_unquote_text(token.con()))
            }
            _ => None,
        }
    }

    pub(super) fn expect_named_label(
        token: &fol_lexer::lexer::stage3::element::Element,
        message: &str,
    ) -> Result<String, ParseError> {
        Self::reject_illegal_token(token)?;

        Self::token_to_named_label(token).ok_or_else(|| {
            ParseError::from_token(token, message.to_string())
        })
    }

    pub(super) fn reject_illegal_token(
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Result<(), ParseError> {
        if token.key().is_illegal() {
            return Err(ParseError::from_token(
                token,
                format!("Parser encountered illegal token '{}'", token.con()),
            ));
        }

        Ok(())
    }

    pub(super) fn collect_comment_nodes(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<Vec<AstNode>, ParseError> {
        let mut comments = Vec::new();

        loop {
            self.skip_layout(tokens)?;

            let token = match tokens.curr(false) {
                Ok(token) => token,
                Err(_) => break,
            };

            if !token.key().is_comment() {
                break;
            }

            comments.push(self.parse_comment_token(&token)?);
            let _ = tokens.bump();
        }

        self.skip_layout(tokens)?;
        Ok(comments)
    }

    pub(super) fn collect_comments_before<F>(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        continues_with: F,
    ) -> Result<Vec<AstNode>, ParseError>
    where
        F: Fn(&KEYWORD) -> bool,
    {
        self.skip_layout(tokens)?;

        let Ok(token) = tokens.curr(false) else {
            return Ok(Vec::new());
        };
        if !token.key().is_comment() {
            return Ok(Vec::new());
        }

        if self
            .next_significant_key_from_window(tokens)
            .is_some_and(|key| continues_with(&key))
        {
            return self.collect_comment_nodes(tokens);
        }

        Ok(Vec::new())
    }

    fn attach_comments(
        node: AstNode,
        mut leading_comments: Vec<AstNode>,
        trailing_comments: Vec<AstNode>,
    ) -> AstNode {
        if leading_comments.is_empty() && trailing_comments.is_empty() {
            return node;
        }

        match node {
            AstNode::Commented {
                leading_comments: mut existing_leading,
                node,
                trailing_comments: mut existing_trailing,
            } => {
                leading_comments.append(&mut existing_leading);
                existing_trailing.extend(trailing_comments);
                AstNode::Commented {
                    leading_comments,
                    node,
                    trailing_comments: existing_trailing,
                }
            }
            node => AstNode::Commented {
                leading_comments,
                node: Box::new(node),
                trailing_comments,
            },
        }
    }

    pub(super) fn attach_leading_comments(
        &self,
        node: AstNode,
        leading_comments: Vec<AstNode>,
    ) -> AstNode {
        Self::attach_comments(node, leading_comments, Vec::new())
    }

    pub(super) fn attach_trailing_comments(
        &self,
        node: AstNode,
        trailing_comments: Vec<AstNode>,
    ) -> AstNode {
        Self::attach_comments(node, Vec::new(), trailing_comments)
    }

    pub(super) fn exact_unquote_text(raw: &str) -> String {
        let trimmed = raw.trim();

        if let Some(inner) = trimmed
            .strip_prefix('"')
            .and_then(|text| text.strip_suffix('"'))
        {
            return inner.to_string();
        }

        if let Some(inner) = trimmed
            .strip_prefix('\'')
            .and_then(|text| text.strip_suffix('\''))
        {
            return inner.to_string();
        }

        trimmed.to_string()
    }

    pub(super) fn unary_prefix_info(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Option<(&'static str, Option<UnaryOperator>)> {
        if matches!(
            token.key(),
            KEYWORD::Operator(OPERATOR::Abstract) | KEYWORD::Symbol(SYMBOL::Minus)
        ) {
            return Some((
                "Expected expression after unary '-'",
                Some(UnaryOperator::Neg),
            ));
        }

        if matches!(
            token.key(),
            KEYWORD::Operator(OPERATOR::Add) | KEYWORD::Symbol(SYMBOL::Plus)
        ) {
            return Some(("Expected expression after unary '+'", None));
        }

        if self.token_is_word(token, "not") {
            return Some((
                "Expected expression after unary 'not'",
                Some(UnaryOperator::Not),
            ));
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::And)) {
            return Some((
                "Expected expression after unary '&'",
                Some(UnaryOperator::Ref),
            ));
        }

        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Star)) {
            return Some((
                "Expected expression after unary '*'",
                Some(UnaryOperator::Deref),
            ));
        }

        None
    }

    pub(super) fn ensure_unary_operand(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
        operator_token: &fol_lexer::lexer::stage3::element::Element,
        message: &str,
    ) -> Result<(), ParseError> {
        self.skip_ignorable(tokens)?;

        match tokens.curr(false) {
            Ok(next) => {
                if next.key().is_void()
                    || matches!(
                        next.key(),
                        KEYWORD::Symbol(SYMBOL::Semi)
                            | KEYWORD::Symbol(SYMBOL::Comma)
                            | KEYWORD::Symbol(SYMBOL::RoundC)
                            | KEYWORD::Symbol(SYMBOL::CurlyC)
                    )
                {
                    return Err(ParseError::from_token(&next, message.to_string()));
                }

                Ok(())
            }
            Err(_) => Err(ParseError::from_token(
                operator_token,
                message.to_string(),
            )),
        }
    }

    pub(super) fn consume_optional_semicolon(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(), ParseError> {
        self.skip_layout(tokens)?;
        if let Ok(token) = tokens.curr(false) {
            if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Semi)) {
                let _ = tokens.bump();
            }
        }
        Ok(())
    }

    pub(super) fn consume_required_semicolon(
        &self,
        tokens: &mut fol_lexer::lexer::stage3::Elements,
    ) -> Result<(), ParseError> {
        self.skip_layout(tokens)?;
        let token = tokens.curr(false).map_err(|_| ParseError {
            kind: ParseErrorKind::Syntax,
            message: "Expected ';' to terminate statement but reached end of input".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        })?;
        if matches!(token.key(), KEYWORD::Symbol(SYMBOL::Semi)) {
            let _ = tokens.bump();
            Ok(())
        } else if matches!(token.key(), KEYWORD::Symbol(SYMBOL::CurlyC)) {
            // Allow omitting semicolon before closing brace (last statement in block)
            Ok(())
        } else {
            Err(ParseError::from_token(
                &token,
                "Expected ';' to terminate statement".to_string(),
            ))
        }
    }

    /// Parse a simple literal for testing
    pub fn parse_literal(&self, value: &str) -> Result<AstNode, ParseError> {
        if value.starts_with('"') && value.ends_with('"') {
            return Ok(Self::lower_width_based_text_literal(
                Self::decode_cooked_literal(&value[1..value.len() - 1]),
            ));
        }

        if value.starts_with('\'') && value.ends_with('\'') {
            return Ok(Self::lower_width_based_text_literal(
                value[1..value.len() - 1].to_string(),
            ));
        }

        let normalized = value.replace('_', "");
        let numeric_error = |message: String| {
            ParseError {
                kind: ParseErrorKind::Literal,
                message,
                file: None,
                line: 0,
                column: 0,
                length: value.trim().len().max(1),
            }
        };

        if let Some(hex) = normalized
            .strip_prefix("0x")
            .or_else(|| normalized.strip_prefix("0X"))
        {
            if let Ok(int_val) = i64::from_str_radix(hex, 16) {
                return Ok(AstNode::Literal(Literal::Integer(int_val)));
            }
            return Err(numeric_error(format!(
                "Hexadecimal literal '{}' is out of range for current parser literal lowering",
                value
            )));
        }

        if let Some(octal) = normalized
            .strip_prefix("0o")
            .or_else(|| normalized.strip_prefix("0O"))
        {
            if let Ok(int_val) = i64::from_str_radix(octal, 8) {
                return Ok(AstNode::Literal(Literal::Integer(int_val)));
            }
            return Err(numeric_error(format!(
                "Octal literal '{}' is out of range for current parser literal lowering",
                value
            )));
        }

        if let Some(binary) = normalized
            .strip_prefix("0b")
            .or_else(|| normalized.strip_prefix("0B"))
        {
            if let Ok(int_val) = i64::from_str_radix(binary, 2) {
                return Ok(AstNode::Literal(Literal::Integer(int_val)));
            }
            return Err(numeric_error(format!(
                "Binary literal '{}' is out of range for current parser literal lowering",
                value
            )));
        }

        if let Ok(int_val) = normalized.parse::<i64>() {
            return Ok(AstNode::Literal(Literal::Integer(int_val)));
        }

        let decimal_looks_integral = normalized
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
            && !normalized.contains('.')
            && !normalized.contains('e')
            && !normalized.contains('E');
        if decimal_looks_integral {
            return Err(numeric_error(format!(
                "Decimal literal '{}' is out of range for current parser literal lowering",
                value
            )));
        }

        if let Ok(float_val) = normalized.parse::<f64>() {
            return Ok(AstNode::Literal(Literal::Float(float_val)));
        }

        if normalized
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
        {
            return Err(numeric_error(format!(
                "Decimal literal '{}' is out of range for current parser literal lowering",
                value
            )));
        }

        // Default to identifier
        Ok(AstNode::Identifier {
            syntax_id: None,
            name: value.to_string(),
        })
    }

    fn lower_width_based_text_literal(content: String) -> AstNode {
        let mut chars = content.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => AstNode::Literal(Literal::Character(ch)),
            _ => AstNode::Literal(Literal::String(content)),
        }
    }

    fn decode_cooked_literal(content: &str) -> String {
        let chars: Vec<char> = content.chars().collect();
        let mut decoded = String::new();
        let mut index = 0;

        while index < chars.len() {
            let ch = chars[index];
            index += 1;
            if ch != '\\' {
                decoded.push(ch);
                continue;
            }

            if index >= chars.len() {
                decoded.push('\\');
                break;
            }

            let next = chars[index];
            index += 1;

            match next {
                '\n' => {
                    while index < chars.len() && Self::is_cooked_continuation_indent(chars[index]) {
                        index += 1;
                    }
                }
                '\r' => {
                    if index < chars.len() && chars[index] == '\n' {
                        index += 1;
                    }
                    while index < chars.len() && Self::is_cooked_continuation_indent(chars[index]) {
                        index += 1;
                    }
                }
                'p' => {
                    if cfg!(windows) {
                        decoded.push('\r');
                    }
                    decoded.push('\n');
                }
                'r' | 'c' => decoded.push('\r'),
                'n' | 'l' => decoded.push('\n'),
                'f' => decoded.push('\u{000C}'),
                't' => decoded.push('\t'),
                'v' => decoded.push('\u{000B}'),
                '\\' => decoded.push('\\'),
                '"' => decoded.push('"'),
                '\'' => decoded.push('\''),
                'a' => decoded.push('\u{0007}'),
                'b' => decoded.push('\u{0008}'),
                'e' => decoded.push('\u{001B}'),
                '0'..='9' => {
                    let start = index - 1;
                    while index < chars.len() && chars[index].is_ascii_digit() {
                        index += 1;
                    }
                    if let Some(escaped) = Self::decode_u32_escape(&chars[start..index], 10) {
                        decoded.push(escaped);
                    } else {
                        decoded.push('\\');
                        decoded.extend(chars[start..index].iter());
                    }
                }
                'x' => {
                    let start = index;
                    if let Some(end) = start.checked_add(2).filter(|end| *end <= chars.len()) {
                        if let Some(escaped) = Self::decode_u32_escape(&chars[start..end], 16) {
                            decoded.push(escaped);
                            index = end;
                        } else {
                            decoded.push('\\');
                            decoded.push('x');
                        }
                    } else {
                        decoded.push('\\');
                        decoded.push('x');
                    }
                }
                'u' => {
                    if index < chars.len() && chars[index] == '{' {
                        let hex_start = index + 1;
                        let mut end = hex_start;
                        while end < chars.len() && chars[end] != '}' {
                            end += 1;
                        }
                        if end < chars.len() && end > hex_start {
                            if let Some(escaped) =
                                Self::decode_u32_escape(&chars[hex_start..end], 16)
                            {
                                decoded.push(escaped);
                                index = end + 1;
                            } else {
                                decoded.push('\\');
                                decoded.push('u');
                            }
                        } else {
                            decoded.push('\\');
                            decoded.push('u');
                        }
                    } else {
                        let start = index;
                        if let Some(end) = start.checked_add(4).filter(|end| *end <= chars.len()) {
                            if let Some(escaped) = Self::decode_u32_escape(&chars[start..end], 16) {
                                decoded.push(escaped);
                                index = end;
                            } else {
                                decoded.push('\\');
                                decoded.push('u');
                            }
                        } else {
                            decoded.push('\\');
                            decoded.push('u');
                        }
                    }
                }
                other => {
                    decoded.push('\\');
                    decoded.push(other);
                }
            }
        }

        decoded
    }

    fn decode_u32_escape(digits: &[char], radix: u32) -> Option<char> {
        let text: String = digits.iter().collect();
        let value = u32::from_str_radix(&text, radix).ok()?;
        char::from_u32(value)
    }

    fn is_cooked_continuation_indent(ch: char) -> bool {
        ch.is_whitespace() && !matches!(ch, '\n' | '\r')
    }
}
