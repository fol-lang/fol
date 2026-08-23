//! The uniform C status values from section 4.7.
//!
//! Rendered as Rust literals for the generated wrappers. The same numbers are
//! frozen as C macros in `examples/v4_contract_header/demo.h`, and
//! `fol_abi::STATUS_VALUES` is the third view; a test holds all three together.

pub const OK: &str = "0i32";
pub const REPORT: &str = "1i32";
pub const INVALID_ARGUMENT: &str = "-1i32";
pub const PANIC: &str = "-2i32";
/// An internal wrapper or runtime failure. Reserved; the generated wrappers
/// have no path that produces it yet, because every failure they can observe
/// is one of the four above.
pub const INTERNAL: &str = "-3i32";
