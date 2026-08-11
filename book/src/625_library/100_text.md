# Text

## `std::strn` — strings

FOL strings are UTF-8, and `.len(...)` counts **bytes**. `std::strn` inherits
that: `sub`, `find`, and `byte_at` are byte-oriented, matching the intrinsics
they wrap. For character-oriented work use `.str_char_len(...)` and friends, and
see the [intrinsics chapter](../300_meta/100_buildin.md) for why the two worlds
are kept apart.

### Searching and testing

```fol
fun[exp] find(text: str, needle: str): int
fun[exp] last_index_of(text: str, needle: str): int
fun[exp] contains(text: str, needle: str): bol
fun[exp] starts_with(text: str, prefix: str): bol
fun[exp] ends_with(text: str, suffix: str): bol
fun[exp] is_empty(text: str): bol
fun[exp] count_matches(text: str, needle: str): int
```

`find` and `last_index_of` return a **byte index, or `-1`**. There is no
optional here: `-1` is the sentinel, so test it before using the result as an
offset.

### Slicing and trimming

```fol
fun[exp] sub(text: str, start: int, count: int): str
fun[exp] byte_at(text: str, index: int): int
fun[exp] from_byte(value: int): str
fun[exp] trim(text: str): str
fun[exp] trim_start(text: str): str
fun[exp] trim_end(text: str): str
fun[exp] strip_prefix(text: str, prefix: str): str
fun[exp] strip_suffix(text: str, suffix: str): str
```

`strip_prefix` and `strip_suffix` return the text unchanged when it does not
start or end with the given piece, so they are safe to chain.

### Building

```fol
fun[exp] replace(text: str, from: str, to: str): str
fun[exp] repeat(text: str, times: int): str
fun[exp] reverse(text: str): str
fun[exp] pad_left(text: str, width: int, fill: str): str
fun[exp] pad_right(text: str, width: int, fill: str): str
fun[exp] split(text: str, sep: str): vec[str]
fun[exp] lines(text: str): vec[str]
fun[exp] join(parts: vec[str], sep: str): str
fun[exp] to_upper(text: str): str
fun[exp] to_lower(text: str): str
fun[exp] to_int(text: str, fallback: int): int
```

`to_upper` and `to_lower` are full Unicode mappings, not per-character ASCII:
`straße` uppercases to `STRASSE`, which is one character longer than the input.

`to_int` takes the fallback as an argument because every sentinel is also a
valid parse result — there is no integer that can mean "not a number".

**Padding counts bytes.** For a table containing non-ASCII text, compute the
width with `.str_width(...)` instead; `日本` is 6 bytes, 2 characters, and 4
terminal columns.

### Normalization

The same word can be typed two ways, and they do not compare equal:

```fol
fun[exp] nfc(text: str): str
fun[exp] nfd(text: str): str
fun[exp] nfkc(text: str): str
fun[exp] nfkd(text: str): str
fun[exp] is_nfc(text: str): bol
fun[exp] same_text(left: str, right: str): bol
fun[exp] search_key(text: str): str
```

`same_text` is the one to reach for when comparing anything a person typed — a
login name, a search box, a filename:

```fol
var typed: str = "e\u{0301}llo";
var stored: str = "éllo";
// typed == stored                      is false
// std::strn::same_text(typed, stored)  is true
```

`search_key` folds harder still — case and compatibility variants both — which
suits duplicate detection and search indexes but discards distinctions, so
never store its output as the user's text.

## `std::chars` — characters

```fol
fun[exp] code(value: chr): int
fun[exp] from_code(value: int): chr
fun[exp] is_digit(value: chr): bol
fun[exp] is_alpha(value: chr): bol
fun[exp] is_alnum(value: chr): bol
fun[exp] is_upper(value: chr): bol
fun[exp] is_lower(value: chr): bol
fun[exp] is_space(value: chr): bol
fun[exp] is_hex_digit(value: chr): bol
fun[exp] to_upper(value: chr): chr
fun[exp] to_lower(value: chr): chr
fun[exp] digit_value(value: chr): int
```

The classifiers are Unicode-aware, not ASCII ranges: `is_alpha` accepts `é` and
`is_space` accepts a non-breaking space. `digit_value` yields `-1` for a
character that is not a digit.

Single-character case mapping cannot express every rule — `ß` uppercases to two
characters — so use `std::strn::to_upper` for text and `chars::to_upper` only
for a character you have already isolated.

## `std::fmt` — numbers to text

```fol
fun[exp] int_to_str(value: int): str
fun[exp] float_to_str(value: flt, decimals: int): str
fun[exp] bol_to_str(value: bol): str
fun[exp] digit_count(value: int): int
fun[exp] pad_int(value: int, width: int): str
fun[exp] with_thousands(value: int, sep: str): str
fun[exp] to_radix(value: int, radix: int): str
fun[exp] to_hex(value: int): str
fun[exp] to_bin(value: int): str
fun[exp] to_oct(value: int): str
```

`to_radix` handles bases 2 through 36 using `0-9a-z`; `to_hex`, `to_bin`, and
`to_oct` are the usual three named.

`pad_int` pads with **zeros**, not spaces — it is for fixed-width numeric fields
such as timestamps and identifiers. For space-aligned columns use
`std::strn::pad_left` with a space fill.

```fol
var stamp: str = std::fmt::pad_int(42, 6) + " " + std::fmt::to_hex(255);
// "000042 ff"
```

The sign is added after padding, so a negative comes out one character wider
than the requested width: `pad_int(-42, 6)` is `-000042`.

For floats, `float_to_str` takes a fixed number of decimals. When the value has
to survive a round trip — writing JSON, say — use the `.flt_to_str_exact(...)`
intrinsic instead, which emits the shortest text that parses back to the
identical value.
