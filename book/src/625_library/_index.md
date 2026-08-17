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

### Container routines are generic, and two bounds carry them

`std::vecs`, `std::sets`, `std::iter`, `std::heap`, and `std::grid` are generic
over their element type:

```fol
var numbers: vec[int] = std::vecs::sort(measurements);
var names: vec[str] = std::vecs::sort(labels);
var places: vec[Point] = std::vecs::sort(waypoints);
```

Two capability bounds do most of the work:

- **`clone`**, because a pure routine has to copy elements out of its argument
  rather than move them — so the argument is still usable afterwards
- **`ord`**, wherever elements are compared. It promises a *total* order, and a
  total order also decides equality, so `contains` and `dedup_sorted` need
  nothing more

A bound is checked against the actual type at every call, including across a
package boundary. Passing a type that cannot satisfy one is an ordinary type
error, not a failure further down the pipeline.

### A remaining `_int` or `_flt` suffix means a real restriction

A few names keep a type in them, and each has a reason that will not go away
soon:

- `vecs::sum_int` and `iter::zip_add_int` add elements together, and FOL has no
  **numeric** capability bound, so `+` is unavailable on a `T`. For any other
  element type, use `iter::fold` and supply the combining step yourself
- `vecs::range_int` generates integers rather than copying them out of an
  argument, so there is nothing to be generic over
- `vecs::sort_flt` exists because `ord` promises a *total* order and `flt` has
  none — NaN compares equal to nothing. The container method behind it needs
  only a partial order, which `flt` does have

So the suffix now marks a genuine restriction. It is not a placeholder for work
still to be done.

### Passing a literal to a generic routine

A bare container literal has no type of its own to infer from, and against a
generic parameter it comes out as an array rather than a vector:

```fol
// rejected: the literal types as `[int]`, not `vec[int]`
var sorted: vec[int] = std::vecs::sort({3, 1, 2});

// bind it first
var values: vec[int] = {3, 1, 2};
var sorted: vec[int] = std::vecs::sort(values);
```

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
