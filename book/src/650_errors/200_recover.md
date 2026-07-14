# Recoverable errors

Recoverable errors are part of the current `V1` language contract, but they are
split into two different surfaces:

- `T / E` for routine-call handling
- `err[...]` for normal storable values

Those are intentionally not the same thing.

## `T / E` is immediate call-site handling

Use `/ ErrorType` after the success type:

```fol
fun read_code(path: str): int / str = {
    when(path == "") {
        case(true) { report "missing path" }
        * { return 7 }
    }
}
```

This means:

- success yields `int`
- `report expr` exits through the routine error path with `str`
- the call result is not a storable plain value

`report expr` must match the declared error type:

```fol
fun read_code(path: str): int / str = {
    report "missing path"
}
```

The following is invalid because the reported value is the wrong type:

```fol
fun read_code(path: str): int / str = {
    report 1
}
```

## No plain propagation

In current `V1`, `/ ErrorType` routine results do not flow through ordinary
expressions.

These are rejected:

```fol
var value = read_code(path)
return read_code(path)
consume(read_code(path))
read_code(path) + 1
```

`/ ErrorType` must be handled immediately at the call site.

## `check(...)`

`check(expr)` asks whether a `/ ErrorType` routine call failed.

It returns `bol`.

```fol
fun main(path: str): bol = {
    return check(read_code(path))
}
```

`check(...)` works on recoverable routine calls, not on `err[...]` shell values.

## `||`

`expr || fallback` handles a `/ ErrorType` routine call immediately.

Rules:

- if `expr` succeeds, use its success value
- if `expr` fails, evaluate `fallback`
- `fallback` may:
  - provide a default value
  - `report`
  - `panic`

Examples:

```fol
fun with_default(path: str): int = {
    return read_code(path) || 0
}

fun with_context(path: str): int / str = {
    return read_code(path) || report "read failed"
}

fun must_succeed(path: str): int = {
    return read_code(path) || panic "read failed"
}
```

## `err[...]` is the storable error form

`err[...]` is a normal value type.

You may store it, pass it, return it, and unwrap it later:

```fol
ali Failure: err[str]

fun keep(value: Failure): Failure = {
    return value
}

fun unwrap(value: Failure): str = {
    return value!
}
```

This is different from:

```fol
fun read_code(path: str): int / str = { ... }
```

A call to `read_code(...)` is not an `err[str]` value. If you need a storable
error container, use `err[...]`. If you use `/ ErrorType`, handle it with
`check(...)` or `||`.

## Current V1 boundary

The current compiler supports:

- declared routine error types with `/`
- `report expr`
- `check(expr)`
- `expr || fallback`
- `err[...]` shell/value behavior

The current compiler rejects:

- plain assignment of `/ ErrorType` call results
- direct returns of `/ ErrorType` call results
- implicit conversion from `/ ErrorType` into `err[...]`
- postfix `!` on `/ ErrorType` routine calls

For backend work:

- `/ ErrorType` routine calls lower through the recoverable runtime ABI
- `err[...]` remains a separate shell/value runtime type
- a recoverable executable entry is adapted through the shared backend-only
  `fol_runtime::process` seam in every runtime model; it does not require
  bundled `std`

Those two categories are intentionally not merged.
