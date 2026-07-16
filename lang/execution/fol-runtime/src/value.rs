//! Shared runtime value-facing aliases and helpers.

use std::convert::Infallible;

/// Canonical runtime integer representation for FOL `int`.
pub type FolInt = i64;

/// Canonical runtime floating-point representation for FOL `flt`.
pub type FolFloat = f64;

/// Canonical runtime boolean representation for FOL `bol`.
pub type FolBool = bool;

/// Canonical runtime character representation for FOL `chr`.
pub type FolChar = char;

/// Canonical runtime uninhabited representation for FOL `never`.
pub type FolNever = Infallible;

/// Convert an impossible `never` value into an unreachable control-flow edge.
pub fn impossible(value: FolNever) -> ! {
    match value {}
}

pub fn module_name() -> &'static str {
    "value"
}

#[cfg(test)]
mod tests {
    use super::{impossible, FolBool, FolChar, FolFloat, FolInt, FolNever};
    use std::{any::type_name, mem::size_of};

    #[test]
    fn scalar_aliases_freeze_v1_runtime_representations() {
        assert_eq!(type_name::<FolInt>(), "i64");
        assert_eq!(type_name::<FolFloat>(), "f64");
        assert_eq!(type_name::<FolBool>(), "bool");
        assert_eq!(type_name::<FolChar>(), "char");
        assert_eq!(type_name::<FolNever>(), "core::convert::Infallible");
        assert_eq!(size_of::<FolInt>(), size_of::<i64>());
        assert_eq!(size_of::<FolFloat>(), size_of::<f64>());
        assert_eq!(size_of::<FolBool>(), size_of::<bool>());
        assert_eq!(size_of::<FolChar>(), size_of::<char>());
    }

    #[test]
    fn impossible_helper_is_callable_for_never_strategy() {
        let helper: fn(FolNever) -> ! = impossible;

        assert_eq!(helper as usize, impossible as fn(FolNever) -> ! as usize);
    }
}
