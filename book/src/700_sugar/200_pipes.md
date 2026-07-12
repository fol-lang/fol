# Pipes

Pipes connect the value on the left to the expression on the right.

The basic idea is still:

```fol
left | right
```

where the right-hand side sees the left-hand side as `this`.

## Ordinary value piping

Current boundary:

- the ordinary value-pipe (`left | ... this`) shown in this section is later
  design work, not part of the current compiler surface
- the recoverable-call surfaces below (`check(...)` and `||`) are the current
  compiler surface

Use `|` when you want to continue transforming a normal value:

```fol
fun add(x: int, y: int): int = {
    return x + y
}

fun main(): int = {
    return add(4, 5) | when(this > 8) {
        case(true) { 6 }
        * { 0 }
    }
}
```

This is ordinary value flow. The pipe itself does not create a special error
model.

## Recoverable calls are separate from ordinary pipes

Routines declared with `/ ErrorType` produce recoverable call results:

```fol
fun read_code(path: str): int / str = {
    when(path == "") {
        case(true) { report "missing path" }
        * { return 7 }
    }
}
```

For these calls, the current `V1` compiler does **not** treat plain `|` as the
main error-handling tool.

Instead, the implemented recoverable-call surfaces are:

- `check(expr)`
- `expr || fallback`

`check` and `panic` are compiler intrinsics in the current `V1` compiler. They
are not imported library functions.

## `check(expr)`

`check(expr)` asks whether a recoverable routine call failed.

It returns `bol`.

```fol
fun main(path: str): int / str = {
    when(check(read_code(path))) {
        case(true) { report "read failed" }
        * { return 0 }
    }
}
```

This is the current `V1` inspection surface for recoverable calls.

## `||` fallback

Double-pipe is the current shorthand for recovery:

```fol
fun read_code(path: str): int / str = {
    when(path == "") {
        case(true) { report "missing path" }
        * { return 7 }
    }
}

fun with_default(path: str): int = {
    return read_code(path) || 5
}
```

Meaning:

- if the call succeeds, use the success value
- if the call fails, evaluate the right-hand side

The fallback may be:

- a default value
- `report ...`
- `panic ...`

Examples:

```fol
fun recover(path: str): int = {
    return read_code(path) || 5
}

fun re_report(path: str): int / str = {
    return read_code(path) || report "read failed"
}

fun must_succeed(path: str): int = {
    return read_code(path) || panic "read failed"
}
```

## What plain `|` does not mean in current V1

The current compiler does not claim that plain `|` automatically forwards both
the success value and the error to the next stage.

That older description is too broad for the current implementation.

For recoverable calls in `V1`, use `check(expr)` or `expr || fallback`, and
treat ordinary `|` as value piping rather than the main recoverable-error
mechanism.
