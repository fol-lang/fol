# Methods

A method is a routine associated with a receiver type. In FOL, methods can be
declared as either `fun` or `pro` and called with dot syntax
(`value.method(...)`).

Methods in FOL are procedural sugar, not object-oriented behavior.

A method call is just sugar for calling a routine whose first explicit input is
the receiver value. In other words:

```fol
tool.parse_msg(10)
```

should be read as the procedural call:

```fol
parse_msg(tool, 10)
```

This syntax does not introduce:

- classes
- inheritance
- object-owned method bodies
- object-method dispatch as a separate runtime model

`typ` declares data. Receiver-qualified routines are still ordinary routines.

Current parser-supported receiver declaration syntax is:

```fol
fun (parser)parse_msg(code: int): str = {
    return "ok";
}

pro (parser)update(code: int): int = {
    return code;
}
```

The receiver type appears in parentheses right after `fun` or `pro`, followed by the method name.

That receiver clause does not move the routine "inside" the type. It only says
which type may be used in dot-call form for that routine.

The current parser accepts a broader receiver syntax than the final V1
semantic subset. At parse time, receiver positions can still accept named,
qualified, builtin-scalar, and bracketed/composite type references. That is a
parser fact, not an object model.

The dedicated parser-level rejection in this hardening phase is still for special
builtin forms such as `any`, `none`, and `non`.

Invalid example:

```fol
fun (any)parse_msg(code: int): str = {
    return "ok";
}
```

This form reports: `Method receiver type cannot be any, non, or none`.

Method calls use standard dot syntax:

```fol
var tool: parser = parser.new()
var msg: str = tool.parse_msg(10)
```

This is equivalent in meaning to passing `tool` as the first routine argument.
The dot form is only the call-site spelling.

Method calls use the same `V1` call-binding rules as free routine calls:

- ordinary positional arguments may appear before named arguments
- named arguments bind by declared parameter name
- defaulted parameters may be omitted
- a final variadic parameter may collect trailing arguments
- variadic collection comes only from the explicit `... T` marker; a plain
  `seq[T]` parameter takes one sequence value
- `...sequence` may appear after named arguments when it feeds that final
  variadic parameter

Example:

```fol
typ Counter: rec = {
    total: int
};

fun (Counter)shift(step: int = 2, extras: ... int): int = {
    return step;
}

fun[] run(current: Counter, extras: seq[int]): int = {
    return current.shift(step = 3, ...extras);
}
```

`V1` does not support indexed pseudo-arguments such as
`current.shift(extras[0] = 1, ...extras)`. That spelling looks like assignment,
not call binding, so the language keeps variadic binding explicit and simple.

Inside a receiver-qualified routine body, the receiver value is available
through `self`. `self` is typed as the receiver type and is lowered as the
routine's ordinary first argument; it does not introduce an object model.

```fol
typ Counter: rec = {
    var total: int;
};

fun (Counter)read(): int = {
    return self.total;
}
```

Generic receiver types participate too: `fun (Box[T])get(T)(): T` can return
`self.value`, and each concrete call site monomorphizes the routine.

For record-focused V1 code, the intended reading is:

- records hold data
- routines stay separate
- the receiver clause enables `value.method(...)` spelling and gives the body
  procedural access to the receiver through `self`

Custom error routines also support reporting method call results when receiver-qualified signatures are available:

```fol
fun (parser)parse_err(code: int): str = {
    return "bad-input";
}

fun run(tool: parser, code: int): int / str = {
    report tool.parse_err(code)
    return 0
}
```
