# Containers

Every routine on this page is pure: it takes a container and returns a new one.
Nothing here mutates its argument, because a free FOL routine cannot take a
`[mut, bor]` parameter.

For in-place work, the container **methods** are the other half of the story —
`values.push(x)`, `values.pop()`, `values.sort()`, `values.swap(i, j)` — and
`cells[i] = v` assigns an element directly. Those are language surface, not
`std`, and they are what these routines are built from.

## `std::vecs` — ordering and search

```fol
fun[exp] sort(T: ord + clone)(values: vec[T]): vec[T]
fun[exp] sort_int(values: vec[int]): vec[int]
fun[exp] sort_str(values: vec[str]): vec[str]
fun[exp] is_sorted_int(values: vec[int]): bol
fun[exp] reverse_int(values: vec[int]): vec[int]
fun[exp] index_of_int(values: vec[int], needle: int): int
fun[exp] contains_int(values: vec[int], needle: int): bol
fun[exp] binary_search_int(values: vec[int], needle: int): int
fun[exp] min_int(values: vec[int], fallback: int): int
fun[exp] max_int(values: vec[int], fallback: int): int
fun[exp] sum_int(values: vec[int]): int
fun[exp] fill_int(count: int, value: int): vec[int]
fun[exp] range_int(start: int, stop: int): vec[int]
fun[exp] slice_int(values: vec[int], start: int, count: int): vec[int]
fun[exp] concat_int(left: vec[int], right: vec[int]): vec[int]
fun[exp] dedup_sorted_int(values: vec[int]): vec[int]
```

`sort` is generic over any `ord + clone` element and delegates to the container
method, so it is a real sort rather than an insertion sort written in FOL.
`sort_int` and `sort_str` are thin named forms of the same thing.

`index_of_int` scans and `binary_search_int` bisects; both return `-1` when
absent, and the bisecting one requires a sorted input, which `is_sorted_int`
can confirm.

`min_int` and `max_int` take a fallback because an empty vector has no answer.
`range_int(start, stop)` is half-open, so `range_int(0, 5)` is `0 1 2 3 4`.
`slice_int` clamps rather than faulting: a start past the end yields an empty
vector.

```fol
var evens: vec[int] = std::vecs::range_int(0, 10);
var top: vec[int] = std::vecs::slice_int(evens, 5, 99);
// top is 5 6 7 8 9 -- the count is clamped, not an error
```

## `std::sets` — growable sets

A set here is a **sorted `vec[int]` with no duplicates**, not the builtin
`set[...]` type, which is a fixed tuple of member types. Membership is a binary
search; the algebra is a single merge each.

```fol
fun[exp] from_vec_int(values: vec[int]): vec[int]
fun[exp] contains_int(members: vec[int], value: int): bol
fun[exp] insert_int(members: vec[int], value: int): vec[int]
fun[exp] remove_int(members: vec[int], value: int): vec[int]
fun[exp] union_int(left: vec[int], right: vec[int]): vec[int]
fun[exp] intersect_int(left: vec[int], right: vec[int]): vec[int]
fun[exp] difference_int(left: vec[int], right: vec[int]): vec[int]
fun[exp] is_subset_int(inner: vec[int], outer: vec[int]): bol
```

`from_vec_int` is the entry point — it sorts and de-duplicates. Every other
routine **assumes that invariant** and will misbehave on an unsorted vector, so
build sets with it rather than passing a raw vector.

```fol
var a: vec[int] = std::sets::from_vec_int(std::vecs::range_int(0, 6));
var b: vec[int] = std::sets::from_vec_int(std::vecs::range_int(4, 10));
var shared: vec[int] = std::sets::intersect_int(a, b);
// shared is 4 5
```

## `std::maps` — map iteration and updates

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

These take **routine values**, so the step is an ordinary FOL closure:

```fol
fun[exp] map_int(values: vec[int], step: {fun (value: int): int}): vec[int]
fun[exp] map_int_to_str(values: vec[int], step: {fun (value: int): str}): vec[str]
fun[exp] map_str(values: vec[str], step: {fun (value: str): str}): vec[str]
fun[exp] filter_int(values: vec[int], keep: {fun (value: int): bol}): vec[int]
fun[exp] filter_str(values: vec[str], keep: {fun (value: str): bol}): vec[str]
fun[exp] fold_int(values: vec[int], start: int, step: {fun (carried: int, value: int): int}): int
fun[exp] any_int(values: vec[int], test: {fun (value: int): bol}): bol
fun[exp] all_int(values: vec[int], test: {fun (value: int): bol}): bol
fun[exp] position_int(values: vec[int], test: {fun (value: int): bol}): int
fun[exp] count_if_int(values: vec[int], test: {fun (value: int): bol}): int
fun[exp] take_int(values: vec[int], count: int): vec[int]
fun[exp] skip_int(values: vec[int], count: int): vec[int]
fun[exp] take_while_int(values: vec[int], test: {fun (value: int): bol}): vec[int]
fun[exp] skip_while_int(values: vec[int], test: {fun (value: int): bol}): vec[int]
fun[exp] zip_add_int(left: vec[int], right: vec[int]): vec[int]
```

They are **eager**: each returns a complete vector, so a chain allocates at
every step. There is no lazy iterator protocol; for a hot loop, write the loop.

```fol
var numbers: vec[int] = std::vecs::range_int(1, 11);
var doubled: vec[int] = std::iter::map_int(numbers, fun (value: int): int = {
    return value * 2;
});
var total: int = std::iter::fold_int(doubled, 0, fun (acc: int, value: int): int = {
    return acc + value;
});
```

`position_int` returns `-1` when nothing matches.

## `std::heap` — stack, queue, and min-heap

Three structures over one `vec[int]`, distinguished by which end they work on:

```fol
fun[exp] stack_push(values: vec[int], value: int): vec[int]
fun[exp] queue_push(values: vec[int], value: int): vec[int]
fun[exp] queue_pop(values: vec[int]): vec[int]
fun[exp] queue_front(values: vec[int], fallback: int): int
fun[exp] heap_push(values: vec[int], value: int): vec[int]
fun[exp] heap_peek(values: vec[int], fallback: int): int
fun[exp] heap_pop(values: vec[int]): vec[int]
fun[exp] heap_drain(values: vec[int]): vec[int]
```

A stack needs no `pop` routine — `values.pop()` is the container method.

The heap is a binary min-heap, so `heap_peek` is the **smallest** element, and
`heap_drain` returns everything in ascending order. `heap_pop` returns the heap
without its smallest element; read it with `heap_peek` first if you need the
value.

## `std::grid` — 2D over one flat vector

A grid is a `vec[int]` plus a width, addressed row-major. Keeping it flat means
one allocation and no nested-vector indexing.

```fol
fun[exp] make(width: int, height: int, fill: int): vec[int]
fun[exp] offset(width: int, row: int, col: int): int
fun[exp] in_bounds(width: int, height: int, row: int, col: int): bol
fun[exp] cell(cells: vec[int], width: int, row: int, col: int, fallback: int): int
fun[exp] put(cells: vec[int], width: int, row: int, col: int, value: int): vec[int]
fun[exp] row_of(cells: vec[int], width: int, row: int): vec[int]
fun[exp] column_of(cells: vec[int], width: int, height: int, col: int): vec[int]
fun[exp] neighbours(cells: vec[int], width: int, row: int, col: int, fallback: int): vec[int]
```

`cell` takes a fallback so an out-of-range read is a value rather than a fault —
which is what a flood fill or a cellular automaton wants at the edges.
`neighbours` returns all **eight** surrounding cells, reading row by row and
skipping the centre, and uses the same fallback beyond the border. For the four
orthogonal ones, call `cell` yourself.

```fol
var board: vec[int] = std::grid::make(3, 3, 0);
var marked: vec[int] = std::grid::put(board, 3, 1, 1, 5);
var centre: int = std::grid::cell(marked, 3, 1, 1, -1);
// centre is 5
```
