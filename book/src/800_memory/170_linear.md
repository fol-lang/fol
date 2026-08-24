# Linear resources

A type that claims `lin` holds a resource whose release **can fail, and whose
failure means something**. Closing a file can lose buffered writes; closing a
socket can report that the data never reached the other end; closing a database
handle can refuse while statements are still open. A program that ignores those
statuses loses data silently.

[Finalization](./160_finalization.md) cannot carry such a failure anywhere. A
finalizer runs when a scope ends, and a scope ending has no caller waiting on a
result: there is no expression to return an error to and no `||` to route one
into. This is the wall Rust's `Drop`, C++ destructors, and Go's
`defer f.Close()` all hit. FOL does not copy any of them.

Instead, a `lin` value is never released implicitly. It must be consumed
**exactly once, explicitly, on every path** — which makes the release an
ordinary call whose error propagates like any other.

```fol
typ Handle()(lin, fin): rec = { descriptor: int };

fun (Handle)close(): int / int = {
    when(self.descriptor) {
        is(0) { report 7; }
        * { }
    };
    return self.descriptor;
};
```

## The two consumers

A linear value is consumed by transferring it out, or by discarding the failure
on purpose:

```fol
var handle: Handle = { descriptor = 3 };
var code: int = [mov]handle.close() || 8;   // the release, failure readable

var spare: Handle = { descriptor = 5 };
[fin]spare;                                 // best-effort, failure discarded
```

`[fin]` runs the type's finalizer, which is typed `: non` and so cannot report.
That is the point: throwing the failure away is allowed, but it has to be
spelled, and `[fin]` is greppable when auditing a codebase for ignored release
failures. A `lin` type that does not also claim `fin` has no finalizer to run,
so `[fin]` on it is refused.

Omitting both is a compile error.

## What the compiler proves

Every path out of a scope is checked:

| Situation | Result |
| --- | --- |
| the scope ends still holding one | refused — the plain leak |
| `return` while holding one | refused |
| one `when` arm releases and another does not | refused — the join has no single answer |
| a resource from outside a loop is released inside it | refused — the second iteration would release it again |
| a call returns one and nobody binds the result | refused |
| released twice | refused by the ownership checker, at the first move |
| `[cpy]` or `[cln]` of one | refused — a duplicate owes a release nothing tracks |
| captured by a closure or spawned task | refused — the obligation would leave the scope that owes it |
| released inside `dfr` or `edf` | refused — those run at scope exit, with no caller for a failed release |

A **borrow** of a linear value carries no obligation, because the obligation
stayed with the owner. `fun peek(h: Handle[bor]): int` is an ordinary routine.
An owned `lin` parameter is the opposite: the caller transferred the resource,
so the routine now owes the release.

## Reporting while holding one

A scope that still holds an unreleased linear resource **may not `report`**:

```fol
fun[] work(): int / int = {
    var handle: Handle = { descriptor = 3 };
    when(handle.descriptor) {
        is(3) { report 9; }   // refused: handle is still held
        * { }
    };
    [fin]handle;
    return 7;
};
```

Two errors would exist at once — the reported one and whatever the release
returns — and a routine has one result channel. Picking a winner would discard
the other, which is the data loss the capability exists to prevent. So the
language refuses the case instead of choosing.

This is deliberately the most restrictive of the available answers. Relaxing it
later to an error that can carry both is a widening, and widenings do not break
existing programs; shipping "the body's error wins" first would be a narrowing
to undo.

## Containment

A type holding a `lin` field must itself claim `lin`:

```fol
typ Holder()(lin, fin): rec = { inner: Handle };
```

Without that rule an ordinary record would be a hole in the analysis: wrapping a
handle in a struct would hide it, and the release obligation would vanish at the
moment the field was assigned. Propagating the claim outward keeps the
obligation attached to whatever now owns the resource.

## Examples

- `examples/v4_linear_handle` — both consumers, and a readable release failure
- `examples/fail_v4_linear_leak` — a scope ending while still holding one
- `examples/fail_v4_linear_report` — `report` while holding one
- `examples/fail_v4_linear_branch` — one branch releases, the other does not
