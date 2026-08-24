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

/// A foreign resource FOL owns but cannot look inside.
///
/// The address comes from a provider and goes back to that provider's destroy;
/// nothing between those two points may read it. The type carries no `Clone`,
/// no `Copy`, and no `Drop`:
///
/// - duplicating it would create a second release obligation nothing tracks,
///   which is why the FOL type that wraps it claims `lin`;
/// - dropping it silently is exactly what the linear capability exists to
///   prevent, so there is no destructor to run at the wrong moment. A handle
///   that is never consumed is a compile error, not a leak at run time.
///
/// It is deliberately neither `Send` nor `Sync`. Whether a particular C
/// resource may cross a thread is a fact about that provider, and the raw
/// pointer inside makes the auto-traits opt out for us.
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct FolHandle(*mut core::ffi::c_void);

/// A null handle, and only as the residue a move leaves behind.
///
/// Moving a value out of a place is emitted as `std::mem::take`, which needs
/// something to put back. Null is the one value that cannot be mistaken for a
/// live resource. Nothing in FOL can construct a handle this way: there is no
/// syntax for one, and the linear analysis has already proven the moved-from
/// binding is never read again -- so if this value is ever observed, the bug is
/// upstream and null is what makes it visible rather than silent.
impl Default for FolHandle {
    fn default() -> Self {
        Self(core::ptr::null_mut())
    }
}

impl FolHandle {
    /// Adopt an address a provider just returned.
    pub fn from_raw(address: *mut core::ffi::c_void) -> Self {
        Self(address)
    }

    /// Hand the address back, consuming the handle.
    ///
    /// By value on purpose: the caller must give up the handle to get the
    /// address, so there is no way to keep one and release the other.
    pub fn into_raw(self) -> *mut core::ffi::c_void {
        self.0
    }

    /// Lend the address for the duration of a call.
    pub fn as_raw(&self) -> *mut core::ffi::c_void {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

impl crate::aggregate::FolEchoFormat for FolHandle {
    /// Prints the domain, never the address.
    ///
    /// An address is not a program's output: it varies between runs, so
    /// echoing one would make any test that touched a handle unstable.
    fn fol_echo_format(&self) -> String {
        "<foreign handle>".to_string()
    }
}

/// End the process because a FOL callback panicked inside a C call.
///
/// Unwinding out of an `extern "C"` function is undefined behaviour, so the
/// panic has to stop here. A callback has no status channel -- the provider is
/// mid-call and takes only a return value -- so the alternative would be
/// returning a made-up value and letting the provider continue on it. That is
/// the silent wrong answer this boundary exists to prevent, so the process ends
/// instead, loudly and with the symbol named.
pub fn callback_panicked(symbol: &str) -> ! {
    eprintln!(
        "fol runtime fault: a FOL callback passed to '{symbol}' panicked. A callback has no \
         channel to report a failure through, and unwinding into C is undefined, so the process \
         is ending here rather than returning a value nobody computed."
    );
    std::process::abort()
}

/// Refuse a null handle from a routine declared to produce one.
///
/// A producer that returns `NULL` is reporting a failure through the one
/// channel C has for it. Adopting it would make a FOL value that owes a release
/// on nothing, and the release would be called on `NULL` later -- at which
/// point the provider is entitled to do anything at all. There is no value to
/// return that would be true, so this ends here with the symbol named.
///
/// A provider whose `NULL` is *meaningful* declares a status convention
/// instead; then the failure travels on FOL's recoverable channel and the
/// handle is only adopted on success.
pub fn handle_produced_null(symbol: &str) -> ! {
    eprintln!(
        "fol runtime fault: '{symbol}' is declared to produce an opaque handle but returned \
         NULL. Adopting it would owe a release on nothing. If NULL is how this routine reports \
         failure, declare a status convention for it in the annotation overlay."
    );
    std::process::abort()
}

/// Refuse a callback invocation whose context pointer is not the one FOL gave.
///
/// Same reasoning as the panic path: there is nothing to return that would be
/// true. A null context means the provider called back after FOL's frame was
/// gone, or called with a context it invented.
pub fn callback_context_invalid(symbol: &str) -> ! {
    eprintln!(
        "fol runtime fault: '{symbol}' invoked a FOL callback with a null context. The context is \
         the closure FOL lent for the duration of the call; a null one means the callback was \
         invoked outside that call."
    );
    std::process::abort()
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

/// Split a recoverable result for a generated C wrapper.
///
/// The stable substrate a wrapper is allowed to call. `FolRecover` itself stays
/// internal -- section 4.7 says the internal tagged result never crosses the
/// boundary -- so a wrapper gets a plain `Result` and writes exactly one out
/// parameter from it.
pub fn split_recoverable<T, E>(value: FolRecover<T, E>) -> Result<T, E> {
    match value {
        FolRecover::Ok(value) => Ok(value),
        FolRecover::Err(error) => Err(error),
    }
}

#[cfg(test)]
mod abi_split_tests {
    use super::{split_recoverable, FolRecover};

    #[test]
    fn a_recoverable_result_splits_into_one_side_only() {
        let ok: FolRecover<i64, i64> = FolRecover::Ok(7);
        assert_eq!(split_recoverable(ok), Ok(7));

        let err: FolRecover<i64, i64> = FolRecover::Err(9);
        assert_eq!(split_recoverable(err), Err(9));
    }
}
