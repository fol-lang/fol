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
}

fun[] bad_cast(value: int): int = {
    return value cast target;
}
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
