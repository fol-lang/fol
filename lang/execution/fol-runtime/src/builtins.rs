//! Runtime-owned builtin and intrinsic hook support.

use crate::{
    containers::FolArray,
    memo::{FolMap, FolSeq, FolSet, FolStr, FolVec},
    value::FolInt,
};

pub trait FolLength {
    fn fol_length(&self) -> FolInt;
}

pub fn len<T: FolLength + ?Sized>(value: &T) -> FolInt {
    value.fol_length()
}

impl FolLength for FolStr {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T, const N: usize> FolLength for FolArray<T, N> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T> FolLength for FolVec<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T> FolLength for FolSeq<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T: Ord> FolLength for FolSet<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<K: Ord, V> FolLength for FolMap<K, V> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

pub fn pow(base: FolInt, exponent: FolInt) -> FolInt {
    base.pow(exponent as u32)
}

pub fn pow_float(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

/// Numeric and character conversions. FOL has no implicit coercion and no cast
/// operator, so every crossing between `int`, `flt` and `chr` is one of these.
pub fn int_to_flt(value: FolInt) -> crate::value::FolFloat {
    value as crate::value::FolFloat
}

/// Truncates toward zero, matching what integer `/` already does, so the two
/// roundings in the language agree.
pub fn flt_to_int(value: crate::value::FolFloat) -> FolInt {
    value.trunc() as FolInt
}

pub fn flt_floor(value: crate::value::FolFloat) -> FolInt {
    value.floor() as FolInt
}

pub fn flt_ceil(value: crate::value::FolFloat) -> FolInt {
    value.ceil() as FolInt
}

/// Halves go away from zero, which is what `f64::round` does.
pub fn flt_round(value: crate::value::FolFloat) -> FolInt {
    value.round() as FolInt
}

pub fn chr_to_int(value: crate::value::FolChar) -> FolInt {
    value as u32 as FolInt
}

/// Not every integer is a code point, and there is no `opt[chr]` on the
/// intrinsic surface, so an invalid value faults the way a bad index does.
pub fn int_to_chr(value: FolInt) -> crate::value::FolChar {
    u32::try_from(value)
        .ok()
        .and_then(char::from_u32)
        .unwrap_or_else(|| {
            panic!("fol runtime fault: {value} is not a Unicode code point");
        })
}

pub fn chr_to_str(value: crate::value::FolChar) -> FolStr {
    FolStr::new(value.to_string())
}

pub fn parse_flt(text: FolStr, fallback: crate::value::FolFloat) -> crate::value::FolFloat {
    text.as_str()
        .trim()
        .parse::<crate::value::FolFloat>()
        .unwrap_or(fallback)
}

/// Integer helpers. `checked_*` faults on overflow in EVERY build profile,
/// which is the point: plain `+` traps in debug and wraps in release, so a
/// program that must not wrap needs a spelling that behaves the same either way.
/// `wrapping_*` and `saturating_*` are the two explicit alternatives.
pub fn min(left: FolInt, right: FolInt) -> FolInt {
    if left < right {
        left
    } else {
        right
    }
}

pub fn max(left: FolInt, right: FolInt) -> FolInt {
    if left > right {
        left
    } else {
        right
    }
}

pub fn abs(value: FolInt) -> FolInt {
    match value.checked_abs() {
        Some(value) => value,
        None => panic!("fol runtime fault: absolute value overflowed"),
    }
}

pub fn checked_add(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_add(right) {
        Some(value) => value,
        None => panic!("fol runtime fault: integer addition overflowed"),
    }
}

pub fn checked_sub(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_sub(right) {
        Some(value) => value,
        None => panic!("fol runtime fault: integer subtraction overflowed"),
    }
}

pub fn wrapping_add(left: FolInt, right: FolInt) -> FolInt {
    left.wrapping_add(right)
}

pub fn wrapping_sub(left: FolInt, right: FolInt) -> FolInt {
    left.wrapping_sub(right)
}

pub fn saturating_add(left: FolInt, right: FolInt) -> FolInt {
    left.saturating_add(right)
}

pub fn saturating_sub(left: FolInt, right: FolInt) -> FolInt {
    left.saturating_sub(right)
}

pub fn asin(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.asin()
}

pub fn acos(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.acos()
}

pub fn atan(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.atan()
}

pub fn log2(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.log2()
}

/// The IEEE-754 bit pattern, and back. Nothing else in the language can observe
/// a float's representation, which is what exact serialization, hashing and
/// bit-level comparison all need.
pub fn flt_bits(value: crate::value::FolFloat) -> FolInt {
    value.to_bits() as FolInt
}

pub fn flt_from_bits(bits: FolInt) -> crate::value::FolFloat {
    crate::value::FolFloat::from_bits(bits as u64)
}

pub fn flt_is_finite(value: crate::value::FolFloat) -> crate::value::FolBool {
    value.is_finite()
}

/// The sign of `sign` applied to the magnitude of `value`. This is the only way
/// to observe the sign of a zero: `-0.0 == 0.0` is true, so a comparison cannot
/// tell them apart.
pub fn flt_copysign(
    value: crate::value::FolFloat,
    sign: crate::value::FolFloat,
) -> crate::value::FolFloat {
    value.copysign(sign)
}

/// Floating remainder. `%` is integer-only, so this is the float counterpart;
/// the result takes the sign of the dividend, like C's `fmod`.
pub fn flt_rem(
    left: crate::value::FolFloat,
    right: crate::value::FolFloat,
) -> crate::value::FolFloat {
    left % right
}

/// Fused multiply-add: computed with ONE rounding instead of two, so it is not
/// the same as `a * b + c` in the last bit. That difference is the point —
/// it is what keeps error from accumulating in dot products and polynomials.
pub fn flt_mul_add(
    value: crate::value::FolFloat,
    multiplier: crate::value::FolFloat,
    addend: crate::value::FolFloat,
) -> crate::value::FolFloat {
    value.mul_add(multiplier, addend)
}

/// The next representable float from `value` toward `target` — one ULP step.
/// Tolerance-based tests need it, and there is no way to express "the smallest
/// possible increment" arithmetically, because it changes with magnitude.
pub fn flt_next_after(
    value: crate::value::FolFloat,
    target: crate::value::FolFloat,
) -> crate::value::FolFloat {
    if value.is_nan() || target.is_nan() {
        return crate::value::FolFloat::NAN;
    }
    if value == target {
        return target;
    }
    if value == 0.0 {
        // Both zeros compare equal, so step off zero explicitly toward the
        // target's sign rather than trying to read the sign of the zero.
        return crate::value::FolFloat::from_bits(1).copysign(target);
    }
    let bits = value.to_bits();
    let stepped = if (target > value) == (value > 0.0) {
        bits + 1
    } else {
        bits - 1
    };
    crate::value::FolFloat::from_bits(stepped)
}

/// The multiplication and division that complete the checked/wrapping/
/// saturating family. `checked_*` faults in EVERY build profile; plain `*`
/// traps in debug and wraps in release, so a program that must not wrap needs a
/// spelling that behaves the same either way.
pub fn checked_mul(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_mul(right) {
        Some(value) => value,
        None => panic!("fol runtime fault: integer multiplication overflowed"),
    }
}

pub fn wrapping_mul(left: FolInt, right: FolInt) -> FolInt {
    left.wrapping_mul(right)
}

pub fn saturating_mul(left: FolInt, right: FolInt) -> FolInt {
    left.saturating_mul(right)
}

/// Division that faults on overflow as well as by zero. `div_int` already
/// faults; this exists so the checked family is complete and callable by name.
pub fn checked_div(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_div(right) {
        Some(value) => value,
        None if right == 0 => panic!("fol runtime fault: division by zero"),
        None => panic!("fol runtime fault: integer division overflowed"),
    }
}

/// Bitwise operations on `int`, which is a signed 64-bit value. FOL has no
/// bitwise operators, so these are the whole surface; emulating them with `/`
/// and `%` is what the mimicry round was forced into, and it breaks on
/// negatives.
pub fn bit_and(left: FolInt, right: FolInt) -> FolInt {
    left & right
}

pub fn bit_or(left: FolInt, right: FolInt) -> FolInt {
    left | right
}

pub fn bit_xor(left: FolInt, right: FolInt) -> FolInt {
    left ^ right
}

/// Rust's `<<` panics in debug and wraps in release once the shift reaches the
/// width. Faulting on both makes the boundary the same in either profile.
pub fn shl(value: FolInt, shift: FolInt) -> FolInt {
    match u32::try_from(shift).ok().filter(|shift| *shift < 64) {
        Some(shift) => value.wrapping_shl(shift),
        None => panic!("fol runtime fault: shift out of range: {shift}"),
    }
}

/// Arithmetic shift: the sign bit is preserved, matching how `int` divides.
pub fn shr(value: FolInt, shift: FolInt) -> FolInt {
    match u32::try_from(shift).ok().filter(|shift| *shift < 64) {
        Some(shift) => value.wrapping_shr(shift),
        None => panic!("fol runtime fault: shift out of range: {shift}"),
    }
}

pub fn rotl(value: FolInt, shift: FolInt) -> FolInt {
    value.rotate_left((shift.rem_euclid(64)) as u32)
}

pub fn rotr(value: FolInt, shift: FolInt) -> FolInt {
    value.rotate_right((shift.rem_euclid(64)) as u32)
}

pub fn pop_count(value: FolInt) -> FolInt {
    value.count_ones() as FolInt
}

pub fn clz(value: FolInt) -> FolInt {
    value.leading_zeros() as FolInt
}

pub fn ctz(value: FolInt) -> FolInt {
    value.trailing_zeros() as FolInt
}

/// Float mathematics. A negative square root faults rather than producing NaN,
/// so a mistake surfaces where it happened instead of propagating silently.
pub fn sqrt(value: crate::value::FolFloat) -> crate::value::FolFloat {
    if value < 0.0 {
        panic!("fol runtime fault: sqrt of a negative number: {value}");
    }
    value.sqrt()
}

pub fn flt_abs(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.abs()
}

pub fn sin(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.sin()
}

pub fn cos(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.cos()
}

pub fn tan(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.tan()
}

pub fn atan2(y: crate::value::FolFloat, x: crate::value::FolFloat) -> crate::value::FolFloat {
    y.atan2(x)
}

pub fn ln(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.ln()
}

pub fn log10(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.log10()
}

pub fn exp(value: crate::value::FolFloat) -> crate::value::FolFloat {
    value.exp()
}

pub fn hypot(x: crate::value::FolFloat, y: crate::value::FolFloat) -> crate::value::FolFloat {
    x.hypot(y)
}

pub fn is_nan(value: crate::value::FolFloat) -> crate::value::FolBool {
    value.is_nan()
}

pub fn is_inf(value: crate::value::FolFloat) -> crate::value::FolBool {
    value.is_infinite()
}

/// Integer division with the documented fault semantics (arithmetics
/// chapter): division by zero faults instead of surfacing a raw Rust panic
/// that points into generated code.
pub fn div_int(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_div(right) {
        Some(value) => value,
        None if right == 0 => panic!("fol runtime fault: division by zero"),
        None => panic!("fol runtime fault: integer division overflowed"),
    }
}

/// Integer remainder; same fault presentation as [`div_int`].
pub fn mod_int(left: FolInt, right: FolInt) -> FolInt {
    match left.checked_rem(right) {
        Some(value) => value,
        None if right == 0 => panic!("fol runtime fault: modulo by zero"),
        None => panic!("fol runtime fault: integer remainder overflowed"),
    }
}

pub fn module_name() -> &'static str {
    "builtins"
}

#[cfg(test)]
mod tests {
    use super::{len, FolLength};
    use crate::{
        containers::FolArray,
        memo::{FolMap, FolSeq, FolSet, FolStr, FolVec},
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn runtime_length_trait_covers_current_v1_families() {
        let text = FolStr::from("Ada");
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2]);
        let sequence = FolSeq::from_items(vec![1, 2, 3, 4]);
        let set = FolSet::new(BTreeSet::from([1, 2, 3]));
        let map = FolMap::new(BTreeMap::from([("ada", 1), ("lin", 2)]));

        assert_eq!(text.fol_length(), 3);
        assert_eq!(array.fol_length(), 3);
        assert_eq!(vector.fol_length(), 2);
        assert_eq!(sequence.fol_length(), 4);
        assert_eq!(set.fol_length(), 3);
        assert_eq!(map.fol_length(), 2);
    }

    #[test]
    fn runtime_len_helper_covers_current_v1_supported_families() {
        let text = FolStr::from("Ada");
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2]);
        let sequence = FolSeq::from_items(vec![1, 2, 3, 4]);
        let set = FolSet::from_items(vec![3, 1, 2]);
        let map = FolMap::from_pairs(vec![(FolStr::from("ada"), 1), (FolStr::from("lin"), 2)]);

        assert_eq!(len(&text), 3);
        assert_eq!(len(&array), 3);
        assert_eq!(len(&vector), 2);
        assert_eq!(len(&sequence), 4);
        assert_eq!(len(&set), 3);
        assert_eq!(len(&map), 2);
    }
}
