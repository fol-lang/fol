# Casting

Casting is the explicit side of value conversion.

The long-term language direction is:

- coercion stays implicit and narrow
- casting stays explicit and source-visible

For the current `V1` compiler milestone, casting syntax is parsed, but casting
semantics are not implemented yet.

That means:

- `value as target`
- `value cast target`

are both valid syntax surfaces, but they are not part of the supported `V1`
type system.

The current compiler behavior is explicit:

- it does not silently reinterpret these expressions
- it does not treat them as ordinary coercions
- it reports them as unsupported `V1` typecheck surfaces

Example:

```fol
fun[] bad_as(value: int): int = {
    return value as text;
};

fun[] bad_cast(value: int): int = {
    return value cast target;
};
```

Both forms currently fail during typechecking.

This boundary is intentional.
Before FOL can support casting for real, the compiler needs a stable legality
contract answering questions such as:

- which scalar casts are allowed
- whether lossy casts are permitted
- whether container casts exist
- how aliases interact with explicit conversion
- how future foreign/ABI types participate in conversion

That last point is deliberately later work:

- C ABI and Rust interop are planned `V4` features
- casting rules for foreign or ABI-facing types should be specified together
  with that `V4` interop contract, not guessed earlier

Until that contract exists, `V1` treats cast syntax as parsed-but-unsupported
instead of guessing semantics.

## Integer widths

Integer widths never mix implicitly. `i32` and `int` are different types, and
adding them is a type error rather than a silent promotion:

```fol
var small: i32 = 1;
var big: int = 2;
var sum: int = small + big;   // rejected: 'i32' and 'int'
```

`.widen(...)` converts a value to a wider type. It takes its result width from
the context, so the binding or parameter it fills decides the target:

```fol
var small: i32 = 2000000000;
var wide: i64 = .widen(small);

var byte: u8 = 200;
var signed: i16 = .widen(byte);   // every u8 fits an i16
```

Widening is checked in both directions rather than assumed. The compiler
refuses a conversion that could lose value, one where nothing could be lost, and
one with no target to read:

```text
'i32' does not hold every 'int', so this is not a widening
the value is already 'i32', so it needs no conversion
'.widen(...)' takes its result width from the context
```

"Wider" means the target holds every value of the source, not that it has more
bits. `u8` widens to `i16` because every byte is a valid `i16`; `i32` does not
widen to `u32`, and `u32` does not widen to `i32`, because each holds values the
other cannot.

`.narrow(...)` converts the other way. It can lose the value, so it reports
through the ordinary error channel and the failure has to be handled:

```fol
var wide: i64 = 5000000000;
var fitted: i32 = .narrow(wide) || 0;   // 5000000000 does not fit, so 0
```

The direction is checked here too. Narrowing to a type that holds every value
of the source is refused, because nothing could be lost:

```text
'int' holds every 'i32', so nothing can be lost; use '.widen(...)'
```
