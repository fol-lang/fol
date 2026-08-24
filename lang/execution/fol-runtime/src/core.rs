//! Source/API surface for the public no-heap, no-hosted-services `core` tier.
//! This capability boundary does not claim that a generated executable is
//! freestanding: backend-only process adaptation remains separate, and a
//! host-compatible core binary may still be launched by the frontend.

pub use crate::abi::{
    callback_context_invalid, callback_panicked, check_recoverable, handle_produced_null,
    recoverable_succeeded, FolHandle, FolRecover,
};
pub use crate::aggregate::{
    render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
    FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
};
pub use crate::builtins::{
    abs, acos, asin, atan, atan2, bit_and, bit_not, bit_or, bit_xor, checked_add, checked_div,
    checked_mul, checked_sub, chr_to_int, clz, cos, ctz, div_int, exp, flt_abs, flt_bits, flt_ceil,
    flt_copysign, flt_floor, flt_from_bits, flt_is_finite, flt_mul_add, flt_next_after, flt_rem,
    flt_round, flt_to_int, hypot, int_to_chr, int_to_flt, is_inf, is_nan, len, ln, log10, log2,
    max, min, mod_int, pop_count, pow, pow_float, rotl, rotr, saturating_add, saturating_mul,
    saturating_sub, shl, shr, sin, sqrt, tan, wrapping_add, wrapping_mul, wrapping_sub, FolLength,
};
pub use crate::containers::{index_array, render_array, store_array, FolArray};
pub use crate::error::{assert_that, require};
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
        assert_eq!(TIER, RuntimeTier::new("core", false, false));
        assert_eq!(capabilities(), TIER);
    }
}
