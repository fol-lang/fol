# Ownership

FOL's V3 memory model uses compile-time ownership. It does not add a garbage
collector, runtime ownership tags, or user-defined destructors.

V3 ships unique heap ownership, move checking, lexical borrowing, and typed
unique/shared pointers. Transfer behavior follows the value type, not merely
the binding's location: clone-safe values are copied by cloning, while unique
ownership and aggregates containing unique ownership move.

## Stack and heap bindings

Bindings live on the stack unless heap allocation is explicit:

```fol
var stack_value: int = 64;
var[new] heap_value: int = 64;
@var other_heap_value: int = 64;
```

`@var` is sugar for a binding with `[new]`. Heap allocation requires the
`memo` capability model. A `memo` artifact with bundled `std` remains
heap-capable, but allocation does not itself require hosted APIs. Ownership
checking is a compile-time rule and is also active in `core`.

There is no `[@]` binding option and no manual `.de_alloc()` or `.free()`.
Unique heap values lower to Rust `Box<T>` and are freed implicitly when their
owning scope ends.

## Assignment and ownership transfer

Assignment and rebinding use the value's ownership class:

- clone-safe values clone, so the source and destination remain usable
- heap-owned values and unique pointers move, so the source becomes
  inaccessible
- an aggregate is move-only when it contains a move-only value
- a shared pointer clones its reference count instead of moving its source

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
The backend emits clone-safe transfers with `.clone()` and unique transfers as
ordinary Rust moves. No runtime tag decides which operation occurs.

Evaluating a move-only value as a standalone expression is also a transfer even
when its result is discarded. Lowering still evaluates that expression, so the
source cannot be used afterward. Clone-safe scalar expression statements remain
usable because their values clone.

A whole mutable binding can be reinitialized after its old value moves:

```fol
var[mut] pointer: ptr[int] = [ref]first;
consume(pointer);
pointer = [ref]second;
return [drf]pointer;
```

The assignment target is a storage place, not a read of the missing old value.
The right-hand side is checked and transferred first, then a successful store
makes the binding usable again. Ordinary mutability and active-borrow rules
still apply. Self-assignment of a live mutable owner transfers the value through
the assignment and leaves the same binding initialized. Assignment is a
statement and does not yield a second copy of the stored value.

Transferring a move-only record field consumes the whole source binding. V3
does not leave a partially moved record available through its other fields.
Any indexed read whose element or map value is move-only remains unsupported,
including selecting a clone-safe field from that element afterward. Index access
materializes the whole element, so these containers need an explicit removal
operation rather than a clone-based read.

Container queries are observations, not transfers. `.len(local)` and indexed
lookup through direct locals preserve the receiver and map key, including when
their types contain unique pointers. A move-only receiver or key reached
through a field projection is not yet observable because V3 has no place-aware
projection IR; such a lookup is rejected instead of partially moving its
source. The same boundary rejects nested field access through a move-only
intermediate.

Dereferencing is a by-value operation whose ownership effect depends on the
pointee. `[drf]pointer` clones a clone-safe pointee and leaves the pointer usable.
If the pointee is move-only, dereferencing a direct unique `ptr[T]` transfers
the pointee and consumes that pointer. A shared or borrowed pointer cannot
surrender a move-only pointee, so those dereferences are rejected; their
read-only dereference is available only when the pointee is clone-safe.

Dereferencing a unique-pointer field is also a place observation, not a whole
field transfer. Until V3 has place-aware projection IR, `[drf]record.pointer` is
rejected even when its pointee is clone-safe; otherwise backend lowering would
partially move the pointer just to observe its target. Direct pointer bindings
remain available. A field containing `ptr[shared, T]` can be dereferenced when
`T` is clone-safe.

Forward slices currently create a new `vec[...]` or `seq[...]` by cloning the
selected elements. Their element type must therefore be clone-safe. Fixed-size
array slices need a distinct runtime-sized result contract and remain
unsupported. Ordinary collection iteration is also index-based and clone-based,
so a collection containing move-only elements cannot use it yet. Channel
receiver iteration is different: it consumes each payload and may carry
move-only values.

Top-level storage is limited to clone-safe, non-borrowed values that are safe in
the backend's shared static cells. Move-only values, borrowed values, full
channels, and values containing `ptr[shared, T]` must be declared inside a
routine. This prevents global loads from duplicating unique ownership, extending
a lexical borrow to static lifetime, or placing `Rc` in thread-safe storage.

A `when` result transfers the final value of the selected branch into its join
value. Branches are checked from the same incoming ownership state, so the same
owner may be transferred by mutually exclusive alternatives. After the `when`,
every owner that could have been transferred by a continuing branch is treated
as moved.

Value-matching `when` arms use the same equality contract as `==`: selector and
case values must have the same equality-safe scalar type. Pointer, record, and
other move-only selectors are rejected rather than being moved by an implicit
comparison. A `when` without value arms is instead a boolean control gate, so it
also rejects move-only selectors rather than evaluating and silently moving an
unused owner.

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

Reinitializing an already-moved binding inside `dfr` or `edf` is rejected in
V3. The assignment runs only at scope exit, so applying its ownership effect
when the deferred body is registered would unsafely expose the binding before
the new value exists.

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

## Borrowing

A borrow is an alias that does not take ownership:

```fol
var owner: Node = { value = 7, next = nil };
var[bor] view: Node = owner;
var value: int = view.value;
```

`[bor]owner` borrows from an owner as an expression:

```fol
var[bor] view: Node = [bor]owner;
```

Borrowing is non-lexical. A borrow stays active only until its last use, not
until the end of the lexical scope where its binding was created. Once a loan's
final read has passed, the owner becomes usable again in the same scope with no
explicit give-back.

While a borrow is active:

- the owner cannot be read, written, moved, or borrowed incompatibly
- an immutable borrower is read-only
- a borrowed value cannot transfer move-only data out of its owner; clone-safe
  observations remain available
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

`[end]view` gives a borrow back before scope exit. A returned borrow cannot be used
again; attempting to read `view` afterward reports `O2004` with the give-back
site as related information:

```fol
var[bor] view: Node = owner;
var value: int = view.value;
[end]view;
return view.value;
```

The `!` prefix is give-back only. It is not deletion or manual memory free.

A borrow binding may be reborrowed: `[bor]view` creates a nested loan from an
existing borrow, released like any other loan by its last use or an explicit
`[end]`. A reborrow cannot outlive the loan it derives from.

## Mutable borrowing

A mutable borrow requires both an explicitly mutable owner and an explicitly
mutable borrow binding:

```fol
var[mut] owner: Node = { value = 7, next = nil };
var[mut, bor] view: Node = owner;
view.value = 9;
[end]view;
return owner.value;
```

`var[bor]` is immutable unless `mut` is also present. A `var[mut, bor]` binding
may update the original owner, but it still does not own the value and cannot
move unique data out of it. Attempting a mutable borrow from `var[imu]`, or
creating another borrow while a mutable borrow is active, is an `O####`
ownership diagnostic.

## Borrow parameters

Routine parameters use the same explicit option position:

```fol
fun[] inspect(item[bor]: Node): int = {
    return item.value;
};
```

The backend emits a Rust reference parameter. Passing an owner requires an
explicit `[bor]owner`; an existing compatible borrow binding can be passed directly
and reused by later `[bor]` calls. The call borrow ends when the call returns,
while an existing borrow binding keeps its surrounding lexical lifetime.
Parameter spelling or casing never changes ownership semantics;
`name[bor]: T` is the only borrow-parameter form. A borrowed parameter can
observe clone-safe data, but it cannot return or forward a move-only value as
an owned argument.

Borrowing is compile-time-only and legal in `core`. It does not allocate or add
runtime reference bookkeeping.

## Named lifetimes

When a routine returns a borrow, the compiler must know which input the result
may alias. A lifetime parameter `L: lif` names a single region and ties the
borrowed inputs and the borrowed result together:

```fol
fun pick(L: lif)(left: Item[bor=L], right: Item[bor=L]): Item[bor=L] = {
    return left;
};
```

`Item[bor=L]` is a borrow bound to region `L`. The returned borrow is valid for
as long as every input it may alias, so the caller's owners must outlive the
result. A routine that borrows a single input and returns a borrow may leave the
region implicit; naming it is required only when several borrows share one
result region.

## Receiver ownership

A method states how it takes its receiver with the same option position; the
receiver binds to `self`:

```fol
fun (Node[bor])inspect(): int = { return self.value; };
pro (Node[mut, bor])update(): non = { self.value = 9; return; };
fun (Node)consume(): int = { return self.value; };
```

A `fun` may take a shared `[bor]` receiver or move its receiver by value; only a
`pro` may take a mutable `[mut, bor]` receiver. At the call site the ownership
operation binds the receiver directly, with no surrounding parentheses:

```fol
[bor]node.inspect();
[mut, bor]node.update();
[mov]node.consume();
```

`[op]receiver.method()` groups as `([op]receiver).method()`: the operation
describes how the receiver crosses the call boundary, not the call's result, and
a `[mut, bor]` receiver's field updates persist to the original binding. The
rule is scoped to a trailing method call — a plain place chain keeps the
operation over the place, so `[mov]bundle.held` still moves the `held` subfield
rather than `bundle`. The same grouping applies to the bracket unary operations,
so `[drf]pointer.method()` dereferences `pointer` and then calls the method.

## Closure captures

An anonymous routine used as a first-class value declares how outer locals
enter its environment with the same capture list delayed blocks use. The
routine value's type is its visible call signature; the environment re-supplies
the captured values on every invocation:

```fol
var base: int = 30;
var adder: {fun (n: int): int} = fun(n: int)[base[cpy]]: int = { return n + base; };
var first: int = adder(12);
```

`[cpy]` and `[cln]` duplicate the outer value into the environment, leaving the
source live. `[mov]` transfers a clone-safe value and invalidates the outer
binding; moving a move-only value into a routine value is rejected because the
closure may run more than once — spawn the routine directly when the value
should transfer into exactly one execution.

A local, nonescaping closure may also borrow: `[bor]` captures infer their
lifetime as the enclosing scope. The owner stays readable but is frozen — no
mutation, no transfer — while the closure can still run, and the closure value
itself cannot escape that scope: returning it, passing it to a call, storing
it, or rebinding it is rejected. The one sanctioned crossing is a parameter
whose routine type names its environment lifetime — `{fun (): int}[bor=L]`
with a declared `L: lif` — which receives the closure and obeys the same
nonescaping rules inside the callee
(`examples/mem_closure_env_lifetime_m2` and
`examples/fail_mem_closure_env_leak_m2` pin that boundary). Channel-endpoint captures never enter routine
values (`examples/mem_closure_capture_m2`, `examples/mem_closure_borrow_m2`,
`examples/fail_mem_closure_move_only_m2`, and
`examples/fail_mem_closure_escape_m2` pin the contract).
