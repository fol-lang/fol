# Capabilities

Six compiler-owned standards describe what may be done with a value. A type
lists the ones it guarantees in its conformance position, and the compiler
verifies each claim against the type's fields.

| Standard | Meaning |
| --- | --- |
| `copy` | `[cpy]value` may duplicate the value; the source stays usable |
| `clone` | `[cln]value` may create an independent copy; the source stays usable |
| `fin` | the type runs custom finalization when it is dropped |
| `send` | owned access may cross a task or thread boundary |
| `share` | shared access may cross a task or thread boundary |
| `ord` | the type has a total order, so `<` and `>` compare its values |

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

`ord` is the one standard that is **never claimed in a conformance header**. It
is structural throughout: a record or entry is ordered when its fields are.
`flt` does **not** satisfy `ord`, because NaN compares equal to nothing and a
total order is what the standard promises.

An aggregate compares **field by field in field-NAME order, not declaration
order.** Record types are interned structurally, and that interning normalizes
field order so `{ a: int, b: int }` and `{ b: int, a: int }` are the same type —
which means there is no declaration order left to compare by. A record declared
`{ row: int, col: int }` therefore compares on `col` first:

```fol
typ Cell: rec = { row: int, col: int };
// sorting these yields (2,0) before (1,3): `col` decides, not `row`
```

So treat `ord` on an aggregate as "some stable total order" — enough to be a map
key, a set member, or to deduplicate. Do not rely on it for presentation order.
To sort records the way a reader expects, sort a vector of the field you care
about, or give the type a field order whose names sort the way you need.

Because a total order decides equality as well, a value bound by `ord` may be
compared with `==` too. Only `ord` is needed to search a container, not `ord`
plus some separate equality standard.

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

Several bounds combine with `+`, which is how most of the standard library's
container routines are written:

```fol
fun[exp] sort(T: ord + clone)(values: vec[T]): vec[T] = {
    var[mut] out: vec[T] = [mov]values;
    out.sort();
    return out;
};
```

`clone` appears there because a pure routine has to copy elements out of its
argument rather than move them, and `ord` because the sort compares.

The parts need not all be capabilities. A declared standard joins the same way,
so one parameter can carry both a contract and a capability, and the standard may
be qualified:

```fol
fun[exp] is_affine(U: scale::unit + copy)(item: U): bol = {
    return magnitude(offset_of([cpy]item)) > 0.000000001;
};
```

The standard half is resolved to its declaration; the capability half is checked
against the concrete type at the call site.

`copy` is the one capability with no structural default. A record whose fields
are all copy-safe still does not satisfy `copy` until it says so, because `copy`
is a claim rather than a shape — the same rule `[cpy]` enforces on a value:

```text
call to 'keep' requires type 'Tally' to satisfy the 'copy' capability for generic
parameter 'T'; the type does not; add a '(copy)' conformance header to it, or
bound the parameter with 'clone'
```

The check applies **across a package boundary** as well. Calling an imported
generic routine with a type that cannot satisfy its bound is an ordinary type
error at the call site, not a failure further down the pipeline:

```text
call to 'std::vecs::sort' requires type 'flt' to satisfy the 'ord' capability
for generic parameter 'T'; the type does not
```
