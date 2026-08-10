# Coercion

The book-level idea of coercion is simple:

- coercion is implicit
- it should only happen when the conversion is safe and unambiguous

For the current `V1` compiler milestone, that contract is intentionally narrow.

`V1` coercion currently allows:

- exact type matches
- alias-wrapped values whose apparent type matches the expected type
- `never` flowing into any expected type
- the current optional/error shell lifting used by ordinary `V1` surfaces

`V1` coercion currently does not allow:

- implicit `int -> flt`
- implicit `flt -> int`
- implicit width or signedness changes
- implicit container reshaping
- implicit string/character/container conversions

So today the rule is:

- if two values are not already the same semantic family, the compiler should
  reject the implicit conversion

This is deliberate.
The conversion chapter in the language design is broader than the current
compiler guarantee, and `V1` chooses explicit rejection over silent guessing.

Examples that are accepted in `V1`:

```fol
var count: int = 1
var ratio: flt = 1.5

fun[] take_int(value: int): int = {
    return value;
};

fun[] take_float(value: flt): flt = {
    return value;
};
```

Examples that are rejected in `V1`:

```fol
var count: int = 1.5
var ratio: flt = 1
```

```fol
fun[] take_float(value: flt): flt = {
    return value;
};

fun[] bad(): flt = {
    return take_float(1);
};
```

Later versions may widen this contract, but `V1` keeps coercion intentionally
small so type behavior stays predictable.
