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
