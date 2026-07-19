# Complex

## Strings

`str` is the heap-backed UTF-8 string type. It requires the `memo` capability
model; a `memo` artifact with bundled `std` remains heap-capable. It is not
available in `core`.

```fol
var label: str = "fol";
```

## Number

`num` is a planned abstraction over integer and floating-point types. It is not
part of the current compiler surface. Imaginary-number support is likewise
outside the active stream, lexer, parser, and lowering contract.

## Pointers

V3 ships typed unique and shared pointers:

```fol
var[mut] unique: ptr[int] = [ref]value;
var shared: ptr[shared, int] = [ref]value;
```

`ptr[T]` is uniquely owned and writable through a mutable pointer binding.
`ptr[shared, T]` is reference-counted and read-only. Pointer types can be
analyzed in `core`, but `[ref]value` constructs an allocation and therefore
requires `memo`. A `memo` artifact may additionally declare bundled `std`, but
the pointer operation itself does not require hosted APIs. Raw `ptr[raw, T]`
remains a V4 interop boundary.

See [Pointers](../800_memory/200_pointers.md) for transfer, dereference, shared
recursion, and current place-projection rules.

## Error shells

`err[T]` is the storable error shell:

```fol
var failure: err[str] = "not found";
```

It is distinct from a routine's recoverable `: Result / Error` contract. See
[Recoverable Errors](../650_errors/200_recover.md) for that boundary.
