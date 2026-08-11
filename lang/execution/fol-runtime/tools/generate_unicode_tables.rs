// Emits fol-runtime's normalization tables.
//
// The data is COMPUTED from unicode-normalization rather than transcribed, so
// it is accurate by derivation. The crate is a build-time tool here, not a
// runtime dependency: fol-runtime is compiled by a bare `rustc` with no
// dependency resolution, so it can only carry plain data.
use std::fmt::Write as _;
use unicode_normalization::char::{canonical_combining_class, compose};
use unicode_normalization::UnicodeNormalization;

// Hangul syllables are excluded from every table: they decompose and compose by
// the arithmetic in the Unicode standard, so 11172 rows of derived data would be
// pure bloat in a file that links into every FOL binary.
const HANGUL_FIRST: u32 = 0xAC00;
const HANGUL_LAST: u32 = 0xD7A3;

fn codepoints() -> impl Iterator<Item = char> {
    (0u32..=0x10FFFF)
        .filter(|value| !(HANGUL_FIRST..=HANGUL_LAST).contains(value))
        .filter_map(char::from_u32)
}

fn main() {
    let mut out = String::new();
    out.push_str(
        "// GENERATED — do not edit by hand.\n\
         //\n\
         // Derived from the unicode-normalization crate by `gen.rs` (recorded in\n\
         // PLAN.md). The crate is a build-time tool, not a dependency:\n\
         // fol-runtime is compiled by a bare `rustc` with no `--extern`, so it can\n\
         // only carry plain data. Regenerate rather than editing.\n\n",
    );

    // Fully expanded decompositions, so the runtime never has to recurse.
    for (name, compat) in [
        ("CANONICAL_DECOMPOSITION", false),
        ("COMPAT_DECOMPOSITION", true),
    ] {
        let mut rows = Vec::new();
        for ch in codepoints() {
            let expanded: Vec<char> = if compat {
                ch.to_string().nfkd().collect()
            } else {
                ch.to_string().nfd().collect()
            };
            if expanded.len() == 1 && expanded[0] == ch {
                continue;
            }
            let items: Vec<String> = expanded
                .iter()
                .map(|c| format!("{:#x}", *c as u32))
                .collect();
            rows.push(format!(
                "    ({:#x}, &[{}]),\n",
                ch as u32,
                items.join(", ")
            ));
        }
        let _ = write!(
            out,
            "pub(crate) const {name}: &[(u32, &[u32])] = &[\n{}];\n\n",
            rows.concat()
        );
        eprintln!("{name}: {} rows", rows.len());
    }

    // Non-zero combining classes only; everything else is 0.
    let mut ccc = Vec::new();
    for ch in codepoints() {
        let class = canonical_combining_class(ch);
        if class != 0 {
            ccc.push(format!("    ({:#x}, {class}),\n", ch as u32));
        }
    }
    let _ = write!(
        out,
        "pub(crate) const COMBINING_CLASS: &[(u32, u8)] = &[\n{}];\n\n",
        ccc.concat()
    );
    eprintln!("COMBINING_CLASS: {} rows", ccc.len());

    // Canonical compositions, via `compose` so the exclusions are already
    // applied.
    //
    // The pair is PRIMARY, not the full decomposition: U+1EDB composes from
    // U+01A1 + U+0301, and U+01A1 is itself a composite, so its full NFD is
    // three characters. Filtering on a two-character NFD silently dropped every
    // multi-level composite -- caught by the all-codepoints conformance test.
    let mut pairs = Vec::new();
    for ch in codepoints() {
        let expanded: Vec<char> = ch.to_string().nfd().collect();
        if expanded.len() < 2 {
            continue;
        }
        let last = expanded[expanded.len() - 1];
        let head: String = expanded[..expanded.len() - 1].iter().collect();
        let composed_head: Vec<char> = head.nfc().collect();
        if composed_head.len() != 1 {
            continue;
        }
        if compose(composed_head[0], last) == Some(ch) {
            pairs.push((composed_head[0] as u32, last as u32, ch as u32));
        }
    }
    pairs.sort_unstable();
    let rendered: String = pairs
        .iter()
        .map(|(a, b, c)| format!("    ({a:#x}, {b:#x}, {c:#x}),\n"))
        .collect();
    let _ = write!(
        out,
        "pub(crate) const CANONICAL_COMPOSITION: &[(u32, u32, u32)] = &[\n{rendered}];\n"
    );
    eprintln!("CANONICAL_COMPOSITION: {} rows", pairs.len());

    print!("{out}");
}
