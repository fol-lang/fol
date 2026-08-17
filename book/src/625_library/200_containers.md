# Containers

Every routine on this page is pure: it takes a container and returns a new one.
Nothing here mutates its argument, because a free FOL routine cannot take a
`[mut, bor]` parameter.

For in-place work, the container **methods** are the other half of the story —
`values.push(x)`, `values.pop()`, `values.sort()`, `values.swap(i, j)` — and
`cells[i] = v` assigns an element directly. Those are language surface, not
`std`, and they are what these routines are built from.

All of it is generic over the element type. `clone` is needed because a pure
routine copies elements out of its argument; `ord` appears wherever elements are
compared, and since it promises a total order it also decides equality. See the
[index chapter](./_index.md) for what a remaining `_int` suffix means, and for
why a bare literal has to be bound before it is passed.

## `std::vecs` — ordering and search

```fol
fun[exp] sort(T: ord + clone)(values: vec[T]): vec[T]
fun[exp] sort_flt(values: vec[flt]): vec[flt]
fun[exp] reverse(T: clone)(values: vec[T]): vec[T]
fun[exp] is_sorted(T: ord + clone)(values: vec[T]): bol
fun[exp] index_of(T: ord + clone)(values: vec[T], needle: T): int
fun[exp] contains(T: ord + clone)(values: vec[T], needle: T): bol
fun[exp] binary_search(T: ord + clone)(values: vec[T], needle: T): int
fun[exp] min_of(T: ord + clone)(values: vec[T], fallback: T): T
fun[exp] max_of(T: ord + clone)(values: vec[T], fallback: T): T
fun[exp] fill(T: clone)(count: int, value: T): vec[T]
fun[exp] slice(T: clone)(values: vec[T], start: int, count: int): vec[T]
fun[exp] concat(T: clone)(left: vec[T], right: vec[T]): vec[T]
fun[exp] dedup_sorted(T: ord + clone)(values: vec[T]): vec[T]
fun[exp] sum_int(values: vec[int]): int
fun[exp] range_int(start: int, stop: int): vec[int]
```

`sort` delegates to the container method, so it is a real sort rather than an
insertion sort written in FOL. `sort_flt` exists because `flt` cannot satisfy
`ord`.

`index_of` scans and `binary_search` bisects; both return `-1` when absent, and
the bisecting one requires a sorted input, which `is_sorted` can confirm.

`min_of` and `max_of` take a fallback because an empty vector has no answer.
`range_int(start, stop)` is half-open, so `range_int(0, 5)` is `0 1 2 3 4`.
`slice` clamps rather than faulting: a start past the end yields an empty
vector.

```fol
var evens: vec[int] = std::vecs::range_int(0, 10);
var top: vec[int] = std::vecs::slice(evens, 5, 99);
// top is 5 6 7 8 9 -- the count is clamped, not an error
```

## `std::sets` — growable sets

A set here is a **sorted vector with no duplicates**, not the builtin `set[...]`
type, which is a fixed tuple of member types. Membership is a binary search; the
algebra is a single merge each.

```fol
fun[exp] from_vec(T: ord + clone)(values: vec[T]): vec[T]
fun[exp] contains(T: ord + clone)(members: vec[T], value: T): bol
fun[exp] insert(T: ord + clone)(members: vec[T], value: T): vec[T]
fun[exp] remove(T: ord + clone)(members: vec[T], value: T): vec[T]
fun[exp] union(T: ord + clone)(left: vec[T], right: vec[T]): vec[T]
fun[exp] intersect(T: ord + clone)(left: vec[T], right: vec[T]): vec[T]
fun[exp] difference(T: ord + clone)(left: vec[T], right: vec[T]): vec[T]
fun[exp] is_subset(T: ord + clone)(inner: vec[T], outer: vec[T]): bol
```

`from_vec` is the entry point — it sorts and de-duplicates. Every other routine
**assumes that invariant** and will misbehave on an unsorted vector, so build
sets with it rather than passing a raw vector. `insert` on an element already
present returns the set unchanged, so calling it twice is the same as once.

```fol
var low: vec[int] = std::vecs::range_int(0, 6);
var high: vec[int] = std::vecs::range_int(4, 10);
var shared: vec[int] = std::sets::intersect(
    std::sets::from_vec(low),
    std::sets::from_vec(high),
);
// shared is 4 5
```

## `std::maps` — map iteration and updates

The one group still spelled per type. A map has two type parameters and the
pair record would have to be generic with it, which is a larger change than the
element-type collapse.

```fol
typ[exp] StrIntPair: rec = { key: str, value: int };
typ[exp] StrPair: rec = { key: str, value: str };

fun[exp] pairs_str_int(source: map[str, int]): vec[StrIntPair]
fun[exp] pairs_str_str(source: map[str, str]): vec[StrPair]
fun[exp] get_or(source: map[str, int], key: str, fallback: int): int
fun[exp] get_or_str(source: map[str, str], key: str, fallback: str): str
fun[exp] bump(source: map[str, int], key: str, amount: int): map[str, int]
fun[exp] count_words(words: vec[str]): map[str, int]
fun[exp] merge_str_int(left: map[str, int], right: map[str, int]): map[str, int]
fun[exp] invert_str_str(source: map[str, str]): map[str, str]
fun[exp] keys_where(source: map[str, int], least: int): vec[str]
fun[exp] sum_values(source: map[str, int]): int
```

`pairs_*` is how you iterate: a map yields a vector of key/value records, which
`for` can then walk. The record is the pair — FOL has no tuple type, so a
two-field record is what carries one.

`bump` is the counter idiom, inserting the key when absent:

```fol
var tally: map[str, int] = std::maps::count_words(words);
var more: map[str, int] = std::maps::bump(tally, "fol", 1);
```

`merge_str_int` lets the right side win on a duplicate key.

## `std::iter` — adapters over vectors

These take **routine values**, so the step is an ordinary FOL closure. Two of
them carry a second type parameter, because they may change the type:

```fol
fun[exp] map_to(T: clone, U: clone)(values: vec[T], step: {fun (value: T): U}): vec[U]
fun[exp] filter(T: clone)(values: vec[T], keep: {fun (value: T): bol}): vec[T]
fun[exp] fold(T: clone, A: clone)(values: vec[T], start: A, step: {fun (carried: A, value: T): A}): A
fun[exp] any(T: clone)(values: vec[T], test: {fun (value: T): bol}): bol
fun[exp] all(T: clone)(values: vec[T], test: {fun (value: T): bol}): bol
fun[exp] position(T: clone)(values: vec[T], test: {fun (value: T): bol}): int
fun[exp] count_if(T: clone)(values: vec[T], test: {fun (value: T): bol}): int
fun[exp] take(T: clone)(values: vec[T], count: int): vec[T]
fun[exp] skip(T: clone)(values: vec[T], count: int): vec[T]
fun[exp] take_while(T: clone)(values: vec[T], test: {fun (value: T): bol}): vec[T]
fun[exp] skip_while(T: clone)(values: vec[T], test: {fun (value: T): bol}): vec[T]
fun[exp] zip_add_int(left: vec[int], right: vec[int]): vec[int]
```

They are **eager**: each returns a complete vector, so a chain allocates at
every step. There is no lazy iterator protocol; for a hot loop, write the loop.

`map_to` and `fold` are where the second parameter earns its place — rendering
numbers to text, or folding a vector down to a single string:

```fol
var numbers: vec[int] = std::vecs::range_int(1, 4);
var texts: vec[str] = std::iter::map_to(numbers, fun (value: int): str = {
    return std::fmt::int_to_str(value);
});
var joined: str = std::iter::fold(texts, "", fun (carried: str, value: str): str = {
    return carried + value;
});
// joined is "123"
```

`take_while` stops at the first element that fails, unlike `filter`, which tests
them all. `position` returns `-1` when nothing matches.

`zip_add_int` adds elements together, which needs a numeric bound FOL does not
have. For any other element type, use `fold` and supply the combining step.

## `std::heap` — stack, queue, and min-heap

Three structures over one vector, distinguished by which end they work on. The
heap compares, so it needs `ord`; the stack and queue only need `clone`.

```fol
fun[exp] stack_push(T: clone)(values: vec[T], value: T): vec[T]
fun[exp] queue_push(T: clone)(values: vec[T], value: T): vec[T]
fun[exp] queue_pop(T: clone)(values: vec[T]): vec[T]
fun[exp] queue_front(T: clone)(values: vec[T], fallback: T): T
fun[exp] heap_push(T: ord + clone)(values: vec[T], value: T): vec[T]
fun[exp] heap_peek(T: ord + clone)(values: vec[T], fallback: T): T
fun[exp] heap_pop(T: ord + clone)(values: vec[T]): vec[T]
fun[exp] heap_drain(T: ord + clone)(values: vec[T]): vec[T]
```

A stack needs no `pop` routine — `values.pop()` is the container method.

The heap is a binary **min**-heap, so `heap_peek` is the smallest element and
`heap_drain` returns everything in ascending order. `heap_pop` returns the heap
without its smallest element; read it with `heap_peek` first if you need the
value.

## `std::grid` — 2D over one flat vector

A grid is a vector plus a width, addressed row-major. Keeping it flat means one
allocation and no nested-vector indexing.

```fol
fun[exp] offset(width: int, row: int, col: int): int
fun[exp] in_bounds(width: int, height: int, row: int, col: int): bol
fun[exp] make(T: clone)(width: int, height: int, fill: T): vec[T]
fun[exp] cell(T: clone)(cells: vec[T], width: int, row: int, col: int, fallback: T): T
fun[exp] put(T: clone)(cells: vec[T], width: int, row: int, col: int, value: T): vec[T]
fun[exp] row_of(T: clone)(cells: vec[T], width: int, row: int, fallback: T): vec[T]
fun[exp] column_of(T: clone)(cells: vec[T], width: int, height: int, col: int, fallback: T): vec[T]
fun[exp] neighbours(T: clone)(cells: vec[T], width: int, row: int, col: int, fallback: T): vec[T]
```

`offset` and `in_bounds` are pure index arithmetic and carry no element type at
all.

Every reader takes a fallback, so an out-of-range read is a value rather than a
fault — which is what a flood fill or a cellular automaton wants at the edges,
and what a generic cell type needs, since there is no zero to default to.
`neighbours` returns all **eight** surrounding cells, reading row by row and
skipping the centre; for the four orthogonal ones, call `cell` yourself.

```fol
var origin: Point = { x = 0, y = 0 };
var mark: Point = { x = 9, y = 9 };
var board: vec[Point] = std::grid::make(3, 3, origin);
var marked: vec[Point] = std::grid::put(board, 3, 1, 1, mark);
var centre: Point = std::grid::cell(marked, 3, 1, 1, origin);
// centre.x is 9
```
