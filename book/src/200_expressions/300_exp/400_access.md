# Access

Current boundary:

- namespace access, receiver-qualified routine access, single-element container
  indexing (`c[i]`), map key access, and record field access are the current
  compiler surface
- bounded forward slicing (`c[1:3]`, `c[:3]`, and `c[1:]`) is implemented for
  `vec[...]` and `seq[...]`; fixed-size array slices still need an explicit
  runtime-sized result type
- reverse slicing (`c[::]`), availability checks (`v:[1]`), in-place
  assignment (`c[x => Y]`), and the whole axiom (`axi`) access family are later
  design work, not part of the current compiler surface
- the sections below are marked where they cross that line

There are four access expressions:

- namespace member access
- receiver-qualified routine access
- container member access
- field member access

## Receiver-Qualified Routine Access

One access form is the receiver-style routine call. In FOL this remains
procedural syntax: the receiver value is the first routine input, and the dot
form is only call-site sugar.

A receiver-qualified call consists of an expression (the receiver), followed by
a single dot `.`, a routine name, and a parenthesized expression list:

```fol
"3.14".cast(float).pow(2);
```

Read:

```fol
value.method(arg1, arg2)
```

as:

```fol
method(value, arg1, arg2)
```

This spelling does not create classes, objects, inheritance, or runtime method
ownership. It is just a shorter way to call a receiver-qualified routine.


## Namespace Access

Accessing namespaces is done through the double-colon operator `::`:

```fol
use std: pkg = {"std"};
var shown: int = std::fmt::math::answer();
```

This particular namespace is part of bundled `std`, so the package must use a
`memo` artifact and declare the internal `standard` dependency in `build.fol`.
The `pkg` target is the declared alias (`"std"`); nested namespaces are reached
with `::`, not by embedding `std/...` in the import target.


## Container Access

### Array, Vectors, Sequences, Sets

Containers can be indexed by writing a square-bracket-enclosed expression of
type `int[arch]` after them.

```fol
var collection: int = { 5, 4, 8, 3, 9, 0, 1, 2, 7, 6 }

collection[5]
collection[-2]
```

Containers can be accessed with a specified range too, by using colon within a square-bracket-enclosed:

The current executable subset applies these forward forms to `vec[...]` and
`seq[...]`. It creates a new container, so V3 rejects slices whose elements are
move-only. Fixed-size `arr[...]` slicing and the reverse forms below remain
design material.

syntax | meaning
--- | ---
`:` | the whole container
elA`:`elB | from element `elA` to element `elB`
`:`elA | from beginning to element `elA`
elA`:` | from element `elA` to end

```fol
collection[-0]                              // last item in the array
{ 6 }
collection[-1:]                             // last two items in the array
{ 7, 6 }
collection[:-2]                             // everything except the last two items
{ 5, 4, 8, 3, 9, 0, 1, 2 }
```

If we use double colon within a square-bracket-enclosed then the collection is inversed:

syntax | meaning
--- | ---
`::` | the whole container in reverse
elA`::`elB | from element `elA` to element `elB` in reverse
`::`elA | from beginning to element `elA` in reverse
elA`::` | from element `elA` to end in reverse

```fol
collection[::]                              // all items in the array, reversed
{ 6, 7, 2, 1, 0, 9, 3, 8, 4, 5 }
collection[2::]                             // the first two items, reversed
{ 4, 5 }
collection[-2::]                            // the last two items, reversed
{ 6, 7 }
collection[::-3]                            // everything except the last three items, reversed
{ 2, 1, 0, 9, 3, 8, 4, 5 }
```
### Matrices

Matrices are 2D+ arrays, so they use nested index access:

```fol
var aMat = mat[int, int] = { {1,2,3}, {4,5,6}, {7,8,9} };

nMat[[1][0]]
```
All other access forms behave like arrays.

### Maps

Accessing maps is done by using the key inside square brackets:

```fol
var someMap: map[str, int] = { {"prolog", 1}, {"lisp", 2}, {"c", 3} }
someMap["lisp"]
```

### Axioms

Accessing axioms is similar to accessing maps, but matching is broader and the
result is always a vector of matches:

```fol
var parent: axi[str, str] = { {"albert","bob"}, {"alice","bob"}, {"bob","carl"}, {"bob","tom"} };

parent["albert",*]
parent["bob",*]

parent[*,_]
```

Matching can be with a vector too:

```fol
var parent: axi[str, str] = { {"albert","bob"}, {"alice","bob"}, {"bob","carl"}, {"bob","tom"}, {"maggie","bill"} };
var aVec: vec[str] = { "tom", "bob" };

parent[*,aVec]
```

A more complex matching example:

```fol
var class: axi;
class.add({"cs340","spring",{"tue","thur"},{12,13},"john","coor_5"})
class.add({"cs340","winter",{"tue","fri"},{12,13},"mike","coor_5"})
class.add({"cs340",winter,{"wed","fri"},{15,16},"bruce","coor_3"})
class.add({"cs101",winter,{"mon","wed"},{10,12},"james","coor_1"})
class.add({"cs101",spring,{"tue","tue"},{16,18},"tom","coor_1"})

var aClass = "cs340"
class[aClass,_,[_,"fri"],_,*,_]
```

### Availability

To check whether an element exists, add `:` before `[]`. The result is `true`
when the element exists.

```fol
var val: vec = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10}

val:[5]
val:[15]


var likes: axi[str, str] = { {"bob","alice"} , {"alice","bob"}, {"dan","sally"} };

likes["bob","alice"]:
likes["sally","dan"]:
```

### In-Place Assignment

Some access forms can bind a value while matching:

```fol
var val: vec = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10}
var even: vec = {2, 4, 6, 8, 10, 12, 14, 16, 18, 20}
val[even => Y]
.echo(Y)
```

This is more useful with axioms:

```fol
var parent: axi[str, str] = { {"albert","bob"}, {"alice","bob"}, {"bob","carl"}, {"bob","tom"}, {"maggie","bill"} };

parent:[* => Y,"bob"]
```

## Field Access

Field access expressions access stored data inside record-like values. Here is
an example record `user`:

```fol
var user1: user = {
    email = "someone@example.com",
    username = "someusername123",
    active = true,
    sign_in_count = 1
};

fun (user)getName(): str = { result = self.username; };
```

There are two things you may access on such a value:

- receiver-qualified routines
- data fields

### Receiver-Qualified Routines

The same dot spelling is used for receiver-qualified routines, but they remain
ordinary routines rather than object-owned behavior:

```fol
user1.getName()
```

### Data

There are multiple ways to access stored data. The simplest is the dot
operator `.`:

```fol
user1.email
```

Another way is by using square bracket enclosed by name:

```fol
user1[email]
```

And lastly, by square bracket enclosed by index:

```fol
user1[0]
```
