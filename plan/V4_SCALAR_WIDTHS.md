# V4 Decision Record: Preserving Scalar Width and Signedness

> **Status: open. A decision is required before any foreign declaration model.**
>
> FOL parses `i8`, `u32`, `f32`, `int[64]` and friends with full fidelity and
> then discards the width and the sign in three lines of typecheck. Every sized
> spelling in the language is therefore decoration: they are one type, `i64`.
>
> This is both a V4 blocker — a C signature cannot be represented at all — and a
> live defect in today's language, because `var x: u8 = 999` is accepted and
> stores 999.
>
> The plumbing is bounded and mechanical. The three questions in section 4 are
> not, and they must be answered first, or each of the ~57 sites will invent its
> own answer.

## 1. The constraint

C distinguishes `int8_t` from `int32_t` from `long`, and signed from unsigned,
and those distinctions are load-bearing: they decide argument registers, struct
offsets, and whether a comparison is wrapping. LINC measures the exact widths
of a target's C types by compiling probes. None of that can reach FOL while
every integer is one type.

## 2. Verified facts

Each was confirmed against the tree, not assumed.

The full chain, from source to emitted Rust:

```text
surface     int  i8..i128  u8..u128  arch uarch  f32 f64
            int[32]  int[u8]  flt[64]  chr[utf16]
parser      FolType::Int { size: Option<IntSize>, signed: bool }   full fidelity
typecheck   decls.rs:2844  FolType::Int { .. } => builtin_types().int   COLLAPSE
checked     BuiltinType::Int                          no width, no sign
lower       LoweredBuiltinType::Int
backend     types.rs:773   => "rt::FolInt"
runtime     value.rs:6     pub type FolInt = i64
```

- **The parser is not the problem.** It already carries size and signedness,
  through both bare names (`u32`) and an option form (`int[32]`, `int[u8]`).
  `lower_integer_option`, `lower_float_option`, and `lower_char_option` in
  `fol-parser/src/ast/parser_parts/type_lowering_parsers.rs` are complete.
- **The collapse is three lines**, at `fol-typecheck/src/decls.rs:2844-2847`.
  `size` and `signed` are matched with `..` and dropped.
- **Every sized spelling is currently the same type.** `var y: i64 = x` where
  `x: i32` is accepted, because after the collapse they are one interned type.
- **The runtime mapping is `i64`** for every integer, whatever it was spelled.
- **No FOL source in the repository uses a sized scalar**, in either spelling.
  Examples, showcases, the standard library, and the book are all clear. The
  single `int[bor]` occurrence is an ownership option, not a size.

## 3. This is a live defect, not only a V4 gap

The language advertises sized integers and does not keep the promise:

```fol
var x: u8  = 999;          // accepted, prints 999
var y: i32 = 5000000000;   // accepted, prints 5000000000
```

Both compile and run today. A reader of the source has every reason to expect
`u8` to hold a byte. Nothing enforces it, and nothing warns.

That the corpus uses no sized types is what has kept this invisible. It also
means **fixing it breaks no existing FOL code** — the migration cost is zero,
which will not be true later.

## 4. The three questions

The plumbing cannot proceed until these are settled, because each of the ~57
sites below needs the same answers.

### 4.1 Is `int` the same type as `i64`?

Today, necessarily yes: both collapse to one type. Once width is preserved they
can be distinguished, so this becomes a choice.

- **A. `int` is an alias for `i64`.** What is already true. `arch`/`uarch`
  remain the pointer-width spellings. Nothing in the corpus changes meaning.
- **B. `int` is a distinct default integer.** A separate type that happens to be
  64-bit, not interchangeable with `i64`. Adds a type without adding an ability.
- **C. `int` is abstract and must be inferred to a concrete width.** Most
  precise, most disruptive: every existing `int` in the corpus becomes an
  inference site.

### 4.2 What is the type of an integer literal?

- **A. Literals are `int`, and in a typed context adopt the target type with a
  compile-time range check when constant.** `var x: u8 = 200` is fine and
  `= 999` is rejected. Fixes the live defect directly.
- **B. Literals are always `int`; assigning to a narrower type needs an explicit
  conversion.** Stricter and noisier; `var x: u8 = 200` would not compile
  without ceremony.
- **C. Full bidirectional literal inference.** Most flexible, most machinery.

### 4.3 What happens between widths?

`var c: i64 = a + b` with `a: i32` compiles today, because there is one type.
Once widths differ this needs a rule — and **FOL has no conversion operator at
all**: `as` and `cast` both parse and are rejected in typecheck
(`fol-typecheck/src/exprs/operators.rs:90-99`), and `LoweredInstrKind::Cast`
exists only for hand-built IR.

- **A. No implicit conversion. Widening explicit and infallible; narrowing
  explicit and fallible.** Narrowing can lose information, which is exactly what
  FOL's recoverable errors are for, and it matches the arithmetic-overflow rule
  already shipped (release builds fault rather than wrap).
- **B. Implicit widening, explicit narrowing.** Familiar from C, and quietly
  reintroduces the integer-promotion surprises FOL otherwise avoids.
- **C. No conversion at all between widths.** Simplest to specify and unusable
  in practice: a C binding constantly moves between `i32` and `int`.

Option A needs a conversion surface, which does not exist. Sub-decision, not
settled here: revive `cast` (already parsed, currently rejected) as the checked
conversion, or add a bracket operation in the family of `[mov]`/`[cpy]`.

## 5. Recommendation

**4.1 = A**, **4.2 = A**, **4.3 = A**.

Together these keep every existing program compiling unchanged, turn the live
defect into a compile error, and give C bindings the exact widths they need.
`int` stays the ordinary integer and stays 64-bit; sized spellings start meaning
what they say; a literal narrows where the target is narrower and is rejected
when it does not fit; and moving between widths is written down rather than
inferred.

The conversion surface in 4.3 is the one genuinely new piece of language. It
should be specified before implementation begins, not discovered during it.

## 6. Scale of the plumbing

Measured, excluding test modules:

```text
BuiltinType::Int          17 sites   fol-typecheck
LoweredBuiltinType::Int   40 sites   13 fol-lower/types.rs
                                      9 fol-backend/types.rs
                                      8 fol-backend/signatures.rs
                                      rest scattered
```

An earlier estimate of 102 sites during this review was wrong: it conflated the
two enums, which live in different layers, and counted test fixtures. The test
fixtures do move, but mechanically, once the enums settle.

Two smaller notes for whoever implements it:

- `i128` has no portable C counterpart and the H5 type matrix already rejects
  128-bit integers. Preserving the width in FOL is fine; projecting it to C is
  not, and GERC will refuse it.
- The option form shares bracket syntax with ownership options (`int[32]` and
  `int[bor]`). They are disjoint by name today, and any new size spelling must
  stay disjoint.

## 7. Consequences for `plan/V4_PLAN.md`

- **§3.1** records the collapse as a blocker and is accurate. It should gain the
  fact that the sized spellings are already reachable from source and silently
  lie, since that changes this from preparation into a fix.
- **M0** ("Contract Freeze, Characterization, and Truth Repair") is where the
  characterization in section 2 belongs.
- Every milestone that describes a C signature depends on this. No foreign
  declaration model can be specified over types that cannot distinguish
  `int32_t` from `int64_t`.
