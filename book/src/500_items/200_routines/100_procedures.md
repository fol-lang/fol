# Procedures

Procedures are most common type of routines in Fol. When a procedure is "called" the program "leaves" the current section of code and begins to execute the first line inside the procedure. Thus the procedure "flow of control" is:

- The program comes to a line of code containing a "procedure call".
- The program enters the procedure (starts at the first line in the procedure code).
- All instructions inside of the procedure are executed from top to bottom.
- The program leaves the procedure and goes back to where it started from.
- Any data computed and RETURNED by the procedure is used in place of the procedure in the original line of code.

Procedures have side-effects, it can modifies some state variable value(s) outside its local environment, that is to say has an observable effect besides returning a value (the main effect) to the invoker of the operation. State data updated "outside" of the operation may be maintained "inside" a stateful object or a wider stateful system within which the operation is performed.

Current milestone note:

- ordinary procedure declarations are part of `V1`
- recoverable procedure errors (`Result / Error`) are part of `V1`
- ownership-aware transfers, unique heap moves, and explicit `[bor]` parameters
  are part of the shipped `V3` memory contract
- `[mux]` parameters and the direct named-routine task boundary are part of the
  shipped `V3` processor contract

Pointer and borrowing examples in this chapter therefore describe current V3
behavior when the artifact's capability mode permits the underlying value.
Broader indirect task dispatch and future ownership/interop designs remain
outside that shipped surface.

Procedures can also declare a custom recoverable error type with `/` after the result type:

```
pro[] write(path: str): int / io_err = {
    report "permission denied";
}
```

The first `:` declares the result type, and `/` declares the routine error type.

Current `V1` note:

- a procedure declared as `pro[] write(...): T / E` does not produce an
  `err[E]` shell value that can be unwrapped with `!`
- it produces a recoverable routine result with a success path and an error path
- use `check(...)` or `expr || fallback` at the call site
- keep postfix `!` for `opt[...]` and `err[...]` shell values only

### Passing values

Procedure parameters are ordinary typed inputs unless the parameter has an
explicit ownership option. Values passed to ordinary parameters follow the
type's transfer rule. A parameter written as `name[bor]: T` borrows its
argument for the call without taking ownership.

Simple example:

```fol
pro[] inspect(value[bor]: int): int = {
    return value;
};

pro[] main(): int = {
    var value: int = 7;
    return inspect(value);
};
```

### Ownership and borrowing

The authoritative version split is:

- ordinary procedures and functions are part of `V1`
- recoverable routine errors are part of `V1`
- explicit `name[bor]: T` borrow parameters are part of the shipped `V3`
  memory contract
- typed pointers and unique heap ownership are also part of that `V3` contract

Borrow give-back uses the `!borrow` prefix. Ownership behavior never depends on
identifier casing, and there is no alternate parenthesized parameter grammar.

See the memory chapters and `plan/VERSIONS.md` for the version boundary.
