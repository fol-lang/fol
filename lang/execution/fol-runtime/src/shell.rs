//! Shell value helpers for optional and error-like runtime wrappers.

use crate::error::{RuntimeError, RuntimeErrorKind};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FolOption<T> {
    Some(T),
    Nil,
}

impl<T> Default for FolOption<T> {
    fn default() -> Self {
        Self::Nil
    }
}

impl<T> FolOption<T> {
    pub fn some(value: T) -> Self {
        Self::Some(value)
    }

    pub fn nil() -> Self {
        Self::Nil
    }

    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    pub fn as_ref(&self) -> FolOption<&T> {
        match self {
            Self::Some(value) => FolOption::Some(value),
            Self::Nil => FolOption::Nil,
        }
    }

    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Some(value) => Some(value),
            Self::Nil => None,
        }
    }
}

impl<T> From<Option<T>> for FolOption<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(item) => Self::Some(item),
            None => Self::Nil,
        }
    }
}

impl<T> From<FolOption<T>> for Option<T> {
    fn from(value: FolOption<T>) -> Self {
        value.into_option()
    }
}

impl<T: fmt::Display> fmt::Display for FolOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Some(value) => write!(f, "some({value})"),
            Self::Nil => f.write_str("nil"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FolError<T>(T);

impl<T: Default> Default for FolError<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<T> FolError<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn as_ref(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for FolError<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: fmt::Display> fmt::Display for FolError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "err({})", self.0)
    }
}

pub fn unwrap_optional_shell<T>(value: FolOption<T>) -> Result<T, RuntimeError> {
    match value {
        FolOption::Some(inner) => Ok(inner),
        FolOption::Nil => Err(RuntimeError::new(
            RuntimeErrorKind::InvalidInput,
            "attempted to unwrap nil optional shell",
        )),
    }
}

pub fn unwrap_optional_shell_ref<T>(value: &FolOption<T>) -> Result<&T, RuntimeError> {
    match value {
        FolOption::Some(inner) => Ok(inner),
        FolOption::Nil => Err(RuntimeError::new(
            RuntimeErrorKind::InvalidInput,
            "attempted to unwrap nil optional shell",
        )),
    }
}

pub fn unwrap_error_shell<T>(value: FolError<T>) -> T {
    value.into_inner()
}

pub fn unwrap_error_shell_ref<T>(value: &FolError<T>) -> &T {
    value.as_ref()
}

pub fn module_name() -> &'static str {
    "shell"
}

#[cfg(test)]
mod tests {
    use super::{
        unwrap_error_shell, unwrap_error_shell_ref, unwrap_optional_shell,
        unwrap_optional_shell_ref, FolError, FolOption,
    };
    use crate::error::RuntimeErrorKind;

    #[test]
    fn fol_option_freezes_some_nil_shape_and_queries() {
        let some = FolOption::some(7);
        let nil = FolOption::<i64>::nil();

        assert!(some.is_some());
        assert!(!some.is_nil());
        assert_eq!(some.as_ref(), FolOption::Some(&7));

        assert!(nil.is_nil());
        assert!(!nil.is_some());
        assert_eq!(nil.as_ref(), FolOption::Nil);
    }

    #[test]
    fn fol_option_converts_to_and_from_rust_option() {
        let some = FolOption::from(Some("Ada"));
        let nil = FolOption::<&str>::from(None);

        assert_eq!(Option::from(some), Some("Ada"));
        assert_eq!(Option::<&str>::from(nil), None);
    }

    #[test]
    fn fol_error_freezes_bare_error_shell_representation() {
        let error = FolError::new("broken");

        assert_eq!(error.as_ref(), &"broken");
        assert_eq!(FolError::from("broken"), error);
        assert_eq!(error.into_inner(), "broken");
    }

    #[test]
    fn shell_unwrap_helpers_cover_optional_and_error_shells() {
        let some = FolOption::some(7);
        let nil = FolOption::<i64>::nil();
        let error = FolError::new("broken");

        assert_eq!(unwrap_optional_shell(some), Ok(7));
        assert_eq!(unwrap_optional_shell_ref(&FolOption::some(9)), Ok(&9));
        assert_eq!(unwrap_error_shell(error.clone()), "broken");
        assert_eq!(unwrap_error_shell_ref(&error), &"broken");

        let failure = unwrap_optional_shell(nil).expect_err("nil unwrap should fail");
        assert_eq!(failure.kind(), RuntimeErrorKind::InvalidInput);
        assert_eq!(failure.message(), "attempted to unwrap nil optional shell");
    }

    #[test]
    fn shell_display_formats_are_stable_for_echo_and_debugging() {
        let some = FolOption::some("Ada");
        let nil = FolOption::<&str>::nil();
        let error = FolError::new("broken");

        assert_eq!(format!("{some}"), "some(Ada)");
        assert_eq!(format!("{nil}"), "nil");
        assert_eq!(format!("{error}"), "err(broken)");
        assert_eq!(format!("{some:?}"), "Some(\"Ada\")");
        assert_eq!(format!("{error:?}"), "FolError(\"broken\")");
    }
}
