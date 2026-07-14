# Pointers

FOL V3 has typed unique and shared pointers. Pointer construction allocates, so
it requires the `memo` capability model. A `memo` artifact with bundled `std`
remains heap-capable, but pointer construction does not itself require hosted
APIs.

## Unique pointers

`ptr[T]` is a uniquely owned pointer to `T`:

```fol
fun[] main(): int = {
    var value: int = 7;
    var inner: ptr[int] = &value;
    var outer: ptr[ptr[int]] = &inner;
    var[mut] extracted: ptr[int] = *outer;
    *extracted = 9;
    return *extracted;
};
```

`&value` allocates the pointed-to value and produces a unique pointer. The
backend represents it as `Box<T>`. Unique pointers move on transfer, just like
other unique heap-owned values, and are freed when their owner leaves scope.
Constructing a pointer from a move-only value transfers that value into the new
allocation.

`*pointer` is a by-value dereference to the `T` pointee:

- if `T` is clone-safe, dereference clones `T` and leaves the pointer usable
- if `T` is move-only, dereferencing a unique pointer transfers `T` out and
  consumes that pointer

The example consumes `outer` because its pointee is another unique pointer.
`extracted` then owns that inner pointer. A direct unique-pointer binding
declared with `var[mut]` supports write-through assignment such as
`*extracted = 9`.

Direct unique-pointer bindings can be dereferenced, but a unique pointer reached
through a record field cannot be dereferenced in V3. That observation needs a
place-aware field projection in lowering; treating the field as an ordinary
value would partially move the pointer merely to read its pointee. This field
boundary also applies when `T` is clone-safe. Keep the unique pointer in a
direct binding, or use `ptr[shared, T]` with a clone-safe pointee when a
read-only pointer field is the intended shape.

## Shared pointers

`ptr[shared, T]` is a reference-counted shared pointer:

```fol
var value: int = 7;
var first: ptr[shared, int] = &value;
var second: ptr[shared, int] = first;
return *first + *second;
```

The backend represents shared pointers as `Rc<T>`. Assigning one clones the
reference count, so `first` and `second` refer to the same allocation. Shared
pointers are read-only; write-through is rejected.

A single `*p` yields `T` for both unique and shared pointers. The reference
counting layer is not exposed as another pointer that needs a second
dereference. Because a shared pointer cannot remove the value from all of its
aliases, this read is available only when `T` is clone-safe. Dereferencing
`ptr[shared, ptr[int]]`, for example, is rejected rather than cloning or moving
the unique inner pointer.

## Borrowed pointers

A pointer can be borrowed like any other owned value:

```fol
fun[] read(pointer[bor]: ptr[int]): int = {
    return *pointer;
};
```

The borrowed pointer is a non-owning, read-only view. It can be passed directly
to another compatible `[bor]` parameter and reused for later calls. Dereference
can clone a clone-safe pointee such as `int`, but it cannot move a move-only
pointee through the borrow. A borrowed `ptr[ptr[int]]` therefore cannot produce
the inner `ptr[int]` by value. Write-through also requires a direct mutable
unique-pointer binding; a borrowed pointer is not such a binding.

## Shared recursive graphs

Shared pointer indirection gives recursive graph edges a finite layout:

```fol
typ Node: rec = {
    value: int,
    next: opt ptr[shared, Node],
};
```

This lowers to an optional `Rc<Node>` edge. Shared recursion is legal, but V3
does not ship weak references or a cycle collector. A reference cycle therefore
leaks. Programs should keep shared structures acyclic unless a later interop
milestone adds an explicit weak-reference contract.

`Rc` is not thread-safe and cannot cross a processor spawn boundary. The V3
processor pillar enforces that boundary.

## Raw pointers are out

`ptr[raw, T]` is reserved but rejected with an explicit V4 interop diagnostic.
V3 does not provide raw pointer construction, manual `.free()`, unsafe delete,
or user-defined destructors. The `!x` prefix remains borrow give-back only; it
never deletes a pointer.

## Sigil alignment

- `#owner` creates a lexical borrow without allocation
- `!borrow` gives that borrow back early
- `&value` constructs a typed allocating pointer
- `*pointer` clones a clone-safe pointee, consumes a unique pointer when its
  pointee is move-only, or writes through a direct mutable unique pointer

Borrowing is compile-time-only and legal in `core`. Pointer type declarations
can be analyzed in `core`, but evaluating `&value` is rejected there because it
allocates.
