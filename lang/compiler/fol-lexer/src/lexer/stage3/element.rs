use crate::lexer::stage2;
use crate::point;
use crate::token::{literal::LITERAL, void::VOID};
use crate::token::{KEYWORD, KEYWORD::*};
use crate::{LexerError, Vod};
use std::fmt;

// Stage 3 owns final disambiguation only.
// It refines the folded token stream into parser-facing numeric and EOF forms
// without redoing the earlier raw scanning or multi-token folding work.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Element {
    key: KEYWORD,
    loc: point::Location,
    con: String,
}

impl std::default::Default for Element {
    fn default() -> Self {
        Self {
            key: KEYWORD::Void(VOID::Space),
            loc: point::Location::default(),
            con: String::new(),
        }
    }
}

impl From<stage2::Element> for Element {
    fn from(stg1: stage2::Element) -> Self {
        Self {
            key: stg1.key().clone(),
            loc: stg1.loc().clone(),
            con: stg1.con().clone(),
        }
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let con = if self.key().is_literal() || self.key().is_comment() || self.key().is_ident() {
            " ".to_string() + &self.con + " "
        } else {
            "".to_string()
        };
        write!(f, "{}\t{}{}", self.loc, self.key, con)
    }
}

impl Element {
    pub fn init(key: KEYWORD, loc: point::Location, con: String) -> Self {
        Self { key, loc, con }
    }
    pub fn key(&self) -> KEYWORD {
        self.key.clone()
    }
    pub fn set_key(&mut self, k: KEYWORD) {
        self.key = k;
    }
    pub fn loc(&self) -> &point::Location {
        &self.loc
    }
    pub fn set_loc(&mut self, l: point::Location) {
        self.loc = l;
    }
    pub fn con(&self) -> &String {
        &self.con
    }
    pub fn set_con(&mut self, c: String) {
        self.con = c;
    }

    pub fn append(&mut self, other: &Element) {
        self.con.push_str(&other.con);
        self.loc.longer(&other.loc.len())
    }

    pub fn analyze(&mut self, el: &mut stage2::Elements) -> Vod {
        while el.curr(false)?.loc().row() == 0 && !el.curr(false)?.key().is_eof() {
            if el.bump().is_none() {
                break;
            }
        }
        if (el.curr(false)?.key().is_decimal()
            && el.peek(0, false)?.key().is_dot()
            && (el.peek(1, false)?.key().is_decimal()
                || Self::is_trailing_float_terminator(&el.peek(1, false)?.key())))
            || (el.curr(false)?.key().is_dot()
                && el.peek(0, false)?.key().is_decimal()
                && (el.seek(0, false)?.key().is_continue()
                    || el.seek(0, false)?.key().is_operator()))
            || (el.curr(false)?.key().is_number() && el.seek(0, false)?.key().is_operator())
        {
            self.make_number(el)?;
        } else if el.curr(false)?.key().is_number()
            && !el.seek(0, false)?.key().is_void()
            && !el.seek(0, false)?.key().is_continue()
            && !el.seek(0, false)?.key().is_operator()
            && !el.seek(0, false)?.key().is_symbol()
        {
            self.set_key(Identifier);
        } else if (el.curr(false)?.key().is_numberish() && el.seek(0, false)?.key().is_continue())
            && (el.peek(0, false)?.key().is_number()
                && (el.seek(0, false)?.key().is_continue()
                    || el.seek(0, false)?.key().is_operator()
                    || el.seek(0, false)?.key().is_eol()))
        {
            self.make_number(el)?;
        }
        Ok(())
    }

    fn is_trailing_float_terminator(key: &KEYWORD) -> bool {
        key.is_void()
            || key.is_eof()
            || key.is_terminal()
            || key.is_comma()
            || key.is_close_bracket()
            || key.is_operator()
    }

    pub fn make_number(&mut self, el: &mut stage2::Elements) -> Vod {
        self.set_key(Literal(LITERAL::Decimal));

        if el.curr(false)?.key().is_decimal()
            && el.peek(0, false)?.key().is_dot()
            && el.peek(1, false)?.key().is_decimal()
        {
            self.set_key(Literal(LITERAL::Float));
            self.append(&el.peek(0, false)?.into());
            self.bump(el);
            self.append(&el.peek(0, false)?.into());
            self.bump(el);
        } else if el.curr(false)?.key().is_decimal()
            && el.peek(0, false)?.key().is_dot()
            && Self::is_trailing_float_terminator(&el.peek(1, false)?.key())
        {
            self.set_key(Literal(LITERAL::Float));
            self.append(&el.peek(0, false)?.into());
            self.bump(el);
        }

        if el.curr(false)?.key().is_dot() && el.peek(0, false)?.key().is_decimal() {
            self.set_key(Literal(LITERAL::Float));
            self.append(&el.peek(0, false)?.into());
            self.bump(el);
        }

        if el.peek(0, false)?.key().is_dot()
            && !(el.peek(1, false)?.key().is_ident() || el.peek(1, false)?.key().is_buildin())
        {
            let mut elem = el.peek(0, false)?;
            elem.append(&el.peek(1, false)?);
            self.bump(el);
            return Err(LexerError::LexerSpaceAdd(format!(
                "Expected {} but {} was given",
                KEYWORD::Void(VOID::Space),
                elem.key()
            )));
        }
        Ok(())
    }
    pub fn bump(&mut self, el: &mut stage2::Elements) {
        el.bump();
    }
}
