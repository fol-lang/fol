//! Source/API surface for the public no-heap, no-hosted-services `core` tier.
//! This capability boundary does not claim that a generated executable is
//! freestanding: backend-only process adaptation remains separate, and a
//! host-compatible core binary may still be launched by the frontend.

pub use crate::abi::{check_recoverable, recoverable_succeeded, FolRecover};
pub use crate::aggregate::{
    render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
    FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
};
pub use crate::builtins::{len, pow, pow_float, FolLength};
pub use crate::containers::{index_array, render_array, FolArray};
pub use crate::shell::{
    unwrap_error_shell, unwrap_error_shell_ref, unwrap_optional_shell, unwrap_optional_shell_ref,
    FolError, FolOption,
};
pub use crate::value::{impossible, FolBool, FolChar, FolFloat, FolInt, FolNever};
pub use crate::{crate_name, CRATE_NAME};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTier {
    pub name: &'static str,
    pub has_heap: bool,
    pub has_os: bool,
}

impl RuntimeTier {
    pub const fn new(name: &'static str, has_heap: bool, has_os: bool) -> Self {
        Self {
            name,
            has_heap,
            has_os,
        }
    }
}

pub const HAS_HEAP: bool = false;
pub const HAS_OS: bool = false;
pub const TIER: RuntimeTier = RuntimeTier::new("core", HAS_HEAP, HAS_OS);

pub fn module_name() -> &'static str {
    "core"
}

pub fn tier_name() -> &'static str {
    TIER.name
}

pub fn capabilities() -> RuntimeTier {
    TIER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_tier_marks_no_heap_and_no_os() {
        assert_eq!(module_name(), "core");
        assert_eq!(tier_name(), "core");
        assert!(!HAS_HEAP);
        assert!(!HAS_OS);
        assert_eq!(capabilities(), TIER);
    }
}
