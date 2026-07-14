//! Backend-only process outcome adapter shared by every runtime tier.
//!
//! This module is not a source-language capability. Generated host binaries
//! use it to translate a recoverable FOL entry result into the process status
//! expected by the frontend. Keeping that adapter outside [`crate::std`]
//! prevents process launching from accidentally granting hosted FOL APIs.

use crate::abi::FolRecover;
use crate::aggregate::FolEchoFormat;

pub const FOL_EXIT_SUCCESS: i32 = 0;
pub const FOL_EXIT_FAILURE: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolProcessOutcome {
    exit_code: i32,
    message: Option<String>,
}

impl FolProcessOutcome {
    pub fn new(exit_code: i32, message: Option<String>) -> Self {
        Self { exit_code, message }
    }

    pub fn success() -> Self {
        Self::new(FOL_EXIT_SUCCESS, None)
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self::new(FOL_EXIT_FAILURE, Some(message.into()))
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn is_success(&self) -> bool {
        self.exit_code == FOL_EXIT_SUCCESS
    }

    pub fn is_failure(&self) -> bool {
        !self.is_success()
    }
}

pub fn failure_outcome_from_error<E: FolEchoFormat>(error: E) -> FolProcessOutcome {
    FolProcessOutcome::failure(error.fol_echo_format())
}

pub fn printable_outcome_message(outcome: &FolProcessOutcome) -> Option<&str> {
    outcome.message()
}

pub fn outcome_from_recoverable<T, E: FolEchoFormat>(value: FolRecover<T, E>) -> FolProcessOutcome {
    match value {
        FolRecover::Ok(_) => FolProcessOutcome::success(),
        FolRecover::Err(error) => failure_outcome_from_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memo::FolStr;

    #[test]
    fn recoverable_entry_results_map_to_minimal_process_outcomes() {
        let success = outcome_from_recoverable(FolRecover::<i64, FolStr>::ok(7));
        let failure =
            outcome_from_recoverable(FolRecover::<i64, FolStr>::err(FolStr::from("bad-input")));

        assert_eq!(success, FolProcessOutcome::success());
        assert!(success.is_success());
        assert_eq!(success.message(), None);

        assert_eq!(failure, FolProcessOutcome::failure("bad-input"));
        assert!(failure.is_failure());
        assert_eq!(failure.message(), Some("bad-input"));
    }

    #[test]
    fn failure_helpers_keep_printable_messages_stable() {
        let failure = failure_outcome_from_error(FolStr::from("broken"));

        assert_eq!(failure, FolProcessOutcome::failure("broken"));
        assert_eq!(printable_outcome_message(&failure), Some("broken"));
        assert_eq!(
            printable_outcome_message(&FolProcessOutcome::success()),
            None
        );
    }

    #[test]
    fn exit_code_constants_freeze_minimal_process_policy() {
        assert_eq!(FOL_EXIT_SUCCESS, 0);
        assert_eq!(FOL_EXIT_FAILURE, 1);
        assert_eq!(FolProcessOutcome::success().exit_code(), FOL_EXIT_SUCCESS);
        assert_eq!(
            FolProcessOutcome::failure("broken").exit_code(),
            FOL_EXIT_FAILURE
        );
    }

    #[test]
    fn scalar_errors_keep_core_compatible_process_messages() {
        let failure = outcome_from_recoverable(FolRecover::<i64, i64>::err(9));

        assert_eq!(failure.exit_code(), FOL_EXIT_FAILURE);
        assert_eq!(printable_outcome_message(&failure), Some("9"));
    }
}
