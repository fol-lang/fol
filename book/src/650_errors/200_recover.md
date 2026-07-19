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
read_code(path)
var value = read_code(path)
return read_code(path)
consume(read_code(path))
read_code(path) + 1
```

`/ ErrorType` must be handled immediately at the call site. Evaluating a
recoverable call as a standalone statement does not count as handling it.

The same rule applies after V3 `| async`: a recoverable eventual must be
awaited, and the awaited result must be consumed by `check(...)` or `||`.
Moving the eventual to another binding moves that obligation; dropping,
overwriting, or leaving the final binding at scope exit is rejected. An
infallible eventual has no recoverable error obligation and may remain
unawaited, although it is still joined at process exit.

The obligation must be discharged before lexical fallthrough and before a
`break`, `return`, or `report` exits any scope that still owns it. At a branch
join, all continuing branches must leave compatible obligation state: all may
preserve the same owner for handling after the join, transfer consistently, or
await and handle it. Consuming or transferring on only some paths is rejected.
Process-exit joining waits for work; it does not count as handling a recoverable
error.

## `check(...)`

`check(expr)` asks whether a `/ ErrorType` expression failed. The expression may
be a direct recoverable routine call or a V3 awaited recoverable eventual.

It returns `bol`.

```fol
fun main(path: str): bol = {
    return check(read_code(path))
}
```

`check(...)` works on direct recoverable routine calls and on expressions such
as `pending | await` when `pending` carries a recoverable result. It does not
inspect `err[...]` shell values.

## `||`

`expr || fallback` handles a `/ ErrorType` expression immediately. That may be
a direct recoverable routine call or a V3 awaited recoverable eventual.

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

`err[...]` is a normal value type. It is a nil-able shell: `nil` is the
no-error (success) state and a present value carries a stored error payload.
`return nil` produces the no-error state; `return payload` stores an error.

You may store it, pass it, return it, and unwrap it later:

```fol
ali Failure: err[str]

fun keep(value: Failure): Failure = {
    return value
}

fun unwrap(value: Failure): str = {
    return [uwp]value
}
```

Because it is nil-able, an `err[T]` scrutinee also drives `when ... on ... *`:
`on(payload)` binds the stored error and `*` takes the `nil` (no-error) branch.
Postfix `[uwp]value` and inner-place `value[]` unwrap the stored error and panic on
`nil`.

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

That process adapter translates the entry outcome for the generated host
wrapper. It is not importable source API, does not promote `core` or `memo` to
the hosted tier, and is present for execution support regardless of whether the
program declares bundled `std`.

Those two categories are intentionally not merged.
