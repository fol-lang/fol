# Capabilities

Five compiler-owned standards describe what may be done with a value. A type
lists the ones it guarantees in its conformance position, and the compiler
verifies each claim against the type's fields.

| Standard | Meaning |
| --- | --- |
| `copy` | `[cpy]value` may duplicate the value; the source stays usable |
| `clone` | `[cln]value` may create an independent copy; the source stays usable |
| `fin` | the type runs custom finalization when it is dropped |
| `send` | owned access may cross a task or thread boundary |
| `share` | shared access may cross a task or thread boundary |

A type states its capabilities in the conformance list:

```fol
typ Point()(copy): rec = { x: int, y: int };
typ Job()(clone): rec = { input: int };
typ Buffer()(clone, send): rec = { bytes: seq[int] };
```

## Verification

Each claim is checked recursively against the type's fields. `clone` has a
structural default: a type is clonable when every field is clonable, so a
`clone` claim needs no extra code for ordinary data. `copy` has no structural
default — every aggregate field of a `copy` type must itself claim `copy`.
`copy` implies `clone`, and `copy` and `fin` cannot coexist.

`send` and `share` require every field to be thread-safe transitively, so a type
holding a non-synchronized shared pointer or a `fin` resource cannot claim them.

## Operations

`[cpy]value` requires the value's type to declare `copy`; a structurally
copy-safe record without the `(copy)` header is clone- or move-only.
`[cln]value` requires `clone`, which the structural default usually satisfies.

## Custom clone

A type may override the structural clone with a pure borrowed-receiver method
named `clone`:

```fol
typ Counter()(clone): rec = { value: int, clones: int };

fun (Counter[bor])clone(): Counter = {
    return { value = self.value, clones = self.clones + 1 };
};
```

`[cln]counter` then dispatches to this method instead of copying fields
structurally. A custom clone is a `fun` with a shared `[bor]` receiver: it
observes the source and returns an independent value.

## Generic bounds

A capability may constrain a generic parameter. The obligation is checked at each
call site against the concrete type argument, so the routine is only usable with
types that satisfy the standard:

```fol
fun keep(T: copy)(value: T): T = { return [cpy]value; };
```
