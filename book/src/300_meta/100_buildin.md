# Intrinsics

Intrinsics are compiler-owned language operations.

They are not ordinary library functions, and they are not imported through
`use`.

FOL currently keeps compiler intrinsics and three API tiers separate:

- intrinsics:
  compiler-owned operations such as `.eq(...)`, `.len(...)`, `check(...)`, and
  `panic(...)`
- `core`:
  the minimal runtime model with no heap and no source-level hosted OS/runtime
  APIs
- `memo`:
  alloc-like heap-backed library/runtime support without source-level hosted
  OS/runtime APIs
- `std`:
  the hosted API tier layered on `memo` by an explicit bundled internal
  dependency, with shipped services such as console I/O

This split is not a source-level import trick and not an object-system feature.
`core` and `memo` are artifact capability models selected through `fol_model`
in `build.fol`. Bundled `std` is not a third model; it is declared separately
with `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
for a `memo` artifact that needs hosted source APIs.

Whether an artifact can be launched is orthogonal. Host-compatible `core` and
`memo` programs can use `fol code run` or `fol code test` without bundled
`std`; the frontend launching them does not expose extra intrinsics.

If an operation can live as an ordinary library API, that is usually the better
home for it. Intrinsics are reserved for surfaces the compiler must understand
directly.

## Surfaces

The current compiler recognizes three intrinsic surfaces.

## Dot-root calls

These are written with a leading dot:

```fol
.eq(a, b)
.not(flag)
.len(items)
.echo(value)
```

Dot-root intrinsics are the main current intrinsic family.

## Keyword calls

These look like language keywords rather than dot calls:

```fol
check(read_code(path))
panic("unreachable state")
```

The current `V1` compiler treats `check` and `panic` as intrinsics too, even
though they are not written with `.`.

## Operator aliases

Some future intrinsic surfaces are written like operators:

```fol
value as target_type
value cast target_type
```

These are registry-owned now, but they are not implemented in the current `V1`
compiler.

## Current `V1` implemented intrinsics

The current compiler implements this subset end to end through type checking and
lowering.

For current `V1`, backend execution of the implemented intrinsic set still goes
through the current runtime layer where policy matters. The runtime contract is
split by the artifact's `fol_model` and active bundled dependency, so the rule
is:

- `core` artifacts must not rely on heap-backed or source-level hosted APIs
- `memo` artifacts may use heap-backed facilities but not source-level hosted
  APIs
- bundled `std` wrappers require a `memo` artifact plus explicit internal
  `standard` dependency

All three API tiers may back executable artifacts. Process entry and
recoverable outcome adaptation are backend-only support, not bundled-std
intrinsics.

In the current implementation that means:

- `.len(...)` uses the runtime length helper
- `.echo(...)` uses the runtime echo hook and formatting contract
- `check(...)` uses the runtime recoverable-result inspection contract
- scalar comparisons and `.not(...)` may lower to native target operations

### Comparison

```fol
.eq(left, right)
.nq(left, right)
.lt(left, right)
.gt(left, right)
.ge(left, right)
.le(left, right)
```

Current `V1` rule:

- `.eq(...)` and `.nq(...)` work on comparable scalar pairs
- `.lt(...)`, `.gt(...)`, `.ge(...)`, and `.le(...)` work on ordered scalar
  pairs

If you call them with the wrong number of arguments or with unsupported type
families, the compiler reports an intrinsic-specific type error.

### Boolean

```fol
.not(flag)
```

Current `V1` rule:

- `.not(...)` accepts exactly one `bol`

### Query

```fol
.len(items)
```

Current `V1` rule:

- `.len(...)` accepts exactly one operand
- the operand must currently be one of:
  - `str`
  - `arr[...]`
  - `vec[...]`
  - `seq[...]`
  - `set[...]`
  - `map[...]`

In the current compiler, `.len(...)` is the only implemented query intrinsic.
Under the runtime model split, array `.len(...)` belongs to `core`, while
string and dynamic-container `.len(...)` belongs to `memo`. It remains
available when a `memo` artifact also declares bundled `std` because the hosted
tier layers on top of `memo`; `.len(...)` does not itself require `std`.

### Diagnostic

```fol
.echo(value)
```

Current `V1` rule:

- `.echo(...)` accepts exactly one argument
- it requires a `memo` artifact with the explicit bundled `std` dependency
- it emits the value through the hosted runtime hook
- it then forwards the same value unchanged

`.echo(...)` belongs to `std`, not `core` or `memo`.

With that build contract, this is valid:

```fol
fun[] main(flag: bol): bol = {
    return .echo(flag)
}
```

### Recoverable and control intrinsics

```fol
check(read_code(path))
panic("fatal")
```

Current `V1` rule:

- `check(expr)` asks whether a recoverable `/ ErrorType` expression failed and
  returns `bol`; that expression may be a direct routine call or, in V3, an
  awaited recoverable eventual
- `panic(...)` aborts control flow immediately

These are described in more detail in the recoverable-error chapter.

## Current `V1` deferred intrinsics

The registry already reserves more names than the compiler implements.

That does **not** mean they work today.

### Reserved but deferred for likely `V1.x`

- `as`
- `cast`
- `assert`
- `.cap(...)`
- `.is_empty(...)`
- `.low(...)`
- `.high(...)`

These are recognized as registry-owned language surfaces, but the current
compiler rejects them with explicit milestone-boundary diagnostics.

### Reserved for later `V2`

- bitwise helpers such as `.bit_and(...)`, `.bit_or(...)`, `.shl(...)`,
  `.shr(...)`, `.rotl(...)`, `.rotr(...)`, `.pop_count(...)`, `.clz(...)`,
  `.ctz(...)`, `.byte_swap(...)`, `.bit_reverse(...)`
- overflow-mode helpers such as `.checked_add(...)`, `.wrapping_add(...)`,
  `.saturating_add(...)`, `.overflowing_add(...)`, and their subtraction forms

These are intentionally reserved now so the language can grow without
accidental user-space name collisions, but they are not part of the current
`V1` compiler.

### Reserved for `V4` interop

- `.de_alloc(...)`

Explicit deallocation is not part of the V3 memory model. Unique heap values
drop implicitly, borrowing uses `[bor]owner` and `[end]borrow`, and typed pointers use
`[ref]value` and `[drf]pointer`. The old dot-root memory spellings are not reserved or
supported aliases.

## Library-preferred surfaces

Some names are kept in the registry roadmap only as placeholders while the
language decides whether they should really stay compiler-owned.

Current examples:

- `.add(...)`
- `.sub(...)`
- `.mul(...)`
- `.div(...)`
- `.abs(...)`
- `.min(...)`
- `.max(...)`
- `.clamp(...)`
- `.floor(...)`
- `.ceil(...)`
- `.round(...)`
- `.trunc(...)`
- `.pow(...)`
- `.sqrt(...)`

The current direction is that many of these may fit better in `core` or `std`
instead of becoming permanent compiler intrinsics.

## Intrinsics are not shell operations

Do not confuse intrinsics with shell syntax such as `nil` and unwrap
`!`.

For example:

```fol
ali MaybeText: opt[str]
ali Failure: err[str]

fun[] unwrap_optional(value: MaybeText): str = {
    return [uwp]value
}

fun[] unwrap_failure(value: Failure): str = {
    return [uwp]value
}
```

That `!` surface is part of shell typing, not the intrinsic registry.

Likewise, recoverable routine calls such as:

```fol
fun[] read_code(path: str): int / str = { ... }
```

and V3 recoverable results produced by `eventual | await` are handled with:

- `check(expr)`
- `expr || fallback`

not with shell unwrap.

## Current compiler truth

The current compiler has one shared intrinsic registry crate:

`fol-intrinsics`

That registry is the source of truth for:

- canonical intrinsic names and aliases
- milestone availability (`V1` / `V2` / `V3`)
- type-checking selection rules
- lowering mode
- backend/runtime role classification

The current runtime companion for implemented `V1` intrinsics is:

`fol-runtime`

- intrinsic names
- aliases
- categories
- current milestone availability
- deferred-roadmap classification
- lowering mode
- backend-facing role

So the short rule is:

- parser recognizes intrinsic syntax
- `fol-intrinsics` owns intrinsic identity
- type checking validates intrinsic calls
- lowering maps them to explicit IR shapes

This page should describe only the subset that is actually implemented, plus
clearly marked deferred surfaces.
