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
    var[mut] pointer: ptr[int] = &value;
    *pointer = 9;
    return *pointer;
};
```

`&value` allocates the pointed-to value and produces a unique pointer. The
backend represents it as `Box<T>`. Unique pointers move on transfer, just like
other unique heap-owned values, and are freed when their owner leaves scope.

`*pointer` dereferences once to the `T` pointee. A unique pointer binding
declared with `var[mut]` supports write-through assignment.

Direct unique-pointer bindings can be dereferenced, but a unique pointer reached
through a record field cannot be dereferenced in V3. That observation needs a
place-aware field projection in lowering; treating the field as an ordinary
value would partially move the pointer merely to read its pointee. Keep the
unique pointer in a direct binding, or use `ptr[shared, T]` when a read-only
pointer field is the intended shape.

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
counting layer is not exposed as another pointer that needs a second dereference.

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
- `*pointer` reads or, for mutable unique pointers, writes through the pointer

Borrowing is compile-time-only and legal in `core`. Pointer type declarations
can be analyzed in `core`, but evaluating `&value` is rejected there because it
allocates.
