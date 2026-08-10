# Finalization

A type that claims the `fin` capability runs custom cleanup when its owner is
released. The cleanup is a compiler-owned receiver contract: a `pro` named
`finalize` that takes the value by value and returns `non`.

```fol
typ File()(fin): rec = { descriptor: int };

pro (File)finalize(): non = {
    close(self.descriptor);
    return;
};
```

The finalizer is a `pro` because releasing a foreign resource is an effect. A
`fin` type cannot also claim `copy`: a value with cleanup is not trivially
duplicable.

## Scope-exit cleanup

The finalizer runs automatically when the owning scope exits, exactly once per
value:

```fol
fun[] main(): int = {
    var handle: File = { descriptor = 3 };
    return handle.descriptor;
    // handle.finalize() runs here, as the scope exits
};
```

When a value is moved, responsibility for finalization moves with it: the new
owner finalizes it, and the original binding does not.

A `[mov]` capture into a `dfr` or `edf` block is the one move that stays in the
frame. The delayed block replays at scope exit, so the value is finalized right
after the block body runs:

```fol
fun[] main(): int = {
    var handle: File = { descriptor = 3 };
    dfr[handle[mov]] {
        observe(handle.descriptor);
    };
    return 0;
    // the deferred body runs, then handle.finalize()
};
```

## What can own a `fin` value

A finalizer runs only for a value that a binding, a parameter or a `dfr`/`edf`
capture owns directly. A `fin` value nested inside another value — a record
field, a `vec`/`arr`/`seq` element, a `set`/`map` member, an `opt` or `err`
payload — has no such owner, so nothing would ever call its finalizer. The
compiler rejects those positions instead of skipping the cleanup silently:

```fol
var handles: vec[File] = { { descriptor = 3 } };
// rejected: a nested 'fin' value would never be finalized
```

Give each `fin` value its own binding and transfer it with `[mov]`.

## Early finalization

`[fin]value` runs the finalizer immediately and invalidates the source. The
value is finalized exactly once, so scope-exit cleanup does not run a second
time:

```fol
var handle: File = { descriptor = 3 };
observe(handle.descriptor);
[fin]handle;
// handle is now consumed; no second finalize at scope exit
```
