//! Common runtime error types used by runtime helpers and future backend glue.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeErrorKind {
    InvariantViolation,
    InvalidInput,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    kind: RuntimeErrorKind,
    message: String,
}

impl RuntimeError {
    pub fn new(kind: RuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> RuntimeErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for RuntimeError {}

pub fn module_name() -> &'static str {
    "error"
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::{RuntimeError, RuntimeErrorKind};

    pub(crate) fn assert_error_kind(error: &RuntimeError, expected: RuntimeErrorKind) {
        assert_eq!(
            error.kind(),
            expected,
            "Expected runtime error kind {expected:?}, got {error:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{test_helpers::assert_error_kind, RuntimeError, RuntimeErrorKind};

    #[test]
    fn runtime_error_display_includes_kind_and_message() {
        let error = RuntimeError::new(RuntimeErrorKind::InvalidInput, "bad argument");

        assert_eq!(format!("{error}"), "InvalidInput: bad argument");
        assert_error_kind(&error, RuntimeErrorKind::InvalidInput);
    }
}

/// Unwrap a runtime result or abort with the fault message alone. Generated
/// code routes every fallible runtime access through this so an ordinary
/// runtime fault (out-of-bounds index, nil unwrap, invalid slice) surfaces as
/// a single readable line instead of Rust `Result` debug noise pointing into
/// the generated build directory.
pub fn require<T>(result: Result<T, RuntimeError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("fol runtime fault: {error}"),
    }
}
