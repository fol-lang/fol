# Data Formats

Everything on this page is FOL, including SHA-256 and CRC-32. None of it needs
the compiler: they are loops over the bitwise intrinsics, and they match the
published reference vectors.

## `std::json` — flat JSON

Escaping, scanning, and single-level objects. For nested documents use
`std::jsondoc` below.

```fol
fun[exp] escape(text: str): str
fun[exp] unescape(text: str): str
fun[exp] encode_string(text: str): str
fun[exp] is_balanced(text: str): bol
fun[exp] split_top_level(text: str): vec[str]
fun[exp] max_depth(text: str): int
fun[exp] key_split_point(entry: str): int
fun[exp] decode_flat(text: str): map[str, str]
fun[exp] decode_array(text: str): vec[str]
fun[exp] encode_flat(values: map[str, str]): str
```

`escape` handles the string body; `encode_string` adds the surrounding quotes.
`split_top_level` splits on commas that are not inside a nested brace, bracket,
or string, which is what makes it usable as a scanner rather than a naive
`split`. It records where each part starts and slices once, rather than growing
the part a character at a time -- accumulating would copy the whole part on every
character, and this runs over untrusted input.

`max_depth` reports the deepest nesting the text reaches, walked iteratively.
`jsondoc::parse` uses it to refuse a document too deep to descend recursively.

`decode_flat` returns every value as a `str`, including numbers and booleans —
a flat map cannot carry mixed types, and converting is the caller's decision.

## `std::jsondoc` — the value tree

A JSON value contains JSON values, and **FOL has no recursive type**. The tree
is therefore an *arena*: every node lives in one flat `vec[Node]` and refers to
its children by integer index.

```fol
typ[exp] Node: rec = {
    kind: int,
    text: str,
    first: int,
    next: int,
};
typ[exp] Doc: rec = { nodes: vec[Node] };
```

Children are a singly-linked list — `first` is the first child's index, each
child's `next` is its sibling, and both are `-1` when absent. That keeps a node
fixed-size, so adding a child never has to reallocate a parent's field. This is
the documented workaround for recursive shapes, and it costs nothing here: a
document is parsed once and read many times.

```fol
con[exp] NULL: int = 0;
con[exp] BOOL: int = 1;
con[exp] NUM: int = 2;
con[exp] STR: int = 3;
con[exp] ARR: int = 4;
con[exp] OBJ: int = 5;
con[exp] MEMBER: int = 6;

fun[exp] none(): int          // the absent-index sentinel, -1
fun[exp] parse(text: str): Doc
fun[exp] kind_of(doc: Doc[bor], index: int): int
fun[exp] text_of(doc: Doc[bor], index: int): str
fun[exp] child_indices(doc: Doc[bor], index: int): vec[int]
fun[exp] member_value(doc: Doc[bor], member: int): int
fun[exp] field(doc: Doc[bor], object: int, name: str): int
fun[exp] element(doc: Doc[bor], array: int, position: int): int
fun[exp] length_of(doc: Doc[bor], index: int): int
fun[exp] path(doc: Doc[bor], from: str): int
fun[exp] render(doc: Doc[bor], index: int): str
```

`none()` is a routine rather than a constant because `con` needs a literal
initializer and `-1` is a negation, not a literal.

### What `parse` promises

`parse` is **lenient, not validating**. It never faults, whatever it is handed --
truncated input, an unterminated string, a lone backslash, a trailing comma, or
text that is not JSON at all. What it does *not* do is reject malformed input, so
check the shape you expect rather than assuming a document is well formed:

- a container it cannot build yields an **empty arena**, so `kind_of(...)` on
  index 0 answers `-1`; that is the signal that nothing parsed
- `NUM` is a **fallthrough**, not a claim. Anything that is not a quoted string,
  `true`, `false` or `null` is classified `NUM` with the raw text kept verbatim,
  so `hello` and the empty string both arrive as `NUM`. Parse the text before
  trusting it as a number
- a trailing comma is accepted; `[1,2,]` reads as a two-element array
- nesting deeper than `MAX_DEPTH` (512) is refused the same way, giving the empty
  arena. Building the tree recurses once per level, and a stack overflow **aborts
  the process** rather than faulting, so no caller could catch it -- the cap is
  what keeps hostile input a refusal instead of a crash

That leniency is deliberate -- a parser that faults on hostile input is a denial
of service -- but it puts the validation burden on the caller.

The document is passed as `Doc[bor]` — a read-only loan — so walking it never
copies the arena. That means **every call takes `[bor]doc`, not `doc`**;
ownership transfers are explicit in FOL, and passing the document bare is an
`O2002` error.

An object **member** is its own node: it holds the key in `text` and its value
as its only child, which is what lets objects and arrays share one walk. Use
`field` to go straight from an object to a value index, and `path` for a dotted
lookup:

```fol
var doc: std::jsondoc::Doc = std::jsondoc::parse(source);
var name: int = std::jsondoc::path([bor]doc, "user.name");
var text: str = std::jsondoc::text_of([bor]doc, name);
```

Every lookup returns an index, and a missing one is `none()`. Check before
reading.

## `std::csv` — RFC 4180

```fol
fun[exp] parse_row(line: str): vec[str]
fun[exp] parse(text: str): vec[str]
fun[exp] field_at(line: str, index: int, fallback: str): str
fun[exp] column_count(line: str): int
fun[exp] encode_field(value: str): str
fun[exp] encode_row(fields: vec[str]): str
```

Quoting is handled on both sides: `parse_row` understands quoted fields with
embedded commas and doubled quotes, and `encode_field` adds quotes only when the
value needs them.

`field_at` takes a fallback so a short row reads as a value rather than a fault,
which is what makes a malformed file survivable.

## `std::codec` — encodings and checksums

```fol
fun[exp] hex_encode(text: str): str
fun[exp] hex_decode(text: str): str
fun[exp] base64_encode(text: str): str
fun[exp] base64_decode(text: str): str
fun[exp] crc32(text: str): int
fun[exp] crc32_hex(text: str): str
```

Both decoders are **all-or-nothing**: malformed input yields the empty string
rather than a partial result, so a caller cannot act on half a value.

CRC-32 detects accidental corruption. It is not a hash and not a signature —
producing a colliding input is trivial.

## `std::hash` — digests

```fol
fun[exp] sha256_hex(text: str): str
fun[exp] fnv1a(text: str): int
fun[exp] round_constants(): vec[int]
```

`sha256_hex` is real SHA-256, matching the published test vectors. Use it when a
digest has to resist an adversary — and pair it with `.bytes_equal_ct(...)` when
comparing digests, since a normal `==` returns early and leaks how much of a
secret was guessed correctly.

`fnv1a` is a small non-cryptographic hash for bucketing. For that job the
`.hash_bytes(...)` intrinsic is usually better: it is SipHash, mixes far more
thoroughly, and is equally stable across runs.

`round_constants` is exposed because SHA-256's table is useful to verify
against; there is rarely a reason to call it.
