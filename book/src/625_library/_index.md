# The Standard Library

`std` is a FOL package that ships with the compiler. It is ordinary FOL source —
you can read every line of it — layered over the intrinsics described in the
[intrinsics chapter](../300_meta/100_buildin.md).

That split is the thing to hold onto. An intrinsic exists because the compiler
must understand it: it crosses an OS boundary, it is the only way to observe a
builtin, or it is something FOL would get subtly wrong (Unicode tables, float
bit layout, calendar arithmetic). Everything else belongs here, in FOL, where
you can read it and fix it. SHA-256, base64, CSV, JSON, sorting, path handling
and the rest are all FOL routines, not compiler surface.

## Depending on it

`std` is not implicit. Declare it in `build.fol`:

```fol
build.add_dep({ alias = "std", source = "internal", target = "standard" });
```

and import it in any source file that uses it:

```fol
use std: pkg = {"std"};
```

Then reach routines through their module:

```fol
fun[] main(): int = {
    std::io::write("total " + std::fmt::int_to_str(6 * 7) + "\n");
    return 0;
};
```

The artifact must be `fol_model = "memo"`. `std` layers on the heap-backed tier;
a `core` artifact has no heap and cannot use it.

## Three conventions that explain most of the API

Reading a few signatures raises the same three questions every time. All three
follow from language rules rather than from taste.

### Routines are pure, so updates return a new value

A free FOL routine cannot take a `[mut, bor]` parameter — only a receiver can.
So a routine that "modifies" a container takes it and returns the new one:

```fol
var members: vec[int] = std::sets::from_vec_int(std::vecs::range_int(0, 5));
var larger: vec[int] = std::sets::insert_int(members, 9);
```

`members` is unchanged; `larger` is the result. Where a routine has to carry
state across calls — the random generator is the clearest case — it returns a
record pairing the new state with the value.

### `_int` and `_str` suffixes

Most container routines are spelled per element type: `sort_int`, `sort_str`,
`contains_int`. This is not a style choice, and it is not laziness either — it
is forced.

**A generic routine cannot be called across a package boundary.** Imported
signatures are translated without their generic parameter list, so the call
would type as a bare `T`; the compiler stops it with a clear message rather
than a baffling mismatch:

```text
imported routine 'sort' is generic; cross-package generic instantiation is
not supported yet — export a non-generic wrapper and call that instead
```

`std` is a package, and every program that uses it is a different one. So the
exported surface has to be concrete, whatever the implementation does
underneath. `std::vecs` is written the way that advice suggests:

```fol
fun[exp] sort(T: ord + clone)(values: vec[T]): vec[T] = { … };
fun[exp] sort_int(values: vec[int]): vec[int] = { return sort([mov]values); };
fun[exp] sort_str(values: vec[str]): vec[str] = { return sort([mov]values); };
```

One generic body, thin concrete forwarders. **Call the suffixed names** — the
generic `sort` is reachable only from inside `std` itself.

Within a single package the restriction does not apply, so your own generic
routines over `vec[T]` work normally, including comparing elements when the
parameter is bound by `ord`.

A few routines are int-specific for a different and permanent reason: FOL has
no numeric capability bound, so `+` is unavailable on a `T`. `sum_int` and
`zip_add_int` could not be generic even if the boundary were lifted.

### A "set" here is a sorted vector

`std::sets` operates on `vec[T]` kept sorted with no duplicates, not on the
builtin `set[...]` type. The builtin is a fixed tuple of member types — closer
to a record than to a growable collection — so the growable set is built in FOL
instead. Membership is a binary search, and the operations are single merges.

## The modules

```text
  text        strn      strings: search, split, pad, case, normalization
              chars     character classification and conversion
              fmt       numbers to text, radix, padding

  containers  vecs      sort, search, slice, fold over vec
              sets      growable sets over sorted vectors
              maps      iteration and updates over map[K, V]
              iter      map/filter/fold with routine values
              heap      stack, queue, and binary min-heap
              grid      2D addressing over one flat vector

  numbers     nums      clamp, gcd, sign, overflow-reporting arithmetic
              rand      seeded, repeatable random numbers

  data        json      JSON escaping, scanning, flat objects
              jsondoc   a full JSON value tree
              csv       RFC 4180 rows and fields
              codec     hex, base64, CRC-32
              hash      SHA-256 and FNV-1a

  system      io        console reads and writes
              fs        files, whole and streaming
              os        arguments, environment, shell
              path      path surgery
              term      terminal size and raw mode
              time      clock and sleep
              sync      atomic counters and thread facts
```

Each group has its own page.
