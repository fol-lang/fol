use crate::point::Location;
use std::fmt;

/// A lexing failure, carrying the source position it was raised at so the
/// parser can turn it into a located diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexerError {
    message: String,
    location: Option<Location>,
}

impl LexerError {
    pub fn new(message: impl Into<String>, location: Option<Location>) -> Self {
        Self {
            message: message.into(),
            location,
        }
    }

    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
    }
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LexerError {}
