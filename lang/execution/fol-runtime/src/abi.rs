//! Recoverable ABI and entrypoint-facing runtime contracts.

use crate::value::FolBool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolRecover<T, E> {
    Ok(T),
    Err(E),
}

impl<T: Default, E> Default for FolRecover<T, E> {
    fn default() -> Self {
        Self::Ok(T::default())
    }
}

impl<T, E> FolRecover<T, E> {
    pub fn ok(value: T) -> Self {
        Self::Ok(value)
    }

    pub fn err(error: E) -> Self {
        Self::Err(error)
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Self::Err(_))
    }

    pub fn value_ref(&self) -> Option<&T> {
        match self {
            Self::Ok(value) => Some(value),
            Self::Err(_) => None,
        }
    }

    pub fn error_ref(&self) -> Option<&E> {
        match self {
            Self::Ok(_) => None,
            Self::Err(error) => Some(error),
        }
    }

    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Ok(value) => Some(value),
            Self::Err(_) => None,
        }
    }

    pub fn into_error(self) -> Option<E> {
        match self {
            Self::Ok(_) => None,
            Self::Err(error) => Some(error),
        }
    }

    pub fn into_result(self) -> Result<T, E> {
        self.into()
    }

    pub fn as_ref(&self) -> FolRecover<&T, &E> {
        match self {
            Self::Ok(value) => FolRecover::Ok(value),
            Self::Err(error) => FolRecover::Err(error),
        }
    }
}

impl<T, E> From<Result<T, E>> for FolRecover<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(success) => Self::Ok(success),
            Err(error) => Self::Err(error),
        }
    }
}

impl<T, E> From<FolRecover<T, E>> for Result<T, E> {
    fn from(value: FolRecover<T, E>) -> Self {
        match value {
            FolRecover::Ok(success) => Ok(success),
            FolRecover::Err(error) => Err(error),
        }
    }
}

/// Runtime helper for the `check(...)` intrinsic.
///
/// Returns `true` when the recoverable value represents a failure path.
pub fn check_recoverable<T, E>(value: &FolRecover<T, E>) -> FolBool {
    value.is_err()
}

/// Explicit success-side mirror of [`check_recoverable`].
pub fn recoverable_succeeded<T, E>(value: &FolRecover<T, E>) -> FolBool {
    value.is_ok()
}

pub fn module_name() -> &'static str {
    "abi"
}

#[cfg(test)]
mod tests {
    use super::FolRecover;
    use crate::{
        memo::FolStr,
        shell::{FolError, FolOption},
    };

    #[test]
    fn fol_recover_freezes_ok_err_mapping_and_helpers() {
        let success = FolRecover::<i64, &str>::ok(7);
        let failure = FolRecover::<i64, &str>::err("bad-input");

        assert!(success.is_ok());
        assert!(!success.is_err());
        assert_eq!(success.value_ref(), Some(&7));
        assert_eq!(success.error_ref(), None);

        assert!(failure.is_err());
        assert!(!failure.is_ok());
        assert_eq!(failure.value_ref(), None);
        assert_eq!(failure.error_ref(), Some(&"bad-input"));
    }

    #[test]
    fn fol_recover_converts_to_and_from_rust_result() {
        let success = FolRecover::<i64, &str>::from(Ok(7));
        let failure = FolRecover::<i64, &str>::from(Err("bad-input"));

        assert_eq!(success.as_ref(), FolRecover::Ok(&7));
        assert_eq!(failure.as_ref(), FolRecover::Err(&"bad-input"));
        assert_eq!(Result::<i64, &str>::from(success), Ok(7));
        assert_eq!(Result::<i64, &str>::from(failure), Err("bad-input"));
    }

    #[test]
    fn recoverable_inspection_helpers_freeze_check_polarity() {
        let success = FolRecover::<i64, &str>::ok(7);
        let failure = FolRecover::<i64, &str>::err("bad-input");

        assert!(!super::check_recoverable(&success));
        assert!(super::recoverable_succeeded(&success));
        assert!(super::check_recoverable(&failure));
        assert!(!super::recoverable_succeeded(&failure));
    }

    #[test]
    fn recoverable_shell_interactions_keep_boundaries_explicit() {
        let success_nil = FolRecover::<FolOption<i64>, FolError<FolStr>>::ok(FolOption::nil());
        let success_some = FolRecover::<FolOption<i64>, FolError<FolStr>>::ok(FolOption::some(7));
        let failure =
            FolRecover::<FolOption<i64>, FolError<FolStr>>::err(FolError::new(FolStr::from("bad")));

        assert!(!super::check_recoverable(&success_nil));
        assert!(!super::check_recoverable(&success_some));
        assert!(super::check_recoverable(&failure));

        assert_eq!(success_nil.value_ref(), Some(&FolOption::nil()));
        assert_eq!(success_some.value_ref(), Some(&FolOption::some(7)));
        assert_eq!(
            failure.error_ref().map(|error| error.as_ref().as_str()),
            Some("bad")
        );
    }
}
