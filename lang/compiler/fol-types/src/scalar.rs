//! Canonical scalar widths, shared by every layer that carries a FOL type.
//!
//! The parser records a width and a sign for every integer and float it reads.
//! Those facts have to survive typecheck, lowering, and codegen unchanged: a C
//! signature is meaningless without them, and a `u8` that silently holds an
//! `i64` is a promise the compiler does not keep. This module is the one
//! spelling of a width, so the layers cannot disagree about what `u32` means.
//!
//! `plan/V4_SCALAR_WIDTHS.md` records why `int` is an alias for `i64` and why
//! widths never mix implicitly.

/// The width and signedness of a FOL integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    /// Pointer-width signed, spelled `arch`.
    Arch,
    U8,
    U16,
    U32,
    U64,
    U128,
    /// Pointer-width unsigned, spelled `uarch`.
    UArch,
}

impl IntWidth {
    /// `int` is an alias for `i64`, not a distinct default integer.
    pub const DEFAULT: Self = Self::I64;

    pub const ALL: &'static [Self] = &[
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::Arch,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::UArch,
    ];

    pub const fn is_signed(self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::Arch
        )
    }

    /// Storage bits, or `None` for the pointer-width spellings, whose size is a
    /// property of the target rather than of the type.
    pub const fn bits(self) -> Option<u16> {
        match self {
            Self::I8 | Self::U8 => Some(8),
            Self::I16 | Self::U16 => Some(16),
            Self::I32 | Self::U32 => Some(32),
            Self::I64 | Self::U64 => Some(64),
            Self::I128 | Self::U128 => Some(128),
            Self::Arch | Self::UArch => None,
        }
    }

    /// The FOL spelling, which is also the Rust spelling for every width whose
    /// size is fixed.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Arch => "arch",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::UArch => "uarch",
        }
    }

    /// How FOL spells this width in source and diagnostics. The default width
    /// is spelled `int`, because `int` is an alias for it rather than a
    /// separate type — so a message about a plain `int` says `int`.
    pub const fn fol_spelling(self) -> &'static str {
        match self {
            Self::I64 => "int",
            other => other.as_str(),
        }
    }

    /// The Rust primitive this width is emitted as. `arch`/`uarch` become
    /// `isize`/`usize`; every other width names itself.
    pub const fn rust_primitive(self) -> &'static str {
        match self {
            Self::Arch => "isize",
            Self::UArch => "usize",
            other => other.as_str(),
        }
    }

    /// The inclusive range a constant must fall in to be stored at this width.
    /// Pointer-width spellings are measured at their guaranteed-minimum 64 bits
    /// on the certified targets, so a constant accepted here fits everywhere
    /// those targets run.
    pub const fn constant_range(self) -> (i128, i128) {
        match self {
            Self::I8 => (i8::MIN as i128, i8::MAX as i128),
            Self::I16 => (i16::MIN as i128, i16::MAX as i128),
            Self::I32 => (i32::MIN as i128, i32::MAX as i128),
            Self::I64 | Self::Arch => (i64::MIN as i128, i64::MAX as i128),
            Self::I128 => (i128::MIN, i128::MAX),
            Self::U8 => (0, u8::MAX as i128),
            Self::U16 => (0, u16::MAX as i128),
            Self::U32 => (0, u32::MAX as i128),
            Self::U64 | Self::UArch => (0, u64::MAX as i128),
            Self::U128 => (0, i128::MAX),
        }
    }

    /// Whether a constant can be stored at this width without losing value.
    pub const fn accepts_constant(self, value: i128) -> bool {
        let (low, high) = self.constant_range();
        value >= low && value <= high
    }
}

/// The width of a FOL float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatWidth {
    F32,
    F64,
    /// Pointer-width float, spelled `arch`; the certified targets make this 64.
    Arch,
}

impl FloatWidth {
    /// `flt` is an alias for `f64`.
    pub const DEFAULT: Self = Self::F64;

    pub const ALL: &'static [Self] = &[Self::F32, Self::F64, Self::Arch];

    pub const fn bits(self) -> u16 {
        match self {
            Self::F32 => 32,
            Self::F64 | Self::Arch => 64,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Arch => "arch",
        }
    }

    /// How FOL spells this width; the default width is spelled `flt`.
    pub const fn fol_spelling(self) -> &'static str {
        match self {
            Self::F64 => "flt",
            other => other.as_str(),
        }
    }

    pub const fn rust_primitive(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 | Self::Arch => "f64",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FloatWidth, IntWidth};

    #[test]
    fn int_widths_report_their_own_sign_and_size() {
        assert!(IntWidth::I32.is_signed());
        assert!(!IntWidth::U32.is_signed());
        assert_eq!(IntWidth::I32.bits(), Some(32));
        assert_eq!(IntWidth::U8.bits(), Some(8));
        // Pointer width is a target property, not a type property.
        assert_eq!(IntWidth::Arch.bits(), None);
        assert_eq!(IntWidth::UArch.bits(), None);
    }

    #[test]
    fn int_default_is_i64_because_int_is_an_alias_for_it() {
        assert_eq!(IntWidth::DEFAULT, IntWidth::I64);
        assert_eq!(FloatWidth::DEFAULT, FloatWidth::F64);
    }

    #[test]
    fn constant_ranges_reject_the_values_that_used_to_be_accepted() {
        // Both of these compiled and stored the full value before widths were
        // preserved.
        assert!(!IntWidth::U8.accepts_constant(999));
        assert!(!IntWidth::I32.accepts_constant(5_000_000_000));

        assert!(IntWidth::U8.accepts_constant(200));
        assert!(IntWidth::U8.accepts_constant(0));
        assert!(!IntWidth::U8.accepts_constant(-1));
        assert!(IntWidth::I8.accepts_constant(-128));
        assert!(!IntWidth::I8.accepts_constant(128));
    }

    #[test]
    fn widths_emit_the_rust_primitive_they_name() {
        assert_eq!(IntWidth::I32.rust_primitive(), "i32");
        assert_eq!(IntWidth::U64.rust_primitive(), "u64");
        assert_eq!(IntWidth::Arch.rust_primitive(), "isize");
        assert_eq!(IntWidth::UArch.rust_primitive(), "usize");
        assert_eq!(FloatWidth::F32.rust_primitive(), "f32");
        assert_eq!(FloatWidth::Arch.rust_primitive(), "f64");
    }
}
