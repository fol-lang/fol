# Regenerating the Unicode tables

`src/unicode_tables.rs` is generated, not hand-written. Normalization needs the
canonical and compatibility decomposition mappings, the combining classes, and
the composition pairs with exclusions applied — data that cannot be recalled
accurately and that `std` does not expose.

It is **generated data, not a dependency**, because `fol-runtime` is compiled
by a bare `rustc` with no `--extern` (see
`fol-backend/src/emit/build.rs::configure_runtime_rustc_command`). Adding a real
dependency would mean the toolchain building third-party crates per target
triple. So the crate is used once, here, at authoring time.

To regenerate:

```sh
cargo new /tmp/gen --lib && cd /tmp/gen
cargo add unicode-normalization
mkdir -p src/bin && cp <repo>/lang/execution/fol-runtime/tools/generate_unicode_tables.rs src/bin/gen.rs
cargo run --release --bin gen 2>/dev/null > <repo>/lang/execution/fol-runtime/src/unicode_tables.rs
```

Then run `cargo test -p fol-runtime normalize`. Those tests check idempotence
and known answers without the crate. To re-check against the reference itself,
copy `src/normalize.rs` and `src/unicode_tables.rs` into the scratch crate and
assert equality with `nfc()`/`nfd()`/`nfkc()`/`nfkd()` over every codepoint —
that is what caught the composition bug described below.

## Two things the generator gets right that are easy to get wrong

- **Composition pairs are primary, not fully decomposed.** `U+1EDB` composes
  from `U+01A1 + U+0301`, and `U+01A1` is itself a composite, so the full NFD is
  three characters. Filtering on a two-character NFD silently drops every
  multi-level composite; the all-codepoints conformance test caught it.
- **Hangul is excluded from every table.** It decomposes and composes by the
  arithmetic in the standard, so a table would add 11172 rows of derived data
  to a file that links into every FOL binary.
