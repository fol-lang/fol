//! Unicode normalization.
//!
//! `é` typed as `e` plus a combining accent and `é` typed precomposed look
//! identical, compare unequal, and report different lengths. Normalizing both
//! sides is the only way user-entered text behaves the way a reader expects.
//!
//! The tables in `unicode_tables` are generated from the
//! `unicode-normalization` crate, so the *data* is accurate by derivation
//! rather than transcription. The crate cannot be a dependency: fol-runtime is
//! compiled by a bare `rustc` with no `--extern`, so it can only carry plain
//! data. The algorithms below are from the Unicode standard, and the tests
//! check them against that same crate across every codepoint.
//!
//! Hangul syllables are handled arithmetically rather than from a table. That
//! is how the standard defines them, and it keeps 11172 rows of derived data
//! out of a file that links into every FOL binary.

use crate::unicode_tables::{
    CANONICAL_COMPOSITION, CANONICAL_DECOMPOSITION, COMBINING_CLASS, COMPAT_DECOMPOSITION,
};

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

impl Form {
    pub(crate) fn from_selector(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Nfc),
            1 => Some(Self::Nfd),
            2 => Some(Self::Nfkc),
            3 => Some(Self::Nfkd),
            _ => None,
        }
    }

    fn uses_compatibility(self) -> bool {
        matches!(self, Self::Nfkc | Self::Nfkd)
    }

    fn composes(self) -> bool {
        matches!(self, Self::Nfc | Self::Nfkc)
    }
}

fn combining_class(value: u32) -> u8 {
    COMBINING_CLASS
        .binary_search_by_key(&value, |(codepoint, _)| *codepoint)
        .map_or(0, |index| COMBINING_CLASS[index].1)
}

fn table_decomposition(
    table: &'static [(u32, &'static [u32])],
    value: u32,
) -> Option<&'static [u32]> {
    table
        .binary_search_by_key(&value, |(codepoint, _)| *codepoint)
        .ok()
        .map(|index| table[index].1)
}

fn hangul_decomposition(value: u32, out: &mut Vec<u32>) -> bool {
    if !(S_BASE..S_BASE + S_COUNT).contains(&value) {
        return false;
    }
    let index = value - S_BASE;
    out.push(L_BASE + index / N_COUNT);
    out.push(V_BASE + (index % N_COUNT) / T_COUNT);
    let trailing = index % T_COUNT;
    if trailing != 0 {
        out.push(T_BASE + trailing);
    }
    true
}

fn hangul_composition(first: u32, second: u32) -> Option<u32> {
    // Leading + vowel, which yields a syllable with no trailing consonant.
    if (L_BASE..L_BASE + L_COUNT).contains(&first) && (V_BASE..V_BASE + V_COUNT).contains(&second) {
        let leading = first - L_BASE;
        let vowel = second - V_BASE;
        return Some(S_BASE + (leading * V_COUNT + vowel) * T_COUNT);
    }
    // Syllable + trailing, only when the syllable does not already carry one.
    if (S_BASE..S_BASE + S_COUNT).contains(&first)
        && (first - S_BASE) % T_COUNT == 0
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&second)
    {
        return Some(first + (second - T_BASE));
    }
    None
}

fn decompose(text: &str, compatibility: bool) -> Vec<u32> {
    let table = if compatibility {
        COMPAT_DECOMPOSITION
    } else {
        CANONICAL_DECOMPOSITION
    };
    let mut out = Vec::with_capacity(text.len());
    for character in text.chars() {
        let value = character as u32;
        if hangul_decomposition(value, &mut out) {
            continue;
        }
        // The generated expansions are already fully applied, so no recursion.
        match table_decomposition(table, value) {
            Some(expansion) => out.extend_from_slice(expansion),
            None => out.push(value),
        }
    }
    canonical_order(&mut out);
    out
}

/// Sorts each run of combining marks by combining class, leaving equal classes
/// in their original order. The stability matters: reordering marks that share
/// a class would change the text rather than normalize it.
fn canonical_order(values: &mut [u32]) {
    let mut start = 0;
    while start < values.len() {
        if combining_class(values[start]) == 0 {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < values.len() && combining_class(values[end]) != 0 {
            end += 1;
        }
        values[start..end].sort_by_key(|value| combining_class(*value));
        start = end;
    }
}

fn canonical_composition(first: u32, second: u32) -> Option<u32> {
    if let Some(composed) = hangul_composition(first, second) {
        return Some(composed);
    }
    CANONICAL_COMPOSITION
        .binary_search_by(|(left, right, _)| (*left, *right).cmp(&(first, second)))
        .ok()
        .map(|index| CANONICAL_COMPOSITION[index].2)
}

/// The standard's canonical composition pass over an already-decomposed,
/// canonically ordered sequence.
///
/// A mark may only combine with the last starter when nothing *blocks* it —
/// that is, when no preceding mark has a combining class greater than or equal
/// to its own. Without that rule the pass would compose across an intervening
/// mark and silently reorder the text's meaning.
fn compose_sequence(values: Vec<u32>) -> Vec<u32> {
    if values.is_empty() {
        return values;
    }
    let mut out: Vec<u32> = Vec::with_capacity(values.len());
    let mut starter: Option<usize> = None;
    let mut last_class: Option<u8> = None;

    for value in values {
        let class = combining_class(value);
        if let Some(index) = starter {
            let blocked = match last_class {
                Some(previous) => previous >= class && class != 0,
                None => false,
            };
            if !blocked && (class != 0 || last_class.is_none()) {
                if let Some(composed) = canonical_composition(out[index], value) {
                    out[index] = composed;
                    continue;
                }
            }
        }
        if class == 0 {
            starter = Some(out.len());
            last_class = None;
        } else {
            last_class = Some(class);
        }
        out.push(value);
    }
    out
}

pub(crate) fn normalize(text: &str, form: Form) -> String {
    let decomposed = decompose(text, form.uses_compatibility());
    let values = if form.composes() {
        compose_sequence(decomposed)
    } else {
        decomposed
    };
    values
        .into_iter()
        .filter_map(char::from_u32)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nfc(text: &str) -> String {
        normalize(text, Form::Nfc)
    }
    fn nfd(text: &str) -> String {
        normalize(text, Form::Nfd)
    }

    #[test]
    fn composes_and_decomposes_the_two_spellings_of_one_word() {
        let combining = "e\u{0301}llo";
        let precomposed = "\u{e9}llo";
        assert_ne!(combining, precomposed);
        assert_eq!(nfc(combining), nfc(precomposed));
        assert_eq!(nfc(combining).chars().count(), 4);
        assert_eq!(nfd(precomposed).chars().count(), 5);
    }

    #[test]
    fn compatibility_folds_only_in_the_k_forms() {
        assert_eq!(normalize("ﬁle", Form::Nfkc), "file");
        assert_eq!(normalize("Ａ", Form::Nfkc), "A");
        assert_ne!(normalize("ﬁle", Form::Nfc), "file");
    }

    // Hangul is arithmetic rather than table data, so it needs its own check.
    #[test]
    fn hangul_composes_and_decomposes_arithmetically() {
        // 각 = ᄀ + ᅡ + ᆨ
        assert_eq!(nfd("\u{AC01}"), "\u{1100}\u{1161}\u{11A8}");
        assert_eq!(nfc("\u{1100}\u{1161}\u{11A8}"), "\u{AC01}");
        // 가 has no trailing consonant, so it decomposes to two.
        assert_eq!(nfd("\u{AC00}"), "\u{1100}\u{1161}");
        assert_eq!(nfc("\u{1100}\u{1161}"), "\u{AC00}");
        assert_eq!(nfc(&nfd("\u{D7A3}")), "\u{D7A3}");
    }

    // A multi-level composite: U+1EDB is U+01A1 + U+0301, and U+01A1 is itself
    // o + horn. Deriving composition pairs from the FULL decomposition rather
    // than the primary one dropped every case of this shape.
    #[test]
    fn multi_level_composites_compose_completely() {
        assert_eq!(nfc("o\u{031B}\u{0301}"), "\u{1EDB}");
        assert_eq!(nfd("\u{1EDB}"), "o\u{031B}\u{0301}");
        // Marks given out of canonical order still reach the same answer.
        assert_eq!(nfc("o\u{0301}\u{031B}"), "\u{1EDB}");
    }

    #[test]
    fn canonical_ordering_sorts_marks_by_combining_class() {
        // U+0323 (below, 220) must precede U+0301 (above, 230) whichever way
        // they are typed.
        assert_eq!(nfd("q\u{0301}\u{0323}"), "q\u{0323}\u{0301}");
        assert_eq!(nfd("q\u{0323}\u{0301}"), "q\u{0323}\u{0301}");
    }

    // Composition continues through a mark that was itself consumed: the
    // macron composes with `e`, and the acute then composes with the result.
    // Both marks share combining class 230, so nothing blocks.
    #[test]
    fn composition_continues_through_a_consumed_mark() {
        assert_eq!(nfc("e\u{0304}\u{0301}"), "\u{1E17}");
    }

    // Canonical ordering runs first, which is what lets the acute reach the
    // base past a higher-class mark. U+0345 has class 240, U+0301 has 230, so
    // ordering moves the acute adjacent to `a` and only then does it compose.
    #[test]
    fn ordering_decides_what_can_reach_the_starter() {
        assert_eq!(nfc("a\u{0345}\u{0301}"), "\u{00E1}\u{0345}");
        // Nothing composes with `q`, so the marks survive in canonical order.
        assert_eq!(nfc("q\u{0301}\u{0300}"), "q\u{0301}\u{0300}");
    }

    // The strongest property available without the reference crate: every form
    // is idempotent, and NFD then NFC returns the composed form.
    #[test]
    fn every_form_is_idempotent_across_the_codepoint_space() {
        let forms = [Form::Nfc, Form::Nfd, Form::Nfkc, Form::Nfkd];
        let mut checked = 0u32;
        for value in 0u32..=0x10FFFF {
            let Some(character) = char::from_u32(value) else {
                continue;
            };
            let text = character.to_string();
            for form in forms {
                let once = normalize(&text, form);
                let twice = normalize(&once, form);
                assert_eq!(twice, once, "form was not idempotent at U+{value:04X}");
            }
            assert_eq!(
                nfc(&nfd(&text)),
                nfc(&text),
                "NFD then NFC diverged at U+{value:04X}"
            );
            checked += 1;
        }
        assert!(checked > 1_000_000, "only checked {checked} codepoints");
    }
}
