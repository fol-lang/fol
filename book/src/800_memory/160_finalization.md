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

A finalizer runs for a value that a binding, a parameter or a `dfr`/`edf`
capture owns — directly, or through a record field. A record names what it
holds, so at scope exit its owner walks the field path and runs each contained
finalizer, deepest field first:

```fol
typ Pair: rec = { left: File, right: File };

var pair: Pair = { left = { descriptor = 1 }, right = { descriptor = 2 } };
// at scope exit: right is finalized, then left
```

Ownership still decides who finalizes. Moving the holder moves the duty with it,
so the receiving routine releases the fields at *its* scope exit and the
original owner does not.

A container finalizes what it holds too. It has no field name for each value,
so at scope exit it hands out its elements one by one instead:

```fol
var pool: vec[File] = { { descriptor = 1 }, { descriptor = 2 } };
// at scope exit: every element is finalized, in order
```

That covers `vec`/`arr`/`seq` elements, `set` members, `map` keys and values,
and `opt`/`err` payloads — including a container reached through a record field.

What is still refused is a `fin` value buried *below* one of those positions:
inside a container's element, an entry variant, or a generic argument. Reaching
those would need a field walk per element at scope exit, which the compiler
cannot emit, so the program is rejected rather than skipping the cleanup:

```fol
typ Holder: rec = { file: File };

var many: vec[Holder] = { { file = { descriptor = 3 } } };
// rejected: the 'fin' value sits below the element, out of reach
```

Hold such a value directly, or give it its own binding and transfer it with
`[mov]`.

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
