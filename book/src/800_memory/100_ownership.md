# Ownership

FOL's V3 memory model uses compile-time ownership. It does not add a garbage
collector, runtime ownership tags, or user-defined destructors.

V3 ships unique heap ownership, move checking, lexical borrowing, and typed
unique/shared pointers.

## Stack and heap bindings

Bindings live on the stack unless heap allocation is explicit:

```fol
var stack_value: int = 64;
var[new] heap_value: int = 64;
@var other_heap_value: int = 64;
```

`@var` is sugar for a binding with `[new]`. Heap allocation requires the
`memo` capability model or bundled `std`; ownership checking itself is a
compile-time rule and is also active in `core`.

There is no `[@]` binding option and no manual `.de_alloc()` or `.free()`.
Unique heap values lower to Rust `Box<T>` and are freed implicitly when their
owning scope ends.

## Assignment and ownership transfer

Assignment and rebinding use the value's storage class:

- stack values clone, so the source and destination remain usable
- heap-owned values move, so the source becomes inaccessible

```fol
fun[] main(): int = {
    var stack_a: int = 2;
    var stack_b: int = stack_a;

    @var heap_a: int = 3;
    @var heap_b: int = heap_a;

    return stack_a + stack_b;
};
```

A later read of `heap_a` is a compile error in the `O####` OWNERSHIP diagnostic
family. The diagnostic points at both the invalid use and the transfer site.
The backend emits stack transfers with `.clone()` and unique heap transfers as
ordinary Rust moves. No runtime tag decides which operation occurs.

Transferring a move-only record field consumes the whole source binding. V3
does not leave a partially moved record available through its other fields.
Moving a value out through an array, vector, sequence, or map index remains
unsupported because those containers need an explicit removal operation rather
than a clone-based read.

A `when` result transfers the final value of the selected branch into its join
value. Branches are checked from the same incoming ownership state, so the same
owner may be transferred by mutually exclusive alternatives. After the `when`,
every owner that could have been transferred by a continuing branch is treated
as moved.

Loop bodies are lexical scopes that execute once per iteration. A move-only
binding declared outside a repeating loop cannot be transferred from the loop
body or its repeated condition: a later iteration would try to consume the
same value again. Create the move-only value inside the loop when each
iteration needs a fresh owner, or transfer it after the loop. A `return` that
transfers a value is allowed because it exits the routine instead of reaching
another iteration.

Deferred bodies also participate in ownership checking. When a `dfr` or `edf`
body references a move-only binding from an enclosing scope, that binding is
reserved until the registration scope exits and the selected deferred work has
run. A later transfer in the same scope is rejected with both the transfer and
deferred-use locations. If the deferred block belongs to a nested scope, the
reservation ends with that nested scope.

## Recursive owned data

Owned heap indirection gives recursive types a finite runtime shape:

```fol
typ Node: rec = {
    value: int,
    next: opt @Node,
};
```

In type position, `@Node` means an owned heap `Node`. The example above lowers
to one nominal Rust structure whose recursive field has the shape
`FolOption<Box<Node>>`.

A recursive value edge without owned indirection is still invalid:

```fol
typ Bad: rec = {
    next: Bad,
};
```

Such a type has no finite value layout. The compiler recommends an owned edge
such as `opt @Bad` instead.

## Lexical borrowing

A borrow is an alias that does not take ownership:

```fol
var owner: Node = { value = 7, next = nil };
var[bor] view: Node = owner;
var value: int = view.value;
```

`#owner` is expression sugar for borrowing from an owner:

```fol
var[bor] view: Node = #owner;
```

Borrowing is scope-granular. A borrow remains active until the end of the
lexical scope where its binding was created. FOL deliberately does not use
flow-sensitive or non-lexical lifetime analysis in V3.

While a borrow is active:

- the owner cannot be read, written, moved, or borrowed incompatibly
- shared borrowers are read-only
- any mutable borrower excludes every other borrower of that owner

Leaving the scope returns access to the owner automatically:

```fol
var owner: Node = { value = 7, next = nil };
{
    var[bor] view: Node = owner;
    var value: int = view.value;
};
return owner.value;
```

`!view` gives a borrow back before scope exit. A returned borrow cannot be used
again:

```fol
var[bor] view: Node = owner;
var value: int = view.value;
!view;
return owner.value;
```

The `!` prefix is give-back only. It is not deletion or manual memory free.

V3 does not support reborrowing a borrow binding. Borrow from the original owner
or pass an existing borrow binding directly to a `[bor]` parameter instead.

## Mutable borrowing

A mutable borrow requires both an explicitly mutable owner and an explicitly
mutable borrow binding:

```fol
var[mut] owner: Node = { value = 7, next = nil };
var[mut, bor] view: Node = owner;
view.value = 9;
!view;
return owner.value;
```

`var[bor]` is immutable unless `mut` is also present. Attempting a mutable
borrow from `var[imu]`, or creating another borrow while a mutable borrow is
active, is an `O####` ownership diagnostic.

## Borrow parameters

Routine parameters use the same explicit option position:

```fol
fun[] inspect(item[bor]: Node): int = {
    return item.value;
};
```

The backend emits a Rust reference parameter. The argument is borrowed for the
call and returned when the call completes. Parameter spelling or casing never
changes ownership semantics; `name[bor]: T` is the only borrow-parameter form.

Borrowing is compile-time-only and legal in `core`. It does not allocate or add
runtime reference bookkeeping.
